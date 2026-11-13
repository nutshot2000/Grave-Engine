//! Dust Hawk: Operation Red Mesa. A complete mission using Grave's reusable
//! height-aware materials, combat, atmosphere and sling-load systems.
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use grave_engine::{
    Materials,
    overhead::{
        FlightInput, Point,
        cargo::CargoState,
        view::{Instance, Viewport, VisualCell},
    },
};
#[path = "../dust_hawk/art.rs"]
mod art;
#[path = "../dust_hawk/audio.rs"]
mod audio;
#[path = "../dust_hawk/mission.rs"]
mod mission;
use mission::{Mission, Phase, Role};
use std::time::Instant;
struct App {
    m: Materials,
    game: Mission,
    view: Viewport,
    texture: Option<egui::TextureHandle>,
    last: Instant,
    clock: f32,
    camera: Point,
    zoom: f32,
    paused: bool,
    map: bool,
    map_texture: Option<egui::TextureHandle>,
    fx: Vec<(Point, f32, bool)>,
    muted: bool,
    tap_fire: bool,
    tap_heavy: bool,
    audio: Option<audio::Audio>,
}
impl App {
    fn new() -> Self {
        let m = Materials::load_dir("materials");
        let game = Mission::new(&m);
        let camera = Point::new(115., 82., 12.);
        let mut view = Viewport::new(900, 560);
        view.projection.scale = 1.85;
        view.ground_color = Some([190, 164, 118]);
        Self {
            m,
            game,
            view,
            texture: None,
            last: Instant::now(),
            clock: 0.,
            camera,
            zoom: 1.85,
            paused: false,
            map: false,
            map_texture: None,
            fx: vec![],
            muted: false,
            tap_fire: false,
            tap_heavy: false,
            audio: None,
        }
    }
    fn reset(&mut self) {
        self.game = Mission::new(&self.m);
        self.clock = 0.;
        self.camera = Point::new(115., 82., 12.);
        self.paused = false;
        self.map = false;
        self.fx.clear();
        self.tap_fire = false;
        self.tap_heavy = false;
        self.last = Instant::now();
    }
    fn tick(&mut self, input: FlightInput, fire: bool, heavy: bool) {
        let ammo = self.game.ammo;
        self.game.step(input, fire, heavy, &self.m);
        if !self.muted {
            if let Some(a) = &self.audio {
                if self.game.ammo[0] < ammo[0] {
                    a.effect(0);
                }
                if self.game.ammo[1] < ammo[1] || self.game.ammo[2] < ammo[2] {
                    a.effect(1);
                }
                if self
                    .game
                    .world
                    .explosions
                    .iter()
                    .any(|e| e.at.distance(self.game.craft.at) < 110.)
                {
                    a.effect(2);
                }
            }
        }
        for e in &self.game.combat.impacts {
            self.fx
                .push((e.at, if e.explosive { 0.35 } else { 0.13 }, e.explosive));
        }
        for e in &self.game.world.explosions {
            self.fx.push((e.at, 0.45, true));
        }
        for (_, life, _) in &mut self.fx {
            *life -= 1. / 60.;
        }
        self.fx.retain(|(_, life, _)| *life > 0.);
        let craft = self.game.craft.at;
        let target = Point::new(
            craft.x + self.game.craft.velocity.x * 0.22,
            craft.y + self.game.craft.velocity.y * 0.22,
            craft.z * 0.46,
        );
        let target = Point::new(
            target.x.clamp(115., 145.),
            target.y.clamp(62., 82.),
            target.z,
        );
        self.camera = self.camera.lerp(target, 0.045);
    }
    fn draw_world(&mut self) {
        let craft = art::helicopter(self.game.world.frame);
        let person = vec![
            VisualCell {
                at: Point::new(0., 0., 0.),
                color: [48, 65, 69],
            },
            VisualCell {
                at: Point::new(0., 0., 1.),
                color: [64, 120, 138],
            },
            VisualCell {
                at: Point::new(0., 0., 2.),
                color: [207, 169, 120],
            },
        ];
        let cargo_art: Vec<Vec<VisualCell>> = self
            .game
            .cargo
            .iter()
            .map(|c| {
                c.cells
                    .iter()
                    .map(|(p, cell)| VisualCell {
                        at: *p,
                        color: grave_engine::asset::paint_rgb(cell.paint)
                            .unwrap_or(self.m.get(cell.mat).color),
                    })
                    .collect()
            })
            .collect();
        let gun: Vec<_> = (0..9)
            .map(|x| VisualCell {
                at: Point::new(x as f32, 0., 0.),
                color: [72, 86, 62],
            })
            .collect();
        let mut instances = vec![Instance {
            at: self.game.craft.at,
            angle: self.game.craft.heading,
            cells: &craft,
            shadow_radius: 8.,
        }];
        for r in &self.game.rescues {
            if !r.collected {
                for n in 0..r.people {
                    instances.push(Instance {
                        at: Point::new(r.at.x + n as f32 * 2. - r.people as f32, r.at.y, r.at.z),
                        angle: (r.id % 2) as f32 * 0.3,
                        cells: &person,
                        shadow_radius: 0.5,
                    });
                }
            }
        }
        for (i, c) in self.game.cargo.iter().enumerate() {
            if matches!(c.state, CargoState::Slung | CargoState::Falling) {
                instances.push(Instance {
                    at: c.at,
                    angle: 0.,
                    cells: &cargo_art[i],
                    shadow_radius: 3.,
                });
            }
        }
        for u in &self.game.units {
            if self.game.alive(u.id) && u.role == Role::Aa {
                let p = self.game.combat.target(u.id).unwrap().bounds.centre();
                instances.push(Instance {
                    at: Point::new(p.x, p.y, p.z + 2.),
                    angle: u.heading,
                    cells: &gun,
                    shadow_radius: 0.,
                });
            }
        }
        let tracers: Vec<_> = self.game.combat.shots.iter().map(|s| s.at).collect();
        let speed = self.game.craft.velocity.x.hypot(self.game.craft.velocity.y);
        let target = (self.zoom - speed * 0.0025).clamp(1.25, 3.3);
        self.view.projection.scale += (target - self.view.projection.scale) * 0.08;
        self.view
            .draw(&self.game.world, &self.m, self.camera, &instances, &tracers);
    }
    fn map_image(&mut self, ctx: &egui::Context) {
        let v = &self.game.world;
        let mut pixels = vec![0u8; v.width * v.depth * 4];
        for y in 0..v.depth {
            for x in 0..v.width {
                let mut color = [56, 52, 41];
                for z in (0..v.height).rev() {
                    let p = Point::new(x as f32, y as f32, z as f32);
                    let id = v.material_at(p);
                    if id != 0 && self.m.get(id).state != grave_engine::State::Gas {
                        let (_, i) = v.index(p).unwrap();
                        color = grave_engine::asset::paint_rgb(v.sections[y].world.paint[i])
                            .unwrap_or(self.m.get(id).color);
                        break;
                    }
                }
                let i = (y * v.width + x) * 4;
                pixels[i..i + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
            }
        }
        let img = egui::ColorImage::from_rgba_unmultiplied([v.width, v.depth], &pixels);
        if let Some(t) = &mut self.map_texture {
            t.set(img, egui::TextureOptions::NEAREST);
        } else {
            self.map_texture =
                Some(ctx.load_texture("mesa-map", img, egui::TextureOptions::NEAREST));
        }
    }
    fn overlay_map(&mut self, ui: &mut egui::Ui, screen: Rect) {
        let p = ui.painter();
        p.rect_filled(screen, 0., Color32::from_black_alpha(225));
        let area = Rect::from_center_size(
            screen.center(),
            Vec2::new(
                (screen.width() - 70.).min(1100.),
                (screen.height() - 80.).min(740.),
            ),
        );
        p.rect_filled(area, 8., Color32::from_rgb(24, 31, 28));
        let map = Rect::from_min_size(
            area.min + Vec2::new(20., 60.),
            Vec2::new(area.width() * 0.69, area.width() * 0.69 * 0.6),
        );
        if let Some(t) = &self.map_texture {
            p.image(
                t.id(),
                map,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                Color32::WHITE,
            );
        }
        p.text(
            area.min + Vec2::new(22., 20.),
            egui::Align2::LEFT_TOP,
            "TACTICAL MAP  /  M to return",
            egui::FontId::proportional(22.),
            Color32::from_rgb(230, 216, 167),
        );
        let point = |q: Point| {
            Pos2::new(
                map.left() + q.x / 240. * map.width(),
                map.top() + q.y / 144. * map.height(),
            )
        };
        let yellow = Color32::from_rgb(245, 198, 85);
        let cyan = Color32::from_rgb(92, 211, 200);
        for u in &self.game.units {
            let target = self.game.combat.target(u.id).unwrap();
            let pos = point(target.bounds.centre());
            if target.alive() {
                let color = if matches!(u.role, Role::Aa | Role::Sam) {
                    Color32::from_rgb(225, 95, 73)
                } else {
                    yellow
                };
                p.circle_filled(pos, 5., color);
                if matches!(u.role, Role::Aa | Role::Sam) {
                    let r = if u.role == Role::Sam { 92. } else { 72. }
                        * if self.game.remaining_radars() == 0 {
                            0.63
                        } else {
                            1.
                        };
                    p.with_clip_rect(map).circle_stroke(
                        pos,
                        r / 240. * map.width(),
                        Stroke::new(0.5, Color32::from_rgba_unmultiplied(220, 80, 55, 50)),
                    );
                } else {
                    p.text(
                        pos + Vec2::new(7., -10.),
                        egui::Align2::LEFT_TOP,
                        &u.name,
                        egui::FontId::proportional(10.),
                        yellow,
                    );
                }
            } else {
                p.line_segment(
                    [pos - Vec2::splat(4.), pos + Vec2::splat(4.)],
                    Stroke::new(2., Color32::GRAY),
                );
            }
        }
        for r in &self.game.rescues {
            if !r.collected {
                p.circle_filled(point(r.at), 5., cyan);
                p.text(
                    point(r.at) + Vec2::new(6., 2.),
                    egui::Align2::LEFT_TOP,
                    format!("{} ({})", r.name, r.people),
                    egui::FontId::proportional(10.),
                    cyan,
                );
            }
        }
        for s in &self.game.supplies {
            if !s.taken {
                p.rect_filled(
                    Rect::from_center_size(point(s.at), Vec2::splat(6.)),
                    0.,
                    Color32::from_rgb(148, 189, 104),
                );
            }
        }
        for b in &self.game.barrels {
            if !b.taken {
                p.circle_stroke(
                    point(Point::new(b.at[0] as f32, b.at[1] as f32, b.at[2] as f32)),
                    4.,
                    Stroke::new(1., Color32::from_rgb(178, 150, 79)),
                );
            }
        }
        p.circle_filled(point(self.game.base), 7., Color32::from_rgb(112, 183, 122));
        p.text(
            point(self.game.base) + Vec2::new(8., 2.),
            egui::Align2::LEFT_TOP,
            "LZ / SERVICE",
            egui::FontId::proportional(10.),
            Color32::LIGHT_GREEN,
        );
        p.circle_filled(point(self.game.craft.at), 5., Color32::WHITE);
        let x = map.right() + 20.;
        let mut y = map.top();
        let defences = self
            .game
            .units
            .iter()
            .filter(|u| matches!(u.role, Role::Aa | Role::Sam) && self.game.alive(u.id))
            .count();
        let objectives = vec![
            ("MISSION OBJECTIVES".to_string(), false),
            (
                format!(
                    "{} Radar arrays: {}/2",
                    if self.game.remaining_radars() == 0 {
                        "✓"
                    } else {
                        "○"
                    },
                    2 - self.game.remaining_radars()
                ),
                self.game.remaining_radars() == 0,
            ),
            (
                format!(
                    "{} Power station",
                    if self.game.alive(12) { "○" } else { "✓" }
                ),
                !self.game.alive(12),
            ),
            (
                format!(
                    "{} Survivors delivered: {}/6",
                    if self.game.rescued == 6 { "✓" } else { "○" },
                    self.game.rescued
                ),
                self.game.rescued == 6,
            ),
            (
                format!(
                    "{} Command centre",
                    if self.game.alive(13) { "○" } else { "✓" }
                ),
                !self.game.alive(13),
            ),
            (
                "Return to LZ to finish".to_string(),
                self.game.phase == Phase::Won,
            ),
            ("".to_string(), false),
            ("OPTIONAL / EXPLORATION".to_string(), false),
            (
                format!(
                    "Ammo depot: {}",
                    if self.game.alive(14) {
                        "intact"
                    } else {
                        "destroyed"
                    }
                ),
                !self.game.alive(14),
            ),
            (format!("Air defences remaining: {defences}"), defences == 0),
            (
                format!(
                    "Supply caches: {}/4",
                    self.game.supplies.iter().filter(|s| s.taken).count()
                ),
                false,
            ),
            ("G: sling and drop an oil barrel".to_string(), false),
            ("".to_string(), false),
            ("WEAPON / TACTICAL NOTES".to_string(), false),
            ("AP: precision, exposed oil".to_string(), false),
            ("HE: walls and grouped targets".to_string(), false),
            ("AT: hold aim to lock on armour".to_string(), false),
            ("F: flares against guided SAMs".to_string(), false),
        ];
        for (text, done) in objectives {
            p.text(
                Pos2::new(x, y),
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::proportional(13.),
                if done {
                    Color32::LIGHT_GREEN
                } else {
                    Color32::from_rgb(222, 214, 183)
                },
            );
            y += 23.;
        }
        p.text(Pos2::new(map.left(),map.bottom()+24.),egui::Align2::LEFT_TOP,"Red rings: detection range. Radar reduces it. Power disables guided SAMs.\nGreen squares: supplies. Cyan: rescue. Gold rings: portable oil barrels.\nHover slowly at low altitude to collect supplies or winch people aboard.",egui::FontId::proportional(14.),Color32::from_rgb(200,203,179));
    }
    fn stress(&mut self) {
        for _ in 0..120 {
            self.game.world.step(&self.m);
        }
        for at in [
            Point::new(54., 25., 49.),
            Point::new(201., 21., 56.),
            Point::new(120., 95., 17.),
            Point::new(196., 78., 18.),
            Point::new(74., 61., 14.),
            Point::new(207., 122., 17.),
            Point::new(102., 126., 18.),
        ] {
            self.game.world.heat(at, 9., 850.);
        }
        let mut steps = Vec::new();
        let mut renders = Vec::new();
        let mut max_fragments = 0;
        for frame in 0..600 {
            let started = Instant::now();
            self.game.world.step(&self.m);
            steps.push(started.elapsed().as_secs_f64() * 1000.);
            max_fragments = max_fragments.max(self.game.world.debris.states().len());
            if frame % 10 == 0 {
                let start = Instant::now();
                self.draw_world();
                renders.push(start.elapsed().as_secs_f64() * 1000.);
            }
            if frame % 120 == 119 {
                println!(
                    "Stress frame {}: {} fragments, {} sky cells",
                    frame + 1,
                    self.game.world.debris.states().len(),
                    self.game.world.plumes.len()
                );
            }
        }
        steps.sort_by(f64::total_cmp);
        renders.sort_by(f64::total_cmp);
        println!(
            "All-seven-site fire stress: tick median {:.1}ms / p95 {:.1}ms / max {:.1}ms; render median {:.1}ms / p95 {:.1}ms; peak {} fragments",
            steps[300], steps[570], steps[599], renders[30], renders[57], max_fragments
        );
        self.draw_world();
        grave_engine::headless::write_png(
            std::path::Path::new("shots/red-mesa-stress.png"),
            self.view.width,
            self.view.height,
            &self.view.pixels,
            2,
        )
        .unwrap();
    }
    fn showcase(&mut self) {
        self.game.start();
        self.game.craft.at = Point::new(161., 91., 50.);
        self.game.aim = self.game.combat.target(13).unwrap().bounds.centre();
        self.game.lock_target = Some(13);
        self.game.lock = 1.;
        self.game.selected = 2;
        for frame in 0..120 {
            if frame == 0 || frame == 55 {
                self.game.fire(false);
            }
            self.game
                .step(FlightInput::default(), false, false, &self.m);
        }
        assert!(
            !self.game.alive(13),
            "actual guided shots failed to destroy command"
        );
        self.camera = Point::new(173., 83., 13.);
        self.zoom = 2.2;
        self.draw_world();
        grave_engine::headless::write_png(
            std::path::Path::new("shots/red-mesa-destruction.png"),
            self.view.width,
            self.view.height,
            &self.view.pixels,
            2,
        )
        .unwrap();
        // A deliberately ignited fuel test checks tall fire and open atmosphere.
        self.game.world.heat(Point::new(119., 94., 17.), 9., 850.);
        for _ in 0..180 {
            self.game.world.step(&self.m);
        }
        self.camera = Point::new(133., 91., 12.);
        self.draw_world();
        grave_engine::headless::write_png(
            std::path::Path::new("shots/red-mesa-fire.png"),
            self.view.width,
            self.view.height,
            &self.view.pixels,
            2,
        )
        .unwrap();
        println!(
            "Actual command destruction and fuel burn screenshots saved; {} plume cells above the grid",
            self.game.world.plumes.len()
        );
    }
    fn verify(&mut self) {
        grave_engine::profile::enable(true);
        self.game.start();
        self.draw_world();
        grave_engine::headless::write_png(
            std::path::Path::new("shots/red-mesa-start.png"),
            self.view.width,
            self.view.height,
            &self.view.pixels,
            2,
        )
        .unwrap();
        // Tour every installation at a safe altitude; exercise actual material steps.
        for at in [
            Point::new(50., 26., 68.),
            Point::new(200., 22., 74.),
            Point::new(120., 95., 45.),
            Point::new(198., 78., 48.),
            Point::new(195., 123., 44.),
        ] {
            self.game.craft.at = at;
            self.camera = Point::new(at.x, at.y, at.z * 0.4);
            for _ in 0..6 {
                self.game.step(
                    FlightInput {
                        heading: 0.,
                        ..Default::default()
                    },
                    false,
                    false,
                    &self.m,
                );
            }
            self.draw_world();
        }
        grave_engine::headless::write_png(
            std::path::Path::new("shots/red-mesa-east.png"),
            self.view.width,
            self.view.height,
            &self.view.pixels,
            2,
        )
        .unwrap();
        println!(
            "Red Mesa verified: {} targets, {} people, {} portable barrels, {:.1} ms/step",
            self.game.units.len(),
            self.game.rescues.iter().map(|r| r.people).sum::<u32>(),
            self.game.barrels.len(),
            self.game.step_ms
        );
        println!("{}", grave_engine::profile::report());
    }
}
impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ui.visuals_mut().override_text_color = Some(Color32::from_rgb(231, 225, 198));
        ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::from_rgb(48, 58, 49);
        ui.visuals_mut().widgets.hovered.weak_bg_fill = Color32::from_rgb(67, 82, 64);
        let elapsed = self.last.elapsed().as_secs_f32().min(0.08);
        self.last = Instant::now();
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && self.game.phase == Phase::Briefing {
            self.game.start();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::M)) {
            self.map = !self.map;
            if self.map {
                self.map_image(&ctx);
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::P) || i.key_pressed(egui::Key::Escape)) {
            self.paused = !self.paused;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::R)) {
            self.reset();
        }
        if !self.map && !self.paused && self.game.phase == Phase::Flying {
            if ctx.input(|i| i.key_pressed(egui::Key::G)) {
                self.game.hook(&self.m);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::F)) {
                self.game.flares();
            }
        }
        for (key, n) in [
            (egui::Key::Num1, 0),
            (egui::Key::Num2, 1),
            (egui::Key::Num3, 2),
        ] {
            if ctx.input(|i| i.key_pressed(key)) {
                self.game.selected = n;
            }
        }
        ui.horizontal(|ui| {
            ui.heading("DUST HAWK");
            ui.label("OPERATION RED MESA");
            if ui.button("Tactical map [M]").clicked() {
                self.map = !self.map;
                if self.map {
                    self.map_image(&ctx);
                }
            }
            if ui
                .button(if self.paused {
                    "Resume [P]"
                } else {
                    "Pause [P]"
                })
                .clicked()
            {
                self.paused = !self.paused;
            }
            if ui.button("Restart [R]").clicked() {
                self.reset();
            }
            ui.checkbox(&mut self.muted, "Mute sound");
        });
        let health = self.game.health();
        ui.horizontal(|ui| {
            ui.colored_label(
                if health < 60. {
                    Color32::LIGHT_RED
                } else {
                    Color32::LIGHT_GREEN
                },
                format!("ARMOUR {:03}", health as u32),
            );
            ui.colored_label(
                if self.game.fuel < 25. {
                    Color32::LIGHT_RED
                } else {
                    Color32::from_rgb(232, 205, 133)
                },
                format!("FUEL {:03}%", self.game.fuel as u32),
            );
            for (i, name) in ["1  AP CANNON", "2  HE ROCKETS", "3  GUIDED AT"]
                .iter()
                .enumerate()
            {
                if ui
                    .selectable_label(
                        self.game.selected == i,
                        format!("{}  {}", name, self.game.ammo[i]),
                    )
                    .clicked()
                {
                    self.game.selected = i;
                }
            }
            ui.label(format!(
                "RESCUED {}/6  ABOARD {}   SCORE {}",
                self.game.rescued, self.game.passengers, self.game.score
            ));
        });
        ui.horizontal(|ui| {
            ui.label(format!("ALT {:.1}m", self.game.craft.at.z * 0.1));
            ui.label(if self.game.held.is_some() {
                "G: release slung barrel"
            } else {
                "G: winch an oil barrel"
            });
            ui.label(format!("F: flares ({})", self.game.flares));
            if self.game.craft.blocked {
                ui.colored_label(Color32::LIGHT_RED, "OBSTACLE — climb [E]");
            } else if self.game.service > 0. {
                ui.colored_label(Color32::LIGHT_GREEN, "LZ SERVICE / UNLOADING");
            } else {
                ui.label(
                    "WASD fly • E/Q altitude • mouse aim/fire • Space heavy weapon • wheel zoom",
                );
            }
        });
        let available = ui.available_rect_before_wrap();
        let scale = (available.width() / 900.).min(available.height() / 560.);
        let rect =
            Rect::from_center_size(available.center(), Vec2::new(900. * scale, 560. * scale));
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let pointer = ctx
            .input(|i| i.pointer.hover_pos())
            .filter(|p| rect.contains(*p));
        let mut selected = None;
        if let Some(pos) = pointer {
            let local = [(pos.x - rect.left()) / scale, (pos.y - rect.top()) / scale];
            self.game.aim = self.view.projection.pick(local, &self.game.world, &self.m);
            let mut best = 22f32;
            for u in &self.game.units {
                let t = self.game.combat.target(u.id).unwrap();
                if !t.alive() {
                    continue;
                }
                let p = self.view.projection.project(t.bounds.centre());
                let d = (p[0] - local[0]).hypot(p[1] - local[1]);
                if d < best {
                    best = d;
                    selected = Some(u.id);
                    self.game.aim = t.bounds.centre();
                }
            }
        }
        if response.hovered() {
            self.zoom =
                (self.zoom + ctx.input(|i| i.smooth_scroll_delta.y) * 0.005).clamp(1.25, 3.3);
        }
        if ctx.input(|i| i.focused) && !self.paused && !self.map && self.game.phase == Phase::Flying
        {
            // Preserve a quick press until the next fixed simulation tick.
            // A press and release may both arrive in a single rendered frame.
            self.tap_fire |= response.hovered()
                && ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
            self.tap_heavy |= ctx.input(|i| i.key_pressed(egui::Key::Space))
                || (response.hovered()
                    && ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Secondary)));
            let (mx, my, climb, fire, heavy) = ctx.input(|i| {
                let a = |x, y| i.key_down(x) as i32 - i.key_down(y) as i32;
                (
                    (a(egui::Key::D, egui::Key::A) + a(egui::Key::ArrowRight, egui::Key::ArrowLeft))
                        as f32,
                    (a(egui::Key::S, egui::Key::W) + a(egui::Key::ArrowDown, egui::Key::ArrowUp))
                        as f32,
                    a(egui::Key::E, egui::Key::Q) as f32,
                    response.hovered() && i.pointer.primary_down(),
                    i.key_down(egui::Key::Space)
                        || (response.hovered() && i.pointer.secondary_down()),
                )
            });
            let input = FlightInput {
                movement: [(mx + my) * 0.7071, (my - mx) * 0.7071],
                climb,
                heading: (self.game.aim.y - self.game.craft.at.y)
                    .atan2(self.game.aim.x - self.game.craft.at.x),
            };
            self.clock = (self.clock + elapsed).min(0.05);
            let mut n = 0;
            while self.clock >= 1. / 60. && n < 2 {
                self.game.update_lock(selected);
                self.tick(input, fire || self.tap_fire, heavy || self.tap_heavy);
                self.tap_fire = false;
                self.tap_heavy = false;
                self.clock -= 1. / 60.;
                n += 1;
            }
        } else {
            self.clock = 0.;
            self.tap_fire = false;
            self.tap_heavy = false;
        }
        self.draw_world();
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [self.view.width, self.view.height],
            &self.view.pixels,
        );
        if let Some(t) = &mut self.texture {
            t.set(image, egui::TextureOptions::NEAREST);
        } else {
            self.texture = Some(ctx.load_texture("red-mesa", image, egui::TextureOptions::NEAREST));
        }
        let p = ui.painter();
        p.image(
            self.texture.as_ref().unwrap().id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
            Color32::WHITE,
        );
        let screen = |q: Point| {
            let v = self.view.projection.project(q);
            Pos2::new(rect.left() + v[0] * scale, rect.top() + v[1] * scale)
        };
        let (task, waypoint) = self.game.next_objective();
        let task_rect = Rect::from_min_size(rect.min + Vec2::splat(15.), Vec2::new(315., 84.));
        p.rect_filled(
            task_rect,
            5.,
            Color32::from_rgba_unmultiplied(20, 30, 25, 215),
        );
        p.text(
            task_rect.min + Vec2::new(12., 10.),
            egui::Align2::LEFT_TOP,
            "NEXT OBJECTIVE",
            egui::FontId::proportional(11.),
            Color32::from_rgb(169, 181, 153),
        );
        p.text(
            task_rect.min + Vec2::new(12., 29.),
            egui::Align2::LEFT_TOP,
            task,
            egui::FontId::proportional(17.),
            Color32::from_rgb(244, 218, 151),
        );
        p.text(
            task_rect.min + Vec2::new(12., 55.),
            egui::Align2::LEFT_TOP,
            format!(
                "{:.0}m away • M: mission map",
                self.game.craft.at.distance(waypoint) * 0.1
            ),
            egui::FontId::proportional(12.),
            Color32::from_rgb(199, 208, 180),
        );
        let goal = screen(waypoint);
        if rect.shrink(24.).contains(goal) {
            p.circle_stroke(
                goal,
                12. + (self.game.time * 3.).sin() * 2.,
                Stroke::new(1.5, Color32::from_rgb(237, 209, 119)),
            );
        } else {
            let edge = Pos2::new(
                goal.x.clamp(rect.left() + 24., rect.right() - 24.),
                goal.y.clamp(rect.top() + 110., rect.bottom() - 45.),
            );
            p.text(
                edge,
                egui::Align2::CENTER_CENTER,
                "◆",
                egui::FontId::proportional(23.),
                Color32::from_rgb(237, 209, 119),
            );
        }
        for u in &self.game.units {
            let t = self.game.combat.target(u.id).unwrap();
            if !t.alive() {
                continue;
            }
            let pos = screen(t.bounds.centre());
            if !rect.contains(pos) {
                continue;
            }
            let color = if matches!(u.role, Role::Aa | Role::Sam) {
                Color32::from_rgb(238, 124, 80)
            } else {
                Color32::from_rgb(241, 205, 111)
            };
            p.circle_stroke(pos, 5., Stroke::new(1., color));
            if selected == Some(u.id) {
                p.text(
                    pos - Vec2::new(0., 28.),
                    egui::Align2::CENTER_BOTTOM,
                    format!("{}  {}%", u.name, (t.health / t.max_health * 100.) as u32),
                    egui::FontId::proportional(12.),
                    color,
                );
                p.line_segment(
                    [
                        pos - Vec2::new(20., 13.),
                        pos + Vec2::new(-20. + 40. * t.health / t.max_health, -13.),
                    ],
                    Stroke::new(3., color),
                );
            }
        }
        for r in &self.game.rescues {
            if !r.collected {
                let pos = screen(r.at);
                if rect.contains(pos) {
                    p.text(
                        pos - Vec2::new(0., 12.),
                        egui::Align2::CENTER_BOTTOM,
                        format!("RESCUE {}", r.people),
                        egui::FontId::proportional(11.),
                        Color32::from_rgb(93, 223, 218),
                    );
                    if r.progress > 0. {
                        p.circle_stroke(pos, 10., Stroke::new(2., Color32::LIGHT_GREEN));
                    }
                }
            }
        }
        for b in &self.game.barrels {
            if !b.taken {
                let pos = screen(Point::new(
                    b.at[0] as f32 + 3.,
                    b.at[1] as f32 + 3.,
                    b.at[2] as f32 + 10.,
                ));
                if rect.contains(pos)
                    && self.game.craft.at.distance(Point::new(
                        b.at[0] as f32,
                        b.at[1] as f32,
                        self.game.craft.at.z,
                    )) < 35.
                {
                    p.text(
                        pos,
                        egui::Align2::CENTER_BOTTOM,
                        "G  OIL BARREL",
                        egui::FontId::proportional(10.),
                        Color32::from_rgb(229, 204, 135),
                    );
                }
            }
        }
        if let Some(c) = self
            .game
            .held
            .and_then(|id| self.game.cargo.iter().find(|c| c.id == id))
        {
            p.line_segment(
                [
                    screen(self.game.craft.at),
                    screen(Point::new(
                        c.centre().x,
                        c.centre().y,
                        c.at.z + c.size[2] as f32,
                    )),
                ],
                Stroke::new(1.5, Color32::from_rgb(191, 186, 149)),
            );
        }
        for s in &self.game.combat.shots {
            let pos = screen(s.at);
            if rect.contains(pos) {
                let old = Point::new(
                    s.at.x - s.velocity.x * 0.03,
                    s.at.y - s.velocity.y * 0.03,
                    s.at.z - s.velocity.z * 0.03,
                );
                p.line_segment(
                    [screen(old), pos],
                    Stroke::new(
                        if s.weapon.explosive { 2.5 } else { 1.3 },
                        if s.faction == 2 {
                            Color32::from_rgb(255, 110, 73)
                        } else {
                            Color32::from_rgb(251, 224, 138)
                        },
                    ),
                );
            }
        }
        for (at, life, big) in &self.fx {
            let pos = screen(*at);
            if rect.contains(pos) {
                p.circle_stroke(
                    pos,
                    (1. - life.min(0.4) / 0.4) * if *big { 20. } else { 5. },
                    Stroke::new(
                        1.5,
                        Color32::from_rgba_unmultiplied(
                            255,
                            183,
                            70,
                            (life.min(0.4) / 0.4 * 220.) as u8,
                        ),
                    ),
                );
            }
        }
        if self.game.combat.shots.iter().any(|s| {
            s.faction == 2
                && s.target == Some(0)
                && s.weapon.guidance > 0.
                && s.at.distance(self.game.craft.at) < 90.
        }) {
            p.text(
                Pos2::new(rect.right() - 24., rect.top() + 24.),
                egui::Align2::RIGHT_TOP,
                "MISSILE INBOUND — FLARES [F]",
                egui::FontId::proportional(19.),
                Color32::from_rgb(255, 116, 79),
            );
        }
        let cross = screen(self.game.aim);
        let lock = self.game.selected == 2 && self.game.lock >= 0.65;
        let in_range = self.game.craft.at.distance(self.game.aim)
            < [mission::CANNON, mission::ROCKET, mission::MISSILE][self.game.selected].range;
        if !in_range && rect.contains(cross) {
            p.text(
                cross + Vec2::new(0., 24.),
                egui::Align2::CENTER_TOP,
                "OUT OF RANGE",
                egui::FontId::proportional(11.),
                Color32::from_rgb(240, 148, 110),
            );
        }
        let color = if !in_range {
            Color32::from_rgb(156, 139, 116)
        } else if lock {
            Color32::LIGHT_GREEN
        } else {
            Color32::from_rgb(255, 208, 101)
        };
        p.circle_stroke(
            cross,
            8. + if self.game.selected == 2 {
                (1. - self.game.lock) * 10.
            } else {
                0.
            },
            Stroke::new(1., color),
        );
        p.line_segment(
            [cross - Vec2::new(12., 0.), cross + Vec2::new(12., 0.)],
            Stroke::new(1., color),
        );
        if self.game.flare_time > 0. {
            let pos = screen(self.game.craft.at);
            for n in 0..5 {
                let offset = Vec2::new(
                    ((self.game.time * 4. + n as f32).sin()) * 35.,
                    n as f32 * 7. + 12.,
                );
                p.circle_filled(pos + offset, 2., Color32::from_rgb(255, 225, 155));
            }
        }
        if self.game.toast_time > 0. {
            p.text(
                Pos2::new(rect.center().x, rect.bottom() - 24.),
                egui::Align2::CENTER_BOTTOM,
                &self.game.toast,
                egui::FontId::proportional(15.),
                Color32::from_rgb(246, 221, 155),
            );
        }
        if self.game.damaged_flash > 0. {
            p.rect_stroke(
                rect.shrink(3.),
                0.,
                Stroke::new(
                    4.,
                    Color32::from_rgba_unmultiplied(
                        255,
                        67,
                        48,
                        (self.game.damaged_flash * 150.) as u8,
                    ),
                ),
                egui::StrokeKind::Inside,
            );
        }
        if self.map {
            self.overlay_map(ui, rect);
        } else if self.game.phase != Phase::Flying {
            let title = match self.game.phase {
                Phase::Briefing => "OPERATION RED MESA",
                Phase::Won => "MISSION COMPLETE",
                Phase::Lost => "AIRCRAFT LOST",
                _ => "",
            };
            let text = match self.game.phase {
                Phase::Briefing => {
                    "An enemy radar network protects Red Mesa.\nDestroy both arrays, cut power to the SAMs,\nrescue six people and neutralise command.\nReturn to the landing zone to finish.\n\nChoose your ammunition. Use cliffs as cover.\nG can sling an oil barrel into an enemy position.\nM opens your map. E/Q changes altitude."
                }
                Phase::Won => {
                    "All primary objectives complete.\nYour crew and survivors made it home.\nTry clearing every defence and the ammo depot\nfor a higher score on the next sortie."
                }
                _ => {
                    "Use the tactical map to plan a safer approach.\nDisable radar and power before the command centre.\nReturn to LZ for repair, fuel and ammunition."
                }
            };
            let area = Rect::from_center_size(rect.center(), Vec2::new(530., 390.));
            ui.painter()
                .rect_filled(area, 8., Color32::from_rgba_unmultiplied(18, 25, 21, 235));
            ui.painter().text(
                area.min + Vec2::new(30., 25.),
                egui::Align2::LEFT_TOP,
                title,
                egui::FontId::proportional(28.),
                Color32::from_rgb(243, 205, 123),
            );
            ui.painter().text(
                area.min + Vec2::new(30., 80.),
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::proportional(17.),
                Color32::from_rgb(220, 221, 199),
            );
            let button =
                Rect::from_min_size(area.min + Vec2::new(130., 330.), Vec2::new(260., 38.));
            if ui
                .put(
                    button,
                    egui::Button::new(if self.game.phase == Phase::Briefing {
                        "LAUNCH SORTIE  [ENTER]"
                    } else {
                        "NEW SORTIE  [R]"
                    }),
                )
                .clicked()
            {
                if self.game.phase == Phase::Briefing {
                    self.game.start();
                } else {
                    self.reset();
                }
            }
        } else if self.paused {
            p.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "PAUSED",
                egui::FontId::proportional(30.),
                Color32::WHITE,
            );
        }
        if let Some(a) = &self.audio {
            a.ambience(
                !self.muted && !self.paused && !self.map && self.game.phase == Phase::Flying,
                self.game.craft.velocity.x.hypot(self.game.craft.velocity.y),
            );
        }
        ctx.request_repaint();
    }
}
fn main() -> eframe::Result {
    let mut app = App::new();
    if std::env::args().any(|s| s == "--stress") {
        app.stress();
        return Ok(());
    }
    if std::env::args().any(|s| s == "--showcase") {
        app.showcase();
        return Ok(());
    }
    if std::env::args().any(|s| s == "--verify") {
        app.verify();
        return Ok(());
    }
    app.audio = audio::Audio::open();
    eframe::run_native(
        "Dust Hawk — Operation Red Mesa",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size(Vec2::new(1500., 980.)),
            ..Default::default()
        },
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(app))
        }),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    #[test]
    fn quick_mouse_taps_fire_once_and_pause_discards_old_input() {
        let mut h = Harness::builder()
            .with_size(Vec2::new(1500., 980.))
            .allow_missing_glyphs()
            .build_eframe(|_| App::new());
        h.state_mut().game.start();
        let pos = Pos2::new(450., 500.);
        h.event(egui::Event::PointerMoved(pos));
        h.run_steps(1);
        h.state_mut().last = Instant::now() - std::time::Duration::from_millis(35);
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.run_steps(1);
        assert_eq!(
            h.state().game.ammo[0],
            699,
            "a quick click was lost between simulation ticks"
        );
        h.state_mut().paused = true;
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.run_steps(1);
        h.state_mut().paused = false;
        h.state_mut().last = Instant::now() - std::time::Duration::from_millis(35);
        h.run_steps(1);
        assert_eq!(
            h.state().game.ammo[0],
            699,
            "paused input fired on resuming"
        );
    }
}
