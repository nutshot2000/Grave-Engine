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
//! What streams: cells (with paint, films, the wall behind, anchoring) and
//! loose objects (crates, debris). Saved regions keep their shapes but not
//! their heat or fires. Objects tied to something (joints, the camera, a
//! trigger, a lamp, a pump) aren't saved away: keep them near the player.

use crate::levelgen::{self, Settings, Tile};
use crate::physics::BodySpec;
use crate::sim::{Sim, Snapshot};
use crate::{Materials, World, scene};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

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
}

impl Endless {
    pub fn new(settings: Settings, tiles: Vec<Tile>, region: usize, dir: PathBuf) -> Self {
        let mut settings = settings;
        settings.open_sides = true;
        Endless { settings, tiles, region: region.clamp(64, 2048), dir, origin: (0, 0), saved: HashSet::new(), stats: (0, 0, 0), problems: Vec::new() }
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
        let _ = std::fs::remove_dir_all(&self.dir);
        self.saved.clear();
        self.stats = (0, 0, 0);
    }

    /// Carry on a run (after the editor restarts into a new build): the
    /// world shows the map at `origin`, and visited regions are on disk.
    pub fn resume(&mut self, origin: (i64, i64)) {
        self.origin = origin;
        self.saved.clear();
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
        let _ = std::fs::remove_dir_all(&self.dir);
        self.saved.clear();
        let mut snap = sim.snapshot();
        let (rw, rh) = (snap.world.w / self.region, snap.world.h / self.region);
        for ry in 0..rh {
            for rx in 0..rw {
                self.fill(&mut snap, mats, rx, ry);
            }
        }
        *sim = restore_keeping(sim, &snap, mats, (0, 0));
    }

    /// Call once a frame with where the player (or camera) is, in world
    /// cells. Near an edge, the world scrolls one region; returns how far
    /// everything moved (cells), so cameras can move with it.
    pub fn update(&mut self, sim: &mut Sim, mats: &Materials, focus: (f32, f32)) -> Option<(i32, i32)> {
        let (w, h) = (sim.world.w as f32, sim.world.h as f32);
        let r = self.region as f32;
        // (Scrolling needs room: at least three regions each way.)
        let (dx, dy) = if w >= 3.0 * r && focus.0 < r {
            (self.region as i32, 0)
        } else if w >= 3.0 * r && focus.0 > w - r {
            (-(self.region as i32), 0)
        } else if h >= 3.0 * r && focus.1 < r {
            (0, self.region as i32)
        } else if h >= 3.0 * r && focus.1 > h - r {
            (0, -(self.region as i32))
        } else {
            return None;
        };
        if !focus.0.is_finite() || !focus.1.is_finite() {
            return None;
        }
        self.scroll(sim, mats, dx, dy);
        Some((dx, dy))
    }

    /// Scroll the window: everything moves (dx, dy) cells (one region).
    pub fn scroll(&mut self, sim: &mut Sim, mats: &Materials, dx: i32, dy: i32) {
        let mut snap = sim.snapshot();
        let r = self.region;
        let (rw, rh) = (snap.world.w / r, snap.world.h / r);
        let (leaving, entering): (Vec<(usize, usize)>, Vec<(usize, usize)>) = if dx > 0 {
            ((0..rh).map(|y| (rw - 1, y)).collect(), (0..rh).map(|y| (0, y)).collect())
        } else if dx < 0 {
            ((0..rh).map(|y| (0, y)).collect(), (0..rh).map(|y| (rw - 1, y)).collect())
        } else if dy > 0 {
            ((0..rw).map(|x| (x, rh - 1)).collect(), (0..rw).map(|x| (x, 0)).collect())
        } else {
            ((0..rw).map(|x| (x, 0)).collect(), (0..rw).map(|x| (x, rh - 1)).collect())
        };
        for &(rx, ry) in &leaving {
            self.save(&mut snap, mats, rx, ry);
        }
        snap.world.shift(dx, dy);
        shift_snapshot(&mut snap, dx, dy);
        self.origin.0 -= dx as i64;
        self.origin.1 -= dy as i64;
        for &(rx, ry) in &entering {
            self.fill(&mut snap, mats, rx, ry);
        }
        *sim = restore_keeping(sim, &snap, mats, (dx, dy));
    }

    /// Which map region world region (rx, ry) is.
    fn map_region(&self, rx: usize, ry: usize) -> (i64, i64) {
        let r = self.region as i64;
        (self.origin.0.div_euclid(r) + rx as i64, self.origin.1.div_euclid(r) + ry as i64)
    }

    /// Save world region (rx, ry) and the loose objects in it to disk (they
    /// leave the snapshot).
    fn save(&mut self, snap: &mut Snapshot, mats: &Materials, rx: usize, ry: usize) {
        let r = self.region;
        let (x0, y0) = (rx * r, ry * r);
        let mut part = World::new(r, r);
        part.ambient = snap.world.ambient;
        copy_block(&snap.world, (x0, y0), &mut part, (0, 0), (r, r));
        let inside = |b: &BodySpec| b.x >= x0 as f32 && b.x < (x0 + r) as f32 && b.y >= y0 as f32 && b.y < (y0 + r) as f32;
        let mut bodies = take_loose_bodies(snap, inside);
        for b in bodies.iter_mut() {
            b.x -= x0 as f32;
            b.y -= y0 as f32;
        }
        let mut sub = Snapshot::of_world(part);
        sub.instances = vec![None; bodies.len()];
        sub.bodies = bodies;
        let file = self.region_file(self.map_region(rx, ry));
        let result = std::fs::create_dir_all(&self.dir).and_then(|_| std::fs::write(&file, scene::save(&sub, mats)));
        match result {
            Ok(()) => {
                self.saved.insert(self.map_region(rx, ry));
                self.stats.0 += 1;
            }
            Err(e) => self.problems.push(format!("couldn't save {}: {e}", file.display())),
        }
    }

    /// Fill world region (rx, ry): from disk if it's been visited this run,
    /// otherwise freshly generated.
    fn fill(&mut self, snap: &mut Snapshot, mats: &Materials, rx: usize, ry: usize) {
        let r = self.region;
        let (x0, y0) = (rx * r, ry * r);
        let map = self.map_region(rx, ry);
        if self.saved.contains(&map)
            && let Ok(text) = std::fs::read_to_string(self.region_file(map))
        {
            let (mut part, problems) = scene::load(&text, mats, None).into_sim(mats);
            self.problems.extend(problems);
            let psnap = part.snapshot();
            copy_block(&psnap.world, (0, 0), &mut snap.world, (x0, y0), (r, r));
            for mut b in psnap.bodies {
                b.x += x0 as f32;
                b.y += y0 as f32;
                snap.bodies.push(b);
                snap.instances.push(None);
            }
            self.stats.1 += 1;
            return;
        }
        levelgen::generate_at(&mut snap.world, mats, (x0, y0, r, r), self.origin, &self.settings, &self.tiles);
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
            "rock" => s.rock = mats.id(val).ok_or_else(|| format!("no material called `{val}`"))?,
            "tiles" => {
                let dir = base.map(|b| b.join(val)).filter(|p| p.is_dir()).unwrap_or_else(|| PathBuf::from(val));
                let (t, problems) = Tile::load_dir(&dir, mats);
                if let Some(p) = problems.into_iter().next() {
                    return Err(p);
                }
                tiles = t;
            }
            _ => return Err(format!("`endless` doesn't know `{key}` (seed, tile, region, open, pools, rooms, rock, tiles)")),
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
        dst.paint[d0..d0 + n].copy_from_slice(&src.paint[s0..s0 + n]);
        dst.coat[d0..d0 + n].copy_from_slice(&src.coat[s0..s0 + n]);
        dst.coat_until[d0..d0 + n].copy_from_slice(&src.coat_until[s0..s0 + n]);
        dst.back[d0..d0 + n].copy_from_slice(&src.back[s0..s0 + n]);
    }
}

/// Remove the loose objects (nothing refers to them: no joint, camera,
/// balance, trigger, lamp or pump) that `pick` chooses, renumbering what
/// refers to the rest. Returns the removed ones.
fn take_loose_bodies(snap: &mut Snapshot, pick: impl Fn(&BodySpec) -> bool) -> Vec<BodySpec> {
    let mut used: HashSet<u64> = HashSet::new();
    for j in &snap.joints {
        used.insert(j.a);
        used.insert(j.b);
    }
    used.extend(snap.follow);
    used.extend(snap.upright.iter().map(|u| u.0));
    for t in &snap.triggers {
        if let crate::trigger::By::Object(n) = t.by {
            used.insert(n);
        }
    }
    used.extend(snap.lighting.lights.iter().filter_map(|l| l.on));
    used.extend(snap.pumps.iter().map(|p| p.drive));
    let mut taken = Vec::new();
    let mut new_number = vec![0u64; snap.bodies.len() + 1];
    let (mut kept, mut kept_instances) = (Vec::new(), Vec::new());
    let instances = std::mem::take(&mut snap.instances);
    for (n, (b, inst)) in std::mem::take(&mut snap.bodies).into_iter().zip(instances.into_iter().chain(std::iter::repeat(None))).enumerate() {
        if !used.contains(&(n as u64 + 1)) && pick(&b) {
            taken.push(b);
        } else {
            kept.push(b);
            kept_instances.push(inst);
            new_number[n + 1] = kept.len() as u64;
        }
    }
    snap.bodies = kept;
    snap.instances = kept_instances;
    let renumber = |n: u64| if n == 0 { 0 } else { new_number.get(n as usize).copied().unwrap_or(0) };
    for j in snap.joints.iter_mut() {
        j.a = renumber(j.a);
        j.b = renumber(j.b);
    }
    snap.follow = snap.follow.map(renumber);
    for u in snap.upright.iter_mut() {
        u.0 = renumber(u.0);
    }
    for t in snap.triggers.iter_mut() {
        if let crate::trigger::By::Object(n) = t.by {
            t.by = crate::trigger::By::Object(renumber(n));
        }
    }
    for l in snap.lighting.lights.iter_mut() {
        l.on = l.on.map(renumber);
    }
    for p in snap.pumps.iter_mut() {
        p.drive = renumber(p.drive);
    }
    taken
}

/// Move everything in a snapshot (besides the cells) by (dx, dy) cells.
fn shift_snapshot(snap: &mut Snapshot, dx: i32, dy: i32) {
    let (fx, fy) = (dx as f32, dy as f32);
    for b in snap.bodies.iter_mut() {
        b.x += fx;
        b.y += fy;
    }
    for j in snap.joints.iter_mut() {
        // Ends on the ground are in world cells.
        if j.a == 0 {
            j.anchor_a = (j.anchor_a.0 + fx, j.anchor_a.1 + fy);
        }
        if j.b == 0 {
            j.anchor_b = (j.anchor_b.0 + fx, j.anchor_b.1 + fy);
        }
    }
    for t in snap.triggers.iter_mut() {
        t.x += fx;
        t.y += fy;
    }
    if let Some(c) = snap.checkpoint.as_mut() {
        c.0 += fx;
        c.1 += fy;
    }
    for l in snap.lighting.lights.iter_mut().filter(|l| l.on.is_none()) {
        l.x += fx;
        l.y += fy;
    }
    let move_box = |b: &mut (usize, usize, usize, usize)| {
        b.0 = (b.0 as i64 + dx as i64).max(0) as usize;
        b.1 = (b.1 as i64 + dy as i64).max(0) as usize;
    };
    for p in snap.pumps.iter_mut() {
        move_box(&mut p.intake);
        move_box(&mut p.outlet);
    }
    for a in snap.actors.iter_mut() {
        a.x += fx;
        a.y += fy;
        a.spawn = (a.spawn.0 + fx, a.spawn.1 + fy);
        if let Some(h) = a.ledge.as_mut() {
            h.edge = (h.edge.0 + dx, h.edge.1 + dy);
        }
    }
}

/// A new running sim from `snap`, keeping what isn't saved in snapshots
/// (camera effects, the weather's state) from the old one.
fn restore_keeping(old: &mut Sim, snap: &Snapshot, mats: &Materials, shift: (i32, i32)) -> Sim {
    let mut sim = Sim::restore(snap, mats);
    sim.camera = std::mem::take(&mut old.camera);
    sim.weather_run = std::mem::take(&mut old.weather_run);
    sim.weather_view = old.weather_view.map(|(x0, y0, x1, y1)| (x0 + shift.0 as f32, y0 + shift.1 as f32, x1 + shift.0 as f32, y1 + shift.1 as f32));
    sim
}
