//! A small flight/material test built entirely on the reusable overhead APIs.
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use grave_engine::{
    Materials,
    overhead::{
        Aircraft, Explosion, FlightInput, Point, Projectile, Volume,
        view::{Instance, Viewport, VisualCell},
    },
};
use std::{path::Path, time::Instant};
const SCENE: &str = "scenes/flight-test.flight";
struct FlightTest {
    m: Materials,
    world: Volume,
    craft: Aircraft,
    shots: Vec<Projectile>,
    view: Viewport,
    texture: Option<egui::TextureHandle>,
    last: Instant,
    clock: f32,
    gun: f32,
    rocket: f32,
    paused: bool,
    aim: Point,
    camera: Point,
    wood_start: usize,
    flashes: Vec<(Point, u32)>,
    step_ms: f32,
}
impl FlightTest {
    fn new() -> Self {
        let m = Materials::load_dir("materials");
        let (world, spawn) = Volume::load(Path::new(SCENE), &m).expect("flight test scene");
        let wood_start = world.count(m.id("weathered_wood").unwrap());
        Self {
            m,
            world,
            craft: Aircraft::new(spawn),
            shots: vec![],
            view: Viewport::new(800, 500),
            texture: None,
            last: Instant::now(),
            clock: 0.,
            gun: 0.,
            rocket: 0.,
            paused: false,
            aim: Point::new(88., 48., 27.),
            camera: Point::new(72., 52., 0.),
            wood_start,
            flashes: vec![],
            step_ms: 0.,
        }
    }
    fn step(&mut self, input: FlightInput, gun: bool, rocket: bool, incendiary: bool) {
        let before = Instant::now();
        let dt = 1. / 60.;
        self.craft.step(input, &self.world, &self.m, dt);
        self.gun = (self.gun - dt).max(0.);
        self.rocket = (self.rocket - dt).max(0.);
        if (rocket || incendiary) && self.rocket == 0. {
            self.shots.push(Projectile::aimed(
                self.craft.at,
                self.aim,
                130.,
                if incendiary { 2.5 } else { 8. },
                if incendiary { 450. } else { 11000. },
                if incendiary { 1100. } else { 750. },
                self.m.id("fire"),
            ));
            self.rocket = 0.55;
        } else if gun && self.gun == 0. {
            self.shots.push(Projectile::aimed(
                self.craft.at,
                self.aim,
                220.,
                1.2,
                1400.,
                500.,
                None,
            ));
            self.gun = 0.10;
        }
        let mut kept = Vec::new();
        for mut shot in self.shots.drain(..) {
            if shot.step(&mut self.world, &self.m, dt) {
                self.flashes.push((shot.at, 14));
            } else {
                kept.push(shot);
            }
        }
        self.shots = kept;
        self.world.step(&self.m);
        for e in &self.world.explosions {
            self.flashes.push((e.at, 22));
        }
        for (_, age) in &mut self.flashes {
            *age = age.saturating_sub(1);
        }
        self.flashes.retain(|(_, age)| *age > 0);
        let target = Point::new(
            self.craft.at.x * 0.35 + 72. * 0.65,
            self.craft.at.y * 0.35 + 52. * 0.65,
            0.,
        );
        self.camera = self.camera.lerp(target, 0.035);
        self.step_ms = before.elapsed().as_secs_f32() * 1000.;
    }
    fn draw_pixels(&mut self) {
        let cells = helicopter(self.world.frame);
        let instances = [Instance {
            at: self.craft.at,
            angle: self.craft.heading,
            cells: &cells,
            shadow_radius: 8.,
        }];
        let mut tracers: Vec<_> = self.shots.iter().map(|s| s.at).collect();
        for (p, age) in &self.flashes {
            if *age > 8 {
                tracers.push(*p);
            }
        }
        self.view
            .draw(&self.world, &self.m, self.camera, &instances, &tracers);
    }
    fn verify() {
        let mut t = Self::new();
        let intact = t.world.count(t.m.id("weathered_wood").unwrap());
        for _ in 0..60 {
            t.world.step(&t.m);
        }
        assert_eq!(
            intact,
            t.world.count(t.m.id("weathered_wood").unwrap()),
            "unlit building must remain intact"
        );
        t.draw_pixels();
        grave_engine::headless::write_png(
            Path::new("shots/flight-test-intact.png"),
            t.view.width,
            t.view.height,
            &t.view.pixels,
            2,
        )
        .unwrap();
        let wood = t.world.count(t.m.id("weathered_wood").unwrap());
        let high = Point::new(55., 48., 35.);
        let far = Point::new(125., 48., 35.);
        assert!(
            t.world.raycast(high, far, &t.m).is_none(),
            "high flight clears the hangar"
        );
        assert!(
            t.world
                .raycast(Point::new(55., 48., 14.), Point::new(125., 48., 14.), &t.m)
                .is_some(),
            "low flight meets the wall"
        );
        t.world.explode(Explosion {
            at: Point::new(87., 47., 27.),
            radius: 11.,
            power: 18000.,
            fire: t.m.id("fire"),
        });
        let mut burning = false;
        let burn_started = Instant::now();
        for frame in 0..1800 {
            if frame < 90 && frame % 20 == 0 {
                t.world.heat(Point::new(86., 47., 26.), 8., 1050.);
            }
            t.world.step(&t.m);
            burning |= t.world.sections.iter().any(|s| {
                s.world
                    .flags
                    .iter()
                    .any(|f| f & grave_engine::world::BURNING != 0)
            });
            if frame == 100 {
                t.draw_pixels();
                grave_engine::headless::write_png(
                    Path::new("shots/flight-test-burning.png"),
                    t.view.width,
                    t.view.height,
                    &t.view.pixels,
                    2,
                )
                .unwrap();
            }
        }
        t.draw_pixels();
        grave_engine::headless::write_png(
            Path::new("shots/flight-test-damaged.png"),
            t.view.width,
            t.view.height,
            &t.view.pixels,
            2,
        )
        .unwrap();
        let after = t.world.count(t.m.id("weathered_wood").unwrap());
        assert!(burning);
        assert!(after < wood, "wood burns and roof damage is real");
        t.step_ms = burn_started.elapsed().as_secs_f32() * 1000. / 1800.;
        println!(
            "Flight test verified: altitude clearance, low collision, material ignition; timber {wood}->{after}; last step {:.1}ms",
            t.step_ms
        );
    }
}
fn helicopter(frame: u32) -> Vec<VisualCell> {
    let mut v = Vec::new();
    let mut add = |x: f32, y: f32, z: f32, c: [u8; 3]| {
        v.push(VisualCell {
            at: Point::new(x, y, z),
            color: c,
        })
    };
    for x in -7i32..=7 {
        for y in -3i32..=3 {
            for z in 0..4 {
                if (x as f32 / 8.).powi(2)
                    + (y as f32 / 4.).powi(2)
                    + ((z as f32 - 1.5) / 2.8).powi(2)
                    < 1.2
                {
                    let color = if x > 2 && z >= 2 {
                        [65, 134, 153]
                    } else if z == 3 {
                        [117, 132, 78]
                    } else {
                        [68, 85, 49]
                    };
                    add(x as f32, y as f32, z as f32, color);
                }
            }
        }
    }
    for x in -18..-6 {
        add(x as f32, 0., 2., [87, 105, 59]);
        add(x as f32, 0., 3., [105, 120, 67]);
    }
    for z in 2..8 {
        add(-17., 0., z as f32, [110, 124, 70]);
    }
    for side in [-4., 4.] {
        for x in -5..6 {
            add(x as f32, side, -1., [55, 61, 62]);
        }
        for x in [-4., 4.] {
            add(x, side, 0., [85, 91, 84]);
        }
    }
    for z in 4..7 {
        add(0., 0., z as f32, [55, 60, 56]);
    }
    let a = frame as f32 * 0.73;
    let (s, c) = a.sin_cos();
    for n in -15..16 {
        let f = n as f32;
        add(f * c, f * s, 7., [90, 94, 86]);
        add(-f * s, f * c, 7., [118, 120, 106]);
    }
    add(0., 0., 7., [174, 174, 139]);
    v
}
impl eframe::App for FlightTest {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ui.visuals_mut().override_text_color = Some(Color32::from_rgb(235, 228, 203));
        ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::from_rgb(48, 57, 51);
        ui.visuals_mut().widgets.hovered.weak_bg_fill = Color32::from_rgb(69, 84, 72);
        ui.visuals_mut().widgets.active.weak_bg_fill = Color32::from_rgb(78, 111, 84);
        let elapsed = self.last.elapsed().as_secs_f32().min(0.1);
        self.last = Instant::now();
        if ctx.input(|i| i.key_pressed(egui::Key::R)) {
            *self = Self::new();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::P)) {
            self.paused = !self.paused;
        }
        ui.horizontal(|ui| {
            ui.heading("DUST HAWK");
            ui.label("FLIGHT & DESTRUCTION LAB");
            if ui
                .button(if self.paused { "Resume" } else { "Pause" })
                .clicked()
            {
                self.paused = !self.paused;
            }
            if ui.button("Reset compound").clicked() {
                *self = Self::new();
            }
            if ui.button("Ignite fuel tank").clicked() {
                self.world.heat(Point::new(44., 33., 12.), 5., 1100.);
            }
            if ui.button("Blast hangar roof").clicked() {
                self.world.explode(Explosion {
                    at: Point::new(87., 47., 27.),
                    radius: 9.,
                    power: 14000.,
                    fire: self.m.id("fire"),
                });
            }
        });
        ui.label("WASD / arrows: fly  •  E: climb  •  Q: descend / land  •  Mouse: aim  •  Hold left click: gun  •  Space / right click: rocket  •  F: incendiary  •  Wheel: zoom  •  R: reset");
        let remains = self.world.count(self.m.id("weathered_wood").unwrap());
        ui.horizontal(|ui| {
            ui.colored_label(
                Color32::from_rgb(209, 201, 151),
                format!(
                    "ALTITUDE {:.1} m   |   SPEED {:.1} m/s   |   TIMBER {}%   |   {:.1} ms / step",
                    self.craft.at.z * 0.1,
                    self.craft.velocity.x.hypot(self.craft.velocity.y) * 0.1,
                    remains * 100 / self.wood_start.max(1),
                    self.step_ms
                ),
            );
            if self.craft.blocked {
                ui.colored_label(
                    Color32::from_rgb(255, 141, 95),
                    "OBSTACLE — climb to clear the roof",
                );
            }
        });
        let available = ui.available_rect_before_wrap();
        let scale = (available.width() / 800.).min(available.height() / 500.);
        let rect =
            Rect::from_center_size(available.center(), Vec2::new(800. * scale, 500. * scale));
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        if let Some(pos) = ctx
            .input(|i| i.pointer.hover_pos())
            .filter(|p| rect.contains(*p))
        {
            let p = [(pos.x - rect.left()) / scale, (pos.y - rect.top()) / scale];
            self.aim = self.view.projection.pick(p, &self.world, &self.m);
        }
        if response.hovered() {
            self.view.projection.scale = (self.view.projection.scale
                + ctx.input(|i| i.smooth_scroll_delta.y) * 0.006)
                .clamp(1.7, 4.);
        }
        let focused = ctx.input(|i| i.focused);
        if focused && !self.paused {
            let (mx, my, climb, gun, rocket, incendiary) = ctx.input(|i| {
                let axis = |a, b| i.key_down(a) as i32 - i.key_down(b) as i32;
                (
                    (axis(egui::Key::D, egui::Key::A)
                        + axis(egui::Key::ArrowRight, egui::Key::ArrowLeft))
                        as f32,
                    (axis(egui::Key::S, egui::Key::W)
                        + axis(egui::Key::ArrowDown, egui::Key::ArrowUp))
                        as f32,
                    axis(egui::Key::E, egui::Key::Q) as f32,
                    response.hovered() && i.pointer.primary_down(),
                    i.key_down(egui::Key::Space)
                        || (response.hovered() && i.pointer.secondary_down()),
                    i.key_down(egui::Key::F),
                )
            });
            let input = FlightInput {
                movement: [(mx + my) * 0.7071, (my - mx) * 0.7071],
                climb,
                heading: (self.aim.y - self.craft.at.y).atan2(self.aim.x - self.craft.at.x),
            };
            self.clock = (self.clock + elapsed).min(0.05);
            let mut n = 0;
            while self.clock >= 1. / 60. && n < 2 {
                self.step(input, gun, rocket, incendiary);
                self.clock -= 1. / 60.;
                n += 1;
            }
        } else {
            self.clock = 0.;
        }
        self.draw_pixels();
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [self.view.width, self.view.height],
            &self.view.pixels,
        );
        if let Some(t) = &mut self.texture {
            t.set(image, egui::TextureOptions::NEAREST);
        } else {
            self.texture =
                Some(ctx.load_texture("flight-world", image, egui::TextureOptions::NEAREST));
        }
        let painter = ui.painter();
        painter.image(
            self.texture.as_ref().unwrap().id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
            Color32::WHITE,
        );
        let cross = self.view.projection.project(self.aim);
        let c = Pos2::new(
            rect.left() + cross[0] * scale,
            rect.top() + cross[1] * scale,
        );
        let color = Color32::from_rgb(255, 194, 87);
        painter.circle_stroke(c, 7., Stroke::new(1., color));
        painter.line_segment(
            [c - Vec2::new(11., 0.), c + Vec2::new(11., 0.)],
            Stroke::new(1., color),
        );
        painter.line_segment(
            [c - Vec2::new(0., 11.), c + Vec2::new(0., 11.)],
            Stroke::new(1., color),
        );
        if self.paused {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "PAUSED",
                egui::FontId::proportional(30.),
                Color32::WHITE,
            );
        }
        ctx.request_repaint();
    }
}
fn main() -> eframe::Result {
    if std::env::args().any(|s| s == "--verify") {
        FlightTest::verify();
        return Ok(());
    }
    eframe::run_native(
        "Dust Hawk — Grave Engine Flight Lab",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size(Vec2::new(1400., 950.)),
            ..Default::default()
        },
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(FlightTest::new()))
        }),
    )
}
