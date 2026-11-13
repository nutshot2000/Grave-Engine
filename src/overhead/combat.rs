//! Reusable height-aware combat: data-defined ammunition, target bounds,
//! armour, guidance and impact events. Weapon names and mission rules are games.
use super::{Explosion, Point, space::Space};
use crate::{Materials, State};
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}
impl Bounds {
    pub fn centre(self) -> Point {
        self.min.lerp(self.max, 0.5)
    }
    pub fn contains(self, p: Point) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }
    pub fn ray(self, a: Point, b: Point) -> Option<f32> {
        let av = [a.x, a.y, a.z];
        let bv = [b.x, b.y, b.z];
        let lo = [self.min.x, self.min.y, self.min.z];
        let hi = [self.max.x, self.max.y, self.max.z];
        let (mut near, mut far) = (0f32, 1f32);
        for k in 0..3 {
            let d = bv[k] - av[k];
            if d.abs() < 0.00001 {
                if av[k] < lo[k] || av[k] > hi[k] {
                    return None;
                }
            } else {
                let x = (lo[k] - av[k]) / d;
                let y = (hi[k] - av[k]) / d;
                near = near.max(x.min(y));
                far = far.min(x.max(y));
                if near > far {
                    return None;
                }
            }
        }
        Some(near)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Weapon {
    pub speed: f32,
    pub range: f32,
    pub damage: f32,
    pub penetration: f32,
    pub radius: f32,
    pub force: f32,
    pub spark_heat: f32,
    pub guidance: f32,
    pub explosive: bool,
}
impl Weapon {
    pub fn launch(self, from: Point, aim: Point, faction: u8, target: Option<u32>) -> Shot {
        let d = from.distance(aim).max(0.01);
        Shot {
            at: from,
            velocity: Point::new(
                (aim.x - from.x) * self.speed / d,
                (aim.y - from.y) * self.speed / d,
                (aim.z - from.z) * self.speed / d,
            ),
            weapon: self,
            faction,
            target,
            life: 6.,
            travelled: 0.,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Target {
    pub id: u32,
    pub faction: u8,
    pub bounds: Bounds,
    pub health: f32,
    pub max_health: f32,
    pub armour: f32,
    pub death_blast: Option<(f32, f32)>,
    pub destroyed: bool,
    /// Material-backed targets take hits at actual surfaces, not empty bounds.
    pub material_backed: bool,
    material_initial: Option<usize>,
}
impl Target {
    pub fn new(id: u32, faction: u8, bounds: Bounds, health: f32, armour: f32) -> Self {
        Self {
            id,
            faction,
            bounds,
            health,
            max_health: health,
            armour,
            death_blast: None,
            destroyed: false,
            material_backed: false,
            material_initial: None,
        }
    }
    pub fn alive(&self) -> bool {
        self.health > 0.
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Shot {
    pub at: Point,
    pub velocity: Point,
    pub weapon: Weapon,
    pub faction: u8,
    pub target: Option<u32>,
    pub life: f32,
    pub travelled: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct Impact {
    pub at: Point,
    pub target: Option<u32>,
    pub faction: u8,
    pub explosive: bool,
}
#[derive(Default)]
pub struct Combat {
    pub targets: Vec<Target>,
    pub shots: Vec<Shot>,
    pub impacts: Vec<Impact>,
    pub destroyed: Vec<u32>,
}
impl Combat {
    pub fn target(&self, id: u32) -> Option<&Target> {
        self.targets.iter().find(|t| t.id == id)
    }
    pub fn target_mut(&mut self, id: u32) -> Option<&mut Target> {
        self.targets.iter_mut().find(|t| t.id == id)
    }
    pub fn alive(&self, id: u32) -> bool {
        self.target(id).is_some_and(|t| t.alive())
    }
    pub fn step(&mut self, v: &mut impl Space, m: &Materials, dt: f32) {
        self.impacts.clear();
        self.destroyed.clear();
        let mut kept = Vec::new();
        for mut s in self.shots.drain(..) {
            if let Some(t) = s
                .target
                .and_then(|id| self.targets.iter().find(|t| t.id == id && t.alive()))
            {
                if s.weapon.guidance > 0. {
                    let aim = t.bounds.centre();
                    let d = s.at.distance(aim).max(0.01);
                    let target = Point::new(
                        (aim.x - s.at.x) * s.weapon.speed / d,
                        (aim.y - s.at.y) * s.weapon.speed / d,
                        (aim.z - s.at.z) * s.weapon.speed / d,
                    );
                    s.velocity = s.velocity.lerp(target, (dt * s.weapon.guidance).min(1.));
                }
            }
            let velocity = s.velocity.distance(Point::default()).max(0.001);
            let dt = dt
                .min(s.life)
                .min((s.weapon.range - s.travelled).max(0.) / velocity);
            if dt <= 0. {
                continue;
            }
            let end = Point::new(
                s.at.x + s.velocity.x * dt,
                s.at.y + s.velocity.y * dt,
                s.at.z + s.velocity.z * dt,
            );
            let mut hit = v.raycast(s.at, end, m).map(|h| {
                (
                    s.at.distance(h.at) / s.at.distance(end).max(0.01),
                    h.at,
                    None,
                )
            });
            for t in self
                .targets
                .iter()
                .filter(|t| t.alive() && t.faction != s.faction)
            {
                if t.material_backed {
                    if let Some(real) = v.raycast(s.at, end, m) {
                        let time = s.at.distance(real.at) / s.at.distance(end).max(0.01);
                        if t.bounds.contains(real.at)
                            && hit.is_none_or(|(n, _, _)| time <= n + 0.0001)
                        {
                            hit = Some((time, real.at, Some(t.id)));
                        }
                    }
                    continue;
                }
                if let Some(time) = t.bounds.ray(s.at, end) {
                    let p = s.at.lerp(end, time);
                    if hit.is_none_or(|(n, point, _)| time <= n + 0.02 || t.bounds.contains(point))
                    {
                        hit = Some((time, p, Some(t.id)));
                    }
                }
            }
            if let Some((_, mut at, target)) = hit {
                if let Some(t) = target.and_then(|id| self.targets.iter().find(|t| t.id == id)) {
                    if let Some(real) = v.raycast(s.at, end, m) {
                        if t.bounds.contains(real.at) {
                            at = real.at;
                        }
                    }
                }
                if let Some(t) = target.and_then(|id| self.targets.iter_mut().find(|t| t.id == id))
                {
                    let damage =
                        (s.weapon.damage - t.armour * (1. - s.weapon.penetration)).max(0.7);
                    t.health = (t.health - damage).max(0.);
                }
                if s.weapon.explosive {
                    v.explode(Explosion {
                        at,
                        radius: s.weapon.radius,
                        power: s.weapon.force,
                        fire: None,
                    });
                } else {
                    v.impact(at, s.weapon.radius, s.weapon.force, m);
                }
                // A hot contact can light exposed volatile liquid without
                // heating every board across an explosive radius.
                let material = m.get(v.material_at(at));
                if material.state == State::Liquid
                    && material
                        .catches_fire_at
                        .is_some_and(|t| t <= s.weapon.spark_heat)
                {
                    v.heat(at, 1.2, s.weapon.spark_heat);
                }
                self.impacts.push(Impact {
                    at,
                    target,
                    faction: s.faction,
                    explosive: s.weapon.explosive,
                });
            } else {
                s.travelled += s.at.distance(end);
                s.at = end;
                s.life -= dt;
                if s.life > 0. && v.contains(s.at) {
                    kept.push(s);
                }
            }
        }
        self.shots = kept;
        self.resolve_deaths(v, m);
    }
    /// Capture authored structure integrity once, before combat starts.
    pub fn bind_materials(&mut self, v: &impl Space, m: &Materials) {
        for t in self.targets.iter_mut().filter(|t| t.material_backed) {
            t.material_initial = Some(material_count(t.bounds, v, m));
        }
    }
    pub fn environment(&mut self, v: &mut impl Space, m: &Materials) {
        for t in self.targets.iter_mut().filter(|t| t.alive()) {
            if v.frame() % 30 == 0 {
                if let Some(initial) = t.material_initial.filter(|n| *n > 0) {
                    let ratio = material_count(t.bounds, v, m) as f32 / initial as f32;
                    t.health = t.health.min(if ratio < 0.15 {
                        0.
                    } else {
                        t.max_health * ratio
                    });
                }
            }
            let p = t.bounds.centre();
            for e in v.explosions() {
                let d = p.distance(e.at);
                if d < e.radius {
                    t.health = (t.health - (e.power * 0.004) * (1. - d / e.radius).powi(2)).max(0.);
                }
            }
            if let Some(cell) = v.cell(p) {
                if cell.temp > 600. {
                    t.health = (t.health - 0.15).max(0.);
                }
            }
        }
        self.resolve_deaths(v, m);
    }
    fn resolve_deaths(&mut self, v: &mut impl Space, _m: &Materials) {
        for t in &mut self.targets {
            if !t.alive() && !t.destroyed {
                t.destroyed = true;
                self.destroyed.push(t.id);
                if let Some((radius, power)) = t.death_blast {
                    v.explode(Explosion {
                        at: t.bounds.centre(),
                        radius,
                        power,
                        fire: None,
                    });
                }
            }
        }
    }
    /// Countermeasures break hostile guidance in a radius. Existing rounds
    /// continue ballistically; they are not silently deleted.
    pub fn decoy(&mut self, at: Point, radius: f32, hostile: u8) {
        for s in &mut self.shots {
            if s.faction == hostile && s.at.distance(at) < radius {
                s.target = None;
                s.weapon.guidance = 0.;
            }
        }
    }
}

fn material_count(bounds: Bounds, v: &impl Space, m: &Materials) -> usize {
    let mut count = 0;
    for y in bounds.min.y.max(0.) as usize..(bounds.max.y.ceil() as usize).min(v.extent()[1]) {
        for z in bounds.min.z.max(0.) as usize..(bounds.max.z.ceil() as usize).min(v.extent()[2]) {
            for x in
                bounds.min.x.max(0.) as usize..(bounds.max.x.ceil() as usize).min(v.extent()[0])
            {
                if m.get(v.material_at(Point::new(x as f32, y as f32, z as f32)))
                    .state
                    != State::Gas
                {
                    count += 1;
                }
            }
        }
    }
    count
}
