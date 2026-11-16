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
    /// Crawling towards its element.
    Seek,
    /// Chewing through rock towards its element.
    Burrow,
    /// Eating its element (the death burst grows).
    Feed,
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
            Self::Seek => "seeking",
            Self::Burrow => "burrowing",
            Self::Feed => "feeding",
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
            Self::Seek,
            Self::Burrow,
            Self::Feed,
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
    /// Where it smelled its element (re-smelled every second; not saved).
    pub food: Option<(f32, f32)>,
    /// Closest it has got to the food, and frames without getting closer.
    pub best: f32,
    pub stuck: u32,
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
            food: None,
            best: f32::MAX,
            stuck: 0,
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
        if let Some(f) = self.food.as_mut() {
            f.0 += dx as f32;
            f.1 += dy as f32;
        }
    }
}
/// How far a creature smells its element, how close it feeds from, and the
/// most a death burst grows to by feeding (cells).
pub const SMELL: f32 = 120.0;
const FEED_REACH: f32 = 10.0;
const MAX_BURST: usize = 400;
/// The hardest ground a creature can chew (rock 0.7 yes; stone, basalt no).
const CHEW: f32 = 0.75;

/// What a creature of this element eats.
fn food_of(element: Option<&str>, m: &Materials) -> Vec<crate::material::MatId> {
    let names: &[&str] = match element {
        Some("water") => &["water"],
        Some("oil") => &["oil", "volatile_gas"],
        Some("fire") => &["fire", "lava", "ember", "torch_glow"],
        Some("lava") => &["lava", "fire", "ember"],
        Some("acid") => &["acid"],
        Some("crystal_cyan") | Some("crystal_violet") => &["crystal_cyan", "crystal_violet"],
        Some("volatile_gas") => &["volatile_gas"],
        Some("gravel") => &["shale", "sandstone", "dirt"],
        Some("ember") => &["volatile_gas", "oil"],
        _ => &[],
    };
    names.iter().filter_map(|n| m.id(n)).collect()
}

/// The nearest cell of its food within `SMELL` (through rock too: a sealed
/// gas pocket is smelled, and burrowed into).
fn smell(w: &World, m: &Materials, p: (f32, f32), food: &[crate::material::MatId]) -> Option<(f32, f32)> {
    if food.is_empty() {
        return None;
    }
    let _ = m;
    let r = SMELL as i32;
    let (cx, cy) = (p.0 as i32, p.1 as i32);
    let mut best: Option<((f32, f32), i32)> = None;
    for dy in (-r..=r).step_by(3) {
        for dx in (-r..=r).step_by(3) {
            let d2 = dx * dx + dy * dy;
            if d2 > r * r || best.is_some_and(|(_, b)| d2 >= b) {
                continue;
            }
            let (x, y) = (cx + dx, cy + dy);
            if !w.in_bounds(x, y) {
                continue;
            }
            let i = w.idx(x as usize, y as usize);
            if !w.is_owned(i) && food.contains(&w.mat[i]) {
                best = Some(((x as f32, y as f32), d2));
            }
        }
    }
    best.map(|(f, _)| f)
}

/// Eat up to 8 cells of its food near `p`; how many.
fn feed(w: &mut World, m: &Materials, p: (f32, f32), food: &[crate::material::MatId]) -> usize {
    let mut eaten = 0;
    let (cx, cy) = (p.0 as i32, p.1 as i32);
    let r = FEED_REACH as i32 + 2;
    for dy in -r..=r {
        for dx in -r..=r {
            if eaten >= 8 || dx * dx + dy * dy > r * r {
                continue;
            }
            let (x, y) = (cx + dx, cy + dy);
            if !w.in_bounds(x, y) {
                continue;
            }
            let i = w.idx(x as usize, y as usize);
            if !w.is_owned(i) && food.contains(&w.mat[i]) {
                w.set(m, x as usize, y as usize, crate::material::AIR);
                eaten += 1;
            }
        }
    }
    eaten
}

/// Chew a hole a little way towards `to`: soft ground (up to `CHEW`) only,
/// never crystals, objects or characters. Some of it falls as dust. How
/// many cells were chewed, or None when only too-hard ground is in the way.
fn burrow(w: &mut World, m: &Materials, from: (f32, f32), to: (f32, f32), element: Option<&str>) -> Option<usize> {
    let d = distance(from, to).max(0.001);
    let dir = ((to.0 - from.0) / d, (to.1 - from.1) / d);
    // Ahead of the body, never under its feet (pulling the ground out from
    // under a creature flings it about).
    let (cx, cy) = (from.0 + dir.0 * 10.0, from.1 + dir.1 * 10.0);
    // Gravel worms leave a trail of gravel; the rest a little dust.
    let (rubble_mat, every) = if element == Some("gravel") { (m.id("gravel"), 3) } else { (m.id("dust"), 6) };
    let mut dug = 0;
    let mut hard = false;
    for dy in -5i32..=5 {
        for dx in -5i32..=5 {
            if dx * dx + dy * dy > 25 {
                continue;
            }
            let (x, y) = (cx as i32 + dx, cy as i32 + dy);
            if !w.in_bounds(x, y) || (y as f32 > from.1 + 5.0 && (x as f32 - from.0).abs() < 9.0) {
                continue;
            }
            let i = w.idx(x as usize, y as usize);
            let mat = m.get(w.mat[i]);
            let crystal = mat.name.starts_with("crystal");
            if w.is_owned(i) || mat.state != crate::material::State::Solid {
                continue;
            }
            if crystal || crate::mining::hardness(mat) > CHEW {
                hard = true;
                continue;
            }
            let rubble = if dug % every == 0 { rubble_mat.unwrap_or(crate::material::AIR) } else { crate::material::AIR };
            w.set(m, x as usize, y as usize, rubble);
            dug += 1;
        }
    }
    if dug == 0 && hard { None } else { Some(dug) }
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
        if w.frame % 60 == i as u32 % 60 && c.mood != Mood::Burrow {
            let food = smell(w, m, p, &food_of(c.element.as_deref(), m));
            if food.is_some_and(|f| c.food.is_none_or(|old| distance(old, f) > 20.0)) {
                c.best = f32::MAX;
                c.stuck = 0;
            }
            c.food = if c.burst as usize >= MAX_BURST { None } else { food };
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
                } else if let Some(f) = c.food {
                    // Its element: crawl to it, chew through rock if stuck,
                    // and eat it once there (its death burst grows).
                    c.home = f;
                    c.target = f;
                    let d = distance(p, f);
                    if d < c.best - 3.0 {
                        c.best = d;
                        c.stuck = 0;
                    } else {
                        c.stuck += 1;
                    }
                    if d < FEED_REACH {
                        c.mood = Mood::Feed;
                        if w.frame % 20 == i as u32 % 20 {
                            let eaten = feed(w, m, p, &food_of(c.element.as_deref(), m));
                            c.burst = (c.burst as usize + eaten).min(MAX_BURST) as u16;
                            if eaten == 0 || c.burst as usize >= MAX_BURST {
                                c.food = None;
                            }
                        }
                    } else if c.stuck > 40 {
                        c.mood = Mood::Burrow;
                        if w.frame % 8 == i as u32 % 8 {
                            match burrow(w, m, p, f, c.element.as_deref()) {
                                // Chewed some: crawl into the hole.
                                Some(n) if n > 0 => {
                                    c.stuck = 28;
                                    c.best = d;
                                }
                                // Nothing ahead to chew: just keep crawling.
                                Some(_) => c.stuck = 0,
                                // Too hard to chew: give up on this one.
                                None => {
                                    c.food = None;
                                    c.stuck = 0;
                                }
                            }
                        }
                    } else {
                        c.mood = Mood::Seek;
                    }
                } else {
                    c.target = c.home;
                    c.mood = if distance(p, c.home) > c.species.leash * 0.8 {
                        Mood::Return
                    } else {
                        Mood::Guard
                    };
                }
                if c.mood == Mood::Feed {
                    walk = 0.0;
                } else if c.mood == Mood::Guard {
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
        // Ember beetles drop sparks as they run (they set off gas).
        if walk != 0.0 && c.element.as_deref() == Some("ember") && w.frame % 25 == i as u32 % 25 {
            if let Some(ember) = m.id("ember") {
                let (x, y) = ((p.0 - a.facing * 7.0) as i32, (p.1 + 2.0) as i32);
                if w.in_bounds(x, y) && w.mat[w.idx(x as usize, y as usize)] == crate::material::AIR {
                    w.set(m, x as usize, y as usize, ember);
                }
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
    let moving = matches!(c.mood, Mood::Hunt | Mood::Flee | Mood::Investigate | Mood::Return | Mood::Seek | Mood::Burrow | Mood::Feed);
    if !dead {
        if let Some(body) = shaped(c, frame, moving) {
            return body;
        }
    }
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
            Mood::Hunt | Mood::Flee | Mood::Investigate | Mood::Return | Mood::Seek | Mood::Burrow | Mood::Feed
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
/// Body shapes of the newer species (alive; corpses use the shared flat
/// shape). Drawn facing right on the 20x16 grid, like the bug body. The
/// picture is also the body: cells for heat, contact and acid.
fn shaped(c: &Creature, frame: u32, moving: bool) -> Option<Vec<(usize, usize, [u8; 3])>> {
    let width = crate::look::MASK_W as i32;
    let mut grid: Vec<Option<[u8; 3]>> = vec![None; crate::look::MASK_W * 16];
    let shell = c.species.color;
    let dark = shell.map(|v| (v as f32 * 0.38) as u8);
    let light = shell.map(|v| v.saturating_add(45));
    let eye = if c.mood == Mood::Windup { [255, 80, 55] } else { [255, 222, 118] };
    let mut dot = |x: i32, y: i32, col: [u8; 3]| {
        if (0..width).contains(&x) && (0..16).contains(&y) {
            grid[(y * width + x) as usize] = Some(col);
        }
    };
    let step = if moving { (frame / 4) % 2 } else { 0 } as i32;
    let blob = |dot: &mut dyn FnMut(i32, i32, [u8; 3]), cx: f32, cy: f32, rx: f32, ry: f32, top: [u8; 3], body: [u8; 3], rim: [u8; 3]| {
        for y in 0..16 {
            for x in 0..width {
                let d = ((x as f32 - cx) / rx).powi(2) + ((y as f32 - cy) / ry).powi(2);
                if d <= 1.0 {
                    dot(x, y, if d > 0.72 { rim } else if (y as f32) < cy - ry * 0.3 { top } else { body });
                }
            }
        }
    };
    match c.species.id.as_str() {
        // A long segmented worm, rippling along the ground; jaws at the front.
        "worm" => {
            for k in 0..6 {
                let x = 2.5 + k as f32 * 2.9;
                let lift = ((k as f32 * 1.3 + frame as f32 * if moving { 0.25 } else { 0.05 }).sin() * 0.8).round();
                let cy = 12.5 - lift;
                blob(&mut dot, x, cy, 2.0, 2.3 - (5 - k) as f32 * 0.06, light, shell, shell);
                // A dark ring between segments.
                for y in (cy - 2.0) as i32..=(cy + 2.0) as i32 {
                    dot((x + 1.5).round() as i32, y, dark);
                }
                dot(x.round() as i32, (cy + 2.2) as i32, dark);
            }
            let hy = (12.5 - ((5.0 * 1.3 + frame as f32 * if moving { 0.25 } else { 0.05 }).sin() * 0.8).round()) as i32;
            dot(18, hy - 1, eye);
            dot(19, hy - 2, dark);
            dot(19, hy + 1, dark);
            // Gravel caught in its skin.
            for (x, y) in [(5, 13), (11, 14), (14, 11)] {
                dot(x, y, [118, 112, 104]);
            }
        }
        // A swollen sac of gas on stubby legs; it slowly breathes.
        "bloater" => {
            let pulse = ((frame as f32 * 0.08).sin() * 0.5) + if c.mood == Mood::Windup { 0.8 } else { 0.0 };
            for n in 0..3 {
                let x = 6 + n * 4;
                dot(x, 14 + (n + step) % 2, dark);
                dot(x, 15, dark);
            }
            blob(&mut dot, 9.0, 7.8, 7.0 + pulse, 6.0 + pulse * 0.6, light, shell, dark);
            // Pale swollen blisters.
            for (x, y) in [(6, 5), (10, 4), (12, 8), (7, 9)] {
                dot(x, y, [206, 222, 150]);
            }
            for y in 8..13 {
                for x in 15..19 {
                    dot(x, y, if y == 8 { light } else { dark });
                }
            }
            dot(17, 9, eye);
            dot(17, 11, eye);
            dot(19, 10, dark);
        }
        // A tiny tick on long spindly legs, crystal shards on its back.
        "mite" => {
            let glint = match c.element.as_deref() {
                Some("crystal_violet") => [205, 160, 255],
                _ => [130, 240, 255],
            };
            for n in 0..4 {
                let x = 5 + n * 3;
                let lift = (n + step) % 2;
                dot(x - 1, 11 - lift, dark);
                dot(x - 2, 12, dark);
                dot(x - 2, 13, dark);
                dot(x - 3, 14 + lift, dark);
                dot(x - 3, 15, dark);
            }
            blob(&mut dot, 10.0, 11.5, 4.0, 2.8, light, shell, dark);
            for (x, h) in [(8, 3), (10, 4), (12, 2)] {
                for y in 0..h {
                    dot(x, 8 - y, if y == h - 1 { [240, 252, 255] } else { glint });
                }
            }
            dot(14, 11, eye);
            dot(15, 12, dark);
        }
        // A low, round beetle: split wing cases with glowing embers in them
        // and a horn at the front.
        "beetle" => {
            for n in 0..3 {
                let x = 6 + n * 4;
                let lift = (n + step) % 2;
                dot(x - 1, 13 + lift, dark);
                dot(x - 2, 14 + lift, dark);
                dot(x - 2, 15, dark);
            }
            blob(&mut dot, 9.5, 10.5, 6.0, 4.0, light, shell, dark);
            for y in 7..14 {
                dot(9, y, dark);
            }
            for (k, (x, y)) in [(6, 10), (12, 9), (7, 12), (13, 12)].into_iter().enumerate() {
                let hot = (frame / 6 + k as u32) % 3 != 0;
                dot(x, y, if hot { [255, 196, 70] } else { [190, 70, 20] });
            }
            for y in 9..13 {
                for x in 15..18 {
                    dot(x, y, dark);
                }
            }
            dot(17, 10, eye);
            dot(18, 9, light);
            dot(19, 8, light);
        }
        _ => return None,
    }
    Some(grid.into_iter().enumerate().filter_map(|(i, c)| c.map(|rgb| (i % width as usize, i / width as usize, rgb))).collect())
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
    let (cyan, violet) = (mats.id("crystal_cyan"), mats.id("crystal_violet"));
    let spacing = 64;
    let mut out = Vec::new();
    for j in 0..h / spacing + 1 {
        for i in 0..w / spacing + 1 {
            if out.len() >= max || roll(i, j, 0) > amount.clamp(0.0, 1.0) * 0.7 {
                continue;
            }
            // Which creature: mostly skitters and shellbacks, with worms,
            // beetles, mites and bloaters among them.
            let kind = roll(i, j, 1);
            let species = if kind < 0.38 { "skitter" } else if kind < 0.58 { "shellback" } else if kind < 0.72 { "worm" } else if kind < 0.84 { "beetle" } else if kind < 0.94 { "mite" } else { "bloater" };
            let tall = match species { "shellback" | "bloater" => 18, _ => 12 };
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
            // Mites gather at crystals; the rest suit the place.
            let crystals = (near(cyan), near(violet));
            let (species, element) = if crystals.0 + crystals.1 > 3 && kind < 0.84 {
                ("mite", if crystals.0 >= crystals.1 { "crystal_cyan" } else { "crystal_violet" })
            } else {
                (species, match species {
                    "mite" => if pick < 0.5 { "crystal_cyan" } else { "crystal_violet" },
                    "bloater" => "volatile_gas",
                    "worm" => "gravel",
                    "beetle" => "ember",
                    _ => "",
                })
            };
            let element = if !element.is_empty() {
                element
            } else if near(water) > 6 {
                "water"
            } else if near(gas) + near(oil) > 6 {
                "oil"
            } else if near(lava) > 0 || (deep && pick < 0.6) {
                if pick < 0.3 { "lava" } else { "fire" }
            } else {
                ["fire", "oil", "water", "fire", "acid", "oil"][(pick * 6.0) as usize % 6]
            };
            let burst = match species { "shellback" => 110, "bloater" => 160, "mite" => 20, "worm" => 90, "beetle" => 50, _ => 80 };
            out.push(Spawn { species: species.into(), x: x as f32, feet: (y + 1) as f32, element, burst });
        }
    }
    out
}
