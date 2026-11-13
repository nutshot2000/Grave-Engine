//! Sparse scrolling terrain. Only authored or damaged 64-cell patches allocate
//! native material volumes. Near patches run at 60 Hz; distant changed patches
//! retain their exact material/body state and run at a staggered 5 Hz detail rate.
use super::{Explosion, Point, Volume, space::Space};
use crate::{MatId, Materials, asset::pack_paint, physics::BodyCell};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
pub const TILE: usize = 64;
pub struct Region {
    pub volume: Volume,
    pub touched: bool,
    pub urgent: u32,
    pub last_step: u32,
}
pub struct WorldMap {
    pub width: usize,
    pub depth: usize,
    pub height: usize,
    pub frame: u32,
    pub regions: BTreeMap<(usize, usize), Region>,
    pub explosions: Vec<Explosion>,
    pub focus: Point,
    pub active_radius: f32,
    pub active_regions: usize,
    /// Fixed work quotas keep distant fires from creating synchronized spikes.
    pub distant_steps: usize,
    far_cursor: usize,
    terrain: Vec<u8>,
    terrain_w: usize,
    pub roads: Vec<[f32; 5]>,
    floor: MatId,
    materials: Materials,
}
impl WorldMap {
    pub fn load(path: &Path, m: &Materials) -> Result<(Self, Point), String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut size = [1536, 1152, 88];
        let mut spawn = Point::new(80., 80., 30.);
        let mut entries = Vec::new();
        let mut roads = Vec::new();
        for raw in text.lines() {
            let t: Vec<_> = raw.split_whitespace().collect();
            if t.is_empty() || t[0].starts_with('#') {
                continue;
            }
            let n = |i: usize| t[i].parse::<usize>().map_err(|e| e.to_string());
            match t[0] {
                "size" if t.len() == 4 => size = [n(1)?, n(2)?, n(3)?],
                "spawn" if t.len() == 4 => {
                    spawn = Point::new(n(1)? as f32, n(2)? as f32, n(3)? as f32)
                }
                "region" if t.len() == 5 => entries.push((t[1].to_string(), n(2)?, n(3)?, n(4)?)),
                "road" if t.len() == 6 => roads.push([
                    n(1)? as f32,
                    n(2)? as f32,
                    n(3)? as f32,
                    n(4)? as f32,
                    n(5)? as f32,
                ]),
                _ => return Err(format!("Bad scrolling-map command: {raw}")),
            }
        }
        if size[0] < 64
            || size[1] < 64
            || size[0] > 8192
            || size[1] > 8192
            || size[2] < 16
            || size[2] > 128
        {
            return Err("Map dimensions outside 64..8192 × 64..8192 × 16..128".into());
        }
        let tw = size[0].div_ceil(8);
        let th = size[1].div_ceil(8);
        let mut terrain = vec![2u8; tw * th];
        for y in 0..th {
            for x in 0..tw {
                // Long stepped ridges with broad passes, all collision-backed.
                let wave = ((x as f32 * 0.105).sin() + (y as f32 * 0.12).cos()) * 0.5;
                terrain[y * tw + x] = if wave > 0.62 {
                    26
                } else if wave > 0.42 {
                    18
                } else if wave > 0.24 {
                    10
                } else {
                    2
                };
            }
        }
        let mut me = Self {
            width: size[0],
            depth: size[1],
            height: size[2],
            frame: 0,
            regions: BTreeMap::new(),
            explosions: vec![],
            focus: spawn,
            active_radius: 150.,
            active_regions: 0,
            distant_steps: 1,
            far_cursor: 0,
            terrain,
            terrain_w: tw,
            roads,
            floor: m.id("stone").ok_or("stone material missing")?,
            materials: m.clone(),
        };
        // Carve roads and flatten authored plots into the same collision heightfield.
        for y in 0..th {
            for x in 0..tw {
                if me.on_road(x as f32 * 8. + 4., y as f32 * 8. + 4.) {
                    me.terrain[y * tw + x] = 2;
                }
            }
        }
        for (file, tx, ty, ground) in entries {
            if tx * TILE + TILE > me.width || ty * TILE + TILE > me.depth || ground + 4 >= me.height
            {
                return Err("Region outside scrolling map".into());
            }
            let (v, _) = Volume::load(&path.parent().unwrap_or(Path::new(".")).join(file), m)?;
            if [v.width, v.depth, v.height] != [TILE, TILE, me.height] {
                return Err("Each region must be 64 × 64 × map height".into());
            }
            for y in ty * 8..ty * 8 + 8 {
                for x in tx * 8..tx * 8 + 8 {
                    me.terrain[y * tw + x] = ground as u8;
                }
            }
            if me
                .regions
                .insert(
                    (tx, ty),
                    Region {
                        volume: v,
                        touched: false,
                        urgent: 0,
                        last_step: 0,
                    },
                )
                .is_some()
            {
                return Err("Duplicate region".into());
            }
        }
        Ok((me, spawn))
    }
    pub fn on_road(&self, x: f32, y: f32) -> bool {
        self.roads.iter().any(|r| {
            let dx = r[2] - r[0];
            let dy = r[3] - r[1];
            let t =
                (((x - r[0]) * dx + (y - r[1]) * dy) / (dx * dx + dy * dy).max(1.)).clamp(0., 1.);
            (x - r[0] - t * dx).hypot(y - r[1] - t * dy) < r[4]
        })
    }
    pub fn ground_height(&self, x: f32, y: f32) -> usize {
        if x < 0. || y < 0. || x >= self.width as f32 || y >= self.depth as f32 {
            return 0;
        }
        self.terrain[(y as usize / 8) * self.terrain_w + x as usize / 8] as usize
    }
    pub fn ground_color(&self, x: f32, y: f32) -> [u8; 3] {
        if self.on_road(x, y) {
            [158, 131, 86]
        } else {
            let h = self.ground_height(x, y);
            if h > 15 {
                [161, 126, 83]
            } else if h > 5 {
                [181, 145, 96]
            } else {
                [190, 164, 118]
            }
        }
    }
    pub fn offset(key: (usize, usize)) -> Point {
        Point::new((key.0 * TILE) as f32, (key.1 * TILE) as f32, 0.)
    }
    fn key(&self, p: Point) -> Option<(usize, usize)> {
        self.contains(p)
            .then_some((p.x as usize / TILE, p.y as usize / TILE))
    }
    pub fn local(p: Point, o: Point) -> Point {
        Point::new(p.x - o.x, p.y - o.y, p.z)
    }
    fn ensure(&mut self, k: (usize, usize), m: &Materials) {
        if self.regions.contains_key(&k) {
            return;
        }
        let mut v = Volume::new(TILE, TILE, self.height, m).unwrap();
        for y in 0..TILE {
            for x in 0..TILE {
                let wx = k.0 * TILE + x;
                let wy = k.1 * TILE + y;
                let h = self.ground_height(wx as f32, wy as f32);
                v.box_part(
                    "terrain",
                    self.floor,
                    [x, y, 0],
                    [1, 1, h + 1],
                    Some(self.ground_color(wx as f32, wy as f32)),
                    m,
                );
            }
        }
        v.parts.clear();
        self.regions.insert(
            k,
            Region {
                volume: v,
                touched: true,
                urgent: self.frame + 2,
                last_step: 0,
            },
        );
    }
    pub fn step(&mut self, m: &Materials) {
        self.frame = self.frame.wrapping_add(1);
        self.explosions.clear();
        self.active_regions = 0;
        let mut distant = Vec::new();
        for (&key, r) in &mut self.regions {
            let o = Self::offset(key);
            let dx = (self.focus.x - o.x - 32.).abs() - 32.;
            let dy = (self.focus.y - o.y - 32.).abs() - 32.;
            if dx.max(0.).hypot(dy.max(0.)) < self.active_radius {
                prepare_ground(
                    key,
                    r,
                    &self.terrain,
                    self.terrain_w,
                    self.width,
                    self.depth,
                );
                step_region(key, r, m, self.frame, &mut self.explosions);
                self.active_regions += 1;
            } else if r.touched {
                distant.push(key);
            }
        }
        // Round robin is deterministic and never lets a busy earlier region
        // starve later ones. Distant simulation is deliberately lower detail.
        let mut done = 0;
        for _ in 0..distant.len() {
            if done >= self.distant_steps {
                break;
            }
            let key = distant[self.far_cursor % distant.len()];
            self.far_cursor = (self.far_cursor + 1) % distant.len();
            let r = self.regions.get_mut(&key).unwrap();
            if r.urgent > self.frame || self.frame.saturating_sub(r.last_step) >= 12 {
                prepare_ground(
                    key,
                    r,
                    &self.terrain,
                    self.terrain_w,
                    self.width,
                    self.depth,
                );
                step_region(key, r, m, self.frame, &mut self.explosions);
                self.active_regions += 1;
                done += 1;
            }
        }
        let mut seen = BTreeSet::new();
        self.explosions.retain(|e| {
            seen.insert((
                e.at.x.to_bits(),
                e.at.y.to_bits(),
                e.at.z.to_bits(),
                e.radius.to_bits(),
                e.power.to_bits(),
                e.fire,
            ))
        });
    }
    pub fn fragment_count(&self) -> usize {
        self.regions.values().map(|r| r.volume.debris.len()).sum()
    }
    pub fn plume_count(&self) -> usize {
        self.regions.values().map(|r| r.volume.plumes.len()).sum()
    }
    pub fn count(&self, id: MatId) -> usize {
        self.regions.values().map(|r| r.volume.count(id)).sum()
    }
    pub fn allocated_cells(&self) -> usize {
        self.regions.len() * TILE * TILE * self.height
    }
    pub fn temperature(&self, p: Point) -> f32 {
        self.cell(p).map_or(20., |c| c.temp)
    }
    pub fn surface(&self, x: f32, y: f32) -> (usize, [u8; 3]) {
        let k = (x.max(0.) as usize / TILE, y.max(0.) as usize / TILE);
        if let Some(r) = self.regions.get(&k) {
            let o = Self::offset(k);
            for z in (0..self.height).rev() {
                let p = Point::new(x - o.x, y - o.y, z as f32);
                if let Some(c) = r.volume.cell(p) {
                    if self.materials.get(c.mat).state != crate::State::Gas {
                        return (
                            z,
                            crate::asset::paint_rgb(c.paint)
                                .unwrap_or(self.materials.get(c.mat).color),
                        );
                    }
                }
            }
        }
        (self.ground_height(x, y), self.ground_color(x, y))
    }
}
impl Space for WorldMap {
    fn extent(&self) -> [usize; 3] {
        [self.width, self.depth, self.height]
    }
    fn frame(&self) -> u32 {
        self.frame
    }
    fn explosions(&self) -> &[Explosion] {
        &self.explosions
    }
    fn material_at(&self, p: Point) -> MatId {
        let Some(k) = self.key(p) else {
            return 0;
        };
        if let Some(r) = self.regions.get(&k) {
            r.volume.material_at(Self::local(p, Self::offset(k)))
        } else if p.z <= self.ground_height(p.x, p.y) as f32 {
            self.floor
        } else {
            0
        }
    }
    fn cell(&self, p: Point) -> Option<BodyCell> {
        let k = self.key(p)?;
        if let Some(r) = self.regions.get(&k) {
            return r.volume.cell(Self::local(p, Self::offset(k)));
        }
        Some(BodyCell {
            mat: if p.z <= self.ground_height(p.x, p.y) as f32 {
                self.floor
            } else {
                0
            },
            temp: 20.,
            burn: 0,
            life: 0,
            flags: 0,
            shade: 128,
            paint: pack_paint(self.ground_color(p.x, p.y)),
        })
    }
    fn take_cell(&mut self, p: Point) -> Option<BodyCell> {
        let k = self.key(p)?;
        if !self.regions.contains_key(&k) {
            let m = self.materials.clone();
            self.ensure(k, &m);
        }
        let r = self.regions.get_mut(&k)?;
        r.touched = true;
        r.urgent = self.frame + 2;
        r.volume.take_cell(Self::local(p, Self::offset(k)))
    }
    fn put_cell(&mut self, p: Point, c: Option<BodyCell>, m: &Materials) {
        if let Some(k) = self.key(p) {
            self.ensure(k, m);
            let r = self.regions.get_mut(&k).unwrap();
            r.volume.put_cell(Self::local(p, Self::offset(k)), c, m);
            r.touched = true;
            r.urgent = self.frame + 2;
        }
    }
    fn heat(&mut self, p: Point, radius: f32, t: f32) {
        let m = self.materials.clone();
        if let Some(k) = self.key(p) {
            self.ensure(k, &m);
        }
        for (&k, r) in &mut self.regions {
            let o = Self::offset(k);
            let local = Self::local(p, o);
            if local.x >= -radius
                && local.x < TILE as f32 + radius
                && local.y >= -radius
                && local.y < TILE as f32 + radius
            {
                r.volume.heat(local, radius, t);
                r.touched = true;
                r.urgent = self.frame + 2;
            }
        }
    }
    fn impact(&mut self, p: Point, radius: f32, power: f32, m: &Materials) {
        if let Some(k) = self.key(p) {
            self.ensure(k, m);
        }
        for (&k, r) in &mut self.regions {
            let local = Self::local(p, Self::offset(k));
            if local.x >= -radius
                && local.x < TILE as f32 + radius
                && local.y >= -radius
                && local.y < TILE as f32 + radius
            {
                r.volume.impact(local, radius, power, m);
                r.touched = true;
                r.urgent = self.frame + 2;
            }
        }
    }
    fn explode(&mut self, e: Explosion) {
        let m = self.materials.clone();
        if let Some(k) = self.key(e.at) {
            self.ensure(k, &m);
        }
        for (&k, r) in &mut self.regions {
            let local = Self::local(e.at, Self::offset(k));
            if local.x >= -e.radius
                && local.x < TILE as f32 + e.radius
                && local.y >= -e.radius
                && local.y < TILE as f32 + e.radius
            {
                r.volume.explode(Explosion { at: local, ..e });
                r.touched = true;
                r.urgent = self.frame + 2;
            }
        }
    }
}

fn step_region(
    key: (usize, usize),
    r: &mut Region,
    m: &Materials,
    frame: u32,
    events: &mut Vec<Explosion>,
) {
    r.volume.step(m);
    r.last_step = frame;
    if !r.volume.debris.is_empty() || !r.volume.plumes.is_empty() || !r.volume.explosions.is_empty()
    {
        r.touched = true;
    }
    let o = WorldMap::offset(key);
    for e in &r.volume.explosions {
        events.push(Explosion {
            at: Point::new(e.at.x + o.x, e.at.y + o.y, e.at.z),
            ..*e
        });
    }
}

fn prepare_ground(
    key: (usize, usize),
    r: &mut Region,
    terrain: &[u8],
    tw: usize,
    width: usize,
    depth: usize,
) {
    if r.volume.debris.is_empty() {
        return;
    }
    let o = WorldMap::offset(key);
    for f in r.volume.debris.states() {
        let cx = (f.at.x / 8.).floor() as i32;
        let cy = (f.at.y / 8.).floor() as i32;
        let radius = 1 + (f.extent.x.max(f.extent.y) / 16.) as i32;
        for y in cy - radius..=cy + radius {
            for x in cx - radius..=cx + radius {
                if x >= 0 && x < 8 && y >= 0 && y < 8 {
                    continue;
                }
                let wx = o.x + x as f32 * 8.;
                let wy = o.y + y as f32 * 8.;
                let h = if wx >= 0. && wy >= 0. && wx < width as f32 && wy < depth as f32 {
                    terrain[(wy as usize / 8) * tw + wx as usize / 8] as f32 + 1.
                } else {
                    1.
                };
                r.volume.debris.ensure_ground_patch(
                    (x, y),
                    Point::new(x as f32 * 8., y as f32 * 8., 0.),
                    Point::new(8., 8., h),
                    0.8,
                );
            }
        }
    }
}
