//! Territorial cave actors: deterministic brains, ordinary material bodies,
//! telegraphed bites, and native articulated pixel art shared by every renderer.
use crate::{
    Materials, World,
    physics::{ActorSpec, ActorStatus, Controls, MobilitySettings, Physics},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Species {
    pub id: String,
    pub name: String,
    pub health: f32,
    pub speed: f32,
    pub sight: f32,
    pub hearing: f32,
    pub leash: f32,
    pub damage: f32,
    pub windup: u32,
    pub recovery: u32,
    pub avoidance: f32,
    pub height: f32,
    pub color: [u8; 3],
}
pub fn species() -> Result<Vec<Species>, String> {
    let text = std::fs::read_to_string("assets/creatures/cave.creatures")
        .unwrap_or_else(|_| include_str!("../assets/creatures/cave.creatures").into());
    let mut result = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parse = (|| -> Option<Species> {
            let p: Vec<_> = line.split('|').map(str::trim).collect();
            if p.len() != 13 {
                return None;
            }
            let f = |i: usize| {
                p[i].parse::<f32>()
                    .ok()
                    .filter(|v| v.is_finite() && *v > 0.0)
            };
            let color = p[11].trim_start_matches('#');
            if color.len() != 6 || !color.is_ascii() {
                return None;
            }
            Some(Species {
                id: p[0].into(),
                name: p[1].into(),
                health: f(2)?.clamp(1., 1000.),
                speed: f(3)?.clamp(0.2, 4.),
                sight: f(4)?.clamp(8., 160.),
                hearing: f(5)?.clamp(8., 200.),
                leash: f(6)?.clamp(12., 200.),
                damage: f(7)?.clamp(1., 100.),
                windup: (f(8)? as u32).clamp(12, 180),
                recovery: (f(9)? as u32).clamp(24, 600),
                avoidance: f(10)?.clamp(4., 40.),
                height: f(12)?.clamp(8., 15.),
                color: [
                    u8::from_str_radix(&color[0..2], 16).ok()?,
                    u8::from_str_radix(&color[2..4], 16).ok()?,
                    u8::from_str_radix(&color[4..6], 16).ok()?,
                ],
            })
        })();
        match parse {
            Some(s) => {
                if result.iter().any(|r: &Species| r.id == s.id) {
                    return Err(format!("duplicate species {}", s.id));
                }
                result.push(s);
            }
            None => return Err(format!("creature species line {} is invalid", n + 1)),
        }
    }
    Ok(result)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Guard,
    Investigate,
    Hunt,
    Windup,
    Recover,
    Flee,
    Return,
}
impl Mood {
    pub fn name(self) -> &'static str {
        match self {
            Self::Guard => "guarding",
            Self::Investigate => "investigating",
            Self::Hunt => "hunting",
            Self::Windup => "BITE!",
            Self::Recover => "recovering",
            Self::Flee => "fleeing",
            Self::Return => "returning",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::Guard,
            Self::Investigate,
            Self::Hunt,
            Self::Windup,
            Self::Recover,
            Self::Flee,
            Self::Return,
        ]
        .into_iter()
        .find(|m| m.name() == s)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Creature {
    pub element: Option<String>,
    pub burst: u16,
    pub erupted: bool,
    pub species: Species,
    pub home: (f32, f32),
    pub target: (f32, f32),
    pub mood: Mood,
    pub timer: u32,
    pub alert: u32,
    pub cooldown: u32,
    pub flee_dir: f32,
}
impl Creature {
    pub fn new(species: Species, home: (f32, f32)) -> Self {
        Self {
            element: None,
            burst: 80,
            erupted: false,
            species,
            home,
            target: home,
            mood: Mood::Guard,
            timer: 0,
            alert: 0,
            cooldown: 0,
            flee_dir: 1.,
        }
    }
    pub fn label(&self) -> String {
        match &self.element {
            Some(e) => format!("{} [{}]", self.species.name, e.replace('_', " ")),
            None => self.species.name.clone(),
        }
    }
    pub fn actor(self, m: &Materials, x: f32, feet: f32) -> ActorSpec {
        let mut a = ActorSpec::standing_at(m.id("flesh").unwrap(), x, feet);
        a.surface = Some(crate::physics::SurfaceWalker::default());
        a.health = self.species.health;
        a.respawn = false;
        a.control_override = Some(Controls {
            crouch: true,
            ..Default::default()
        });
        a.mobility = Some(MobilitySettings {
            crouch_height: self.species.height,
            roll_height: 8.,
            crouch_speed: self.species.speed,
            ..Default::default()
        });
        a.posture.height = self.species.height;
        a.posture.crouched = true;
        a.creature = Some(self);
        a
    }
    pub fn shift(&mut self, dx: i32, dy: i32) {
        self.home.0 += dx as f32;
        self.home.1 += dy as f32;
        self.target.0 += dx as f32;
        self.target.1 += dy as f32;
    }
}
fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}
/// Terrain and furniture occlude sight and bites. The bodies at either end
/// are ignored; solid walls are never ignored, even next to the creature.
pub fn clear(w: &World, m: &Materials, a: (f32, f32), b: (f32, f32)) -> bool {
    let n = distance(a, b);
    if n < 0.01 {
        return true;
    }
    for step in 0..=n.ceil() as usize {
        let t = step as f32 / n;
        let (x, y) = (
            (a.0 + (b.0 - a.0) * t).floor() as i32,
            (a.1 + (b.1 - a.1) * t).floor() as i32,
        );
        if !w.in_bounds(x, y) {
            return false;
        }
        let i = w.idx(x as usize, y as usize);
        if w.is_owned(i) {
            if step > 9 && n - step as f32 > 9. {
                return false;
            }
            continue;
        }
        if matches!(
            m.get(w.mat[i]).state,
            crate::material::State::Solid | crate::material::State::Powder
        ) {
            return false;
        }
    }
    true
}
fn hazard(
    w: &World,
    m: &Materials,
    p: (f32, f32),
    radius: f32,
    affinity: Option<crate::MatId>,
) -> Option<(f32, f32)> {
    let heat_limit = affinity.map_or(180., |id| m.get(id).temperature.max(180.));
    let dangerous = |id: crate::MatId, temp: f32| {
        temp > heat_limit || (m.get(id).hurts_on_touch > 0. && Some(id) != affinity)
    };
    let mut nearest = None;
    let mut best = radius * radius;
    for y in (p.1 - radius).max(1.) as usize..=((p.1 + radius) as usize).min(w.h - 2) {
        for x in (p.0 - radius).max(1.) as usize..=((p.0 + radius) as usize).min(w.w - 2) {
            let i = w.idx(x, y);
            if w.is_owned(i) || !dangerous(w.mat[i], w.temp[i]) {
                continue;
            }
            let q = (x as f32, y as f32);
            let d = (q.0 - p.0).powi(2) + (q.1 - p.1).powi(2);
            if d < best && clear(w, m, p, q) {
                best = d;
                nearest = Some(q);
            }
        }
    }
    for f in &w.flying {
        let d = (f.x - p.0).powi(2) + (f.y - p.1).powi(2);
        if d < best && dangerous(f.mat, f.temp) && clear(w, m, p, (f.x, f.y)) {
            best = d;
            nearest = Some((f.x, f.y));
        }
    }
    nearest
}
/// Alert guards to a real tool impact/shot. Hearing does not grant a bite or
/// sight through rock; it only creates a temporary investigation target.
pub fn noise(physics: &mut Physics, at: (f32, f32)) {
    let actors = physics.actor_status();
    for (i, a) in actors.iter().enumerate().filter(|(_, a)| !a.dead) {
        let Some(c) = &a.creature else {
            continue;
        };
        if distance((a.x, a.y), at) <= c.species.hearing
            && distance(c.home, at) <= c.species.leash + 20.
        {
            physics.edit_actor(i, |a| {
                let c = a.creature.as_mut().unwrap();
                c.target = at;
                c.alert = 150;
                if !matches!(c.mood, Mood::Flee | Mood::Windup | Mood::Recover) {
                    c.mood = Mood::Investigate;
                }
            });
        }
    }
}
/// Live species tuning preserves the run's location, health loss and brain state.
pub fn refresh(physics: &mut Physics) -> Result<(), String> {
    let defs = species()?;
    for (i, a) in physics.actor_status().iter().enumerate() {
        let Some(c) = &a.creature else {
            continue;
        };
        let Some(def) = defs.iter().find(|d| d.id == c.species.id) else {
            continue;
        };
        physics.edit_actor(i, |a| {
            a.health = a.health.min(def.health);
            a.creature.as_mut().unwrap().species = def.clone();
            a.posture.height = def.height;
            if let Some(m) = a.mobility.as_mut() {
                m.crouch_speed = def.speed;
                m.crouch_height = def.height;
            }
        });
    }
    Ok(())
}

/// A death releases the authored payload exactly once. Cells are real finite
/// material, so flowing, burning, neutralising and chain reactions stay in World.
pub fn death_bursts(physics: &mut Physics, w: &mut World, m: &Materials) {
    let actors = physics.actor_status();
    for (index, a) in actors.iter().enumerate() {
        let Some(c) = a.creature.as_ref().filter(|c| !c.erupted && a.dead) else {
            continue;
        };
        physics.edit_actor(index, |a| a.creature.as_mut().unwrap().erupted = true);
        let Some(id) = c.element.as_deref().and_then(|name| m.id(name)) else {
            continue;
        };
        let center = (a.x, a.y + (16. - a.body_height) * 0.5);
        let mut emitted = 0;
        for n in 0..c.burst {
            let angle = n as f32 * 2.399963 + (index as f32 * 0.71);
            let (dy, dx) = angle.sin_cos();
            let radius = (n % 5) as f32 * 0.45;
            let origin = (center.0 + dx * radius, center.1 + dy * radius);
            let mut start = None;
            for (ox, oy) in [
                (0., 0.),
                (0., -2.),
                (-2., 0.),
                (2., 0.),
                (0., 2.),
                (0., -4.),
                (-4., 0.),
                (4., 0.),
            ] {
                let (x, y) = (origin.0 + ox, origin.1 + oy);
                if x < 1. || y < 1. || x >= w.w as f32 - 1. || y >= w.h as f32 - 1. {
                    continue;
                }
                let i = w.idx(x as usize, y as usize);
                if !w.is_owned(i) && m.get(w.mat[i]).state == crate::material::State::Gas {
                    start = Some((x, y));
                    break;
                }
            }
            if let Some((x, y)) = start {
                let speed = 0.35 + (n % 9) as f32 * 0.1;
                if w.spawn_flyer(m, id, x, y, dx * speed, dy * speed - 0.15) {
                    emitted += 1;
                }
            }
        }
        if emitted > 0 {
            w.sound(crate::sound::Sound {
                kind: crate::sound::Kind::Blast,
                x: center.0,
                y: center.1,
                mat: id,
                other: id,
                loud: 0.7,
                size: (emitted as f32).sqrt() * 2.,
            });
        }
    }
}
// Compare both contour routes. This keeps a ceiling crawler moving around
// the wall towards a player below, instead of oscillating directly overhead.
fn route_direction(
    w: &World,
    m: &Materials,
    p: (f32, f32),
    normal: (f32, f32),
    height: f32,
    target: (f32, f32),
    previous: f32,
) -> f32 {
    let score = |direction: f32| {
        let (mut at, mut n) = (p, normal);
        let mut best = distance(p, target);
        for step in 0..180 {
            let Some((next, next_n)) =
                crate::physics::surface_advance(w, m, at, n, height, direction * 1.2)
            else {
                break;
            };
            at = next;
            n = next_n;
            best = best.min(distance(at, target) + step as f32 * 0.035);
        }
        best
    };
    let (a, b) = (score(1.), score(-1.));
    if (a - b).abs() < 1.5 {
        previous
    } else if a < b {
        1.
    } else {
        -1.
    }
}
/// One AI tick before normal actor physics. Every motion uses actor controls;
/// brains cannot teleport, pass through terrain or damage through a wall.
/// Creatures further than this from the player rest (cells).
pub const REST_DISTANCE: f32 = 260.0;

pub fn think(physics: &mut Physics, w: &mut World, m: &Materials) {
    physics.clear_rest();
    let actors = physics.actor_status();
    let player = actors
        .iter()
        .enumerate()
        .find(|(_, p)| p.creature.is_none() && !p.dead);
    for (i, a) in actors.iter().enumerate() {
        let Some(mut c) = a.creature.clone() else {
            continue;
        };
        if a.dead {
            continue;
        }
        // Far from the player and not stirred up: rest (no thinking or
        // moving), so the cave around it can sleep. Wakes as you come near.
        if c.alert == 0 && player.is_some_and(|(_, pl)| (pl.x - a.x).hypot(pl.y - a.y) > REST_DISTANCE) {
            physics.rest_actor(i);
            continue;
        }
        let p = (a.x, a.y + (16.0 - a.body_height) * 0.5);
        let normal = a.surface.map_or((0., -1.), |s| s.normal);
        let tangent = crate::physics::surface_tangent(normal);
        c.cooldown = c.cooldown.saturating_sub(1);
        c.alert = c.alert.saturating_sub(1);
        let threat = if w.frame % 6 == i as u32 % 6 {
            hazard(
                w,
                m,
                p,
                c.species.avoidance,
                c.element.as_deref().and_then(|name| m.id(name)),
            )
        } else {
            None
        };
        if let Some(q) = threat {
            c.mood = Mood::Flee;
            c.timer = 60;
            let away = (p.0 - q.0) * tangent.0 + (p.1 - q.1) * tangent.1;
            c.flee_dir = if away.abs() > 0.5 {
                away.signum()
            } else {
                -a.facing
            };
        }
        let target = player.map(|(_, p)| (p.x, p.y + 2.));
        let seen = target.is_some_and(|t| {
            distance(p, t) <= c.species.sight
                && distance(c.home, t) <= c.species.leash
                && clear(w, m, p, t)
        });
        let mut walk = 0.;
        let mut bite = false;
        match c.mood {
            Mood::Flee => {
                c.timer = c.timer.saturating_sub(1);
                walk = c.flee_dir;
                if c.timer == 0 {
                    c.mood = Mood::Return;
                }
            }
            Mood::Windup => {
                c.timer = c.timer.saturating_sub(1);
                if c.timer == 0 {
                    bite = target.is_some_and(|t| distance(p, t) < 13. && clear(w, m, p, t));
                    c.mood = Mood::Recover;
                    c.timer = c.species.recovery;
                    c.cooldown = c.species.recovery;
                }
            }
            Mood::Recover => {
                c.timer = c.timer.saturating_sub(1);
                if c.timer == 0 {
                    c.mood = Mood::Return;
                }
            }
            _ => {
                if seen {
                    c.target = target.unwrap();
                    c.alert = 120;
                    c.mood = Mood::Hunt;
                } else if c.alert > 0 && distance(c.home, c.target) <= c.species.leash + 20. {
                    c.mood = Mood::Investigate;
                } else {
                    c.target = c.home;
                    c.mood = if distance(p, c.home) > c.species.leash * 0.8 {
                        Mood::Return
                    } else {
                        Mood::Guard
                    };
                }
                if c.mood == Mood::Guard {
                    walk = if (w.frame / 30 + (c.home.0.abs() as u32 / 13)) % 6 == 0 {
                        0.0
                    } else {
                        0.45 * c.flee_dir
                    };
                } else {
                    if w.frame % 24 == i as u32 % 24 || c.timer == 0 {
                        c.flee_dir =
                            route_direction(w, m, p, normal, a.body_height, c.target, c.flee_dir);
                        c.timer = 24;
                    } else {
                        c.timer = c.timer.saturating_sub(1);
                    }
                    if distance(p, c.target) > 3. {
                        walk = c.flee_dir;
                    }
                    if seen && distance(p, c.target) < 13. && c.cooldown == 0 {
                        c.mood = Mood::Windup;
                        c.timer = c.species.windup;
                        walk = 0.;
                    }
                }
            }
        }
        if bite {
            if let Some((index, _)) = player {
                physics.damage_actor(index, c.species.damage, "were bitten");
                w.sound(crate::sound::Sound {
                    kind: crate::sound::Kind::Hit,
                    x: a.x,
                    y: a.y,
                    mat: m.id("flesh").unwrap_or(0),
                    other: m.id("flesh").unwrap_or(0),
                    loud: 0.65,
                    size: 5.,
                });
            }
        }
        let controls = Controls {
            walk,
            crouch: true,
            ..Default::default()
        };
        physics.edit_actor(i, |a| {
            a.creature = Some(c);
            a.control_override = Some(controls);
        });
    }
}
/// Coloured, articulated native art. The same silhouette supplies real body
/// cells for heat/contact/acid, and is rendered inside the normal lighting pass.
pub fn pixels(c: &Creature, frame: u32, dead: bool) -> Vec<(usize, usize, [u8; 3])> {
    let mut grid = vec![None; crate::look::MASK_W * 16];
    let width = crate::look::MASK_W;
    let shell = c.species.color;
    let dark = shell.map(|v| (v as f32 * 0.38) as u8);
    let light = shell.map(|v| v.saturating_add(45));
    let mut dot = |x: i32, y: i32, col: [u8; 3]| {
        if x >= 0 && x < width as i32 && (0..16).contains(&y) {
            grid[y as usize * width + x as usize] = Some(col);
        }
    };
    let phase = if !dead
        && matches!(
            c.mood,
            Mood::Hunt | Mood::Flee | Mood::Investigate | Mood::Return
        ) {
        (frame / 4) % 2
    } else {
        0
    };
    for n in 0..4 {
        let y = 8 + n * 2;
        for side in [-1, 1] {
            let x = 10 + side * (5 + (n + phase as i32) % 2);
            dot(x, y, dark);
            dot(x + side, y + 1, light);
            dot(x + side, y + 2, dark);
        }
    }
    let cy = if dead {
        13.0
    } else {
        15.0 - (c.species.height - 1.0) * 0.5
    };
    let ry = if dead {
        1.8
    } else {
        (c.species.height - 2.0) * 0.5
    };
    let rx = if c.species.height > 10. { 5.5 } else { 4.6 };
    for y in 3..16 {
        for x in 4..16 {
            let d = ((x as f32 - 9.5) / rx).powi(2) + ((y as f32 - cy) / ry).powi(2);
            if d <= 1. {
                dot(
                    x,
                    y,
                    if x == 9 || d > 0.78 {
                        dark
                    } else if (y as f32) < cy {
                        light
                    } else {
                        shell
                    },
                );
            }
        }
    }
    let hy = cy as i32 - 1;
    for y in hy..hy + 4 {
        for x in 14..17 {
            dot(x, y, if y == hy { light } else { dark });
        }
    }
    let eyes = if dead {
        [65, 50, 40]
    } else if c.mood == Mood::Windup {
        [255, 80, 55]
    } else {
        [255, 222, 118]
    };
    dot(16, hy, eyes);
    dot(16, hy + 2, eyes);
    let jaw = if c.mood == Mood::Windup { 2 } else { 1 };
    dot(17, hy - jaw, light);
    dot(18, hy - 1 - jaw, dark);
    dot(17, hy + 2 + jaw, light);
    dot(18, hy + 3 + jaw, dark);
    grid.into_iter()
        .enumerate()
        .filter_map(|(i, c)| c.map(|rgb| (i % width, i / width, rgb)))
        .collect()
}
pub fn mask(c: &Creature, frame: u32) -> Vec<bool> {
    let mut result = vec![false; crate::look::MASK_W * 16];
    for (x, y, _) in pixels(c, frame, false) {
        result[y * crate::look::MASK_W + x] = true;
    }
    result
}
pub fn draws(actors: &[ActorStatus]) -> Vec<ActorStatus> {
    actors
        .iter()
        .filter(|a| a.creature.is_some())
        .cloned()
        .collect()
}

/// A creature placed by `scatter`: species id, feet position, element, burst.
pub struct Spawn {
    pub species: String,
    pub x: f32,
    pub feet: f32,
    pub element: &'static str,
    pub burst: u16,
}

/// Elemental creatures scattered over a whole level (`creatures AMOUNT` in a
/// scene, after the caves are made). Spots are real floors with headroom,
/// away from `avoid` (x, y, radius: the camp). The element suits the place:
/// water near pools, oil near gas or oil, lava and fire deep down, and a mix
/// elsewhere. Seeded, so the same scene always gets the same creatures.
pub fn scatter(world: &crate::World, mats: &crate::Materials, amount: f32, seed: u32, avoid: Option<(f32, f32, f32)>, max: usize) -> Vec<Spawn> {
    use crate::material::State;
    let (w, h) = (world.w as i32, world.h as i32);
    let mat = |x: i32, y: i32| (x >= 0 && y >= 0 && x < w && y < h).then(|| world.mat[world.idx(x as usize, y as usize)]);
    let solid = |x: i32, y: i32| mat(x, y).is_some_and(|m| mats.get(m).state == State::Solid);
    let air = |x: i32, y: i32| mat(x, y) == Some(crate::material::AIR) && !world.is_owned(world.idx(x as usize, y as usize));
    let roll = |i: i32, j: i32, k: u32| {
        let mut v = seed ^ 0xc2ea_7u32.wrapping_mul(k + 1) ^ (i as u32).wrapping_mul(0x9E37_79B9) ^ (j as u32).wrapping_mul(0x85EB_CA6B);
        v ^= v >> 15;
        v = v.wrapping_mul(0x2C1B_3C6D);
        v ^= v >> 12;
        (v & 0xFFFF) as f32 / 65535.0
    };
    let (water, gas, oil, lava) = (mats.id("water"), mats.id("volatile_gas"), mats.id("oil"), mats.id("lava"));
    let spacing = 64;
    let mut out = Vec::new();
    for j in 0..h / spacing + 1 {
        for i in 0..w / spacing + 1 {
            if out.len() >= max || roll(i, j, 0) > amount.clamp(0.0, 1.0) * 0.7 {
                continue;
            }
            let shell = roll(i, j, 1) < 0.35;
            let tall = if shell { 18 } else { 12 };
            // A few tries in this patch: down to a floor (open cell, solid
            // ground under it) with footing and room to stand.
            let found = (0..8u32).find_map(|k| {
                let x = i * spacing + (roll(i, j, 10 + k) * spacing as f32) as i32;
                let start = j * spacing + (roll(i, j, 20 + k) * spacing as f32) as i32;
                let y = (start..(start + spacing).min(h - 2)).find(|&y| air(x, y) && solid(x, y + 1))?;
                if avoid.is_some_and(|(cx, cy, r)| (x as f32 - cx).hypot(y as f32 - cy) < r) {
                    return None;
                }
                let footing = (-2..=2).all(|dx| solid(x + dx, y + 1) || solid(x + dx, y + 2));
                let room = (-4..=4).all(|dx| (1..tall + 2).all(|dy| air(x + dx, y - dy)));
                (footing && room).then_some((x, y))
            });
            let Some((x, y)) = found else { continue };
            // What's around decides the element.
            let near = |m: Option<crate::material::MatId>| {
                let Some(m) = m else { return 0 };
                (-28..=28).step_by(2).map(|dy| (-28..=28).step_by(2).filter(|&dx| mat(x + dx, y + dy) == Some(m)).count()).sum::<usize>()
            };
            let deep = y as f32 > h as f32 * 0.65;
            let pick = roll(i, j, 4);
            let element = if near(water) > 6 {
                "water"
            } else if near(gas) + near(oil) > 6 {
                "oil"
            } else if near(lava) > 0 || (deep && pick < 0.6) {
                if pick < 0.3 { "lava" } else { "fire" }
            } else {
                ["fire", "oil", "water", "fire", "acid", "oil"][(pick * 6.0) as usize % 6]
            };
            out.push(Spawn { species: if shell { "shellback" } else { "skitter" }.into(), x: x as f32, feet: (y + 1) as f32, element, burst: if shell { 110 } else { 80 } });
        }
    }
    out
}
