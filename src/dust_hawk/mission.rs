//! Operation Red Mesa: mission state using shared overhead combat and cargo.
use grave_engine::{
    Materials,
    overhead::{
        Aircraft, FlightInput, Point,
        cargo::Cargo,
        combat::{Bounds, Combat, Target, Weapon},
        map::WorldMap,
    },
};
use std::path::Path;
const DT: f32 = 1. / 60.;
pub const CANNON: Weapon = Weapon {
    speed: 260.,
    range: 78.,
    damage: 9.,
    penetration: 0.65,
    radius: 1.25,
    force: 70000.,
    spark_heat: 240.,
    guidance: 0.,
    explosive: false,
};
pub const ROCKET: Weapon = Weapon {
    speed: 135.,
    range: 100.,
    damage: 62.,
    penetration: 0.3,
    radius: 7.,
    force: 24000.,
    spark_heat: 0.,
    guidance: 0.,
    explosive: true,
};
pub const MISSILE: Weapon = Weapon {
    speed: 160.,
    range: 135.,
    damage: 170.,
    penetration: 1.,
    radius: 5.,
    force: 65000.,
    spark_heat: 0.,
    guidance: 5.,
    explosive: true,
};
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Role {
    Radar,
    Power,
    Command,
    Armoury,
    Aa,
    Sam,
}
#[derive(Clone)]
pub struct Unit {
    pub id: u32,
    pub role: Role,
    pub name: String,
    pub timer: f32,
    pub heading: f32,
    pub seen_dead: bool,
}
#[derive(Clone)]
pub struct Rescue {
    pub id: u32,
    pub name: String,
    pub at: Point,
    pub people: u32,
    pub collected: bool,
    pub progress: f32,
}
#[derive(Clone)]
pub struct Supply {
    pub kind: String,
    pub at: Point,
    pub taken: bool,
}
#[derive(Clone)]
pub struct Barrel {
    pub id: u32,
    pub at: [usize; 3],
    pub taken: bool,
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Briefing,
    Flying,
    Won,
    Lost,
}
pub struct Mission {
    pub world: WorldMap,
    pub craft: Aircraft,
    pub combat: Combat,
    pub units: Vec<Unit>,
    pub rescues: Vec<Rescue>,
    pub supplies: Vec<Supply>,
    pub barrels: Vec<Barrel>,
    pub cargo: Vec<Cargo>,
    pub held: Option<u32>,
    pub base: Point,
    pub service_bases: Vec<Point>,
    pub fuel: f32,
    pub ammo: [u32; 3],
    pub selected: usize,
    pub cooldown: [f32; 3],
    pub flares: u32,
    pub flare_time: f32,
    pub flare_cooldown: f32,
    pub passengers: u32,
    pub rescued: u32,
    pub score: u32,
    pub time: f32,
    pub phase: Phase,
    pub aim: Point,
    pub lock_target: Option<u32>,
    pub lock: f32,
    pub toast: String,
    pub toast_time: f32,
    pub damaged_flash: f32,
    pub service: f32,
    pub step_ms: f32,
}
impl Mission {
    pub fn new(m: &Materials) -> Self {
        let (world, spawn) =
            WorldMap::load(Path::new("scenes/red-mesa.map"), m).expect("Red Mesa material level");
        let mut me = Self {
            world,
            craft: Aircraft::new(spawn),
            combat: Combat::default(),
            units: vec![],
            rescues: vec![],
            supplies: vec![],
            barrels: vec![],
            cargo: vec![],
            held: None,
            base: Point::new(160., 992., 5.),
            service_bases: vec![],
            fuel: 100.,
            ammo: [700, 36, 8],
            selected: 0,
            cooldown: [0.; 3],
            flares: 8,
            flare_time: 0.,
            flare_cooldown: 0.,
            passengers: 0,
            rescued: 0,
            score: 0,
            time: 0.,
            phase: Phase::Briefing,
            aim: Point::new(54., 35., 45.),
            lock_target: None,
            lock: 0.,
            toast: "Welcome to Red Mesa".into(),
            toast_time: 6.,
            damaged_flash: 0.,
            service: 0.,
            step_ms: 0.,
        };
        let text = std::fs::read_to_string("assets/missions/red-mesa.ops").unwrap();
        for raw in text.lines() {
            let t: Vec<_> = raw.split_whitespace().collect();
            if t.is_empty() || t[0].starts_with('#') {
                continue;
            }
            let n = |i: usize| t[i].parse::<f32>().unwrap();
            match t[0] {
                "base" => me.base = Point::new(n(1), n(2), n(3)),
                "service" => me.service_bases.push(Point::new(n(1), n(2), n(3))),
                "target" => {
                    let id = t[1].parse().unwrap();
                    let role = match t[2] {
                        "radar" => Role::Radar,
                        "power" => Role::Power,
                        "command" => Role::Command,
                        "armoury" => Role::Armoury,
                        "aa" => Role::Aa,
                        "sam" => Role::Sam,
                        _ => panic!("target role"),
                    };
                    let min = Point::new(n(4), n(5), n(6));
                    let max = Point::new(min.x + n(7), min.y + n(8), min.z + n(9));
                    let mut target = Target::new(id, 2, Bounds { min, max }, n(10), n(11));
                    target.material_backed = true;
                    target.death_blast = Some(if role == Role::Radar {
                        (10., 70000.)
                    } else if role == Role::Power {
                        (13., 32000.)
                    } else if role == Role::Command {
                        (18., 48000.)
                    } else if role == Role::Armoury {
                        (14., 45000.)
                    } else {
                        (7., 40000.)
                    });
                    me.combat.targets.push(target);
                    me.units.push(Unit {
                        id,
                        role,
                        name: t[3].replace('_', " "),
                        timer: 1.5 + (id % 3) as f32 * 0.4,
                        heading: 0.,
                        seen_dead: false,
                    });
                }
                "rescue" => me.rescues.push(Rescue {
                    id: t[1].parse().unwrap(),
                    name: t[2].replace('_', " "),
                    at: Point::new(n(3), n(4), n(5)),
                    people: t[6].parse().unwrap(),
                    collected: false,
                    progress: 0.,
                }),
                "cache" => me.supplies.push(Supply {
                    kind: t[2].into(),
                    at: Point::new(n(3), n(4), n(5)),
                    taken: false,
                }),
                "barrel" => me.barrels.push(Barrel {
                    id: t[1].parse().unwrap(),
                    at: [n(2) as usize, n(3) as usize, n(4) as usize],
                    taken: false,
                }),
                _ => panic!("mission metadata"),
            }
        }
        me.combat
            .targets
            .push(Target::new(0, 1, me.craft_bounds(), 220., 3.));
        me.combat.bind_materials(&me.world, m);
        me.craft.settings.acceleration = 145.;
        me.craft.settings.speed = 65.;
        me.craft.settings.damping = 2.2;
        me
    }
    pub fn craft_bounds(&self) -> Bounds {
        Bounds {
            min: Point::new(
                self.craft.at.x - 5.,
                self.craft.at.y - 4.,
                self.craft.at.z - 1.,
            ),
            max: Point::new(
                self.craft.at.x + 5.,
                self.craft.at.y + 4.,
                self.craft.at.z + 5.,
            ),
        }
    }
    pub fn health(&self) -> f32 {
        self.combat.target(0).unwrap().health
    }
    pub fn alive(&self, id: u32) -> bool {
        self.combat.alive(id)
    }
    pub fn start(&mut self) {
        self.phase = Phase::Flying;
        self.message("Destroy the two radar arrays to shorten enemy detection range.");
    }
    pub fn message(&mut self, s: impl Into<String>) {
        self.toast = s.into();
        self.toast_time = 5.;
    }
    pub fn remaining_radars(&self) -> usize {
        self.units
            .iter()
            .filter(|u| u.role == Role::Radar && self.alive(u.id))
            .count()
    }
    pub fn primary_complete(&self) -> bool {
        [10, 11, 12, 13].iter().all(|id| !self.alive(*id)) && self.rescued >= 6
    }
    pub fn next_objective(&self) -> (&str, Point) {
        for (id, text) in [
            (10, "Destroy the west radar"),
            (11, "Destroy the east radar"),
            (12, "Cut power to the SAM network"),
        ] {
            if self.alive(id) {
                return (text, self.combat.target(id).unwrap().bounds.centre());
            }
        }
        if self.passengers >= 6 || self.rescues.iter().all(|r| r.collected) && self.passengers > 0 {
            return ("Deliver survivors to the LZ", self.base);
        }
        if let Some(r) = self.rescues.iter().filter(|r| !r.collected).min_by(|a, b| {
            self.craft
                .at
                .distance(a.at)
                .total_cmp(&self.craft.at.distance(b.at))
        }) {
            return ("Hover low to rescue survivors", r.at);
        }
        if self.alive(13) {
            return (
                "Neutralise the command centre",
                self.combat.target(13).unwrap().bounds.centre(),
            );
        }
        ("Return to the LZ and land", self.base)
    }
    pub fn fire(&mut self, heavy: bool) {
        let which = if heavy && self.selected == 0 {
            1
        } else {
            self.selected
        };
        if self.cooldown[which] > 0. || self.ammo[which] == 0 {
            return;
        }
        if which == 2 && self.craft.at.distance(self.aim) > MISSILE.range {
            self.message("Target outside missile range — move closer or flank around cover.");
            return;
        }
        if which == 2 && (self.lock < 0.65 || self.lock_target.is_none()) {
            self.message("Guided missile: keep the reticle over a target to acquire a lock.");
            return;
        }
        let weapon = [CANNON, ROCKET, MISSILE][which];
        let target = if which == 2 { self.lock_target } else { None };
        let aim = target
            .and_then(|id| self.combat.target(id))
            .map_or(self.aim, |t| t.bounds.centre());
        self.combat
            .shots
            .push(weapon.launch(self.craft.at, aim, 1, target));
        self.ammo[which] -= 1;
        self.cooldown[which] = [0.09, 0.45, 0.8][which];
    }
    pub fn flares(&mut self) {
        if self.flares == 0 || self.flare_cooldown > 0. {
            return;
        }
        self.flares -= 1;
        self.flare_time = 1.8;
        self.flare_cooldown = 1.2;
        self.combat.decoy(self.craft.at, 95., 2);
        self.message("Countermeasures deployed");
    }
    pub fn hook(&mut self, m: &Materials) {
        if let Some(id) = self.held.take() {
            let velocity = self.craft.velocity;
            if let Some(c) = self.cargo.iter_mut().find(|c| c.id == id) {
                c.release(velocity);
            }
            self.message("Load released. Shoot exposed oil with the cannon to ignite it.");
            return;
        }
        let centre = self.craft.at;
        let nearest = self
            .barrels
            .iter()
            .enumerate()
            .filter(|(_, b)| !b.taken)
            .filter_map(|(i, b)| {
                let p = Point::new(
                    b.at[0] as f32 + 3.,
                    b.at[1] as f32 + 3.,
                    b.at[2] as f32 + 4.,
                );
                let d = (p.x - centre.x).hypot(p.y - centre.y);
                (d < 12. && centre.z > p.z + 5. && centre.z < p.z + 40.).then_some((i, d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((i, _)) = nearest {
            let b = &mut self.barrels[i];
            let load = Cargo::capture(b.id, &mut self.world, b.at, [6, 6, 9]);
            if load.cells.is_empty() {
                b.taken = true;
                self.message("That barrel has already been destroyed.");
                return;
            }
            b.taken = true;
            self.held = Some(b.id);
            self.cargo.push(load);
            self.message("Oil barrel attached. G releases it; a hard drop ruptures the case.");
        } else {
            let _ = m;
            self.message("Winch: hover above an oil barrel, within the marked pickup radius.");
        }
    }
    pub fn update_lock(&mut self, id: Option<u32>) {
        if id == self.lock_target && id.is_some() {
            self.lock = (self.lock + DT).min(1.);
        } else {
            self.lock_target = id;
            self.lock = 0.;
        }
    }
    pub fn step(&mut self, input: FlightInput, fire: bool, heavy: bool, m: &Materials) {
        if self.phase != Phase::Flying {
            return;
        }
        let started = std::time::Instant::now();
        self.time += DT;
        self.toast_time = (self.toast_time - DT).max(0.);
        self.damaged_flash = (self.damaged_flash - DT * 2.).max(0.);
        for c in &mut self.cooldown {
            *c = (*c - DT).max(0.);
        }
        self.flare_time = (self.flare_time - DT).max(0.);
        self.flare_cooldown = (self.flare_cooldown - DT).max(0.);
        self.craft.settings.speed = if self.held.is_some() { 45. } else { 65. };
        self.craft.settings.acceleration = if self.held.is_some() { 100. } else { 145. };
        let old = self.craft.at;
        self.craft.step(input, &self.world, m, DT);
        let speed = self.craft.velocity.x.hypot(self.craft.velocity.y);
        if self.craft.blocked && old.distance(self.base) > 20. {
            if let Some(t) = self.combat.target_mut(0) {
                t.health = (t.health - 0.04).max(0.);
            }
        }
        self.fuel = (self.fuel - (0.08 + speed * 0.003) * DT).max(0.);
        let bounds = self.craft_bounds();
        self.combat.target_mut(0).unwrap().bounds = bounds;
        if fire {
            self.fire(false);
        }
        if heavy {
            self.fire(true);
        }
        let radars = self.remaining_radars();
        let power = self.alive(12);
        let safe = std::iter::once(&self.base)
            .chain(self.service_bases.iter())
            .any(|p| (self.craft.at.x - p.x).hypot(self.craft.at.y - p.y) < 30.);
        for u in &mut self.units {
            if !self.combat.alive(u.id) || !matches!(u.role, Role::Aa | Role::Sam) {
                continue;
            }
            u.timer -= DT;
            let p = self.combat.target(u.id).unwrap().bounds.centre();
            let distance = p.distance(self.craft.at);
            let range =
                if u.role == Role::Sam { 92. } else { 72. } * if radars == 0 { 0.63 } else { 1. };
            u.heading = (self.craft.at.y - p.y).atan2(self.craft.at.x - p.x);
            if u.timer <= 0. && distance < range && !safe {
                let mut weapon = Weapon {
                    speed: if u.role == Role::Sam { 83. } else { 125. },
                    range: if u.role == Role::Sam { 120. } else { 96. },
                    damage: if u.role == Role::Sam { 25. } else { 6. },
                    penetration: 0.5,
                    radius: 0.5,
                    force: 0.,
                    spark_heat: 0.,
                    guidance: if u.role == Role::Sam && power && radars > 0 {
                        2.1
                    } else {
                        0.
                    },
                    explosive: false,
                };
                if self.flare_time > 0. {
                    weapon.guidance = 0.;
                }
                let lead = if weapon.guidance > 0. { 0. } else { 0.2 };
                let aim = Point::new(
                    self.craft.at.x + self.craft.velocity.x * lead,
                    self.craft.at.y + self.craft.velocity.y * lead,
                    self.craft.at.z + 1.,
                );
                self.combat.shots.push(weapon.launch(
                    Point::new(p.x, p.y, p.z + 3.),
                    aim,
                    2,
                    if weapon.guidance > 0. { Some(0) } else { None },
                ));
                u.timer = if u.role == Role::Sam { 2.4 } else { 0.9 };
            }
        }
        let before = self.health();
        self.combat.step(&mut self.world, m, DT);
        self.world.focus = self.craft.at;
        self.world.step(m);
        self.combat.environment(&mut self.world, m);
        if self.health() < before {
            self.damaged_flash = 1.;
        }
        let mut notifications = Vec::new();
        for u in &mut self.units {
            if !u.seen_dead && !self.combat.alive(u.id) {
                u.seen_dead = true;
                self.score += match u.role {
                    Role::Radar => 200,
                    Role::Power => 250,
                    Role::Command => 400,
                    Role::Armoury => 180,
                    _ => 100,
                };
                notifications.push(match u.role {
                    Role::Radar => "Radar destroyed — enemy detection reduced",
                    Role::Power => "Power station destroyed — SAM guidance disabled",
                    Role::Command => "Command centre destroyed — finish rescues and return to LZ",
                    Role::Armoury => "Ammunition dump destroyed — bonus objective complete",
                    _ => "Defence destroyed",
                });
            }
        }
        if let Some(s) = notifications.last() {
            self.message(*s);
        }
        for c in &mut self.cargo {
            let anchor = (self.held == Some(c.id)).then_some(self.craft.at);
            c.step(&mut self.world, m, anchor, DT);
        }
        let mut rescued_msg = None;
        for r in &mut self.rescues {
            if r.collected {
                continue;
            }
            let d = (r.at.x - self.craft.at.x).hypot(r.at.y - self.craft.at.y);
            if d < 10.
                && self.craft.at.z > r.at.z + 8.
                && self.craft.at.z < r.at.z + 32.
                && speed < 11.
                && self.passengers + r.people <= 6
            {
                r.progress += DT;
                if r.progress > 1.4 {
                    r.collected = true;
                    self.passengers += r.people;
                    rescued_msg = Some(format!(
                        "{} aboard. Return to the landing zone to unload.",
                        r.name
                    ));
                }
            } else {
                r.progress = (r.progress - DT).max(0.);
            }
        }
        if let Some(s) = rescued_msg {
            self.message(s);
        }
        let mut supply_msg = None;
        for s in &mut self.supplies {
            if s.taken {
                continue;
            }
            if (s.at.x - self.craft.at.x).hypot(s.at.y - self.craft.at.y) < 9.
                && self.craft.at.z < s.at.z + 28.
                && speed < 16.
            {
                s.taken = true;
                self.score += 40;
                match s.kind.as_str() {
                    "ammo" => {
                        self.ammo[0] = (self.ammo[0] + 350).min(900);
                        self.ammo[1] = (self.ammo[1] + 18).min(48);
                        self.ammo[2] = (self.ammo[2] + 3).min(10);
                    }
                    "fuel" => self.fuel = (self.fuel + 60.).min(100.),
                    _ => {
                        let p = self.combat.target_mut(0).unwrap();
                        p.health = (p.health + 90.).min(p.max_health);
                    }
                }
                supply_msg = Some(format!("{} supplies collected", s.kind));
            }
        }
        if let Some(s) = supply_msg {
            self.message(s);
        }
        let base_distance = std::iter::once(&self.base)
            .chain(self.service_bases.iter())
            .map(|p| (p.x - self.craft.at.x).hypot(p.y - self.craft.at.y))
            .fold(f32::INFINITY, f32::min);
        if base_distance < 13. && self.craft.at.z < 15. && speed < 9. {
            self.service += DT;
            if self.service > 1.5 {
                if self.passengers > 0 {
                    self.rescued += self.passengers;
                    self.score += self.passengers * 150;
                    self.passengers = 0;
                    self.message("Survivors safely delivered. Repairing and rearming.");
                }
                self.fuel = (self.fuel + DT * 20.).min(100.);
                let p = self.combat.target_mut(0).unwrap();
                p.health = (p.health + DT * 18.).min(p.max_health);
                if self.service > 3. {
                    self.ammo = [700, 36, 8];
                    self.flares = 8;
                }
                if self.primary_complete() {
                    self.phase = Phase::Won;
                    self.score += (self.health() * 3. + self.fuel * 5.) as u32;
                }
            }
        } else {
            self.service = 0.;
        }
        if self.health() <= 0. || self.fuel <= 0. {
            self.phase = Phase::Lost;
            self.message(if self.fuel <= 0. {
                "Fuel exhausted"
            } else {
                "Aircraft lost"
            });
        }
        self.step_ms = started.elapsed().as_secs_f32() * 1000.;
    }
}
