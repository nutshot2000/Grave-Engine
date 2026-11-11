//! Voidrunner: a horizontal shooter whose ships, fuel and wreckage are engine cells.
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use grave_engine::{
    material::{AIR, Materials, State},
    physics::{Balance, BodySpec},
    render::{Renderer, View},
    sim::Sim,
};
use std::time::Instant;
const W: usize = 480;
const H: usize = 270;
#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Player,
    Scout,
    Interceptor,
    Tanker,
    Frigate,
    Asteroid,
    Boss,
}
impl Kind {
    fn parse(s: &str) -> Self {
        match s {
            "scout" => Self::Scout,
            "interceptor" => Self::Interceptor,
            "tanker" => Self::Tanker,
            "frigate" => Self::Frigate,
            "asteroid" => Self::Asteroid,
            "boss" => Self::Boss,
            _ => panic!("unknown ship {s}"),
        }
    }
}
struct Ship {
    id: u64,
    kind: Kind,
    born: u32,
    base_y: f32,
    cells: usize,
    fuel: usize,
    dead: bool,
}
#[derive(Clone, Copy)]
struct Shot {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    friendly: bool,
    power: i32,
    ttl: u32,
}
struct Pickup {
    x: f32,
    y: f32,
    weapon: bool,
}
struct Wave {
    at: u32,
    kind: Kind,
    y: f32,
}
struct Game {
    sim: Sim,
    mats: Materials,
    renderer: Renderer,
    texture: Option<egui::TextureHandle>,
    ships: Vec<Ship>,
    shots: Vec<Shot>,
    pickups: Vec<Pickup>,
    waves: Vec<Wave>,
    next: usize,
    frame: u32,
    score: u32,
    kills: u32,
    shield: f32,
    lives: u32,
    invuln: u32,
    weapon: u32,
    bombs: u32,
    fire_cd: u32,
    bomb_cd: u32,
    started: bool,
    paused: bool,
    over: bool,
    won: bool,
    boss_spawned: bool,
    shake: f32,
    flash: f32,
    clock: f64,
    last: Instant,
}
impl Game {
    fn new() -> Self {
        let mats = Materials::load_dir("materials");
        assert!(mats.problems.is_empty(), "{:?}", mats.problems);
        let sim = Sim::empty(W, H, &mats);
        let waves = std::fs::read_to_string("assets/voidrunner/level.txt")
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .map(|l| {
                let a: Vec<_> = l.split_whitespace().collect();
                Wave {
                    at: (a[0].parse::<f32>().unwrap() * 60.) as u32,
                    kind: Kind::parse(a[1]),
                    y: a[2].parse().unwrap(),
                }
            })
            .collect();
        let mut g = Self {
            sim,
            mats,
            renderer: Renderer::default(),
            texture: None,
            ships: vec![],
            shots: vec![],
            pickups: vec![],
            waves,
            next: 0,
            frame: 0,
            score: 0,
            kills: 0,
            shield: 100.,
            lives: 3,
            invuln: 120,
            weapon: 1,
            bombs: 3,
            fire_cd: 0,
            bomb_cd: 0,
            started: false,
            paused: false,
            over: false,
            won: false,
            boss_spawned: false,
            shake: 0.,
            flash: 0.,
            clock: 0.,
            last: Instant::now(),
        };
        g.spawn(Kind::Player, 75., 135.);
        g
    }
    // A layered hull, struts, panel seams, cockpit, exhaust and exposed fuel ports.
    // Every visible ship pixel is one of the engine's materials.
    fn blueprint(&self, kind: Kind, x: f32, y: f32) -> BodySpec {
        let (w, h) = match kind {
            Kind::Player => (40, 26),
            Kind::Scout => (32, 24),
            Kind::Interceptor => (34, 30),
            Kind::Tanker => (64, 38),
            Kind::Frigate => (70, 48),
            Kind::Asteroid => (30, 30),
            Kind::Boss => (112, 116),
        };
        let mut grid = vec![None; w * h];
        let mut paint = |name: &str, x0: usize, y0: usize, x1: usize, y1: usize| {
            let m = self.mats.id(name).unwrap();
            for yy in y0..y1.min(h) {
                for xx in x0..x1.min(w) {
                    grid[yy * w + xx] = Some(m);
                }
            }
        };
        if kind == Kind::Asteroid {
            for yy in 0..h {
                for xx in 0..w {
                    let dx = xx as f32 - w as f32 / 2.;
                    let dy = yy as f32 - h as f32 / 2.;
                    if dx * dx + dy * dy < (12 + (xx * 7 + yy * 13) % 5) as f32 * 12. {
                        paint(
                            if (xx + yy) % 7 == 0 { "iron" } else { "rock" },
                            xx,
                            yy,
                            xx + 1,
                            yy + 1,
                        );
                    }
                }
            }
        } else if kind == Kind::Boss {
            paint("steel", 15, 8, 100, 108);
            paint("aluminium", 8, 28, 105, 88);
            paint("plastic", 24, 12, 90, 20);
            paint("plastic", 24, 96, 90, 104);
            paint("steel", 0, 20, 40, 31);
            paint("steel", 0, 85, 40, 96);
            paint("copper", 5, 22, 28, 26);
            paint("copper", 5, 90, 28, 94);
            paint("iron", 23, 37, 90, 79);
            paint("steel", 45, 44, 62, 72);
            // Thin glass inspection port over the leading side of the core.
            paint("glass", 23, 48, 45, 68);
            paint("copper", 25, 43, 43, 47);
            paint("copper", 25, 69, 43, 73);
            for yy in (14..106).step_by(9) {
                paint("copper", 93, yy, 108, yy + 3);
            }
            for yy in [5, 100] {
                paint("steel", 60, yy, 110, yy + 11);
                paint("glass", 94, yy + 2, 108, yy + 6);
            }
            for yy in (32..85).step_by(8) {
                paint("steel", 64, yy, 93, yy + 2);
            }
            // Three separated real explosive reactors: one blast cannot clear all three.
            for yy in [20, 58, 96] {
                paint("copper", 14, yy - 6, 51, yy + 6);
                paint("glass", 14, yy - 4, 46, yy + 4);
                paint("gunpowder", 46, yy, 47, yy + 1);
            }
        } else {
            let player = kind == Kind::Player;
            // Pointed central fuselage and swept wings.
            for yy in 0..h {
                for xx in 0..w {
                    let cy = (yy as f32 - h as f32 / 2.).abs();
                    let nose = if player { xx } else { w - 1 - xx };
                    if cy < 4. + (w - 1 - nose) as f32 * 0.12 && nose > 3 {
                        paint("aluminium", xx, yy, xx + 1, yy + 1);
                    }
                    if nose > 5
                        && nose < w * 2 / 3
                        && cy < (h as f32 / 2. - (nose as f32 - w as f32 / 3.).abs() * 0.45)
                    {
                        paint("steel", xx, yy, xx + 1, yy + 1);
                    }
                }
            }
            let mid = h / 2;
            paint("iron", w / 4, mid - 5, w * 3 / 4, mid + 5);
            paint("copper", w / 4, mid - 2, w * 3 / 4, mid + 2);
            if player {
                paint("glass", 22, mid - 4, 32, mid + 4);
                paint("copper", 8, 3, 20, 5);
                paint("copper", 8, h - 5, 20, h - 3);
                paint("steel", 2, mid - 3, 10, mid + 3);
            } else {
                paint("plastic", w / 3, 3, w * 3 / 4, 6);
                paint("plastic", w / 3, h - 6, w * 3 / 4, h - 3);
                paint("gunpowder", w / 2 - 3, mid - 4, w / 2 + 4, mid + 4);
                paint("glass", w / 2 - 7, mid - 3, w / 2 - 3, mid + 3);
                paint("copper", 3, mid - 1, w / 3, mid + 1);
                if kind == Kind::Tanker || kind == Kind::Frigate {
                    for yy in [6, h - 14] {
                        paint("steel", 18, yy, w - 5, yy + 10);
                        paint("oil", 23, yy + 2, w - 10, yy + 8);
                        paint("gunpowder", 26, yy + 3, w - 13, yy + 7);
                        paint("glass", 18, yy + 3, 23, yy + 7);
                    }
                    paint("plastic", w - 13, mid - 5, w - 4, mid + 5);
                }
            }
        }
        let mut b = BodySpec::from_mats(w, h, &grid, &self.mats, x, y);
        b.depth = 0.025;
        b
    }
    fn spawn(&mut self, kind: Kind, x: f32, y: f32) -> u64 {
        let b = self.blueprint(kind, x, y);
        let cells = b.cell_count();
        let fuel = b
            .cells
            .iter()
            .flatten()
            .filter(|c| c.mat == self.mats.id("gunpowder").unwrap())
            .count();
        let id = self
            .sim
            .physics
            .add_exact(&mut self.sim.world, &self.mats, b)
            .unwrap();
        self.sim.physics.set_balance(
            id,
            Balance {
                strength: 10.,
                air: 1.,
                heavy: 0.,
            },
        );
        self.ships.push(Ship {
            id,
            kind,
            born: self.frame,
            base_y: y,
            cells,
            fuel,
            dead: false,
        });
        id
    }
    fn player(&self) -> Option<(u64, f32, f32)> {
        self.ships
            .iter()
            .filter(|s| s.kind == Kind::Player && !s.dead)
            .find_map(|s| self.sim.physics.centre(s.id).map(|(x, y)| (s.id, x, y)))
    }
    fn shoot(&mut self, x: f32, y: f32, angle: f32, speed: f32, friendly: bool, power: i32) {
        self.shots.push(Shot {
            x,
            y,
            vx: angle.cos() * speed,
            vy: angle.sin() * speed,
            friendly,
            power,
            ttl: 150,
        });
    }
    // Sweep every projectile path a cell at a time: fast bullets cannot skip armour.
    fn impact(&mut self, x: i32, y: i32, power: i32) {
        if !self.sim.world.in_bounds(x, y) {
            return;
        }
        let powder = self.mats.id("gunpowder").unwrap();
        self.sim.world.heat_circle(x, y, power + 1, 700.);
        for dy in -power..=power {
            for dx in -power..=power {
                if dx * dx + dy * dy > power * power {
                    continue;
                }
                let (xx, yy) = (x + dx, y + dy);
                if !self.sim.world.in_bounds(xx, yy) {
                    continue;
                }
                let i = self.sim.world.idx(xx as usize, yy as usize);
                let m = self.sim.world.mat[i];
                if m == AIR {
                    continue;
                }
                // Heat explosive cells instead of deleting them: the engine detonates them.
                if m == powder {
                    self.sim.world.temp[i] = 1300.;
                    self.sim.world.wake_at(i);
                    continue;
                }
                let name = &self.mats.get(m).name;
                if (dx.abs() + dy.abs()) > power && name == "steel" {
                    continue;
                }
                self.sim.world.throw(
                    &self.mats,
                    xx as usize,
                    yy as usize,
                    dx as f32 * 0.7,
                    dy as f32 * 0.7,
                );
                self.sim.world.make_air(i);
            }
        }
        self.shake = self.shake.max(0.6);
    }
    fn damage_player(&mut self, amount: f32) {
        if self.invuln > 0 || self.over {
            return;
        }
        self.shield -= amount;
        self.invuln = 30;
        self.flash = 0.5;
        self.shake = 2.;
        if self.shield <= 0. {
            self.lose_life();
        }
    }
    fn lose_life(&mut self) {
        if self.over {
            return;
        }
        self.lives = self.lives.saturating_sub(1);
        if let Some((id, x, y)) = self.player() {
            self.sim.world.heat_circle(x as i32, y as i32, 12, 1600.);
            self.sim.physics.remove(&mut self.sim.world, id);
            for s in &mut self.ships {
                if s.id == id {
                    s.dead = true;
                }
            }
        }
        for s in &mut self.ships {
            if s.kind == Kind::Player {
                s.dead = true;
            }
        }
        if self.lives == 0 {
            self.over = true;
            self.started = false;
        } else {
            self.spawn(Kind::Player, 65., 135.);
            self.shield = 100.;
            self.invuln = 180;
            self.weapon = self.weapon.max(2) - 1;
        }
    }
    fn bomb(&mut self) {
        if self.bombs == 0 || self.bomb_cd > 0 {
            return;
        }
        self.bombs -= 1;
        self.bomb_cd = 90;
        self.flash = 0.7;
        self.shake = 4.;
        self.shots.retain(|b| b.friendly);
        let targets: Vec<_> = self
            .ships
            .iter()
            .filter(|s| s.kind != Kind::Player && s.kind != Kind::Asteroid && !s.dead)
            .filter_map(|s| self.sim.physics.centre(s.id))
            .collect();
        for (x, y) in targets {
            self.impact((x - 8.) as i32, y as i32, 8);
        }
    }
    fn tick(&mut self, mx: f32, my: f32, fire: bool, bomb: bool) {
        if self.over || self.won {
            return;
        }
        self.frame += 1;
        self.invuln = self.invuln.saturating_sub(1);
        self.fire_cd = self.fire_cd.saturating_sub(1);
        self.bomb_cd = self.bomb_cd.saturating_sub(1);
        self.shake *= 0.9;
        self.flash *= 0.88;
        while self.next < self.waves.len() && self.frame >= self.waves[self.next].at {
            let (k, y) = (self.waves[self.next].kind, self.waves[self.next].y);
            if k == Kind::Boss {
                // The carrier enters a fresh arena after the convoy, rather than overlapping
                // old hot fragments at the right-hand spawn edge.
                let player_id = self.player().map(|p| p.0);
                for id in self.sim.physics.ids() {
                    if Some(id) != player_id {
                        self.sim.physics.remove(&mut self.sim.world, id);
                    }
                }
                for i in 0..self.sim.world.mat.len() {
                    if !self.sim.world.is_owned(i) {
                        self.sim.world.make_air(i);
                    }
                }
                for s in &mut self.ships {
                    if s.kind != Kind::Player {
                        s.dead = true;
                    }
                }
                self.shots.clear();
                self.sim.world.flying.clear();
            }
            self.spawn(
                k,
                if k == Kind::Boss {
                    W as f32 - 60.
                } else {
                    W as f32 - 36.
                },
                y,
            );
            if k == Kind::Boss {
                self.boss_spawned = true;
            }
            self.next += 1;
        }
        let Some((pid, px, py)) = self.player() else {
            self.lose_life();
            return;
        };
        let speed = 125.;
        let mut vx = mx * speed;
        let mut vy = my * speed;
        if px < 25. {
            vx = vx.max(55.);
        }
        if px > W as f32 - 35. {
            vx = vx.min(-55.);
        }
        if py < 22. {
            vy = vy.max(55.);
        }
        if py > H as f32 - 22. {
            vy = vy.min(-55.);
        }
        self.sim.physics.set_kinematic(pid, true, true);
        self.sim.physics.set_velocity(pid, vx, vy, 0.);
        if fire && self.fire_cd == 0 {
            self.fire_cd = 7;
            self.shoot(px + 23., py, 0., 410., true, 2);
            if self.weapon >= 2 {
                self.shoot(px + 18., py - 7., -0.045, 400., true, 2);
                self.shoot(px + 18., py + 7., 0.045, 400., true, 2);
            }
            if self.weapon >= 3 {
                self.shoot(px + 16., py - 10., -0.13, 380., true, 2);
                self.shoot(px + 16., py + 10., 0.13, 380., true, 2);
            }
        }
        if bomb {
            self.bomb();
        }
        let mut enemy_fire = vec![];
        for s in &self.ships {
            if s.dead || s.kind == Kind::Player {
                continue;
            }
            let Some((x, y)) = self.sim.physics.centre(s.id) else {
                continue;
            };
            let age = self.frame - s.born;
            let (vx, vy) = match s.kind {
                Kind::Boss => {
                    let ty = 135. + (age as f32 * 0.014).sin() * 42.;
                    ((385. - x) * 1.8, (ty - y) * 1.4)
                }
                Kind::Asteroid => (-30., (s.base_y - 135.).signum() * 3.),
                Kind::Tanker => (-23., (s.base_y + (age as f32 * 0.018).sin() * 12. - y) * 2.),
                Kind::Frigate => (-18., (s.base_y + (age as f32 * 0.014).sin() * 20. - y) * 2.),
                Kind::Interceptor => (-66., (s.base_y + (age as f32 * 0.035).sin() * 28. - y) * 3.),
                _ => (-44., (s.base_y + (age as f32 * 0.025).sin() * 20. - y) * 3.),
            };
            self.sim.physics.set_velocity(
                s.id,
                vx,
                vy,
                if s.kind == Kind::Asteroid { 0.12 } else { 0. },
            );
            let period = match s.kind {
                Kind::Boss => 48,
                Kind::Frigate => 85,
                Kind::Tanker => 150,
                Kind::Interceptor => 105,
                _ => 130,
            };
            if age > 30 && age % period == 0 && s.kind != Kind::Asteroid && x > px + 35. {
                let aim = (py - y).atan2(px - x);
                let spread = if s.kind == Kind::Boss || s.kind == Kind::Frigate {
                    2
                } else {
                    0
                };
                for n in -spread..=spread {
                    enemy_fire.push((
                        x - 18.,
                        y,
                        aim + n as f32 * 0.19,
                        if s.kind == Kind::Boss { 90. } else { 75. },
                    ));
                }
            }
        }
        for (x, y, a, v) in enemy_fire {
            self.shoot(x, y, a, v, false, 1);
        }
        let old = std::mem::take(&mut self.shots);
        for mut shot in old {
            let (nx, ny) = (shot.x + shot.vx / 60., shot.y + shot.vy / 60.);
            let steps = ((nx - shot.x).hypot(ny - shot.y).ceil() as u32).max(1);
            let mut hit = false;
            for n in 1..=steps {
                let t = n as f32 / steps as f32;
                let (x, y) = (shot.x + (nx - shot.x) * t, shot.y + (ny - shot.y) * t);
                if !self.sim.world.in_bounds(x as i32, y as i32) {
                    continue;
                }
                let i = self.sim.world.idx(x as usize, y as usize);
                if self.sim.world.mat[i] == AIR
                    || self.mats.get(self.sim.world.mat[i]).state == State::Gas
                {
                    continue;
                }
                let owner = self
                    .sim
                    .physics
                    .id_at(&self.sim.world, x as usize, y as usize);
                if shot.friendly && owner == Some(pid) {
                    continue;
                }
                if !shot.friendly && owner != Some(pid) {
                    continue;
                }
                if shot.friendly
                    && self.ships.iter().any(|s| {
                        s.kind == Kind::Boss
                            && !s.dead
                            && Some(s.id) == owner
                            && self.boss_shielded(s.born)
                    })
                {
                    hit = true;
                    break;
                }
                if !shot.friendly {
                    self.damage_player(18.);
                    if self.invuln > 1 {
                        hit = true;
                        break;
                    }
                }
                self.impact(x as i32, y as i32, shot.power);
                hit = true;
                break;
            }
            shot.x = nx;
            shot.y = ny;
            shot.ttl = shot.ttl.saturating_sub(1);
            if !hit && shot.ttl > 0 && nx > 0. && nx < W as f32 && ny > 0. && ny < H as f32 {
                self.shots.push(shot);
            }
        }
        self.sim.step(&self.mats);
        if !self.sim.world.blasts.is_empty() {
            self.shake = self.shake.max(2.);
        }
        let powder = self.mats.id("gunpowder").unwrap();
        let mut rewards = vec![];
        let reactors_left = self.remaining_reactors();
        for s in &mut self.ships {
            if s.dead {
                continue;
            }
            let b = self.sim.physics.spec_of(&self.sim.world, s.id);
            if s.kind == Kind::Player {
                continue;
            }
            let destroyed = if s.kind == Kind::Boss {
                reactors_left == 0
            } else {
                b.as_ref().is_none_or(|b| {
                    b.cell_count() < s.cells / 3
                        || (s.fuel > 0
                            && b.cells.iter().flatten().filter(|c| c.mat == powder).count()
                                < s.fuel / 3)
                })
            };
            if destroyed {
                s.dead = true;
                let (x, y) = b
                    .as_ref()
                    .map_or((W as f32 - 50., s.base_y), |b| (b.x, b.y));
                self.score += if s.kind == Kind::Boss {
                    10000
                } else if s.kind == Kind::Asteroid {
                    100
                } else {
                    500
                };
                self.kills += 1;
                if s.kind == Kind::Boss {
                    self.won = true;
                    self.started = false;
                    self.flash = 1.;
                    self.score += self.lives * 1000;
                } else if self.kills % 2 == 0 {
                    rewards.push((x, y, self.kills % 4 == 0));
                }
            } else if let Some(b) = b {
                if b.x < 18. || b.y > H as f32 - 12. {
                    s.dead = true;
                }
            }
        }
        for (x, y, weapon) in rewards {
            self.pickups.push(Pickup { x, y, weapon });
        }
        // Collision with a still-active ship damages shields; native impacts can break hulls.
        if let Some((_, px, py)) = self.player() {
            let collision = self
                .ships
                .iter()
                .filter(|s| !s.dead && s.kind != Kind::Player)
                .any(|s| {
                    self.sim
                        .physics
                        .centre(s.id)
                        .is_some_and(|(x, y)| (x - px).abs() < 25. && (y - py).abs() < 16.)
                });
            if collision {
                self.damage_player(35.);
            }
        }
        if self.player().is_none() {
            self.lose_life();
        }
        for p in &mut self.pickups {
            p.x -= 0.65;
        }
        if let Some((_, px, py)) = self.player() {
            self.pickups.retain(|p| {
                if (p.x - px).hypot(p.y - py) < 22. {
                    if p.weapon {
                        self.weapon = (self.weapon + 1).min(3);
                    } else {
                        self.shield = (self.shield + 35.).min(100.);
                    }
                    self.score += 150;
                    false
                } else {
                    p.x > 0.
                }
            });
        }
        // Cull only offscreen wrecks, retaining visible physical fragments and fire.
        if self.frame % 30 == 0 {
            let live: Vec<_> = self
                .ships
                .iter()
                .filter(|s| !s.dead)
                .map(|s| s.id)
                .collect();
            for id in self.sim.physics.ids() {
                if let Some((x, y)) = self.sim.physics.centre(id) {
                    if id != pid && (x < 14. || x > W as f32 + 40. || y > H as f32 - 15. || y < 5.)
                    {
                        self.sim.physics.remove(&mut self.sim.world, id);
                    } else if !live.contains(&id) {
                        let core = self.boss_spawned
                            && self
                                .sim
                                .physics
                                .spec_of(&self.sim.world, id)
                                .is_some_and(|b| b.cells.iter().flatten().any(|c| c.mat == powder));
                        if core {
                            self.sim.physics.set_kinematic(id, true, true);
                            self.sim.physics.set_velocity(id, -2., 0., 0.);
                        } else {
                            self.sim.physics.set_velocity(id, -32., 22., 0.3);
                        }
                    }
                }
            }
            if self.sim.physics.body_count() > 150 {
                for id in self.sim.physics.ids() {
                    if self.sim.physics.body_count() <= 100 {
                        break;
                    }
                    if !live.contains(&id)
                        && !self
                            .sim
                            .physics
                            .spec_of(&self.sim.world, id)
                            .is_some_and(|b| b.cells.iter().flatten().any(|c| c.mat == powder))
                    {
                        self.sim.physics.remove(&mut self.sim.world, id);
                    }
                }
            }
            for y in H - 8..H {
                for x in 0..W {
                    let i = self.sim.world.idx(x, y);
                    if !self.sim.world.is_owned(i) {
                        self.sim.world.make_air(i);
                    }
                }
            }
            self.ships
                .retain(|s| !s.dead || self.sim.physics.centre(s.id).is_some());
        }
    }
    fn remaining_reactors(&self) -> usize {
        let fuel = self.mats.id("gunpowder").unwrap();
        self.sim.world.mat.iter().filter(|m| **m == fuel).count()
            + self
                .sim
                .world
                .flying
                .iter()
                .filter(|f| f.mat == fuel)
                .count()
    }
    fn boss_shielded(&self, born: u32) -> bool {
        let age = self.frame.saturating_sub(born);
        age < 180 || age % 360 < 180
    }
    fn stage(&self) -> (&str, &str) {
        match self.frame {
            0..=1199 => ("01 / ORBITAL APPROACH", "INTERCEPT THE FUEL CONVOY"),
            1200..=2699 => (
                "02 / DEBRIS BELT",
                "CLEAR THE ASTEROIDS / WATCH THE INTERCEPTORS",
            ),
            2700..=4799 => (
                "03 / REFINERY ASSAULT",
                "BREACH ARMOUR / IGNITE THE AMBER FUEL PORTS",
            ),
            _ => (
                "04 / REACTOR CARRIER",
                "WAIT FOR SHIELD TO DROP / DETONATE ALL THREE REACTORS",
            ),
        }
    }
}
impl eframe::App for Game {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dt = self.last.elapsed().as_secs_f64().min(0.1);
        self.last = Instant::now();
        if ctx.input(|i| i.key_pressed(egui::Key::R)) {
            *self = Self::new();
            self.started = true;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::P) || i.key_pressed(egui::Key::Escape)) {
            self.paused = !self.paused;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && !self.over && !self.won {
            self.started = true;
        }
        let mx = ctx.input(|i| {
            (i.key_down(egui::Key::D) || i.key_down(egui::Key::ArrowRight)) as i32
                - (i.key_down(egui::Key::A) || i.key_down(egui::Key::ArrowLeft)) as i32
        }) as f32;
        let my = ctx.input(|i| {
            (i.key_down(egui::Key::S) || i.key_down(egui::Key::ArrowDown)) as i32
                - (i.key_down(egui::Key::W) || i.key_down(egui::Key::ArrowUp)) as i32
        }) as f32;
        let fire = ctx.input(|i| i.key_down(egui::Key::Space) || i.key_down(egui::Key::Z));
        let mut bomb = ctx.input(|i| i.key_pressed(egui::Key::X));
        if self.started && !self.paused && ctx.input(|i| i.focused) {
            self.clock += dt;
            let mut n = 0;
            while self.clock >= 1. / 60. && n < 6 && !self.over && !self.won {
                let len = (mx * mx + my * my).sqrt().max(1.);
                self.tick(mx / len, my / len, fire, bomb);
                bomb = false;
                self.clock -= 1. / 60.;
                n += 1;
            }
        } else {
            self.clock = 0.;
        }
        let available = ui.available_size();
        let scale = (available.x / W as f32).min(available.y / H as f32);
        let size = Vec2::new(W as f32 * scale, H as f32 * scale);
        let (outer, _) = ui.allocate_exact_size(available, egui::Sense::hover());
        let mut rect = Rect::from_center_size(outer.center(), size);
        let shake = Vec2::new(
            (self.frame as f32 * 2.7).sin(),
            (self.frame as f32 * 3.9).cos(),
        ) * self.shake
            * scale;
        rect = rect.translate(shake);
        let p = ui.painter().with_clip_rect(outer);
        p.rect_filled(outer, 0., Color32::BLACK);
        let to = |x: f32, y: f32| Pos2::new(rect.left() + x * scale, rect.top() + y * scale);
        let t = self.frame as f32;
        let sector = (self.frame / 1200).min(3);
        p.rect_filled(rect, 0., Color32::from_rgb(5, 8, 20));
        // Distant nebula bands, planet and three independent star layers scroll forever.
        for n in 0..28 {
            let y = n as f32 * 10.;
            let c = if sector == 2 {
                Color32::from_rgb(12 + (n % 4) as u8, 10, 25)
            } else {
                Color32::from_rgb(7, 10 + (n % 5) as u8, 23 + (n % 6) as u8)
            };
            p.rect_filled(
                Rect::from_min_size(to(0., y), Vec2::new(size.x, 11. * scale)),
                0.,
                c,
            );
        }
        let planet_x = 390. - t * 0.012;
        p.circle_filled(
            to(planet_x, 90.),
            58. * scale,
            Color32::from_rgb(24, 31, 55),
        );
        p.circle_stroke(
            to(planet_x, 90.),
            59. * scale,
            Stroke::new(2. * scale, Color32::from_rgb(45, 65, 91)),
        );
        for layer in 0..3 {
            for n in 0..75 {
                let seed = (n * 7919 + layer * 104729) as f32;
                let x = (seed * 1.371 - t * (0.12 + layer as f32 * 0.16)).rem_euclid(W as f32);
                let y = (seed * 0.719).rem_euclid(H as f32);
                let c = 75 + layer as u8 * 55;
                p.circle_filled(
                    to(x, y),
                    (0.25 + layer as f32 * 0.18) * scale,
                    Color32::from_rgb(c, c, c.saturating_add(20)),
                );
            }
        }
        // Distant refinery structures move more slowly than combat ships.
        if sector >= 2 {
            for n in 0..9 {
                let x = (n as f32 * 83. - t * 0.21).rem_euclid(650.) - 80.;
                let y = if n % 2 == 0 { H as f32 - 12. } else { 12. };
                p.line_segment(
                    [to(x - 40., y), to(x + 42., y)],
                    Stroke::new(9. * scale, Color32::from_rgb(20, 29, 40)),
                );
                p.line_segment(
                    [to(x, y), to(x, y + if n % 2 == 0 { -18. } else { 18. })],
                    Stroke::new(3. * scale, Color32::from_rgb(36, 42, 52)),
                );
            }
        }
        self.renderer.light(&self.sim);
        let mut rgba = self
            .renderer
            .draw(&self.sim.world, &self.mats, View::default())
            .to_vec();
        let air = self.mats.get(AIR).color;
        for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
            if self.sim.world.mat[i] == AIR && (0..3).all(|k| px[k].abs_diff(air[k]) < 4) {
                px[3] = 0;
            }
        }
        let image = egui::ColorImage::from_rgba_unmultiplied([W, H], &rgba);
        if let Some(tx) = &mut self.texture {
            tx.set(image, egui::TextureOptions::NEAREST);
        } else {
            self.texture =
                Some(ctx.load_texture("voidrunner-world", image, egui::TextureOptions::NEAREST));
        }
        p.image(
            self.texture.as_ref().unwrap().id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
            Color32::WHITE,
        );
        for s in &self.ships {
            if s.dead {
                continue;
            }
            if let Some((x, y)) = self.sim.physics.centre(s.id) {
                if s.kind != Kind::Asteroid {
                    let engine_x = if s.kind == Kind::Player {
                        x - 20.
                    } else {
                        x + if s.kind == Kind::Boss { 53. } else { 18. }
                    };
                    let dir = if s.kind == Kind::Player { -1. } else { 1. };
                    let flicker = 5. + (t * 1.9).sin().abs() * 5.;
                    p.line_segment(
                        [to(engine_x, y), to(engine_x + dir * flicker, y)],
                        Stroke::new(4. * scale, Color32::from_rgb(35, 117, 170)),
                    );
                    p.line_segment(
                        [to(engine_x, y), to(engine_x + dir * flicker * 0.7, y)],
                        Stroke::new(1.7 * scale, Color32::from_rgb(160, 245, 255)),
                    );
                }
                if s.kind == Kind::Player && self.invuln > 0 {
                    p.circle_stroke(
                        to(x, y),
                        24. * scale,
                        Stroke::new(scale, Color32::from_rgba_unmultiplied(70, 210, 255, 100)),
                    );
                }
                if s.kind == Kind::Boss && self.boss_shielded(s.born) {
                    p.line_segment(
                        [to(x - 57., y - 57.), to(x - 57., y + 57.)],
                        Stroke::new(2. * scale, Color32::from_rgb(50, 180, 255)),
                    );
                }
                if s.kind == Kind::Tanker || s.kind == Kind::Boss {
                    p.circle_stroke(
                        to(x - 5., y),
                        if s.kind == Kind::Boss {
                            14. * scale
                        } else {
                            5. * scale
                        },
                        Stroke::new(scale, Color32::from_rgba_unmultiplied(255, 170, 45, 150)),
                    );
                }
            }
        }
        for b in &self.shots {
            let c = if b.friendly {
                Color32::from_rgb(100, 235, 255)
            } else {
                Color32::from_rgb(255, 85, 105)
            };
            let d = Vec2::new(b.vx, b.vy).normalized() * if b.friendly { 5. } else { 3. };
            p.line_segment(
                [to(b.x - d.x, b.y - d.y), to(b.x, b.y)],
                Stroke::new(2. * scale, c),
            );
            p.circle_filled(to(b.x, b.y), 0.65 * scale, Color32::WHITE);
        }
        for pick in &self.pickups {
            p.circle_stroke(
                to(pick.x, pick.y),
                5. * scale,
                Stroke::new(
                    1.2 * scale,
                    if pick.weapon {
                        Color32::from_rgb(255, 190, 55)
                    } else {
                        Color32::from_rgb(65, 255, 180)
                    },
                ),
            );
            p.text(
                to(pick.x, pick.y),
                egui::Align2::CENTER_CENTER,
                if pick.weapon { "W" } else { "+" },
                egui::FontId::proportional(9. * scale),
                Color32::WHITE,
            );
        }
        // Restrained HUD keeps the centre of the combat area clear.
        p.rect_filled(
            Rect::from_min_max(to(0., 0.), to(W as f32, 19.)),
            0.,
            Color32::from_rgba_unmultiplied(4, 8, 18, 225),
        );
        let font = egui::FontId::proportional(7. * scale);
        p.text(
            to(8., 5.),
            egui::Align2::LEFT_TOP,
            "VOIDRUNNER / OPERATION CINDER",
            font.clone(),
            Color32::from_rgb(110, 215, 235),
        );
        p.text(
            to(472., 5.),
            egui::Align2::RIGHT_TOP,
            format!(
                "{:07}   LIVES {}   GUN {}   BOMBS {}",
                self.score, self.lives, self.weapon, self.bombs
            ),
            font.clone(),
            Color32::WHITE,
        );
        p.rect_filled(
            Rect::from_min_max(to(8., 21.), to(88., 24.)),
            0.,
            Color32::from_rgb(35, 45, 60),
        );
        p.rect_filled(
            Rect::from_min_max(to(8., 21.), to(8. + self.shield.max(0.) * 0.8, 24.)),
            0.,
            Color32::from_rgb(65, 205, 210),
        );
        let (stage, objective) = self.stage();
        p.text(
            to(472., 23.),
            egui::Align2::RIGHT_TOP,
            stage,
            font.clone(),
            Color32::from_rgb(220, 160, 80),
        );
        p.text(
            to(8., 261.),
            egui::Align2::LEFT_BOTTOM,
            "WASD / ARROWS: MOVE    SPACE / Z: FIRE    X: PULSE BOMB    P: PAUSE    R: RESTART",
            egui::FontId::proportional(5.8 * scale),
            Color32::from_rgb(145, 165, 187),
        );
        let progress = (self.frame as f32 / 4800.).min(1.);
        p.line_segment(
            [to(8., 267.), to(8. + 464. * progress, 267.)],
            Stroke::new(scale, Color32::from_rgb(80, 175, 210)),
        );
        if self.frame % 1200 < 180 && self.started {
            p.text(
                to(240., 45.),
                egui::Align2::CENTER_TOP,
                objective,
                egui::FontId::proportional(8. * scale),
                Color32::from_rgb(230, 205, 150),
            );
        }
        if self.boss_spawned && !self.won {
            let fuel = self.mats.id("gunpowder").unwrap();
            for (i, m) in self.sim.world.mat.iter().enumerate() {
                if *m == fuel {
                    p.circle_stroke(
                        to((i % W) as f32, (i / W) as f32),
                        5. * scale,
                        Stroke::new(scale, Color32::from_rgb(255, 190, 65)),
                    );
                }
            }
            if let Some(s) = self.ships.iter().find(|s| s.kind == Kind::Boss && !s.dead) {
                let remaining = self.remaining_reactors() as f32 / 3.;
                p.text(
                    to(240., 29.),
                    egui::Align2::CENTER_TOP,
                    if self.boss_shielded(s.born) {
                        "REACTOR CARRIER / SHIELD UP"
                    } else {
                        "REACTOR CARRIER / CORE PORTS OPEN"
                    },
                    font.clone(),
                    Color32::from_rgb(255, 130, 100),
                );
                p.line_segment(
                    [to(165., 40.), to(165. + 150. * remaining, 40.)],
                    Stroke::new(2. * scale, Color32::from_rgb(240, 90, 85)),
                );
            }
        }
        if self.flash > 0.02 {
            p.rect_filled(
                rect,
                0.,
                Color32::from_rgba_unmultiplied(180, 220, 255, (self.flash * 90.) as u8),
            );
        }
        if !self.started || self.paused {
            p.rect_filled(
                Rect::from_min_max(to(65., 70.), to(415., 216.)),
                8.,
                Color32::from_rgba_unmultiplied(4, 10, 23, 235),
            );
            let title = if self.won {
                "REACTOR DESTROYED"
            } else if self.over {
                "MISSION LOST"
            } else if self.paused {
                "PAUSED"
            } else {
                "VOIDRUNNER"
            };
            p.text(
                to(240., 91.),
                egui::Align2::CENTER_TOP,
                title,
                egui::FontId::proportional(23. * scale),
                Color32::from_rgb(160, 235, 250),
            );
            p.text(
                to(240., 126.),
                egui::Align2::CENTER_TOP,
                if self.over || self.won {
                    format!("FINAL SCORE {:07}  /  {} KILLS", self.score, self.kills)
                } else {
                    "OPERATION CINDER / BREACH THE REACTOR CONVOY".into()
                },
                font.clone(),
                Color32::from_rgb(240, 190, 110),
            );
            p.text(to(240.,148.),egui::Align2::CENTER_TOP,"Chip away armour. Shoot amber fuel ports.\nDetonate the carrier core to complete the mission.",egui::FontId::proportional(8.*scale),Color32::from_rgb(185,200,220));
            p.text(
                to(240., 187.),
                egui::Align2::CENTER_TOP,
                if self.over || self.won {
                    "R / RESTART"
                } else if self.paused {
                    "P / RESUME"
                } else {
                    "ENTER / LAUNCH"
                },
                egui::FontId::proportional(11. * scale),
                Color32::from_rgb(90, 240, 195),
            );
        }
        ctx.request_repaint();
    }
}
fn main() -> eframe::Result {
    std::env::set_current_dir(env!("CARGO_MANIFEST_DIR")).unwrap();
    if std::env::args().any(|s| s == "--verify") {
        let mut g = Game::new();
        g.started = true;
        g.invuln = 100000;
        for _ in 0..14000 {
            if g.won || g.over {
                break;
            }
            let py = g.player().map_or(135., |p| p.2);
            let mut target = g
                .ships
                .iter()
                .filter(|s| !s.dead && s.kind != Kind::Player && s.kind != Kind::Asteroid)
                .filter_map(|s| g.sim.physics.centre(s.id))
                .filter(|(x, _)| *x > 115.)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map_or(135., |p| p.1);
            if let Some(b) = g
                .ships
                .iter()
                .find(|s| s.kind == Kind::Boss && !s.dead)
                .and_then(|s| g.sim.physics.spec_of(&g.sim.world, s.id))
            {
                if let Some(k) = b
                    .cells
                    .iter()
                    .position(|c| c.is_some_and(|c| c.mat == g.mats.id("gunpowder").unwrap()))
                {
                    target = b.y + (k / b.w) as f32 - b.h as f32 / 2.;
                }
            }
            if g.boss_spawned {
                let fuel = g.mats.id("gunpowder").unwrap();
                if let Some(i) = g.sim.world.mat.iter().position(|m| *m == fuel) {
                    target = (i / W) as f32;
                }
            }
            let dy = if (target - py).abs() > 3. {
                (target - py).signum()
            } else {
                0.
            };
            g.tick(0., dy, true, false);
            if g.frame % 600 == 0 {
                println!(
                    "frame={} score={} kills={} bodies={} won={}",
                    g.frame,
                    g.score,
                    g.kills,
                    g.sim.physics.body_count(),
                    g.won
                );
            }
        }
        grave_engine::headless::shot(
            &g.sim.world,
            &g.mats,
            &mut g.renderer,
            std::path::Path::new("shots/voidrunner-verify.png"),
            3,
            false,
        )
        .unwrap();
        println!(
            "END won={} frame={} kills={} score={}",
            g.won, g.frame, g.kills, g.score
        );
        if !g.won {
            std::process::exit(1);
        }
        return Ok(());
    }
    eframe::run_native(
        "Voidrunner — Grave Engine",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size(Vec2::new(1440., 850.)),
            ..Default::default()
        },
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(Game::new()))
        }),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shooting_fuel_uses_engine_explosions() {
        let mut g = Game::new();
        let id = g.spawn(Kind::Tanker, 300., 135.);
        g.sim.step(&g.mats);
        let powder = g.mats.id("gunpowder").unwrap();
        let before = g
            .sim
            .physics
            .spec_of(&g.sim.world, id)
            .unwrap()
            .cells
            .iter()
            .flatten()
            .filter(|c| c.mat == powder)
            .count();
        g.sim.world.heat_circle(300, 135, 15, 1500.);
        let mut blasts = 0;
        for _ in 0..30 {
            g.sim.world.step(&g.mats);
            blasts += g.sim.world.blasts.len();
            g.sim.physics.update(&mut g.sim.world, &g.mats);
        }
        let after = g.sim.physics.spec_of(&g.sim.world, id).map_or(0, |b| {
            b.cells.iter().flatten().filter(|c| c.mat == powder).count()
        });
        println!(
            "fuel before {before} after {after} blasts {blasts} centre {:?}",
            g.sim.physics.centre(id)
        );
        assert!(blasts > 0 && after < before);
    }
    #[test]
    fn projectiles_sweep_through_thin_hulls() {
        let mut g = Game::new();
        let id = g.spawn(Kind::Scout, 220., 135.);
        g.sim.step(&g.mats);
        let count = g
            .sim
            .physics
            .spec_of(&g.sim.world, id)
            .unwrap()
            .cell_count();
        g.shots.push(Shot {
            x: 190.,
            y: 135.,
            vx: 3600.,
            vy: 0.,
            friendly: true,
            power: 2,
            ttl: 10,
        });
        g.tick(0., 0., false, false);
        assert!(
            g.sim
                .physics
                .spec_of(&g.sim.world, id)
                .is_none_or(|b| b.cell_count() < count)
        );
    }
    #[test]
    fn level_has_varied_waves_and_a_final_boss() {
        let g = Game::new();
        assert!(g.waves.len() > 30);
        assert_eq!(g.waves.last().unwrap().kind, Kind::Boss);
        assert!(g.waves.iter().any(|w| w.kind == Kind::Tanker));
        assert!(g.waves.iter().any(|w| w.kind == Kind::Asteroid));
    }
    #[test]
    fn boss_survives_entrance_without_player_fire() {
        let mut g = Game::new();
        let id = g.spawn(Kind::Boss, 420., 135.);
        for _ in 0..180 {
            g.tick(0., 0., false, false);
        }
        let b = g.sim.physics.spec_of(&g.sim.world, id);
        println!(
            "boss survivor {:?}",
            b.as_ref().map(|b| (
                b.cell_count(),
                b.cells
                    .iter()
                    .flatten()
                    .filter(|c| c.mat == g.mats.id("gunpowder").unwrap())
                    .count()
            ))
        );
        assert!(!g.won && b.is_some());
    }
    #[test]
    fn pickups_do_not_rotate_or_change_player_control() {
        let mut g = Game::new();
        let (id, x, y) = g.player().unwrap();
        g.pickups.push(Pickup { x, y, weapon: true });
        for _ in 0..60 {
            g.tick(1., 0., false, false);
        }
        let b = g.sim.physics.spec_of(&g.sim.world, id).unwrap();
        assert_eq!(g.weapon, 2);
        assert!((b.angle).abs() < 0.001);
        assert!(b.x > x + 115.);
        assert!((b.y - y).abs() < 0.5);
        for _ in 0..20 {
            g.tick(-1., 0., true, false);
        }
        let b2 = g.sim.physics.spec_of(&g.sim.world, id).unwrap();
        assert!(b2.x < b.x - 35.);
        assert!(b2.angle.abs() < 0.001);
    }
    #[test]
    fn boss_does_not_win_while_any_reactor_survives() {
        let mut g = Game::new();
        g.boss_spawned = true;
        g.spawn(Kind::Boss, 420., 135.);
        g.tick(0., 0., false, false);
        let fuel = g.mats.id("gunpowder").unwrap();
        let i = g.sim.world.mat.iter().position(|m| *m == fuel).unwrap();
        g.sim
            .world
            .heat_circle((i % W) as i32, (i / W) as i32, 1, 1500.);
        for _ in 0..10 {
            g.tick(0., 0., false, false);
        }
        assert!(g.remaining_reactors() > 0);
        assert!(!g.won);
    }
    #[test]
    fn shields_lives_and_bombs_have_limits() {
        let mut g = Game::new();
        g.invuln = 0;
        g.damage_player(110.);
        assert_eq!(g.lives, 2);
        assert_eq!(g.shield, 100.);
        g.bomb();
        assert_eq!(g.bombs, 2);
        g.bomb();
        assert_eq!(g.bombs, 2);
    }
}
