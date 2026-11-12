//! Height-aware worlds built from aligned X/height sections of the existing
//! material/rigid-body simulation. XY is the ground; Z is height, all in cells.
//! This preserves the material solver instead of inventing game-only fire rules.
use crate::{
    MatId, Materials, Sim, State,
    material::AIR,
    world::{ANCHORED, Blast},
};
use std::{collections::HashSet, path::Path};
pub mod view;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
impl Point {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    pub fn distance(self, b: Self) -> f32 {
        ((self.x - b.x).powi(2) + (self.y - b.y).powi(2) + (self.z - b.z).powi(2)).sqrt()
    }
    pub fn lerp(self, b: Self, t: f32) -> Self {
        Self::new(
            self.x + (b.x - self.x) * t,
            self.y + (b.y - self.y) * t,
            self.z + (b.z - self.z) * t,
        )
    }
}
#[derive(Clone, Debug)]
pub struct Part {
    pub name: String,
    pub material: MatId,
    pub at: [usize; 3],
    pub size: [usize; 3],
}
#[derive(Clone, Copy, Debug)]
pub struct Explosion {
    pub at: Point,
    pub radius: f32,
    pub power: f32,
    pub fire: Option<MatId>,
}
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub at: Point,
    pub material: MatId,
}
/// Independent games can use the world, flight controller, projection and
/// projectile sweeps separately. No helicopter or mission rules live here.
pub struct Volume {
    pub width: usize,
    pub depth: usize,
    pub height: usize,
    pub parts: Vec<Part>,
    pub frame: u32,
    pub sections: Vec<Sim>,
    pending: Vec<Explosion>,
    pub explosions: Vec<Explosion>,
}
impl Volume {
    pub fn new(width: usize, depth: usize, height: usize, m: &Materials) -> Result<Self, String> {
        if width < 8
            || depth < 4
            || height < 8
            || width > 512
            || depth > 256
            || height > 256
            || width * depth * height > 4_000_000
        {
            return Err(
                "Overhead world needs 8..512 × 4..256 × 8..256 cells, at most 4 million cells"
                    .into(),
            );
        }
        Ok(Self {
            width,
            depth,
            height,
            parts: vec![],
            frame: 0,
            sections: (0..depth).map(|_| Sim::empty(width, height, m)).collect(),
            pending: vec![],
            explosions: vec![],
        })
    }
    pub fn index(&self, p: Point) -> Option<(usize, usize)> {
        if !p.x.is_finite()
            || !p.y.is_finite()
            || !p.z.is_finite()
            || p.x < 0.
            || p.y < 0.
            || p.z < 0.
            || p.x >= self.width as f32
            || p.y >= self.depth as f32
            || p.z >= self.height as f32
        {
            return None;
        }
        Some((
            p.y as usize,
            (self.height - 1 - p.z as usize) * self.width + p.x as usize,
        ))
    }
    pub fn material_at(&self, p: Point) -> MatId {
        self.index(p)
            .map_or(AIR, |(y, i)| self.sections[y].world.mat[i])
    }
    pub fn solid_at(&self, p: Point, m: &Materials) -> bool {
        let id = self.material_at(p);
        id != AIR && m.get(id).state != State::Gas
    }
    /// Box placements are clipped, preserve named physical materials, and are
    /// reusable for walls, roofs, vehicles, furniture or entire prefabs.
    pub fn box_part(
        &mut self,
        name: &str,
        id: MatId,
        at: [usize; 3],
        size: [usize; 3],
        paint: Option<[u8; 3]>,
        m: &Materials,
    ) {
        self.parts.push(Part {
            name: name.into(),
            material: id,
            at,
            size,
        });
        for y in at[1]..at[1].saturating_add(size[1]).min(self.depth) {
            for z in at[2]..at[2].saturating_add(size[2]).min(self.height) {
                for x in at[0]..at[0].saturating_add(size[0]).min(self.width) {
                    let row = self.height - 1 - z;
                    let w = &mut self.sections[y].world;
                    w.set(m, x, row, id);
                    let i = row * self.width + x;
                    if let Some(c) = paint {
                        w.paint[i] = crate::asset::pack_paint(c);
                    }
                    if z < 2 && id != AIR {
                        w.flags[i] |= ANCHORED;
                    }
                }
            }
        }
    }
    pub fn heat(&mut self, at: Point, radius: f32, temp: f32) {
        if !radius.is_finite() || radius <= 0. || !temp.is_finite() {
            return;
        }
        for (y, s) in self.sections.iter_mut().enumerate() {
            let dy = (y as f32 + 0.5 - at.y).abs();
            if dy > radius {
                continue;
            }
            let r = (radius * radius - dy * dy).sqrt();
            s.world.heat_circle(
                at.x as i32,
                (self.height as f32 - 1. - at.z) as i32,
                r.ceil() as i32,
                temp,
            );
        }
    }
    pub fn explode(&mut self, e: Explosion) {
        if self.index(e.at).is_some()
            && e.radius.is_finite()
            && e.radius > 0.
            && e.power.is_finite()
            && e.power > 0.
        {
            self.pending.push(e);
        }
    }
    /// Swept height-aware collision: fast bullets cannot skip thin roofs or
    /// walls; a shot above a roof passes it without damaging the building.
    pub fn raycast(&self, a: Point, b: Point, m: &Materials) -> Option<Hit> {
        let n = (a.distance(b) / 0.35).ceil().clamp(1., 4096.) as usize;
        for k in 0..=n {
            let p = a.lerp(b, k as f32 / n as f32);
            if self.solid_at(p, m) {
                return Some(Hit {
                    at: p,
                    material: self.material_at(p),
                });
            }
        }
        None
    }
    pub fn count(&self, id: MatId) -> usize {
        self.sections
            .iter()
            .map(|s| s.world.mat.iter().filter(|v| **v == id).count())
            .sum()
    }
    /// Adjacent section faces exchange heat and oxygen. Gravity remains along
    /// height, not screen Y. Rigid fragments currently remain in their section.
    pub fn step(&mut self, m: &Materials) {
        self.frame = self.frame.wrapping_add(1);
        let feeding: Vec<Vec<bool>> = self
            .sections
            .iter()
            .map(|s| s.world.mat.iter().map(|id| m.get(*id).feeds_fire).collect())
            .collect();
        for y in 0..self.depth {
            let w = &mut self.sections[y].world;
            for i in 0..w.mat.len() {
                let air =
                    (y == 0 || feeding[y - 1][i]) || (y + 1 == self.depth || feeding[y + 1][i]);
                if w.depth_air[i] != air {
                    w.depth_air[i] = air;
                    w.wake_at(i);
                }
            }
        }
        for y in 1..self.depth {
            let (a, b) = self.sections.split_at_mut(y);
            crate::layers::exchange_heat(&mut a[y - 1].world, &mut b[0].world, m);
        }
        let threads = std::thread::available_parallelism()
            .map_or(1, |n| n.get())
            .min(4);
        let chunk = self.depth.div_ceil(threads);
        std::thread::scope(|scope| {
            for rows in self.sections.chunks_mut(chunk) {
                scope.spawn(move || {
                    for s in rows {
                        s.world.step(m);
                    }
                });
            }
        });
        // Nearby explosive cells form one volume blast rather than launching
        // the same charge once for every overlapping cross section.
        let mut occupied = HashSet::new();
        let mut events = std::mem::take(&mut self.pending);
        for (y, s) in self.sections.iter_mut().enumerate() {
            for b in s.world.blasts.drain(..) {
                let p = Point::new(b.x, y as f32, self.height as f32 - 1. - b.y);
                let key = ((p.x / 6.) as i32, (p.y / 6.) as i32, (p.z / 6.) as i32);
                if occupied.insert(key) {
                    events.push(Explosion {
                        at: p,
                        radius: b.size,
                        power: b.power,
                        fire: b.fire,
                    });
                }
            }
        }
        self.explosions = events;
        for e in &self.explosions {
            for (y, s) in self.sections.iter_mut().enumerate() {
                let dy = (y as f32 - e.at.y).abs();
                if dy >= e.radius {
                    continue;
                }
                let r = (e.radius * e.radius - dy * dy).sqrt();
                let wy = self.height as f32 - 1. - e.at.z;
                s.world
                    .heat_circle(e.at.x as i32, wy as i32, r.ceil() as i32, 1050.);
                s.world.blasts.push(Blast {
                    x: e.at.x,
                    y: wy,
                    size: r,
                    power: e.power * (1. - dy / e.radius).max(0.1),
                    fire: e.fire,
                });
            }
        }
        std::thread::scope(|scope| {
            for rows in self.sections.chunks_mut(chunk) {
                scope.spawn(move || {
                    for s in rows {
                        s.physics.update(&mut s.world, m);
                    }
                });
            }
        });
    }
    /// Simple authored boxes and reusable prefab files. Unknown commands and
    /// invalid materials produce errors instead of silently changing a level.
    pub fn load(path: &Path, m: &Materials) -> Result<(Self, Point), String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut v = None;
        let mut spawn = Point::new(20., 20., 24.);
        for (line, raw) in text.lines().enumerate() {
            let t: Vec<_> = raw.split_whitespace().collect();
            if t.is_empty() || t[0].starts_with('#') {
                continue;
            }
            let err = |s: String| format!("{}:{}: {s}", path.display(), line + 1);
            match t[0] {
                "size" if t.len() == 4 => {
                    let a = parse_xyz(&t[1..]).map_err(err)?;
                    v = Some(Self::new(a[0], a[1], a[2], m)?);
                }
                "spawn" if t.len() == 4 => {
                    let a = parse_xyz(&t[1..]).map_err(err)?;
                    spawn = Point::new(a[0] as f32, a[1] as f32, a[2] as f32);
                }
                "prop" if t.len() == 5 => {
                    let a = parse_xyz(&t[2..]).map_err(err)?;
                    let p = path.parent().unwrap_or(Path::new(".")).join(t[1]);
                    let data = std::fs::read_to_string(&p).map_err(|e| err(e.to_string()))?;
                    load_boxes(v.as_mut().ok_or("size must come first")?, &data, a, m)
                        .map_err(err)?;
                }
                "box" => load_boxes(v.as_mut().ok_or("size must come first")?, raw, [0; 3], m)
                    .map_err(err)?,
                _ => return Err(err(format!("Unknown overhead command: {}", t[0]))),
            }
        }
        let v = v.ok_or("Missing overhead size")?;
        if v.index(spawn).is_none() {
            return Err("Spawn is outside the flight volume".into());
        }
        Ok((v, spawn))
    }
}
fn parse_xyz(t: &[&str]) -> Result<[usize; 3], String> {
    if t.len() < 3 {
        return Err("Expected X Y Z".into());
    }
    Ok([
        t[0].parse().map_err(|_| "Invalid coordinate")?,
        t[1].parse().map_err(|_| "Invalid coordinate")?,
        t[2].parse().map_err(|_| "Invalid coordinate")?,
    ])
}
fn load_boxes(v: &mut Volume, text: &str, offset: [usize; 3], m: &Materials) -> Result<(), String> {
    for raw in text.lines() {
        let t: Vec<_> = raw.split_whitespace().collect();
        if t.is_empty() || t[0].starts_with('#') {
            continue;
        }
        if t.len() != 9 && t.len() != 10 {
            return Err("Use: box PART MATERIAL X Y Z WIDTH DEPTH HEIGHT [#RRGGBB]".into());
        }
        if t[0] != "box" {
            return Err("Prefab contains an unknown command".into());
        }
        let id = m.id(t[2]).ok_or(format!("Unknown material {}", t[2]))?;
        let mut at = parse_xyz(&t[3..6])?;
        let size = parse_xyz(&t[6..9])?;
        for i in 0..3 {
            at[i] = at[i].checked_add(offset[i]).ok_or("Coordinate overflow")?;
        }
        let paint = if t.len() == 10 {
            Some(
                crate::asset::parse_paint(t[9])
                    .and_then(crate::asset::paint_rgb)
                    .ok_or("Invalid paint colour")?,
            )
        } else {
            None
        };
        v.box_part(t[1], id, at, size, paint, m);
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, Default)]
pub struct FlightInput {
    pub movement: [f32; 2],
    pub climb: f32,
    pub heading: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct FlightSettings {
    pub acceleration: f32,
    pub speed: f32,
    pub climb_speed: f32,
    pub damping: f32,
    pub radius: f32,
}
impl Default for FlightSettings {
    fn default() -> Self {
        Self {
            acceleration: 75.,
            speed: 38.,
            climb_speed: 18.,
            damping: 3.2,
            radius: 3.,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Aircraft {
    pub at: Point,
    pub velocity: Point,
    pub heading: f32,
    pub settings: FlightSettings,
    pub blocked: bool,
}
impl Aircraft {
    pub fn new(at: Point) -> Self {
        Self {
            at,
            velocity: Point::default(),
            heading: 0.,
            settings: Default::default(),
            blocked: false,
        }
    }
    pub fn step(&mut self, input: FlightInput, v: &Volume, m: &Materials, dt: f32) {
        if !dt.is_finite() || dt <= 0. {
            return;
        }
        let dt = dt.min(0.05);
        let c = self.settings;
        let mut axis = input
            .movement
            .map(|x| if x.is_finite() { x.clamp(-1., 1.) } else { 0. });
        let len = axis[0].hypot(axis[1]).max(1.);
        for x in &mut axis {
            *x /= len;
        }
        self.velocity.x = (self.velocity.x + axis[0] * c.acceleration * dt) / (1. + c.damping * dt);
        self.velocity.y = (self.velocity.y + axis[1] * c.acceleration * dt) / (1. + c.damping * dt);
        let speed = self.velocity.x.hypot(self.velocity.y);
        if speed > c.speed {
            self.velocity.x *= c.speed / speed;
            self.velocity.y *= c.speed / speed;
        }
        let climb = if input.climb.is_finite() {
            input.climb.clamp(-1., 1.)
        } else {
            0.
        };
        self.velocity.z += (climb * c.climb_speed - self.velocity.z) * (dt * 8.).min(1.);
        let desired = Point::new(
            (self.at.x + self.velocity.x * dt).clamp(4., v.width as f32 - 5.),
            (self.at.y + self.velocity.y * dt).clamp(4., v.depth as f32 - 5.),
            (self.at.z + self.velocity.z * dt).clamp(5., v.height as f32 - 10.),
        );
        self.blocked = false;
        // Sweep the fuselage in small steps; rotor cosmetics do not snag walls.
        let steps = (self.at.distance(desired) / 0.5).ceil().max(1.) as usize;
        let start = self.at;
        for k in 1..=steps {
            let p = start.lerp(desired, k as f32 / steps as f32);
            let hit = [
                (0., 0.),
                (c.radius, 0.),
                (-c.radius, 0.),
                (0., c.radius),
                (0., -c.radius),
            ]
            .iter()
            .any(|(x, y)| v.solid_at(Point::new(p.x + x, p.y + y, p.z), m));
            if hit {
                self.velocity = Point::default();
                self.blocked = true;
                break;
            }
            self.at = p;
        }
        if input.heading.is_finite() {
            let diff = (input.heading - self.heading + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            self.heading += diff * (dt * 7.).min(1.);
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Projectile {
    pub at: Point,
    pub velocity: Point,
    pub remaining: f32,
    pub blast_radius: f32,
    pub power: f32,
    pub heat: f32,
    pub fire: Option<MatId>,
}
impl Projectile {
    pub fn aimed(
        from: Point,
        target: Point,
        speed: f32,
        blast_radius: f32,
        power: f32,
        heat: f32,
        fire: Option<MatId>,
    ) -> Self {
        let d = from.distance(target).max(0.01);
        Self {
            at: from,
            velocity: Point::new(
                (target.x - from.x) / d * speed,
                (target.y - from.y) / d * speed,
                (target.z - from.z) / d * speed,
            ),
            remaining: 3.,
            blast_radius,
            power,
            heat,
            fire,
        }
    }
    /// Returns true when consumed, whether by impact or its lifetime limit.
    pub fn step(&mut self, v: &mut Volume, m: &Materials, dt: f32) -> bool {
        let end = Point::new(
            self.at.x + self.velocity.x * dt,
            self.at.y + self.velocity.y * dt,
            self.at.z + self.velocity.z * dt,
        );
        if let Some(hit) = v.raycast(self.at, end, m) {
            if self.blast_radius > 0. {
                v.explode(Explosion {
                    at: hit.at,
                    radius: self.blast_radius,
                    power: self.power,
                    fire: self.fire,
                });
            }
            v.heat(hit.at, self.blast_radius.max(2.), self.heat);
            self.at = hit.at;
            return true;
        }
        self.at = end;
        self.remaining -= dt;
        self.remaining <= 0. || v.index(self.at).is_none()
    }
}
