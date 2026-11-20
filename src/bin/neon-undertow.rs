//! Neon Undertow, a top-down shooter in native Grave Engine material scenes.

use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};
#[path = "neon-undertow/arena.rs"]
mod arena;
use arena::{Arena, HEIGHT, PIXELS, WIDTH};
use grave_engine::{input::Input, render::Renderer, sound};
use std::path::Path;
use std::{collections::VecDeque, time::Instant};

const CELL: f32 = 16.;
const COLS: usize = (WIDTH as f32 * PIXELS / CELL) as usize;
const ROWS: usize = (HEIGHT as f32 * PIXELS / CELL) as usize;
const SIZE: Vec2 = Vec2::new(WIDTH as f32 * PIXELS, HEIGHT as f32 * PIXELS);
const DT: f32 = 1. / 120.;
const INK: Color32 = Color32::from_rgb(8, 12, 22);
const MINT: Color32 = Color32::from_rgb(91, 255, 202);
const CORAL: Color32 = Color32::from_rgb(255, 104, 117);
const GOLD: Color32 = Color32::from_rgb(255, 198, 103);
const DIM: Color32 = Color32::from_rgb(145, 163, 188);

struct Station {
    name: &'static str,
    path: &'static str,
    scene: &'static str,
    hint: &'static str,
}

const MAPS: [Station; 3] = [
    Station {
        name: "TIDAL INTAKE",
        path: "scenes/neon-undertow-intake.txt",
        scene: include_str!("../../scenes/neon-undertow-intake.txt"),
        hint: "Shoot through glass to open new sightlines.",
    },
    Station {
        name: "CRACKING PLANT",
        path: "scenes/neon-undertow-filter.txt",
        scene: include_str!("../../scenes/neon-undertow-filter.txt"),
        hint: "Use the powder charges. Give spilled fuel room to burn.",
    },
    Station {
        name: "SIGNAL CORE",
        path: "scenes/neon-undertow-core.txt",
        scene: include_str!("../../scenes/neon-undertow-core.txt"),
        hint: "Flank the core and use the glass screens as cover.",
    },
];
struct Guard {
    pos: Pos2,
    aim: Vec2,
    gunner: bool,
    heat: f32,
    warning: f32,
    cooldown: f32,
}

struct Bullet {
    pos: Pos2,
    velocity: Vec2,
    friendly: bool,
}

struct Spark {
    pos: Pos2,
    velocity: Vec2,
    life: f32,
    color: Color32,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Title,
    Play,
    Paused,
    Down,
    Clear,
    Won,
}

#[derive(Default, Clone, Copy)]
struct Controls {
    movement: Vec2,
    aim: Vec2,
    fire: bool,
}

#[derive(Clone, Copy)]
enum Cue {
    Shot,
    Threat,
    Impact,
    Glass,
    Down,
    Clear,
}

struct Run {
    room: usize,
    arena: Arena,
    heat: f32,
    guards: Vec<Guard>,
    bullets: Vec<Bullet>,
    sparks: Vec<Spark>,
    cues: Vec<Cue>,
    player: Pos2,
    aim: Vec2,
    exit: Pos2,
    mode: Mode,
    reload: f32,
    seconds: f32,
    advance_materials: bool,
    carry: f32,
    attempts: u32,
    shake: f32,
}

fn center(x: usize, y: usize) -> Pos2 {
    Pos2::new((x as f32 + 0.5) * CELL, (y as f32 + 0.5) * CELL)
}

fn index(pos: Pos2) -> (usize, usize) {
    (
        (pos.x / CELL).floor() as usize,
        (pos.y / CELL).floor() as usize,
    )
}

fn slide(arena: &Arena, mut pos: Pos2, delta: Vec2) -> Pos2 {
    let horizontal = pos + Vec2::new(delta.x, 0.);
    if arena.clear(horizontal) {
        pos = horizontal;
    }
    let vertical = pos + Vec2::new(0., delta.y);
    if arena.clear(vertical) {
        pos = vertical;
    }
    pos
}

fn visible(arena: &Arena, from: Pos2, to: Pos2) -> bool {
    let delta = to - from;
    let steps = (delta.length() / 2.).ceil() as usize;
    let edge = delta.normalized().rot90() * 1.5;
    (1..steps).all(|i| {
        let p = from.lerp(to, i as f32 / steps as f32);
        [p, p + edge, p - edge]
            .into_iter()
            .all(|sample| !arena.wall(sample))
    })
}
/// Distance to the player through walkable tiles, shared by every guard.
fn routes(arena: &Arena, player: Pos2) -> [[u16; COLS]; ROWS] {
    let mut distance = [[u16::MAX; COLS]; ROWS];
    let (x, y) = index(player);
    let mut queue = VecDeque::from([(x, y)]);
    distance[y][x] = 0;
    while let Some((x, y)) = queue.pop_front() {
        for (nx, ny) in neighbors(x, y) {
            if distance[ny][nx] == u16::MAX
                && !arena.heat(center(nx, ny))
                && arena.clear(center(nx, ny))
                && arena.clear(center(x, y).lerp(center(nx, ny), 0.5))
            {
                distance[ny][nx] = distance[y][x] + 1;
                queue.push_back((nx, ny));
            }
        }
    }
    distance
}

fn neighbors(x: usize, y: usize) -> impl Iterator<Item = (usize, usize)> {
    [
        (x.wrapping_sub(1), y),
        (x + 1, y),
        (x, y.wrapping_sub(1)),
        (x, y + 1),
    ]
    .into_iter()
    .filter(|&(x, y)| x < COLS && y < ROWS)
}

impl Run {
    fn load(room: usize, carry: f32, attempts: u32) -> Self {
        let station = &MAPS[room];
        let (arena, markers) = Arena::load(Path::new(station.path), station.scene);
        let mut run = Self {
            room,
            arena,
            heat: 0.,
            guards: Vec::new(),
            bullets: Vec::new(),
            sparks: Vec::new(),
            cues: Vec::new(),
            player: Pos2::ZERO,
            aim: Vec2::X,
            exit: Pos2::ZERO,
            mode: Mode::Play,
            reload: 0.,
            seconds: 0.,
            advance_materials: false,
            carry,
            attempts,
            shake: 0.,
        };
        for (marker, pos) in markers {
            match marker.as_str() {
                "spawn" => run.player = pos,
                "exit" => run.exit = pos,
                "gunner" | "runner" => run.guards.push(Guard {
                    pos,
                    aim: -Vec2::X,
                    gunner: marker == "gunner",
                    heat: 0.,
                    warning: 0.,
                    cooldown: 0.36 + (pos.x as usize % 7) as f32 * 0.04,
                }),
                _ => {}
            }
        }
        run
    }

    fn restart(&mut self) {
        *self = match self.mode {
            Mode::Title => Self::load(self.room, 0., 1),
            Mode::Won => Self::load(0, 0., 1),
            _ => Self::load(self.room, self.carry, self.attempts + 1),
        };
    }

    fn step(&mut self, controls: Controls) {
        self.shake = (self.shake - DT * 52.).max(0.);
        self.sparks.retain_mut(|spark| {
            spark.pos += spark.velocity * DT;
            spark.velocity *= 0.96;
            spark.life -= DT;
            spark.life > 0.
        });
        if self.mode != Mode::Play {
            return;
        }
        self.seconds += DT;
        let slowdown = if self.arena.wet(self.player) {
            0.67
        } else {
            1.
        };
        self.player = slide(
            &self.arena,
            self.player,
            controls.movement * (218. * DT * slowdown),
        );
        if controls.aim.length_sq() > 0.001 {
            self.aim = controls.aim.normalized();
        }
        self.reload -= DT;
        if controls.fire && self.reload <= 0. {
            self.reload = 0.16;
            self.bullets.push(Bullet {
                pos: self.player,
                velocity: self.aim * 950.,
                friendly: true,
            });
            self.burst(self.player + self.aim * 12., GOLD, 5);
            self.cues.push(Cue::Shot);
        }

        let distance = routes(&self.arena, self.player);
        for guard in &mut self.guards {
            let delta = self.player - guard.pos;
            let sight = visible(&self.arena, guard.pos, self.player);
            guard.cooldown -= DT;
            if guard.warning > 0. {
                guard.warning -= DT;
                if guard.warning <= 0. {
                    self.bullets.push(Bullet {
                        pos: guard.pos,
                        velocity: guard.aim * 414.,
                        friendly: false,
                    });
                    guard.cooldown = 0.85;
                    self.cues.push(Cue::Threat);
                }
                continue;
            }
            if sight && guard.gunner && delta.length() < 370. && guard.cooldown <= 0. {
                guard.aim = delta.normalized();
                guard.warning = 0.30;
                continue;
            }
            if sight && guard.gunner && delta.length() < 230. {
                guard.aim = delta.normalized();
                continue;
            }
            let (x, y) = index(guard.pos);
            let goal = if sight {
                self.player
            } else {
                let (nx, ny) = neighbors(x, y)
                    .into_iter()
                    .min_by_key(|&(nx, ny)| distance[ny][nx])
                    .unwrap();
                if distance[ny][nx] >= distance[y][x] {
                    continue;
                }
                center(nx, ny)
            };
            guard.aim = (goal - guard.pos).normalized();
            let speed = if guard.gunner { 79. } else { 137. };
            guard.pos = slide(&self.arena, guard.pos, guard.aim * (speed * DT));
        }

        self.step_bullets();
        self.advance_materials = !self.advance_materials;
        if self.advance_materials {
            let before = self.arena.sim.world.sounds.len();
            self.arena.sim.step(&self.arena.materials);
            let blasts: Vec<_> = self.arena.sim.world.sounds[before..]
                .iter()
                .filter(|cue| cue.kind == sound::Kind::Blast)
                .map(|cue| Pos2::new(cue.x, cue.y) * PIXELS)
                .collect();
            for blast in blasts {
                self.burst(blast, GOLD, 27);
                self.shake = 8.;
            }
            self.hazards();
        }
        if self.mode == Mode::Play
            && self
                .guards
                .iter()
                .any(|g| g.pos.distance(self.player) < 17.)
        {
            self.down();
        }
        if self.mode == Mode::Play
            && self.guards.is_empty()
            && self.player.distance(self.exit) < 20.
        {
            self.cues.push(Cue::Clear);
            self.mode = if self.room + 1 == MAPS.len() {
                Mode::Won
            } else {
                Mode::Clear
            };
        }
    }

    fn step_bullets(&mut self) {
        let mut bullets = std::mem::take(&mut self.bullets);
        bullets.retain_mut(|bullet| {
            let delta = bullet.velocity * DT / 4.;
            for _ in 0..4 {
                bullet.pos += delta;
                let brittle = self.arena.material(bullet.pos).is_some_and(|material| {
                    material.toughness.is_some_and(|toughness| toughness < 250.)
                });
                if self.arena.strike(bullet.pos, bullet.velocity) {
                    self.burst(
                        bullet.pos,
                        if brittle { MINT } else { DIM },
                        if brittle { 10 } else { 4 },
                    );
                    if brittle {
                        self.cues.push(Cue::Glass);
                    }
                    return false;
                }
                if bullet.friendly {
                    if let Some(index) = self
                        .guards
                        .iter()
                        .position(|g| g.pos.distance(bullet.pos) < 11.)
                    {
                        self.guards.swap_remove(index);
                        self.burst(bullet.pos, CORAL, 15);
                        self.cues.push(Cue::Impact);
                        self.shake = 2.8;
                        return false;
                    }
                } else if bullet.pos.distance(self.player) < 9. {
                    self.down();
                    return false;
                }
            }
            true
        });
        self.bullets = bullets;
    }

    fn hazards(&mut self) {
        let exposure = |heat: &mut f32, position| {
            if self.arena.heat(position) {
                *heat += DT * 2.;
            } else {
                *heat = (*heat - DT * 4.).max(0.);
            }
            *heat >= 0.18
        };
        let burning = exposure(&mut self.heat, self.player);
        self.guards
            .retain_mut(|guard| !exposure(&mut guard.heat, guard.pos));
        if burning {
            self.down();
        }
    }

    fn down(&mut self) {
        if self.mode == Mode::Play {
            self.mode = Mode::Down;
            self.shake = 6.;
            self.burst(self.player, MINT, 22);
            self.cues.push(Cue::Down);
        }
    }

    fn burst(&mut self, pos: Pos2, color: Color32, count: usize) {
        for i in 0..count {
            let angle = i as f32 * 2.4 + self.seconds * 8.;
            self.sparks.push(Spark {
                pos,
                velocity: Vec2::angled(angle) * (32. + (i % 5) as f32 * 32.),
                life: 0.13 + (i % 4) as f32 * 0.055,
                color,
            });
        }
    }

    fn advance(&mut self) {
        *self = Self::load(self.room + 1, self.carry + self.seconds, 1);
    }
}

struct View<'a> {
    painter: &'a Painter,
    origin: Pos2,
    scale: f32,
}

impl View<'_> {
    fn point(&self, p: Pos2) -> Pos2 {
        self.origin + p.to_vec2() * self.scale
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.point(Pos2::new(x, y)), Vec2::new(w, h) * self.scale)
    }

    fn line(&self, a: Pos2, b: Pos2, width: f32, color: Color32) {
        self.painter.line_segment(
            [self.point(a), self.point(b)],
            Stroke::new(width * self.scale, color),
        );
    }

    fn disc(&self, p: Pos2, radius: f32, color: Color32) {
        self.painter
            .circle_filled(self.point(p), radius * self.scale, color);
    }

    fn text(&self, x: f32, y: f32, anchor: Align2, text: &str, size: f32, color: Color32) {
        self.painter.text(
            self.point(Pos2::new(x, y)),
            anchor,
            text,
            FontId::monospace(size * self.scale),
            color,
        );
    }

    fn actor(&self, pos: Pos2, aim: Vec2, color: Color32, armed: bool) {
        let side = aim.rot90();
        self.disc(pos + Vec2::new(3., 5.), 12., INK.gamma_multiply(0.7));
        self.disc(pos, 14., color.gamma_multiply(0.11));
        self.line(
            pos - side * 7.,
            pos + side * 7.,
            9.,
            color.gamma_multiply(0.75),
        );
        self.disc(pos, 5., INK);
        self.disc(pos + aim * 2., 4., color);
        if armed {
            self.line(
                pos + aim * 5. + side * 4.,
                pos + aim * 15. + side * 4.,
                3.,
                color,
            );
        }
    }
}

fn draw_board(view: &View<'_>, run: &Run) {
    let p = view.painter;
    let unlocked = run.guards.is_empty();
    let exit_color = if unlocked { MINT } else { CORAL };
    let exit = view.rect(run.exit.x - 13., run.exit.y - 13., 26., 26.);
    p.rect_filled(
        exit.expand(4. * view.scale),
        3.,
        exit_color.gamma_multiply(0.15),
    );
    p.rect_stroke(
        exit,
        2.,
        Stroke::new(1.5 * view.scale, exit_color),
        StrokeKind::Inside,
    );
    view.text(
        run.exit.x,
        run.exit.y,
        Align2::CENTER_CENTER,
        if unlocked { ">" } else { "×" },
        17.,
        exit_color,
    );
    for guard in &run.guards {
        if guard.warning > 0. {
            let length = (1..=110)
                .map(|step| step as f32 * 4.)
                .find(|length| run.arena.wall(guard.pos + guard.aim * *length))
                .unwrap_or(440.);
            for step in (15..length as usize).step_by(12) {
                let a = guard.pos + guard.aim * step as f32;
                let b = guard.pos + guard.aim * (step as f32 + 6.).min(length);
                view.line(a, b, 1.5, CORAL);
            }
            view.disc(guard.pos, 18., CORAL.gamma_multiply(0.18));
        }
        view.actor(
            guard.pos,
            guard.aim,
            if guard.gunner { CORAL } else { GOLD },
            guard.gunner,
        );
    }
    for bullet in &run.bullets {
        let color = if bullet.friendly { GOLD } else { CORAL };
        let tail = bullet.pos - bullet.velocity.normalized() * 12.;
        view.line(tail, bullet.pos, 7., color.gamma_multiply(0.15));
        view.line(tail, bullet.pos, 2., color);
        view.disc(bullet.pos, 2., Color32::WHITE);
    }
    for spark in &run.sparks {
        view.line(
            spark.pos,
            spark.pos - spark.velocity * 0.024,
            2.,
            spark.color.gamma_multiply((spark.life * 6.).min(1.)),
        );
    }
    if run.mode != Mode::Down {
        if run.heat > 0. {
            view.disc(run.player, 20., CORAL.gamma_multiply(0.35 + run.heat));
        }
        view.actor(run.player, run.aim, MINT, true);
        let reticle = run.player + run.aim * 30.;
        p.circle_stroke(
            view.point(reticle),
            4. * view.scale,
            Stroke::new(view.scale, MINT),
        );
    }
}

struct Game {
    input: Input,
    run: Run,
    last: Instant,
    accumulator: f32,
    fire_tapped: bool,
    stick_aim: bool,
    audio: Option<rodio::OutputStream>,
    material_audio: sound::Player,
    renderer: Renderer,
    background: Option<egui::TextureHandle>,
    shot: bool,
    shot_requested: bool,
}

impl Game {
    fn new(room: usize, shot: bool) -> Self {
        let mut run = Run::load(room, 0., 1);
        if !shot {
            run.mode = Mode::Title;
        }
        Self {
            input: Input::load_or(
                "assets/input/neon-undertow.input",
                include_str!("../../assets/input/neon-undertow.input"),
            )
            .unwrap_or_else(|error| {
                eprintln!("{error}. Using default Neon Undertow controls.");
                Input::load_or(
                    std::path::Path::new(""),
                    include_str!("../../assets/input/neon-undertow.input"),
                )
                .expect("bundled controls")
            }),
            run,
            last: Instant::now(),
            accumulator: 0.,
            fire_tapped: false,
            stick_aim: false,
            audio: if shot || std::env::var_os("GRAVE_NO_SOUND").is_some() {
                None
            } else {
                rodio::OutputStreamBuilder::open_default_stream().ok()
            },
            shot,
            material_audio: if shot {
                sound::Player::silent()
            } else {
                sound::Player::new()
            },
            renderer: Renderer::default(),
            background: None,
            shot_requested: false,
        }
    }

    fn sound(&mut self) {
        for cue in self.run.cues.drain(..) {
            let Some(audio) = &self.audio else { continue };
            let (pitch, end, duration, grit) = match cue {
                Cue::Shot => (190., 55., 0.09, 0.58),
                Cue::Threat => (320., 82., 0.10, 0.43),
                Cue::Impact => (105., 38., 0.14, 0.35),
                Cue::Glass => (1850., 820., 0.21, 0.72),
                Cue::Down => (150., 30., 0.38, 0.28),
                Cue::Clear => (430., 790., 0.42, 0.00),
            };
            let rate = 22050;
            let count = (duration * rate as f32) as usize;
            let mut phase = 0.;
            let mut noise = 72461u32;
            let samples: Vec<f32> = (0..count)
                .map(|i| {
                    let t = i as f32 / count as f32;
                    let envelope = (1. - t).powi(3) * (i as f32 / 18.).min(1.);
                    phase += std::f32::consts::TAU * (pitch + (end - pitch) * t) / rate as f32;
                    noise = noise.wrapping_mul(1664525).wrapping_add(1013904223);
                    let white = (noise >> 16) as f32 / 32768. - 1.;
                    envelope * 0.20 * ((1. - grit) * phase.sin() + grit * white)
                })
                .collect();
            audio
                .mixer()
                .add(rodio::buffer::SamplesBuffer::new(1, rate, samples));
        }
    }

    fn controls(&mut self, ctx: &egui::Context, origin: Pos2, scale: f32) -> Controls {
        let stick = Vec2::new(self.input.axis("aim_x"), self.input.axis("aim_y"));
        let mouse = ctx
            .pointer_hover_pos()
            .filter(|p| Rect::from_min_size(origin, SIZE * scale).contains(*p));
        if stick != Vec2::ZERO {
            self.stick_aim = true;
        } else if self.input.pressed("fire") && self.input.controller().is_none()
            || ctx.input(|i| i.pointer.delta() != Vec2::ZERO)
        {
            self.stick_aim = false;
        }
        let aim = if self.stick_aim {
            stick
        } else if let Some(mouse) = mouse {
            Pos2::ZERO + (mouse - origin) / scale - self.run.player
        } else {
            Vec2::ZERO
        };
        let movement = Vec2::new(self.input.axis("move_x"), self.input.axis("move_y"));
        Controls {
            movement: movement / movement.length().max(1.),
            aim,
            fire: stick.length_sq() > 0.12 || self.input.down("fire") || self.fire_tapped,
        }
    }

    fn draw(&mut self, painter: &Painter, origin: Pos2, scale: f32) {
        let board = Rect::from_min_size(origin, SIZE * scale);
        let clipped = painter.with_clip_rect(board);
        let shake = Vec2::angled(self.run.seconds * 91. + self.run.shake) * self.run.shake * scale;
        let sim = &self.run.arena.sim;
        self.renderer.light(sim);
        let rgba = self
            .renderer
            .draw(&sim.world, &self.run.arena.materials, Default::default());
        let image = egui::ColorImage::from_rgba_unmultiplied([WIDTH, HEIGHT], rgba);
        let texture = match &mut self.background {
            Some(texture) => {
                texture.set(image, egui::TextureOptions::NEAREST);
                texture
            }
            None => self.background.insert(painter.ctx().load_texture(
                "undertow station",
                image,
                egui::TextureOptions::NEAREST,
            )),
        };
        clipped.image(
            texture.id(),
            board.translate(shake),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
            Color32::WHITE,
        );
        draw_board(
            &View {
                painter: &clipped,
                origin: origin + shake,
                scale,
            },
            &self.run,
        );
        let view = View {
            painter,
            origin,
            scale,
        };
        let gamepad = self.input.controller().is_some();
        let confirm = if gamepad { "A" } else { "ENTER" };
        let retry = if gamepad { "Y" } else { "R" };
        let pause = if gamepad { "START" } else { "ESC" };
        let controls = if gamepad {
            "LEFT STICK  MOVE     RIGHT STICK  AIM + FIRE"
        } else {
            "WASD  MOVE     MOUSE  AIM     LEFT CLICK  FIRE"
        };
        view.text(0., -44., Align2::LEFT_CENTER, "NEON UNDERTOW", 25., MINT);
        view.text(
            SIZE.x,
            -44.,
            Align2::RIGHT_CENTER,
            &format!(
                "{:02} / {:02}   {}",
                self.run.room + 1,
                MAPS.len(),
                MAPS[self.run.room].name
            ),
            18.,
            Color32::WHITE,
        );
        let objective = if self.run.guards.is_empty() {
            "EXIT OPEN".to_owned()
        } else {
            format!("{} HOSTILES", self.run.guards.len())
        };
        view.text(
            0.,
            -17.,
            Align2::LEFT_CENTER,
            &objective,
            13.,
            if self.run.guards.is_empty() {
                MINT
            } else {
                CORAL
            },
        );
        view.text(
            SIZE.x,
            -17.,
            Align2::RIGHT_CENTER,
            &format!(
                "{:05.1}s    ATTEMPT {}",
                self.run.carry + self.run.seconds,
                self.run.attempts
            ),
            13.,
            DIM,
        );
        view.text(0., SIZE.y + 24., Align2::LEFT_CENTER, controls, 12., DIM);
        view.text(
            SIZE.x,
            SIZE.y + 24.,
            Align2::RIGHT_CENTER,
            &format!("{retry}  RESTART    {pause}  PAUSE"),
            12.,
            DIM,
        );

        view.text(
            0.,
            SIZE.y + 43.,
            Align2::LEFT_CENTER,
            MAPS[self.run.room].hint,
            12.,
            GOLD,
        );
        let (title, detail, prompt, color) = match self.run.mode {
            Mode::Title => (
                "NEON UNDERTOW",
                "Clear three relay stations and reach the green hatch.",
                format!("{confirm}  BEGIN"),
                MINT,
            ),
            Mode::Paused => ("PAUSED", "", format!("{pause}  RESUME"), MINT),
            Mode::Down => (
                "SIGNAL LOST",
                "Player and hostiles go down in one hit.",
                format!("{retry} / {confirm}  RETRY ROOM"),
                CORAL,
            ),
            Mode::Clear => (
                "RELAY SECURED",
                MAPS[self.run.room + 1].name,
                format!("{confirm}  NEXT STATION"),
                MINT,
            ),
            Mode::Won => (
                "SIGNAL RESTORED",
                "All three relay stations secured.",
                format!("{confirm}  NEW RUN"),
                MINT,
            ),
            Mode::Play => return,
        };
        painter.rect_filled(board, 0., Color32::from_black_alpha(152));
        let mid = SIZE.y / 2.;
        painter.rect_filled(view.rect(86., mid - 76., SIZE.x - 172., 152.), 6., INK);
        painter.rect_stroke(
            view.rect(86., mid - 76., SIZE.x - 172., 152.),
            6.,
            Stroke::new(scale, color.gamma_multiply(0.6)),
            StrokeKind::Inside,
        );
        view.text(
            SIZE.x / 2.,
            mid - 33.,
            Align2::CENTER_CENTER,
            title,
            34.,
            color,
        );
        view.text(
            SIZE.x / 2.,
            mid + 9.,
            Align2::CENTER_CENTER,
            detail,
            15.,
            DIM,
        );
        view.text(
            SIZE.x / 2.,
            mid + 47.,
            Align2::CENTER_CENTER,
            &prompt,
            14.,
            Color32::WHITE,
        );
    }
}

impl eframe::App for Game {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.input.update(&ctx);
        let elapsed = self.last.elapsed().as_secs_f32().min(0.05);
        self.last = Instant::now();
        if !self.shot {
            if self.input.pressed("restart") {
                self.run.restart();
            } else if self.input.pressed("start") {
                match self.run.mode {
                    Mode::Title | Mode::Down | Mode::Won => self.run.restart(),
                    Mode::Clear => self.run.advance(),
                    _ => {}
                }
            } else if self.input.pressed("pause") {
                self.run.mode = match self.run.mode {
                    Mode::Play => Mode::Paused,
                    Mode::Paused => Mode::Play,
                    other => other,
                };
            }
            if self.run.mode == Mode::Play && !ctx.input(|input| input.focused) {
                self.run.mode = Mode::Paused;
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(INK))
            .show(ui, |ui| {
                let area = ui.max_rect();
                let scale = ((area.width() - 30.) / SIZE.x)
                    .min((area.height() - 20.) / (SIZE.y + 134.))
                    .max(0.15);
                let origin = area.center() - Vec2::new(SIZE.x / 2., SIZE.y / 2. - 4.) * scale;
                if !self.shot && self.run.mode != Mode::Paused {
                    self.fire_tapped |= self.run.mode == Mode::Play && self.input.pressed("fire");
                    self.accumulator += elapsed;
                    while self.accumulator >= DT {
                        let controls = self.controls(&ctx, origin, scale);
                        self.run.step(controls);
                        self.fire_tapped = false;
                        self.accumulator -= DT;
                    }
                }
                self.draw(ui.painter(), origin, scale);
            });
        self.sound();
        self.material_audio.listen(
            &mut self.run.arena.sim,
            &self.run.arena.materials,
            (0., 0., WIDTH as f32, HEIGHT as f32),
            if self.run.mode == Mode::Play { 1. } else { 0. },
        );

        if self.shot && !self.shot_requested {
            self.shot_requested = true;
            let ctx = ctx.clone();
            let path = format!("shots/neon-undertow-{}.png", self.run.room + 1);
            ui.ctx().request_screenshot(move |image| {
                std::fs::create_dir_all("shots").expect("screenshot directory");
                image::save_buffer(
                    path,
                    image.as_raw(),
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                )
                .expect("write screenshot");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            });
        }
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result {
    std::env::set_current_dir(env!("CARGO_MANIFEST_DIR")).expect("project directory");
    let args: Vec<_> = std::env::args().skip(1).collect();
    let shot = args.iter().any(|arg| arg == "--shot");
    let room = args
        .iter()
        .find_map(|arg| {
            let path = arg.strip_prefix("--station=")?;
            let filename = Path::new(path).file_name()?;
            MAPS.iter()
                .position(|station| Path::new(station.path).file_name() == Some(filename))
        })
        .unwrap_or(0);
    eframe::run_native(
        "Neon Undertow",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1100., 760.])
                .with_min_inner_size([700., 500.]),
            ..Default::default()
        },
        Box::new(move |_| Ok(Box::new(Game::new(room, shot)))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stations_load_in_grave_and_have_routes_to_all_objectives() {
        for (room, station) in MAPS.iter().enumerate() {
            let materials = grave_engine::Materials::load_dir("materials");
            let (_, problems) = grave_engine::scene::load(
                station.scene,
                &materials,
                Path::new(station.path).parent(),
            )
            .into_sim(&materials);
            assert!(problems.is_empty(), "{}: {problems:?}", station.name);
            let run = Run::load(room, 0., 1);
            let distance = routes(&run.arena, run.player);
            for pos in run.guards.iter().map(|g| g.pos).chain([run.exit]) {
                let (x, y) = index(pos);
                assert_ne!(
                    distance[y][x],
                    u16::MAX,
                    "{} cannot reach {pos:?}",
                    station.name
                );
            }
        }
    }

    #[test]
    fn shots_fracture_native_glass_but_not_concrete() {
        let mut run = Run::load(0, 0., 1);
        let glass = Pos2::new(58.5, 45.5) * PIXELS;
        let concrete = Pos2::new(56.5, 63.5) * PIXELS;
        assert!(run.arena.wall(glass) && run.arena.wall(concrete));
        assert!(run.arena.strike(glass, Vec2::X));
        for _ in 0..12 {
            run.arena.sim.step(&run.arena.materials);
        }
        assert!(
            !run.arena.wall(glass),
            "glass survived as {:?}, body count {}",
            run.arena.material(glass).map(|m| &m.name),
            run.arena.sim.physics.body_count()
        );
        assert!(run.arena.strike(concrete, Vec2::X));
        assert!(run.arena.wall(concrete));
        run.mode = Mode::Down;
        run.restart();
        assert!(run.arena.wall(glass));
        assert_eq!(run.attempts, 2);
    }

    #[test]
    fn powder_ignites_and_engine_heat_can_kill_a_guard() {
        let mut run = Run::load(1, 0., 1);
        let charge = Pos2::new(52.5, 41.5) * PIXELS;
        assert!(run.arena.material(charge).unwrap().explosion_size > 0.);
        assert!(run.arena.strike(charge, Vec2::X));
        run.arena.sim.step(&run.arena.materials);
        assert!(
            run.arena
                .sim
                .world
                .sounds
                .iter()
                .any(|cue| cue.kind == sound::Kind::Blast)
        );
        let guard = run.guards[0].pos / PIXELS;
        let count = run.guards.len();
        run.arena
            .sim
            .world
            .heat_circle(guard.x as i32, guard.y as i32, 3, 900.);
        for _ in 0..12 {
            run.hazards();
        }
        assert_eq!(run.guards.len(), count - 1);
    }

    #[test]
    fn fuel_ignites_in_the_tank_and_a_charge_breaks_adjacent_cover() {
        let mut run = Run::load(1, 0., 1);
        let fuel = Pos2::new(90.5, 65.5) * PIXELS;
        assert!(run.arena.material(fuel).unwrap().catches_fire_at.is_some());
        assert!(run.arena.strike(fuel, Vec2::X));
        let glass = Pos2::new(59.5, 40.5) * PIXELS;
        assert!(run.arena.wall(glass));
        assert!(run.arena.strike(Pos2::new(52.5, 41.5) * PIXELS, Vec2::X));
        for _ in 0..6 {
            run.arena.sim.step(&run.arena.materials);
        }
        assert!(
            run.arena
                .sim
                .world
                .flags
                .iter()
                .any(|flag| flag & grave_engine::world::BURNING != 0)
        );
        assert!(!run.arena.wall(glass));
    }
    #[test]
    fn a_scripted_run_clears_all_three_stations() {
        let mut run = Run::load(0, 0., 1);
        for room in 0..MAPS.len() {
            let mut waypoint: Option<Pos2> = None;
            for _ in 0..(120 * 35) {
                let target = run
                    .guards
                    .iter()
                    .filter(|g| visible(&run.arena, run.player, g.pos))
                    .min_by(|a, b| {
                        a.pos
                            .distance_sq(run.player)
                            .total_cmp(&b.pos.distance_sq(run.player))
                    });
                let controls = if let Some(guard) = target {
                    waypoint = None;
                    Controls {
                        aim: guard.pos - run.player,
                        fire: true,
                        ..Controls::default()
                    }
                } else {
                    let destination = run.guards.first().map_or(run.exit, |g| g.pos);
                    let next = waypoint
                        .filter(|p| p.distance(run.player) > 1.4 && !run.arena.heat(*p))
                        .unwrap_or_else(|| {
                            let (x, y) = index(run.player);
                            if index(destination) == (x, y) {
                                return destination;
                            }
                            let distance = routes(&run.arena, destination);
                            neighbors(x, y)
                                .filter(|&(nx, ny)| {
                                    !run.arena.heat(center(nx, ny))
                                        && run.arena.clear(center(nx, ny))
                                        && run.arena.clear(center(x, y).lerp(center(nx, ny), 0.5))
                                })
                                .min_by_key(|&(nx, ny)| distance[ny][nx])
                                .map_or(center(x, y), |(nx, ny)| center(nx, ny))
                        });
                    waypoint = Some(next);
                    let delta = next - run.player;
                    Controls {
                        movement: delta / delta.length().max(218. * DT),
                        ..Controls::default()
                    }
                };
                run.step(controls);
                if run.mode != Mode::Play {
                    break;
                }
            }
            let expected = if room + 1 == MAPS.len() {
                Mode::Won
            } else {
                Mode::Clear
            };
            assert!(
                run.mode == expected,
                "{} ended after {:.2}s with {} hostiles and player at {:?}, waypoint {waypoint:?}, heat {}, guards {:?}",
                MAPS[room].name,
                run.seconds,
                run.guards.len(),
                run.player,
                run.heat,
                run.guards.iter().map(|g| g.pos).collect::<Vec<_>>()
            );
            if run.mode == Mode::Clear {
                run.advance();
            }
        }
    }
}
