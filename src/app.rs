//! The sandbox window: paint materials, press Play, watch things burn.

use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Vec2};
use grave_engine::material::{AIR, MatId, Materials};
use grave_engine::render::{Renderer, View};
use grave_engine::import;
use grave_engine::physics::BodySpec;
use grave_engine::scene;
use grave_engine::sim::{Sim, Snapshot};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

const SIM_HZ: f64 = 60.0;

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Paint,
    Erase,
    Heat,
    Cool,
    /// Drag a box: everything solid inside becomes a loose object.
    Object,
}

struct Sandbox {
    mats: Materials,
    sim: Sim,
    /// How things were when Play was last pressed. Reset goes back here.
    snapshot: Snapshot,
    ran_since_snapshot: bool,
    playing: bool,
    speed: f32,
    view: View,
    renderer: Renderer,
    texture: Option<TextureHandle>,
    tool: Tool,
    brush_mat: MatId,
    brush_size: i32,
    last_paint: Option<(i32, i32)>,
    /// Where an Object-tool drag started.
    box_start: Option<(i32, i32)>,
    /// Dropped files land as fixed scenery instead of loose objects.
    drop_static: bool,
    hover: Option<(usize, usize)>,
    scene_path: String,
    scene_stamp: Option<SystemTime>,
    messages: Vec<(Instant, String)>,
    last_file_check: Instant,
    sim_clock: f64,
    last_frame: Instant,
    fps: f32,
    sim_ms: f32,
}

pub fn run(materials: PathBuf, scene_file: Option<PathBuf>) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Grave Engine - Sandbox")
            .with_inner_size([1400.0, 860.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Grave Engine",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.global_style_mut(|s| {
                s.spacing.button_padding = Vec2::new(10.0, 6.0);
                s.spacing.item_spacing = Vec2::new(8.0, 6.0);
                for (style, font) in s.text_styles.iter_mut() {
                    if matches!(style, egui::TextStyle::Body | egui::TextStyle::Button) {
                        font.size = 16.0;
                    }
                }
            });
            Ok(Box::new(Sandbox::new(materials, scene_file)))
        }),
    )
    .map_err(|e| e.to_string())
}

fn stamp(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn color32(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

impl Sandbox {
    fn new(materials: PathBuf, scene_file: Option<PathBuf>) -> Self {
        let mats = Materials::load_dir(&materials);
        let mut sim = Sim::empty(scene::DEFAULT_W, scene::DEFAULT_H, &mats);
        let brush_mat = mats.id("sand").unwrap_or(AIR);
        let mut app = Sandbox {
            snapshot: sim.snapshot(),
            sim,
            mats,
            ran_since_snapshot: false,
            playing: false,
            speed: 1.0,
            view: View::default(),
            renderer: Renderer::default(),
            texture: None,
            tool: Tool::Paint,
            brush_mat,
            brush_size: 6,
            last_paint: None,
            box_start: None,
            drop_static: false,
            hover: None,
            scene_path: "scenes/my_scene.txt".into(),
            scene_stamp: None,
            messages: Vec::new(),
            last_file_check: Instant::now(),
            sim_clock: 0.0,
            last_frame: Instant::now(),
            fps: 60.0,
            sim_ms: 0.0,
        };
        for p in app.mats.problems.clone() {
            app.say(p);
        }
        let start = scene_file.or_else(|| {
            let p = PathBuf::from("scenes/objects.txt");
            p.exists().then_some(p)
        });
        if let Some(p) = start {
            app.scene_path = p.display().to_string();
            app.load_scene();
            app.scene_path = "scenes/my_scene.txt".into();
            app.scene_stamp = None;
        }
        app
    }

    fn say(&mut self, msg: impl Into<String>) {
        self.messages.push((Instant::now(), msg.into()));
        if self.messages.len() > 8 {
            self.messages.remove(0);
        }
    }

    fn load_scene(&mut self) {
        let path = PathBuf::from(&self.scene_path);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let (sim, problems) = scene::load(&text, &self.mats, path.parent()).into_sim(&self.mats);
                self.sim = sim;
                self.snapshot = self.sim.snapshot();
                self.ran_since_snapshot = false;
                self.playing = false;
                self.scene_stamp = stamp(&path);
                self.say(format!("Loaded {}", path.display()));
                for p in problems {
                    self.say(p);
                }
            }
            Err(e) => self.say(format!("Couldn't load {}: {e}", path.display())),
        }
    }

    fn save_scene(&mut self) {
        let path = PathBuf::from(&self.scene_path);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let snap = self.sim.snapshot();
        match std::fs::write(&path, scene::save(&snap, &self.mats)) {
            Ok(()) => {
                self.scene_stamp = stamp(&path);
                self.say(format!("Saved {}", path.display()));
            }
            Err(e) => self.say(format!("Couldn't save {}: {e}", path.display())),
        }
    }

    /// Live reload: material files and the open scene file.
    fn check_files(&mut self) {
        if self.last_file_check.elapsed().as_secs_f32() < 0.25 {
            return;
        }
        self.last_file_check = Instant::now();
        if self.mats.changed_on_disk() {
            self.mats.reload();
            if self.mats.problems.is_empty() {
                self.say("Materials reloaded");
            } else {
                for p in self.mats.problems.clone() {
                    self.say(p);
                }
            }
        }
        if let Some(old) = self.scene_stamp {
            let path = PathBuf::from(&self.scene_path);
            if stamp(&path).is_some_and(|s| s != old) {
                self.say("Scene file changed on disk, reloading");
                self.load_scene();
            }
        }
    }

    fn take_snapshot_if_fresh(&mut self) {
        if !self.ran_since_snapshot {
            self.snapshot = self.sim.snapshot();
        }
    }

    fn play(&mut self) {
        self.snapshot = self.sim.snapshot();
        self.ran_since_snapshot = false;
        self.playing = true;
        self.sim_clock = 0.0;
    }

    fn step_once(&mut self) {
        self.take_snapshot_if_fresh();
        let t = Instant::now();
        self.sim.step(&self.mats);
        self.sim_ms = self.sim_ms * 0.9 + t.elapsed().as_secs_f32() * 1000.0 * 0.1;
        self.ran_since_snapshot = true;
    }

    fn reset(&mut self) {
        self.sim = Sim::restore(&self.snapshot, &self.mats);
        self.ran_since_snapshot = false;
        self.playing = false;
    }

    /// Add an image or Blender model, centred on the mouse (or the top middle).
    fn import_file(&mut self, path: &Path) {
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if ext == "txt" {
            self.scene_path = path.display().to_string();
            self.load_scene();
            return;
        }
        match import::load(path, &self.mats, None) {
            Ok((shape, warnings)) => {
                for w in warnings {
                    self.say(w);
                }
                let (cx, cy) = match self.hover {
                    Some((x, y)) => (x as f32, y as f32),
                    None => (self.sim.world.w as f32 / 2.0, shape.h as f32 / 2.0 + 4.0),
                };
                if self.drop_static {
                    let (x0, y0) = (cx as i32 - shape.w as i32 / 2, cy as i32 - shape.h as i32 / 2);
                    for y in 0..shape.h {
                        for x in 0..shape.w {
                            if let Some(id) = shape.cells[y * shape.w + x] {
                                let (wx, wy) = (x0 + x as i32, y0 + y as i32);
                                if self.sim.world.in_bounds(wx, wy) {
                                    self.sim.world.set(&self.mats, wx as usize, wy as usize, id);
                                }
                            }
                        }
                    }
                } else {
                    let spec = BodySpec::from_mats(shape.w, shape.h, &shape.cells, &self.mats, cx, cy);
                    self.sim.physics.add(&mut self.sim.world, &self.mats, spec);
                }
                self.say(format!(
                    "Added {} ({}x{} cells)",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    shape.w,
                    shape.h
                ));
            }
            Err(e) => self.say(e),
        }
    }

    fn apply_brush(&mut self, x: i32, y: i32, tool: Tool) {
        let r = self.brush_size - 1;
        match tool {
            Tool::Paint => self.sim.world.paint_circle(&self.mats, x, y, r, self.brush_mat),
            Tool::Erase => self.sim.world.paint_circle(&self.mats, x, y, r, AIR),
            Tool::Heat => self.sim.world.heat_circle(x, y, r, 40.0),
            Tool::Cool => self.sim.world.heat_circle(x, y, r, -40.0),
            Tool::Object => {}
        }
    }

    fn stroke(&mut self, x: i32, y: i32, tool: Tool) {
        // Fill the gap since last frame so fast strokes don't leave dots.
        let (x0, y0) = self.last_paint.unwrap_or((x, y));
        let steps = (x - x0).abs().max((y - y0).abs()).max(1);
        let every = (self.brush_size / 2).max(1);
        let mut s = 0;
        while s <= steps {
            let px = x0 + (x - x0) * s / steps;
            let py = y0 + (y - y0) * s / steps;
            if s == steps || s % every == 0 {
                self.apply_brush(px, py, tool);
            }
            s += 1;
        }
        self.last_paint = Some((x, y));
    }

    // ----------------------------------------------------------------- UI --

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let big = |t: &str| RichText::new(t).size(18.0).strong();
            if self.playing {
                if ui.add(egui::Button::new(big("⏸ Pause")).min_size(Vec2::new(110.0, 34.0))).clicked() {
                    self.playing = false;
                }
            } else if ui
                .add(egui::Button::new(big("▶ Play")).min_size(Vec2::new(110.0, 34.0)).fill(Color32::from_rgb(40, 110, 60)))
                .on_hover_text("Run the simulation (Space)")
                .clicked()
            {
                self.play();
            }
            if ui
                .add(egui::Button::new(big("Step")).min_size(Vec2::new(70.0, 34.0)))
                .on_hover_text("Advance one frame")
                .clicked()
            {
                self.playing = false;
                self.step_once();
            }
            if ui
                .add(egui::Button::new(big("↺ Reset")).min_size(Vec2::new(90.0, 34.0)))
                .on_hover_text("Go back to how it was when you pressed Play (R)")
                .clicked()
            {
                self.reset();
            }
            if ui
                .add(egui::Button::new(big("Clear")).min_size(Vec2::new(70.0, 34.0)))
                .on_hover_text("Empty everything")
                .clicked()
            {
                self.sim = Sim::empty(self.sim.world.w, self.sim.world.h, &self.mats);
                self.snapshot = self.sim.snapshot();
                self.ran_since_snapshot = false;
                self.playing = false;
            }
            ui.separator();
            ui.toggle_value(&mut self.view.heat, big("🌡 Heat view"))
                .on_hover_text("Show temperature: blue is cold, red/yellow/white is hot (H)");
            ui.separator();
            ui.label("Speed");
            ui.add(egui::Slider::new(&mut self.speed, 0.25..=4.0).step_by(0.25).suffix("x"));
            ui.separator();
            ui.label("Scene");
            ui.add(egui::TextEdit::singleline(&mut self.scene_path).desired_width(200.0));
            if ui.button("💾 Save").clicked() {
                self.save_scene();
            }
            if ui.button("📂 Load").clicked() {
                self.load_scene();
            }
            ui.menu_button("Scenes...", |ui| {
                let mut files: Vec<PathBuf> = std::fs::read_dir("scenes")
                    .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect())
                    .unwrap_or_default();
                files.sort();
                if files.is_empty() {
                    ui.label("No scenes in the scenes folder yet");
                }
                for f in files {
                    if ui.button(f.file_stem().unwrap_or_default().to_string_lossy()).clicked() {
                        self.scene_path = f.display().to_string();
                        self.load_scene();
                        ui.close();
                    }
                }
            });
        });
    }

    fn palette(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.heading("Tool");
        ui.horizontal_wrapped(|ui| {
            for (tool, label, tip) in [
                (Tool::Paint, "🖌 Paint", "Paint the chosen material"),
                (Tool::Erase, "Erase", "Turn things back into air (or right-click)"),
                (Tool::Heat, "🔥 Heat", "Make things hotter"),
                (Tool::Cool, "❄ Cool", "Make things colder"),
                (Tool::Object, "📦 Make object", "Drag a box around something you painted: it becomes a loose object that falls, tumbles and breaks"),
            ] {
                ui.selectable_value(&mut self.tool, tool, RichText::new(label).size(16.0))
                    .on_hover_text(tip);
            }
        });
        ui.add_space(4.0);
        ui.label("Brush size (mouse wheel)");
        ui.add(egui::Slider::new(&mut self.brush_size, 1..=40));
        ui.separator();
        ui.heading("Materials");
        egui::ScrollArea::vertical().max_height(ui.available_height() - 170.0).show(ui, |ui| {
            let mut order: Vec<MatId> = (0..self.mats.list.len() as MatId).filter(|&i| i != AIR).collect();
            order.sort_by(|a, b| self.mats.get(*a).name.cmp(&self.mats.get(*b).name));
            for id in order {
                let m = self.mats.get(id);
                let selected = self.brush_mat == id && self.tool == Tool::Paint;
                let c = color32(m.color);
                let lum = 0.3 * m.color[0] as f32 + 0.59 * m.color[1] as f32 + 0.11 * m.color[2] as f32;
                let text_col = if lum > 140.0 { Color32::BLACK } else { Color32::WHITE };
                let label = RichText::new(m.name.replace('_', " ")).size(16.0).color(text_col);
                let mut b = egui::Button::new(label).fill(c).min_size(Vec2::new(ui.available_width(), 30.0));
                if selected {
                    b = b.stroke(Stroke::new(3.0, Color32::WHITE));
                }
                let mut tip = format!("{} ({})", m.about, m.state);
                if let Some(t) = m.catches_fire_at {
                    tip += &format!("\nCatches fire at {t}°C");
                }
                if let Some(c) = m.melts {
                    tip += &format!("\nMelts at {}°C", c.at);
                }
                if let Some(c) = m.boils {
                    tip += &format!("\nBoils at {}°C", c.at);
                }
                if let Some(c) = m.freezes {
                    tip += &format!("\nFreezes at {}°C", c.at);
                }
                if ui.add(b).on_hover_text(tip).clicked() {
                    self.brush_mat = id;
                    self.tool = Tool::Paint;
                }
            }
        });
        ui.separator();
        ui.label(RichText::new("Import").strong());
        ui.label(RichText::new("Drop a .png or Blender .glb file onto the sandbox to add it.").size(13.0));
        ui.checkbox(&mut self.drop_static, "Drop as fixed scenery");
        ui.separator();
        ui.label(RichText::new("Messages").strong());
        for (when, msg) in self.messages.iter().rev().take(5) {
            let age = when.elapsed().as_secs_f32();
            let alpha = (1.0 - (age - 8.0) / 4.0).clamp(0.35, 1.0);
            let col = ui.visuals().text_color().gamma_multiply(alpha);
            ui.label(RichText::new(msg).size(13.0).color(col));
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some((x, y)) = self.hover {
                let i = self.sim.world.idx(x, y);
                let m = self.mats.get(self.sim.world.mat[i]);
                let mut state = m.state.to_string();
                if self.sim.world.is_burning(i) {
                    state += ", burning";
                }
                if self.sim.world.is_owned(i) {
                    state += ", object";
                }
                ui.label(RichText::new(m.name.replace('_', " ")).strong().size(16.0).color(color32(m.color).gamma_multiply(1.4)));
                ui.label(RichText::new(format!("{:.0}°C", self.sim.world.temp[i])).size(16.0));
                ui.label(RichText::new(state).size(16.0));
                ui.label(RichText::new(format!("at {x}, {y}")).weak());
            } else {
                ui.label(RichText::new("Left-click to paint, right-click to erase. Space = Play/Pause, H = heat view, R = reset.").weak());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "{:.0} fps · sim {:.1} ms · {} objects · {}x{} · frame {}",
                        self.fps,
                        self.sim_ms,
                        self.sim.physics.body_count(),
                        self.sim.world.w,
                        self.sim.world.h,
                        self.sim.world.frame
                    ))
                    .weak(),
                );
            });
        });
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (w, h) = (self.sim.world.w, self.sim.world.h);
        let rgba = self.renderer.draw(&self.sim.world, &self.mats, self.view);
        let image = egui::ColorImage::from_rgba_unmultiplied([w, h], rgba);
        match &mut self.texture {
            Some(t) if t.size() == [w, h] => t.set(image, TextureOptions::NEAREST),
            _ => self.texture = Some(ui.ctx().load_texture("grid", image, TextureOptions::NEAREST)),
        }
        let tex = self.texture.as_ref().unwrap();

        // Fit the grid in the space we have, keeping its shape.
        let avail = ui.available_size();
        let scale = (avail.x / w as f32).min(avail.y / h as f32).max(0.1);
        let size = Vec2::new(w as f32 * scale, h as f32 * scale);
        let (outer, _) = ui.allocate_exact_size(avail, Sense::hover());
        let rect = Rect::from_center_size(outer.center(), size);
        let resp = ui.interact(rect, ui.scope_id().with("canvas"), Sense::click_and_drag());
        let painter = ui.painter_at(outer);
        painter.image(tex.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);

        let to_cell = |p: Pos2| -> (i32, i32) {
            (((p.x - rect.min.x) / scale).floor() as i32, ((p.y - rect.min.y) / scale).floor() as i32)
        };
        self.hover = None;
        if let Some(p) = resp.hover_pos() {
            let (cx, cy) = to_cell(p);
            if self.sim.world.in_bounds(cx, cy) {
                self.hover = Some((cx as usize, cy as usize));
            }
            // Brush outline.
            let col = match self.tool {
                Tool::Paint => color32(self.mats.get(self.brush_mat).color),
                Tool::Erase => Color32::WHITE,
                Tool::Heat => Color32::from_rgb(255, 120, 40),
                Tool::Cool => Color32::from_rgb(100, 180, 255),
                Tool::Object => Color32::from_rgb(255, 230, 120),
            };
            let r = (self.brush_size as f32 - 0.5).max(0.5) * scale;
            painter.circle_stroke(p, r, Stroke::new(1.5, col));

            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let step = if scroll > 0.0 { 1 } else { -1 };
                self.brush_size = (self.brush_size + step).clamp(1, 40);
            }
        }

        let (primary, secondary) = ui.input(|i| (i.pointer.primary_down(), i.pointer.secondary_down()));
        if self.tool == Tool::Object && !secondary {
            let cell = resp.interact_pointer_pos().or(resp.hover_pos()).map(to_cell);
            if primary && (resp.dragged() || resp.is_pointer_button_down_on()) {
                if self.box_start.is_none() {
                    self.box_start = cell;
                }
            }
            if let (Some((sx, sy)), Some((cx, cy))) = (self.box_start, cell) {
                let a = Pos2::new(rect.min.x + sx.min(cx) as f32 * scale, rect.min.y + sy.min(cy) as f32 * scale);
                let b = Pos2::new(rect.min.x + (sx.max(cx) + 1) as f32 * scale, rect.min.y + (sy.max(cy) + 1) as f32 * scale);
                painter.rect_stroke(Rect::from_two_pos(a, b), 0.0, Stroke::new(2.0, Color32::from_rgb(255, 230, 120)), egui::StrokeKind::Middle);
                if !primary {
                    let n = self.sim.physics.make_objects(&mut self.sim.world, &self.mats, sx, sy, cx, cy);
                    self.say(if n == 0 { "Nothing solid in that box".to_string() } else { format!("Made {n} object(s)") });
                    self.box_start = None;
                }
            } else if !primary {
                self.box_start = None;
            }
            return;
        }
        if (primary || secondary) && (resp.dragged() || resp.is_pointer_button_down_on()) {
            if let Some(p) = resp.interact_pointer_pos() {
                let (cx, cy) = to_cell(p);
                let tool = if secondary { Tool::Erase } else { self.tool };
                self.stroke(cx, cy, tool);
            }
        } else {
            self.last_paint = None;
        }
    }
}

impl eframe::App for Sandbox {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64().min(0.1);
        self.last_frame = now;
        if dt > 0.0 {
            self.fps = self.fps * 0.95 + (1.0 / dt as f32) * 0.05;
        }

        self.check_files();
        let dropped: Vec<PathBuf> = ui.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        for path in dropped {
            self.import_file(&path);
        }

        // Keyboard shortcuts (unless typing in a text box).
        if !ctx.egui_wants_keyboard_input() {
            let (space, h, r) = ui.input(|i| {
                (i.key_pressed(egui::Key::Space), i.key_pressed(egui::Key::H), i.key_pressed(egui::Key::R))
            });
            if space {
                if self.playing { self.playing = false } else { self.play() }
            }
            if h {
                self.view.heat = !self.view.heat;
            }
            if r {
                self.reset();
            }
        }

        // Run the simulation at a steady 60 steps per second (times speed).
        if self.playing {
            self.sim_clock += dt * SIM_HZ * self.speed as f64;
            let mut steps = 0;
            while self.sim_clock >= 1.0 && steps < 8 {
                self.step_once();
                self.sim_clock -= 1.0;
                steps += 1;
            }
            if steps == 8 {
                self.sim_clock = 0.0;
            }
        }

        egui::Panel::top("top").show(ui, |ui| {
            ui.add_space(4.0);
            self.top_bar(ui);
            ui.add_space(2.0);
        });
        egui::Panel::left("palette").resizable(false).exact_size(200.0).show(ui, |ui| self.palette(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(10, 10, 14)))
            .show(ui, |ui| self.canvas(ui));

        ctx.request_repaint();
    }
}
