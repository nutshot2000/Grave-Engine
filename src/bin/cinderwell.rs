//! Cinderwell, a short spellcraft run built on Grave Engine.
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Vec2};
use grave_engine::{
    headless,
    input::Input,
    material::{Materials, State},
    physics::Controls,
    render::{Renderer, View},
    scene,
    sim::Sim,
    sound::Player,
    trigger::Event,
};
use std::{path::Path, time::Instant};

const INPUT: &str = include_str!("../../assets/input/cinderwell.input");
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Stage {
    #[default]
    First,
    Workshop,
    Glassworks,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Upgrade {
    Acid,
    Reach,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Element {
    #[default]
    Ember,
    Frost,
    Acid,
}

#[derive(Clone, Copy, Default)]
struct Wand {
    element: Element,
    burst: bool,
    upgrade: Option<Upgrade>,
}

impl Wand {
    fn cycle_element(&mut self) {
        self.element = match self.element {
            Element::Ember => Element::Frost,
            Element::Frost if self.upgrade == Some(Upgrade::Acid) => Element::Acid,
            _ => Element::Ember,
        };
    }

    fn label(self) -> &'static str {
        match self.element {
            Element::Ember => "Ember",
            Element::Frost => "Frost",
            Element::Acid => "Acid",
        }
    }

    fn color(self) -> Color32 {
        match self.element {
            Element::Ember => Color32::from_rgb(255, 155, 82),
            Element::Frost => Color32::from_rgb(125, 220, 255),
            Element::Acid => Color32::from_rgb(167, 231, 106),
        }
    }

    fn cast(self, sim: &mut Sim, mats: &Materials, origin: (f32, f32), aim: Vec2) -> (f32, f32) {
        let mut end = origin;
        let mut direct_hit = None;
        let actors = sim.physics.actor_status();
        let range = if self.upgrade == Some(Upgrade::Reach) {
            136
        } else {
            88
        };
        for distance in (6..=range).map(|n| n as f32) {
            let (x, y) = (origin.0 + aim.x * distance, origin.1 + aim.y * distance);
            if x < 1. || y < 1. || x >= (sim.world.w - 1) as f32 || y >= (sim.world.h - 1) as f32 {
                break;
            }
            end = (x, y);
            let i = sim.world.idx(x as usize, y as usize);
            let state = mats.get(sim.world.mat[i]).state;
            if !sim.world.is_owned(i)
                && matches!(state, State::Solid | State::Powder | State::Liquid)
            {
                break;
            }
            direct_hit = actors.iter().position(|a| {
                a.creature.is_some()
                    && !a.dead
                    && (a.x - x).abs() < 5.
                    && (a.y - y).abs() < a.body_height / 2.
            });
            if direct_hit.is_some() {
                break;
            }
        }
        let radius = if self.burst { 11 } else { 4 };
        let (x, y) = end;
        if self.burst {
            sim.dig_circle(mats, x, y, 7., 5200.);
        }
        let deposit = match self.element {
            Element::Ember => {
                sim.world.heat_circle(x as i32, y as i32, radius, 630.);
                Some(("fire", 3))
            }
            Element::Frost => {
                sim.world.heat_circle(x as i32, y as i32, radius, -140.);
                None
            }
            Element::Acid => Some(("acid", if self.burst { 6 } else { 3 })),
        };
        if let Some((material, spread)) = deposit {
            let material = mats.id(material).unwrap();
            for dy in -spread..=spread {
                for dx in -spread..=spread {
                    let (cx, cy) = (x as i32 + dx, y as i32 + dy);
                    if sim.world.in_bounds(cx, cy) && dx * dx + dy * dy <= spread * spread {
                        let i = sim.world.idx(cx as usize, cy as usize);
                        if sim.world.mat[i] == 0 {
                            sim.world.set(mats, cx as usize, cy as usize, material);
                        }
                    }
                }
            }
        }
        for (n, a) in actors.iter().enumerate() {
            if a.creature.is_some()
                && (a.x - x).abs() < radius as f32 + 4.
                && (a.y - y).abs() < radius as f32 + a.body_height / 2.
                && !a.dead
            {
                if self.burst || direct_hit == Some(n) {
                    sim.physics
                        .damage_actor(n, if self.burst { 30. } else { 17. }, "wand strike");
                }
            }
        }
        end
    }
}

fn load(mats: &Materials, stage: Stage) -> Sim {
    let (path, fallback) = match stage {
        Stage::First => (
            "scenes/cinderwell.txt",
            include_str!("../../scenes/cinderwell.txt"),
        ),
        Stage::Workshop => (
            "scenes/cinderwell-workshop.txt",
            include_str!("../../scenes/cinderwell-workshop.txt"),
        ),
        Stage::Glassworks => (
            "scenes/cinderwell-glassworks.txt",
            include_str!("../../scenes/cinderwell-glassworks.txt"),
        ),
    };
    let text = std::fs::read_to_string(path).unwrap_or_else(|_| fallback.into());
    let (sim, problems) = scene::load(&text, mats, Some(Path::new("scenes"))).into_sim(mats);
    assert!(problems.is_empty(), "{problems:?}");
    sim
}

struct Game {
    mats: Materials,
    sim: Sim,
    input: Input,
    wand: Wand,
    renderer: Renderer,
    sound: Player,
    texture: Option<egui::TextureHandle>,
    last: Instant,
    clock: f32,
    cooldown: u32,
    pending_cast: bool,
    trail: Option<((f32, f32), (f32, f32), Wand)>,
    aim: Vec2,
    paused: bool,
    won: bool,
    stage: Stage,
}

impl Game {
    fn new() -> Self {
        let mats = Materials::load_dir("materials");
        let sim = load(&mats, Stage::First);
        Self {
            mats,
            sim,
            input: Input::load_or("assets/input/cinderwell.input", INPUT).expect("controls"),
            wand: Wand::default(),
            renderer: Renderer::default(),
            sound: Player::new(),
            texture: None,
            last: Instant::now(),
            clock: 0.,
            cooldown: 0,
            pending_cast: false,
            trail: None,
            aim: Vec2::X,
            paused: false,
            won: false,
            stage: Stage::First,
        }
    }

    fn tick(&mut self, controls: Controls) {
        let p = self.sim.physics.player_status().unwrap();
        if p.dead || self.won {
            return;
        }
        self.cooldown = self.cooldown.saturating_sub(1);
        if self.cooldown == 0 && (self.pending_cast || self.input.down("cast")) {
            self.pending_cast = false;
            let origin = (p.x, p.y);
            let end = self.wand.cast(&mut self.sim, &self.mats, origin, self.aim);
            self.trail = Some((origin, end, self.wand));
            self.cooldown = if self.wand.burst { 36 } else { 17 };
        }
        self.sim.physics.set_controls(controls);
        self.sim.step(&self.mats);
        let reached_lift = self
            .sim
            .events
            .drain(..)
            .any(|event| matches!(event, Event::Finish));
        if reached_lift && !self.sim.physics.player_status().unwrap().dead {
            match self.stage {
                Stage::First => self.enter(Stage::Workshop),
                Stage::Glassworks => self.won = true,
                Stage::Workshop => {}
            }
        }
    }

    fn enter(&mut self, stage: Stage) {
        self.stage = stage;
        self.sim = load(&self.mats, stage);
        self.cooldown = 0;
        self.pending_cast = false;
        self.trail = None;
        self.clock = 0.;
    }

    fn choose(&mut self, upgrade: Upgrade) {
        if self.stage != Stage::Workshop {
            return;
        }
        self.wand.upgrade = Some(upgrade);
        if upgrade == Upgrade::Acid {
            self.wand.element = Element::Acid;
        }
        self.enter(Stage::Glassworks);
    }

    fn workshop(&mut self, ui: &mut egui::Ui) {
        let (acid, reach) = if self.input.controller().is_some() {
            ("X", "Y")
        } else {
            ("1", "2")
        };
        let mut choice = None;
        egui::Window::new("Lift Workshop")
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.set_min_width(340.);
                ui.label("Health and lift refilled. Choose one upgrade for the Glassworks.");
                ui.add_space(10.);
                if ui.button(format!("{acid}   Acid spell")).clicked() {
                    choice = Some(Upgrade::Acid);
                }
                ui.label("Adds Acid to your wand. Burst spreads it over a larger area.");
                ui.add_space(8.);
                if ui.button(format!("{reach}   Reach modifier")).clicked() {
                    choice = Some(Upgrade::Reach);
                }
                ui.label("Extends every cast, including Burst.");
            });
        if self.input.pressed("choose_acid") {
            choice = Some(Upgrade::Acid);
        }
        if self.input.pressed("choose_reach") {
            choice = Some(Upgrade::Reach);
        }
        if let Some(upgrade) = choice {
            self.choose(upgrade);
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui, dt: f32) {
        let p = self.sim.physics.player_status().unwrap();
        let rect = ui.max_rect();
        let scale = (rect.width() / 204.).min(rect.height() / 130.);
        if scale <= 0. {
            return;
        }
        let w = (rect.width() / scale).ceil().min(self.sim.world.w as f32) as usize;
        let h = (rect.height() / scale).ceil().min(self.sim.world.h as f32) as usize;
        let left = (p.x + 13. - w as f32 / 2.).clamp(0., (self.sim.world.w - w) as f32) as usize;
        let top = (p.y - h as f32 * 0.58).clamp(0., (self.sim.world.h - h) as f32) as usize;
        let corner = rect.center() - Vec2::new(w as f32, h as f32) * scale / 2.;
        let point = |x: f32, y: f32| corner + Vec2::new(x - left as f32, y - top as f32) * scale;
        let stick = Vec2::new(self.input.axis("aim_x"), self.input.axis("aim_y"));
        let direction = if stick.length() > 0.2 {
            stick
        } else if self.input.controller().is_some() {
            Vec2::new(p.facing, 0.)
        } else {
            ui.ctx()
                .pointer_hover_pos()
                .map_or(self.aim, |m| m - point(p.x, p.y))
        };
        if direction.length() > 0.1 {
            self.aim = direction.normalized();
        }

        if self.stage != Stage::Workshop && !self.paused && !self.won && !p.dead {
            self.pending_cast |= self.input.pressed("cast");
            self.clock += dt;
            while self.clock >= 1. / 60. {
                self.clock -= 1. / 60.;
                let stage = self.stage;
                self.tick(Controls {
                    walk: self.input.axis("walk"),
                    thrust: self.input.down("thrust"),
                    ..Default::default()
                });
                if stage != self.stage {
                    return;
                }
            }
        }
        self.sound.listen(
            &mut self.sim,
            &self.mats,
            (left as f32, top as f32, w as f32, h as f32),
            if self.paused || self.won || p.dead {
                0.
            } else {
                1.
            },
        );
        self.renderer.light(&self.sim);
        let pixels = self.renderer.draw_window(
            &self.sim.world,
            &self.mats,
            View::default(),
            (left, top, w, h),
        );
        let image = egui::ColorImage::from_rgba_unmultiplied([w, h], pixels);
        let painter = ui.painter();
        painter.rect_filled(rect, 0., Color32::from_rgb(12, 13, 20));
        let texture = match &mut self.texture {
            Some(texture) => {
                texture.set(image, egui::TextureOptions::NEAREST);
                texture.id()
            }
            slot => {
                let texture =
                    ui.ctx()
                        .load_texture("cinderwell", image, egui::TextureOptions::NEAREST);
                let id = texture.id();
                *slot = Some(texture);
                id
            }
        };
        painter.image(
            texture,
            Rect::from_min_size(corner, Vec2::new(w as f32, h as f32) * scale),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
            Color32::WHITE,
        );
        let p = self.sim.physics.player_status().unwrap();
        if !p.dead {
            let at = point(p.x, p.y);
            painter.circle_filled(
                at - Vec2::new(0., 3. * scale),
                3. * scale,
                Color32::from_rgb(220, 204, 251),
            );
            painter.line_segment([at, at + self.aim * scale * 6.], (2., self.wand.color()));
            if p.boosting {
                painter.circle_filled(
                    point(p.x, p.y + 10.),
                    2. * scale,
                    Color32::from_rgb(182, 143, 255),
                );
            }
        }
        if let Some((from, to, wand)) = self
            .trail
            .filter(|(_, _, wand)| self.cooldown > if wand.burst { 28 } else { 12 })
        {
            painter.line_segment(
                [point(from.0, from.1), point(to.0, to.1)],
                (2., wand.color()),
            );
            painter.circle_stroke(
                point(to.0, to.1),
                scale * if wand.burst { 10. } else { 3. },
                (1.5, wand.color()),
            );
        }
        for a in self
            .sim
            .physics
            .actor_status()
            .into_iter()
            .filter(|a| a.creature.is_some() && !a.dead)
        {
            let creature = a.creature.as_ref().unwrap();
            let at = point(a.x, a.y - a.body_height / 2. - 4.);
            painter.circle_filled(
                point(a.x, a.y),
                scale * 1.15,
                Color32::from_rgb(255, 122, 93),
            );
            let bar = Rect::from_min_size(at - Vec2::new(15., 0.), Vec2::new(30., 4.));
            painter.rect_filled(bar, 2., Color32::from_black_alpha(200));
            painter.rect_filled(
                Rect::from_min_size(
                    bar.min,
                    Vec2::new(30. * (a.health / creature.species.health).clamp(0., 1.), 4.),
                ),
                2.,
                Color32::LIGHT_RED,
            );
        }
        let active = self.wand.label();
        let name = if self.stage == Stage::Glassworks {
            "GLASSWORKS"
        } else {
            "CINDERWELL"
        };
        let reach = if self.wand.upgrade == Some(Upgrade::Reach) {
            " + Reach"
        } else {
            ""
        };
        let shape = if self.wand.burst { "Burst" } else { "Bolt" };
        let fuel = p.jet_fuel.map_or(0., |(fuel, max)| fuel / max);
        let font = FontId::proportional(17.);
        painter.rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(rect.width(), 57.)),
            0.,
            Color32::from_black_alpha(220),
        );
        painter.text(
            rect.min + Vec2::new(14., 8.),
            Align2::LEFT_TOP,
            format!(
                "{name}   •   HP {:.0}   •   Lift {:.0}%   •   {active} + {shape}{reach}",
                p.health.max(0.),
                100. * fuel
            ),
            font.clone(),
            self.wand.color(),
        );
        let controls = if self.input.controller().is_some() {
            "Left stick move   A levitate   Right stick aim   RT cast   LB element   RB burst"
        } else {
            "A/D move   Space levitate   Mouse aim   Left click cast   Q element   Right click burst"
        };
        painter.text(
            rect.min + Vec2::new(14., 32.),
            Align2::LEFT_TOP,
            controls,
            FontId::proportional(14.),
            Color32::LIGHT_GRAY,
        );
        let hint = if self.stage == Stage::Workshop {
            "Choose one upgrade to descend."
        } else if self.stage == Stage::Glassworks {
            "Use the ledges to cross. The powder cache can clear the lower gallery."
        } else if p.x < 104. {
            "Reach the lift to the right. Try Ember on the oil below."
        } else if p.x < 279. {
            "Burst breaks sandstone. Breach the orange plug from a safe distance."
        } else {
            "Frost Burst freezes more water. Levitate up to the green lift."
        };
        painter.rect_filled(
            Rect::from_min_max(rect.left_bottom() - Vec2::new(0., 30.), rect.right_bottom()),
            0.,
            Color32::from_black_alpha(210),
        );
        painter.text(
            rect.center_bottom() - Vec2::new(0., 6.),
            Align2::CENTER_BOTTOM,
            hint,
            font,
            Color32::WHITE,
        );
        if self.won || p.dead || self.paused {
            painter.rect_filled(rect, 0., Color32::from_black_alpha(185));
            let label = if self.won {
                "Glassworks cleared"
            } else if p.dead {
                p.hurt_by.unwrap_or("You fell in Cinderwell")
            } else {
                "Paused"
            };
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(32.),
                Color32::WHITE,
            );
            painter.text(
                rect.center() + Vec2::new(0., 45.),
                Align2::CENTER_CENTER,
                if self.paused {
                    "Esc / Start to resume   •   R / Back to restart"
                } else {
                    "R / Back to try again"
                },
                FontId::proportional(18.),
                Color32::LIGHT_GRAY,
            );
        }
    }
}

impl eframe::App for Game {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.input.update(&ctx);
        let dt = self.last.elapsed().as_secs_f32().min(0.1);
        self.last = Instant::now();
        if self.input.pressed("restart") {
            self.enter(Stage::First);
            self.won = false;
            self.paused = false;
            self.wand = Wand::default();
        }
        if self.stage != Stage::Workshop
            && self.input.pressed("pause")
            && !self.won
            && !self.sim.physics.player_status().unwrap().dead
        {
            self.paused = !self.paused;
        }
        if self.input.pressed("element") {
            self.wand.cycle_element();
        }
        if self.input.pressed("burst") {
            self.wand.burst = !self.wand.burst;
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.draw(ui, dt));
        if self.stage == Stage::Workshop {
            self.workshop(ui);
        }
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result {
    std::env::set_current_dir(env!("CARGO_MANIFEST_DIR")).expect("project directory");
    if let Some((stage, filename)) = std::env::args().find_map(|arg| match arg.as_str() {
        "--shot" => Some((Stage::First, "shots/cinderwell.png")),
        "--shot-workshop" => Some((Stage::Workshop, "shots/cinderwell-workshop.png")),
        "--shot-glassworks" => Some((Stage::Glassworks, "shots/cinderwell-glassworks.png")),
        _ => None,
    }) {
        let mats = Materials::load_dir("materials");
        let mut sim = load(&mats, stage);
        for _ in 0..35 {
            sim.step(&mats);
        }
        let mut renderer = Renderer::default();
        renderer.light(&sim);
        std::fs::create_dir_all("shots").unwrap();
        headless::shot(
            &sim.world,
            &mats,
            &mut renderer,
            Path::new(filename),
            3,
            false,
        )
        .unwrap();
        return Ok(());
    }
    eframe::run_native(
        "Cinderwell",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1140., 690.]),
            ..Default::default()
        },
        Box::new(|_| Ok(Box::new(Game::new()))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_upgrades_carry_into_a_completable_second_cave() {
        for upgrade in [Upgrade::Acid, Upgrade::Reach] {
            let mut game = Game::new();
            game.sound = Player::silent();
            game.wand.burst = true;
            fly_right(&mut game, |p| if p.x > 330. { 117. } else { 115. });
            assert!(game.stage == Stage::Workshop && !game.won);
            game.choose(upgrade);
            assert!(game.stage == Stage::Glassworks);
            assert_eq!(game.wand.upgrade, Some(upgrade));
            assert!(game.wand.burst);
            let acid = game.mats.id("acid").unwrap();
            match upgrade {
                Upgrade::Acid => {
                    assert!(game.wand.element == Element::Acid);
                    game.wand
                        .cast(&mut game.sim, &game.mats, (55., 140.), Vec2::X);
                    assert!(game.sim.world.mat.contains(&acid));
                }
                Upgrade::Reach => {
                    let origin = (103., 132.);
                    let end = game.wand.cast(&mut game.sim, &game.mats, origin, Vec2::X);
                    assert!(end.0 - origin.0 > 88.);
                }
            }
            fly_right(&mut game, |p| if p.x > 333. { 102. } else { 112. });
            assert!(game.won);
        }
    }

    fn fly_right(game: &mut Game, altitude: impl Fn(&grave_engine::physics::ActorStatus) -> f32) {
        let stage = game.stage;
        for _ in 0..720 {
            let p = game.sim.physics.player_status().unwrap();
            assert!(!p.dead, "died at ({:.0}, {:.0}): {:?}", p.x, p.y, p.hurt_by);
            game.tick(Controls {
                walk: 1.,
                thrust: p.y > altitude(&p),
                ..Default::default()
            });
            if stage != game.stage || game.won {
                return;
            }
        }
        let p = game.sim.physics.player_status().unwrap();
        panic!("lift not reached at ({:.0}, {:.0})", p.x, p.y);
    }

    #[test]
    fn spells_ignite_oil_freeze_water_and_release_lava() {
        let mats = Materials::load_dir("materials");
        let mut sim = load(&mats, Stage::First);
        assert_eq!(sim.physics.actor_count(), 4);
        let oil = mats.id("oil").unwrap();
        Wand::default().cast(&mut sim, &mats, (112., 130.), Vec2::Y);
        for _ in 0..8 {
            sim.step(&mats);
        }
        assert!(
            sim.world
                .mat
                .iter()
                .enumerate()
                .any(|(i, &id)| id == oil && sim.world.is_burning(i))
        );

        let mut sim = load(&mats, Stage::First);
        let frost = Wand {
            element: Element::Frost,
            burst: true,
            ..Default::default()
        };
        frost.cast(&mut sim, &mats, (328., 132.), Vec2::Y);
        for _ in 0..3 {
            sim.step(&mats);
        }
        let ice = mats.id("ice").unwrap();
        assert!(sim.world.mat.contains(&ice));

        let mut sim = load(&mats, Stage::First);
        let plug = sim.world.idx(246, 91);
        Wand::default().cast(&mut sim, &mats, (246., 126.), -Vec2::Y);
        assert_eq!(sim.world.mat[plug], mats.id("sandstone").unwrap());
        frost.cast(&mut sim, &mats, (246., 126.), -Vec2::Y);
        assert_eq!(sim.world.mat[plug], 0);
        for _ in 0..70 {
            sim.step(&mats);
        }
        let lava = mats.id("lava").unwrap();
        assert!((100..170).any(|y| (230..267).any(|x| sim.world.mat[sim.world.idx(x, y)] == lava)));
    }

    #[test]
    fn player_can_levitate_across_the_cave_and_reach_the_lift() {
        let mats = Materials::load_dir("materials");
        let mut sim = load(&mats, Stage::First);
        let start = sim.physics.player_status().unwrap();
        let mut rose = false;
        for _ in 0..600 {
            let player = sim.physics.player_status().unwrap();
            let thrust = player.y > if player.x > 330. { 117. } else { 115. };
            sim.physics.set_controls(Controls {
                walk: 1.,
                thrust,
                ..Default::default()
            });
            sim.step(&mats);
            let player = sim.physics.player_status().unwrap();
            assert!(
                !player.dead,
                "died at {}, {}: {:?}",
                player.x, player.y, player.hurt_by
            );
            rose |= player.y < start.y - 18.;
            if sim.events.drain(..).any(|e| matches!(e, Event::Finish)) {
                assert!(rose);
                return;
            }
        }
        let p = sim.physics.player_status().unwrap();
        panic!("lift not reached: ({}, {}) fuel {:?}", p.x, p.y, p.jet_fuel);
    }
}
