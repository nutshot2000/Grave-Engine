//! Endless worlds, Noita-style: the world in memory is a window onto an
//! endless map, cut into square regions (256 x 256 cells). When the player
//! (or whatever the camera follows) gets near an edge, the window scrolls
//! one region that way: everything in it slides over, the strip that falls
//! off the far side is saved to disk, and the strip coming in is loaded if
//! it's been visited, or generated fresh by the level generator (levelgen.rs,
//! which makes the same map wherever and whenever it's asked).
//!
//! Scene line: `endless seed 21 [tile 64] [tiles tiles/caves] [region 256]`:
//! the world is the map around the start (its size should be a whole number
//! of regions), and the editor (or a game, with `Endless::from_scene`) keeps
//! it streaming. Every load is a new run: saved regions from an old one are
//! cleared (`begin`).
//!
//! Smooth: the scroll slides everything in place (`Sim::shift`: objects keep
//! moving, only the new strip's rock outlines are traced), saving runs on a
//! background thread (with a copy kept in memory for coming straight back),
//! and the strip ahead is generated on another core before it's needed.
//!
//! What streams: cells (with paint, films, the wall behind, anchoring) and
//! loose objects (crates, debris). Saved regions keep their shapes but not
//! their heat or fires. Objects tied to something (joints, the camera, a
//! trigger, a lamp, a pump) aren't saved away: keep them near the player.

use crate::levelgen::{self, Settings, Tile};
use crate::sim::Sim;
use crate::{Materials, World, scene};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;

/// Regions left behind that stay ready in memory (a few MB each); older
/// ones are read back from disk.
const KEEP: usize = 48;

/// A running endless map.
pub struct Endless {
    pub settings: Settings,
    pub tiles: Vec<Tile>,
    /// Region size, cells.
    pub region: usize,
    /// Where visited regions are kept.
    pub dir: PathBuf,
    /// The map cell at the world's (0, 0). Always whole regions.
    pub origin: (i64, i64),
    /// Regions saved this run (map region coordinates).
    saved: HashSet<(i64, i64)>,
    /// (saved, loaded, generated) regions so far.
    pub stats: (usize, usize, usize),
    pub problems: Vec<String>,
    /// Saved regions kept in memory too, ready-made (cells and objects), and
    /// saves still being written to disk in the background.
    cache: HashMap<(i64, i64), Arc<(World, Vec<crate::physics::BodySpec>)>>,
    /// Which were kept most recently (only the last `KEEP` stay in memory).
    cache_order: std::collections::VecDeque<(i64, i64)>,
    saving: HashMap<(i64, i64), JoinHandle<Result<(), String>>>,
    /// Regions being generated ahead of time, on another core.
    ahead: HashMap<(i64, i64), JoinHandle<World>>,
    /// Shared with the background threads.
    shared: Option<(Arc<Materials>, Arc<Vec<Tile>>)>,
}

impl Endless {
    pub fn new(settings: Settings, tiles: Vec<Tile>, region: usize, dir: PathBuf) -> Self {
        let mut settings = settings;
        settings.open_sides = true;
        Endless {
            settings,
            tiles,
            region: region.clamp(64, 2048),
            dir,
            origin: (0, 0),
            saved: HashSet::new(),
            stats: (0, 0, 0),
            problems: Vec::new(),
            cache: HashMap::new(),
            cache_order: std::collections::VecDeque::new(),
            saving: HashMap::new(),
            ahead: HashMap::new(),
            shared: None,
        }
    }

    /// The `endless` line of a scene, if it has one (`base` = the scene's folder).
    pub fn from_scene(text: &str, mats: &Materials, base: Option<&Path>, name: &str) -> Option<Endless> {
        let line = text.lines().map(|l| l.split('#').next().unwrap_or("").trim()).find(|l| l.split_whitespace().next() == Some("endless"))?;
        let (settings, tiles, region) = parse(line, mats, base).ok()?;
        let safe: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
        Some(Endless::new(settings, tiles, region, PathBuf::from("saves").join("endless").join(safe)))
    }

    fn region_file(&self, r: (i64, i64)) -> PathBuf {
        self.dir.join(format!("r{}_{}.txt", r.0, r.1))
    }

    /// A new run on a world that already shows the map at `origin` (a scene
    /// with an `endless` line has generated it): forget saved regions.
    pub fn new_run(&mut self) {
        self.settle();
        let _ = std::fs::remove_dir_all(&self.dir);
        self.saved.clear();
        self.cache.clear();
        self.cache_order.clear();
        self.ahead.clear();
        self.stats = (0, 0, 0);
    }

    /// Wait for background saves to finish (before quitting, say).
    pub fn settle(&mut self) {
        let pending: Vec<_> = self.saving.keys().copied().collect();
        for map in pending {
            self.finish_save(map);
        }
    }

    /// Carry on a run (after the editor restarts into a new build): the
    /// world shows the map at `origin`, and visited regions are on disk.
    pub fn resume(&mut self, origin: (i64, i64)) {
        self.settle();
        self.origin = origin;
        self.saved.clear();
        self.cache.clear();
        self.cache_order.clear();
        self.ahead.clear();
        if let Ok(rd) = std::fs::read_dir(&self.dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if let Some((x, y)) = name.strip_prefix('r').and_then(|n| n.strip_suffix(".txt")).and_then(|n| n.split_once('_'))
                    && let (Ok(x), Ok(y)) = (x.parse(), y.parse())
                {
                    self.saved.insert((x, y));
                }
            }
        }
    }

    /// Start a new run: forget saved regions and make the whole world from
    /// the map at `origin`.
    pub fn begin(&mut self, sim: &mut Sim, mats: &Materials) {
        self.new_run();
        let (rw, rh) = (sim.world.w / self.region, sim.world.h / self.region);
        for ry in 0..rh {
            for rx in 0..rw {
                self.fill(sim, mats, rx, ry);
            }
        }
    }

    /// Call once a frame with where the player (or camera) is, in world
    /// cells. Near an edge, the world scrolls one region; returns how far
    /// everything moved (cells), so cameras can move with it. A little
    /// further in, the strip ahead starts being made in the background.
    pub fn update(&mut self, sim: &mut Sim, mats: &Materials, focus: (f32, f32)) -> Option<(i32, i32)> {
        if !focus.0.is_finite() || !focus.1.is_finite() {
            return None;
        }
        let (w, h) = (sim.world.w as f32, sim.world.h as f32);
        let r = self.region as f32;
        let (can_x, can_y) = (w >= 3.0 * r, h >= 3.0 * r);
        let ri = self.region as i32;
        let near = |d: f32| {
            if can_x && focus.0 < d {
                Some((ri, 0))
            } else if can_x && focus.0 > w - d {
                Some((-ri, 0))
            } else if can_y && focus.1 < d {
                Some((0, ri))
            } else if can_y && focus.1 > h - d {
                Some((0, -ri))
            } else {
                None
            }
        };
        if let Some((dx, dy)) = near(r) {
            self.scroll(sim, mats, dx, dy);
            return Some((dx, dy));
        }
        if let Some((dx, dy)) = near(2.0 * r) {
            self.prepare(sim, mats, dx, dy);
        }
        // Tidy finished background saves into the memory copy.
        let done: Vec<(i64, i64)> = self.saving.iter().filter(|(_, j)| j.is_finished()).map(|(k, _)| *k).collect();
        for k in done {
            self.finish_save(k);
        }
        None
    }

    /// The world regions that leave and come in when scrolling (dx, dy).
    fn strips(&self, w: usize, h: usize, dx: i32, dy: i32) -> (Vec<(usize, usize)>, Vec<(usize, usize)>) {
        let r = self.region;
        let (rw, rh) = (w / r, h / r);
        if dx > 0 {
            ((0..rh).map(|y| (rw - 1, y)).collect(), (0..rh).map(|y| (0, y)).collect())
        } else if dx < 0 {
            ((0..rh).map(|y| (0, y)).collect(), (0..rh).map(|y| (rw - 1, y)).collect())
        } else if dy > 0 {
            ((0..rw).map(|x| (x, rh - 1)).collect(), (0..rw).map(|x| (x, 0)).collect())
        } else {
            ((0..rw).map(|x| (x, 0)).collect(), (0..rw).map(|x| (x, rh - 1)).collect())
        }
    }

    fn shared(&mut self, mats: &Materials) -> (Arc<Materials>, Arc<Vec<Tile>>) {
        let stale = self.shared.as_ref().is_none_or(|(m, t)| m.list.len() != mats.list.len() || t.len() != self.tiles.len());
        if stale {
            self.shared = Some((Arc::new(mats.clone()), Arc::new(self.tiles.clone())));
        }
        self.shared.clone().unwrap()
    }

    /// Start making the strip that would come in when scrolling (dx, dy),
    /// on other cores, so the scroll itself only has to copy it in.
    fn prepare(&mut self, sim: &Sim, mats: &Materials, dx: i32, dy: i32) {
        let (_, entering) = self.strips(sim.world.w, sim.world.h, dx, dy);
        let r = self.region as i64;
        let (sx, sy) = (-(dx as i64) / r, -(dy as i64) / r);
        for (rx, ry) in entering {
            let map = self.map_region(rx, ry);
            let map = (map.0 + sx, map.1 + sy);
            if self.saved.contains(&map) || self.ahead.contains_key(&map) {
                continue;
            }
            let (m, tiles) = self.shared(mats);
            let (settings, size) = (self.settings.clone(), self.region);
            self.ahead.insert(
                map,
                std::thread::spawn(move || {
                    let mut part = World::new(size, size);
                    levelgen::generate_at(&mut part, &m, (0, 0, size, size), (map.0 * size as i64, map.1 * size as i64), &settings, &tiles);
                    part
                }),
            );
        }
    }

    /// Scroll the window: everything moves (dx, dy) cells (one region).
    pub fn scroll(&mut self, sim: &mut Sim, mats: &Materials, dx: i32, dy: i32) {
        let _all = crate::profile::scope("endless.scroll");
        let (leaving, entering) = self.strips(sim.world.w, sim.world.h, dx, dy);
        for &(rx, ry) in &leaving {
            let _p = crate::profile::scope("endless.save");
            self.save(sim, mats, rx, ry);
        }
        {
            let _p = crate::profile::scope("endless.shift");
            sim.shift(dx, dy);
        }
        self.origin.0 -= dx as i64;
        self.origin.1 -= dy as i64;
        for &(rx, ry) in &entering {
            let _p = crate::profile::scope("endless.fill");
            self.fill(sim, mats, rx, ry);
        }
        // What was made ahead for the other way is no use now.
        let keep: HashSet<(i64, i64)> = entering.iter().map(|&(x, y)| self.map_region(x, y)).collect();
        self.ahead.retain(|k, _| keep.contains(k));
    }

    /// Which map region world region (rx, ry) is.
    fn map_region(&self, rx: usize, ry: usize) -> (i64, i64) {
        let r = self.region as i64;
        (self.origin.0.div_euclid(r) + rx as i64, self.origin.1.div_euclid(r) + ry as i64)
    }

    /// Save world region (rx, ry): its loose objects leave the world, and
    /// its cells and those objects are written out in the background.
    fn save(&mut self, sim: &mut Sim, mats: &Materials, rx: usize, ry: usize) {
        let r = self.region;
        let (x0, y0) = (rx * r, ry * r);
        // Loose objects in the region: nothing refers to them.
        let mut used: HashSet<u64> = HashSet::new();
        for j in sim.physics.joints() {
            used.insert(j.a);
            used.insert(j.b);
        }
        used.extend(sim.follow);
        used.extend(sim.physics.upright().into_iter().map(|u| u.0));
        for t in &sim.triggers {
            if let crate::trigger::By::Object(id) = t.by {
                used.insert(id);
            }
        }
        used.extend(sim.lighting.lights.iter().filter_map(|l| l.on));
        used.extend(sim.pumps.iter().map(|p| p.drive));
        let inside = |x: f32, y: f32| x >= x0 as f32 && x < (x0 + r) as f32 && y >= y0 as f32 && y < (y0 + r) as f32;
        let mut bodies = Vec::new();
        for id in sim.physics.ids() {
            if used.contains(&id) || !sim.physics.pose_of(id).is_some_and(|(x, y, _)| inside(x, y)) {
                continue;
            }
            if let Some(mut b) = sim.physics.remove(&mut sim.world, id) {
                sim.instances.remove(&id);
                b.x -= x0 as f32;
                b.y -= y0 as f32;
                bodies.push(b);
            }
        }
        let mut part = World::new(r, r);
        part.ambient = sim.world.ambient;
        copy_block(&sim.world, (x0, y0), &mut part, (0, 0), (r, r));
        // (Cells of objects that stay are left out.)
        for i in 0..part.mat.len() {
            if part.is_owned(i) {
                part.mat[i] = crate::material::AIR;
                part.flags[i] = 0;
            }
        }
        let map = self.map_region(rx, ry);
        let file = self.region_file(map);
        let dir = self.dir.clone();
        let (m, _) = self.shared(mats);
        let kept = Arc::new((part, bodies));
        self.cache.insert(map, kept.clone());
        self.cache_order.retain(|k| *k != map);
        self.cache_order.push_back(map);
        while self.cache_order.len() > KEEP {
            if let Some(old) = self.cache_order.pop_front() {
                self.cache.remove(&old);
            }
        }
        // An older save of this region still being written: let it finish first.
        self.finish_save(map);
        self.saving.insert(
            map,
            std::thread::spawn(move || {
                let (part, bodies) = &*kept;
                let mut sub = crate::sim::Snapshot::of_world(part.clone());
                sub.instances = vec![None; bodies.len()];
                sub.bodies = bodies.clone();
                let text = scene::save(&sub, &m);
                std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&file, text)).map_err(|e| format!("couldn't save {}: {e}", file.display()))
            }),
        );
        self.saved.insert(map);
        self.stats.0 += 1;
    }

    /// Wait for a background save to finish (and hear if it failed).
    fn finish_save(&mut self, map: (i64, i64)) {
        if let Some(job) = self.saving.remove(&map) {
            match job.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => self.problems.push(e),
                Err(_) => self.problems.push("a background save failed".into()),
            }
        }
    }

    /// Fill world region (rx, ry): from what was saved if it's been visited
    /// this run, otherwise freshly made (ahead of time if possible).
    fn fill(&mut self, sim: &mut Sim, mats: &Materials, rx: usize, ry: usize) {
        let r = self.region;
        let (x0, y0) = (rx * r, ry * r);
        let map = self.map_region(rx, ry);
        if self.saved.contains(&map) {
            // Ready-made in memory, or read back from disk (after a restart).
            let kept = self.cache.get(&map).cloned().or_else(|| {
                self.finish_save(map);
                let path = self.region_file(map);
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| self.problems.push(format!("could not read {}: {e}", path.display())))
                    .ok()?;
                let (mut part, problems) = scene::load(&text, mats, None).into_sim(mats);
                if !problems.is_empty() || (part.world.w, part.world.h) != (r, r) {
                    self.problems.push(format!("could not restore {} as a {r}x{r} region", path.display()));
                    self.problems.extend(problems);
                    return None;
                }
                let psnap = part.snapshot();
                Some(Arc::new((psnap.world, psnap.bodies)))
            });
            if let Some(kept) = kept {
                let (part, bodies) = &*kept;
                copy_block(part, (0, 0), &mut sim.world, (x0, y0), (r, r));
                sim.world.wake_box(x0 as i32, y0 as i32, (x0 + r) as i32 - 1, (y0 + r) as i32 - 1);
                for b in bodies {
                    let mut b = b.clone();
                    b.x += x0 as f32;
                    b.y += y0 as f32;
                    sim.physics.add(&mut sim.world, mats, b);
                }
                self.stats.1 += 1;
                return;
            }
        }
        let part = match self.ahead.remove(&map).map(|job| job.join()) {
            Some(Ok(part)) => part,
            _ => {
                let mut part = World::new(r, r);
                levelgen::generate_at(&mut part, mats, (0, 0, r, r), (map.0 * r as i64, map.1 * r as i64), &self.settings, &self.tiles);
                part
            }
        };
        copy_block(&part, (0, 0), &mut sim.world, (x0, y0), (r, r));
        sim.world.wake_box(x0 as i32, y0 as i32, (x0 + r) as i32 - 1, (y0 + r) as i32 - 1);
        self.stats.2 += 1;
    }
}

/// Read an `endless` line: settings, tiles and region size.
pub fn parse(line: &str, mats: &Materials, base: Option<&Path>) -> Result<(Settings, Vec<Tile>, usize), String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let mut s = Settings::new(mats);
    s.open_sides = true;
    let (mut tiles, mut region) = (Vec::new(), 256usize);
    let mut k = 1;
    while k < words.len() {
        let (key, val) = (words[k], words.get(k + 1).copied().unwrap_or(""));
        let number = || val.parse::<f32>().map_err(|_| format!("`endless {key}` needs a number"));
        match key {
            "seed" => s.seed = number()? as u32,
            "tile" => s.tile = (number()? as usize).clamp(16, 256),
            "region" => region = (number()? as usize).clamp(64, 2048),
            "open" => s.open = number()?.clamp(0.0, 1.0),
            "pools" => s.pools = number()?.clamp(0.0, 1.0),
            "rooms" => s.rooms = number()?.clamp(0.0, 1.0),
            "caverns" => s.caverns = number()?.clamp(0.0, 1.0),
            "deadends" => s.dead_ends = number()?.clamp(0.0, 1.0),
            "bend" => s.bend = number()?.clamp(0.0, 1.0),
            "dangers" => s.dangers = number()?.clamp(0.0, 1.0),
            "crystals" => s.crystals = number()?.clamp(0.0, 1.0),
            "torches" => s.torches = number()?.clamp(0.0, 1.0),
            "rock" => s.rock = mats.id(val).ok_or_else(|| format!("no material called `{val}`"))?,
            "tiles" => {
                let dir = base.map(|b| b.join(val)).filter(|p| p.is_dir()).unwrap_or_else(|| PathBuf::from(val));
                let (t, problems) = Tile::load_dir(&dir, mats);
                if let Some(p) = problems.into_iter().next() {
                    return Err(p);
                }
                tiles = t;
            }
            _ => return Err(format!("`endless` doesn't know `{key}` (seed, tile, region, open, pools, rooms, caverns, deadends, bend, dangers, crystals, torches, rock, tiles)")),
        }
        k += 2;
    }
    Ok((s, tiles, region))
}

/// Every per-cell layer of a block, from one world to another.
fn copy_block(src: &World, from: (usize, usize), dst: &mut World, to: (usize, usize), size: (usize, usize)) {
    for y in 0..size.1 {
        let (sy, dy) = (from.1 + y, to.1 + y);
        if sy >= src.h || dy >= dst.h {
            continue;
        }
        let n = size.0.min(src.w - from.0).min(dst.w - to.0);
        let (s0, d0) = (sy * src.w + from.0, dy * dst.w + to.0);
        dst.mat[d0..d0 + n].copy_from_slice(&src.mat[s0..s0 + n]);
        dst.temp[d0..d0 + n].copy_from_slice(&src.temp[s0..s0 + n]);
        dst.life[d0..d0 + n].copy_from_slice(&src.life[s0..s0 + n]);
        dst.burn[d0..d0 + n].copy_from_slice(&src.burn[s0..s0 + n]);
        dst.flags[d0..d0 + n].copy_from_slice(&src.flags[s0..s0 + n]);
        dst.shade[d0..d0 + n].copy_from_slice(&src.shade[s0..s0 + n]);
        dst.depth_air[d0..d0 + n].copy_from_slice(&src.depth_air[s0..s0 + n]);
        dst.tex[d0..d0 + n].copy_from_slice(&src.tex[s0..s0 + n]);
        dst.paint[d0..d0 + n].copy_from_slice(&src.paint[s0..s0 + n]);
        dst.coat[d0..d0 + n].copy_from_slice(&src.coat[s0..s0 + n]);
        dst.coat_until[d0..d0 + n].copy_from_slice(&src.coat_until[s0..s0 + n]);
        dst.back[d0..d0 + n].copy_from_slice(&src.back[s0..s0 + n]);
    }
}
