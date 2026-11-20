//! Neon Undertow, a top-down relay-station shooter using Grave Engine's remappable input.

use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};
use grave_engine::input::Input;
use std::{collections::VecDeque, time::Instant};

const CELL: f32 = 32.;
const COLS: usize = 24;
const ROWS: usize = 14;
const SIZE: Vec2 = Vec2::new(COLS as f32 * CELL, ROWS as f32 * CELL);
const DT: f32 = 1. / 120.;
const INK: Color32 = Color32::from_rgb(8, 12, 22);
const MINT: Color32 = Color32::from_rgb(91, 255, 202);
const CORAL: Color32 = Color32::from_rgb(255, 104, 117);
const GOLD: Color32 = Color32::from_rgb(255, 198, 103);
const DIM: Color32 = Color32::from_rgb(145, 163, 188);

const MAPS: [(&str, &str); 3] = [
    (
        "INTAKE",
        include_str!("../../assets/neon-undertow/intake.txt"),
    ),
    (
        "FILTER HALL",
        include_str!("../../assets/neon-undertow/filter.txt"),
    ),
    (
        "SIGNAL CORE",
        include_str!("../../assets/neon-undertow/core.txt"),
    ),
];

#[derive(Clone, Copy, PartialEq)]
enum Tile {
    Floor,
    Wall,
    Glass,
}

struct Guard {
    pos: Pos2,
    aim: Vec2,
    gunner: bool,
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
    tiles: [[Tile; COLS]; ROWS],
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

fn tile(tiles: &[[Tile; COLS]; ROWS], pos: Pos2) -> Tile {
    if !Rect::from_min_size(Pos2::ZERO, SIZE).contains(pos) {
        return Tile::Wall;
    }
    let (x, y) = index(pos);
    tiles
        .get(y)
        .and_then(|row| row.get(x))
        .copied()
        .unwrap_or(Tile::Wall)
}

fn slide(tiles: &[[Tile; COLS]; ROWS], mut pos: Pos2, delta: Vec2) -> Pos2 {
    let clear = |p: Pos2| {
        [[-9., -9.], [9., -9.], [-9., 9.], [9., 9.]]
            .into_iter()
            .all(|[x, y]| tile(tiles, p + Vec2::new(x, y)) == Tile::Floor)
    };
    let horizontal = pos + Vec2::new(delta.x, 0.);
    if clear(horizontal) {
        pos = horizontal;
    }
    let vertical = pos + Vec2::new(0., delta.y);
    if clear(vertical) {
        pos = vertical;
    }
    pos
}

fn visible(tiles: &[[Tile; COLS]; ROWS], from: Pos2, to: Pos2) -> bool {
    let delta = to - from;
    let steps = (delta.length() / 2.).ceil() as usize;
    let edge = delta.normalized().rot90() * 1.5;
    (1..steps).all(|i| {
        let p = from.lerp(to, i as f32 / steps as f32);
        [p, p + edge, p - edge]
            .into_iter()
            .all(|sample| tile(tiles, sample) == Tile::Floor)
    })
}
/// Distance to the player through walkable tiles, shared by every guard.
fn routes(tiles: &[[Tile; COLS]; ROWS], player: Pos2) -> [[u16; COLS]; ROWS] {
    let mut distance = [[u16::MAX; COLS]; ROWS];
    let (x, y) = index(player);
    let mut queue = VecDeque::from([(x, y)]);
    distance[y][x] = 0;
    while let Some((x, y)) = queue.pop_front() {
        for (nx, ny) in neighbors(x, y) {
            if tiles[ny][nx] == Tile::Floor && distance[ny][nx] == u16::MAX {
                distance[ny][nx] = distance[y][x] + 1;
                queue.push_back((nx, ny));
            }
        }
    }
    distance
}

fn neighbors(x: usize, y: usize) -> [(usize, usize); 4] {
    [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
}

impl Run {
    fn load(room: usize, carry: f32, attempts: u32) -> Self {
        let mut run = Self {
            room,
            tiles: [[Tile::Floor; COLS]; ROWS],
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
            carry,
            attempts,
            shake: 0.,
        };
        for (y, line) in MAPS[room].1.lines().enumerate() {
            for (x, mark) in line.bytes().enumerate() {
                let pos = center(x, y);
                match mark {
                    b'#' => run.tiles[y][x] = Tile::Wall,
                    b'+' => run.tiles[y][x] = Tile::Glass,
                    b'@' => run.player = pos,
                    b'E' => run.exit = pos,
                    b'g' | b'r' => run.guards.push(Guard {
                        pos,
                        aim: -Vec2::X,
                        gunner: mark == b'g',
                        warning: 0.,
                        cooldown: 0.28 + (x % 4) as f32 * 0.13,
                    }),
                    _ => {}
                }
            }
        }
        run
    }

    fn restart(&mut self) {
        *self = match self.mode {
            Mode::Title | Mode::Won => Self::load(0, 0., 1),
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
        self.player = slide(&self.tiles, self.player, controls.movement * (218. * DT));
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

        let distance = routes(&self.tiles, self.player);
        for guard in &mut self.guards {
            let delta = self.player - guard.pos;
            let sight = visible(&self.tiles, guard.pos, self.player);
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
            guard.pos = slide(&self.tiles, guard.pos, guard.aim * (speed * DT));
        }

        self.step_bullets();
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
                match tile(&self.tiles, bullet.pos) {
                    Tile::Wall => {
                        self.burst(bullet.pos, DIM, 4);
                        return false;
                    }
                    Tile::Glass => {
                        let (x, y) = index(bullet.pos);
                        self.tiles[y][x] = Tile::Floor;
                        self.burst(bullet.pos, MINT, 16);
                        self.cues.push(Cue::Glass);
                        return false;
                    }
                    Tile::Floor => {}
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
    let wall_top = Color32::from_rgb(38, 58, 76);
    for (y, row) in run.tiles.iter().enumerate() {
        for (x, t) in row.iter().enumerate() {
            let left = x as f32 * CELL;
            let top = y as f32 * CELL;
            let base = match (x * 17 + y * 7 + x * y) % 5 {
                0 => Color32::from_rgb(21, 39, 55),
                1 => Color32::from_rgb(17, 32, 48),
                _ => Color32::from_rgb(19, 35, 51),
            };
            p.rect_filled(view.rect(left, top, CELL, CELL), 0., INK);
            p.rect_filled(
                view.rect(left + 0.5, top + 0.5, CELL - 1., CELL - 1.),
                0.,
                base,
            );
            let drain = (x * 11 + y * 9) % 29 == 3;
            if *t == Tile::Wall {
                p.rect_filled(
                    view.rect(left, top, CELL, CELL),
                    0.,
                    Color32::from_rgb(15, 24, 36),
                );
                p.rect_filled(
                    view.rect(left + 1., top + 1., CELL - 2., CELL - 8.),
                    0.,
                    wall_top,
                );
                p.rect_filled(
                    view.rect(left + 1., top + 1., CELL - 2., 2.),
                    0.,
                    Color32::from_rgb(78, 111, 130),
                );
                if drain {
                    p.rect_filled(
                        view.rect(left + 5., top + 9., 22., 4.),
                        0.,
                        MINT.gamma_multiply(0.6),
                    );
                }
            } else if *t == Tile::Glass {
                p.rect_filled(
                    view.rect(left + 2., top + 2., CELL - 4., CELL - 4.),
                    0.,
                    MINT.gamma_multiply(0.15),
                );
                p.rect_stroke(
                    view.rect(left + 3., top + 3., CELL - 6., CELL - 6.),
                    0.,
                    Stroke::new(view.scale, MINT),
                    StrokeKind::Inside,
                );
                view.line(
                    Pos2::new(left + 8., top + 23.),
                    Pos2::new(left + 23., top + 8.),
                    2.,
                    MINT.gamma_multiply(0.6),
                );
            } else if drain {
                for i in 0..4 {
                    p.rect_filled(
                        view.rect(left + 8., top + 10. + i as f32 * 3., 16., 1.),
                        0.,
                        INK,
                    );
                }
            }
        }
    }

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
                .find(|length| tile(&run.tiles, guard.pos + guard.aim * *length) != Tile::Floor)
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
    shot: bool,
    shot_requested: bool,
}

impl Game {
    fn new(shot: bool) -> Self {
        let mut run = Run::load(0, 0., 1);
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

    fn draw(&self, painter: &Painter, origin: Pos2, scale: f32) {
        let board = Rect::from_min_size(origin, SIZE * scale);
        let clipped = painter.with_clip_rect(board);
        let shake = Vec2::angled(self.run.seconds * 91. + self.run.shake) * self.run.shake * scale;
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
                MAPS[self.run.room].0
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
                MAPS[self.run.room + 1].0,
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
                    .min((area.height() - 20.) / (SIZE.y + 112.))
                    .max(0.15);
                let origin = area.center() - Vec2::new(SIZE.x / 2., SIZE.y / 2. - 12.) * scale;
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

        if self.shot && !self.shot_requested {
            self.shot_requested = true;
            let ctx = ctx.clone();
            ui.ctx().request_screenshot(move |image| {
                std::fs::create_dir_all("shots").expect("screenshot directory");
                image::save_buffer(
                    "shots/neon-undertow.png",
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
    let shot = std::env::args().any(|arg| arg == "--shot");
    eframe::run_native(
        "Neon Undertow",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1100., 760.])
                .with_min_inner_size([700., 500.]),
            ..Default::default()
        },
        Box::new(move |_| Ok(Box::new(Game::new(shot)))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_station_has_a_walkable_route_to_each_guard_and_exit() {
        for (room, (_, map)) in MAPS.iter().enumerate() {
            let lines: Vec<_> = map.lines().collect();
            assert_eq!(lines.len(), ROWS);
            for (y, line) in lines.iter().enumerate() {
                assert_eq!(line.len(), COLS);
                for (x, mark) in line.bytes().enumerate() {
                    assert!(b"#.+@Egr".contains(&mark));
                    if x == 0 || y == 0 || x == COLS - 1 || y == ROWS - 1 {
                        assert_eq!(mark, b'#');
                    }
                }
            }
            assert_eq!(map.matches('@').count(), 1);
            assert_eq!(map.matches('E').count(), 1);
            let run = Run::load(room, 0., 1);
            let distance = routes(&run.tiles, run.player);
            for pos in run.guards.iter().map(|g| g.pos).chain([run.exit]) {
                let (x, y) = index(pos);
                assert_ne!(distance[y][x], u16::MAX);
            }
        }
    }

    #[test]
    fn shots_break_glass_before_hitting_a_guard_and_enemy_hits_allow_a_retry() {
        let mut run = Run::load(0, 0., 1);
        let glass = center(11, 3);
        run.player = glass - Vec2::X * 52.;
        run.guards.truncate(1);
        run.guards[0].pos = glass + Vec2::X * 50.;
        let fire = Controls {
            aim: Vec2::X,
            fire: true,
            ..Controls::default()
        };

        for _ in 0..12 {
            run.step(fire);
        }
        assert_eq!(run.guards.len(), 1);
        assert!(tile(&run.tiles, glass) == Tile::Floor);
        for _ in 0..30 {
            run.step(fire);
        }
        assert!(run.guards.is_empty());
        run.bullets.push(Bullet {
            pos: run.player + Vec2::X * 20.,
            velocity: -Vec2::X * 414.,
            friendly: false,
        });
        for _ in 0..10 {
            run.step(Controls::default());
        }
        assert!(run.mode == Mode::Down);
        run.restart();
        assert!(run.mode == Mode::Play);
        assert_eq!((run.room, run.attempts, run.guards.len()), (0, 2, 3));
        assert!(tile(&run.tiles, glass) == Tile::Glass);
    }

    #[test]
    fn a_scripted_run_clears_all_three_stations() {
        let mut run = Run::load(0, 0., 1);
        for room in 0..MAPS.len() {
            let mut waypoint = None;
            for _ in 0..(120 * 35) {
                let target = run
                    .guards
                    .iter()
                    .filter(|g| visible(&run.tiles, run.player, g.pos))
                    .min_by(|a, b| {
                        a.pos
                            .distance_sq(run.player)
                            .total_cmp(&b.pos.distance_sq(run.player))
                    });
                let controls = if let Some(guard) = target {
                    waypoint = None;
                    let delta = guard.pos - run.player;
                    Controls {
                        aim: delta,
                        fire: true,
                        movement: Vec2::ZERO,
                    }
                } else {
                    let destination = run.guards.first().map_or(run.exit, |g| g.pos);
                    let next = waypoint
                        .filter(|p: &Pos2| p.distance(run.player) > 1.8)
                        .unwrap_or_else(|| {
                            let (x, y) = index(run.player);
                            let distance = routes(&run.tiles, destination);
                            if index(destination) == (x, y) {
                                return destination;
                            }
                            let (nx, ny) = neighbors(x, y)
                                .into_iter()
                                .min_by_key(|&(x, y)| distance[y][x])
                                .unwrap();
                            center(nx, ny)
                        });
                    waypoint = Some(next);
                    let delta = next - run.player;
                    let movement = delta / delta.length().max(218. * DT);
                    Controls {
                        movement,
                        ..Controls::default()
                    }
                };
                run.step(controls);
                if run.mode != Mode::Play {
                    break;
                }
            }
            assert!(
                run.mode
                    == if room + 1 == MAPS.len() {
                        Mode::Won
                    } else {
                        Mode::Clear
                    },
                "station {room} ended after {:.2}s with {} hostiles and player at {:?}; waypoint {:?}, guards {:?}",
                run.seconds,
                run.guards.len(),
                run.player,
                waypoint,
                run.guards.iter().map(|g| g.pos).collect::<Vec<_>>()
            );
            if run.mode == Mode::Clear {
                run.advance();
            }
        }
    }
}
