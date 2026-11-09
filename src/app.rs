//! The editor window: paint materials, select and change things, press Play.

use eframe::egui::{self, Color32, Key, Modifiers, Pos2, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Vec2};
use grave_engine::edit::{self, Clip, History, Region, Transform};
use grave_engine::import;
use grave_engine::material::{AIR, MatId, Materials};
use grave_engine::physics::{BodySpec, CELL};
use grave_engine::render::{Renderer, View};
use grave_engine::scene;
use grave_engine::sim::{Sim, Snapshot};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

const SIM_HZ: f64 = 60.0;
const SELECT_COLOR: Color32 = Color32::from_rgb(255, 214, 90);
const AREA_COLOR: Color32 = Color32::from_rgb(90, 200, 255);

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    /// Click or drag a box to select; drag to move; handles to turn and resize.
    Select,
    Paint,
    Erase,
    Heat,
    Cool,
    /// Drag a box: everything solid inside becomes a loose object.
    Object,
}

/// What's selected.
#[derive(Clone, PartialEq)]
enum Selection {
    None,
    /// Objects, by id.
    Objects(Vec<u64>),
    /// An area of scenery.
    Area(Region),
}

/// What a mouse drag on the canvas is doing.
#[derive(Clone, Copy)]
enum Drag {
    None,
    /// Drawing a selection box from this cell.
    Box((i32, i32)),
    /// Moving the selected objects; last pointer position (in cells).
    Objects((f32, f32)),
    /// Moving the selected area from where the drag started.
    Area((i32, i32)),
    /// Turning one object.
    Rotate(u64),
    /// Resizing one object: distance from its centre when the drag started.
    Scale(u64, f32),
    /// Painting with a brush tool.
    Paint,
    /// Make-object box.
    MakeObject((i32, i32)),
}

/// What Copy put away.
enum Clipboard {
    Empty,
    Objects(Vec<BodySpec>),
    Area(Clip),
}

struct Sandbox {
    mats: Materials,
    sim: Sim,
    history: History,
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
    /// Mouse-wheel movement not yet turned into brush size steps.
    scroll_acc: f32,
    selection: Selection,
    drag: Drag,
    clipboard: Clipboard,
    /// Dropped files land as fixed scenery instead of loose objects.
    drop_static: bool,
    hover: Option<(usize, usize)>,
    /// Mouse position in cells (with fractions), when over the canvas.
    hover_f: Option<(f32, f32)>,
    scene_path: String,
    scene_stamp: Option<SystemTime>,
    messages: Vec<(Instant, String)>,
    last_file_check: Instant,
    sim_clock: f64,
    last_frame: Instant,
    fps: f32,
    sim_ms: f32,
    /// Where the world is drawn on screen (top-left) and how big a cell is.
    canvas_at: (Pos2, f32),
}

pub fn run(materials: PathBuf, scene_file: Option<PathBuf>) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Grave Engine")
            .with_inner_size([1500.0, 900.0])
            .with_min_inner_size([900.0, 550.0]),
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

/// The four corners of an object, in cells.
fn corners(b: &BodySpec, grow: f32) -> [(f32, f32); 4] {
    let (c, s) = (b.angle.cos(), b.angle.sin());
    let (hw, hh) = (b.w as f32 / 2.0 * grow, b.h as f32 / 2.0 * grow);
    [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)].map(|(x, y)| (b.x + c * x - s * y, b.y + s * x + c * y))
}

/// Where an object's turn handle and resize handle are, in cells.
fn handles(b: &BodySpec) -> ((f32, f32), (f32, f32)) {
    let (c, s) = (b.angle.cos(), b.angle.sin());
    let up = b.h as f32 / 2.0 + 5.0;
    let rot = (b.x + s * up, b.y - c * up);
    let k = corners(b, 1.0)[2];
    (rot, k)
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
            history: History::new(),
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
            scroll_acc: 0.0,
            selection: Selection::None,
            drag: Drag::None,
            clipboard: Clipboard::Empty,
            drop_static: false,
            hover: None,
            hover_f: None,
            scene_path: "scenes/my_scene.txt".into(),
            scene_stamp: None,
            messages: Vec::new(),
            last_file_check: Instant::now(),
            sim_clock: 0.0,
            last_frame: Instant::now(),
            fps: 60.0,
            sim_ms: 0.0,
            canvas_at: (Pos2::ZERO, 1.0),
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
            app.load_scene_inner();
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

    // ------------------------------------------------------------ editing --

    /// Do one undoable change right now.
    fn change(&mut self, label: &str, f: impl FnOnce(&mut Self)) {
        self.history.commit(&mut self.sim);
        self.history.touch(&mut self.sim, label);
        f(self);
        self.history.commit(&mut self.sim);
    }

    fn undo(&mut self) {
        self.playing = false;
        self.drag = Drag::None;
        match self.history.undo(&mut self.sim, &self.mats) {
            Some(label) => self.say(format!("Undid: {label}")),
            None => self.say("Nothing to undo"),
        }
        self.selection = Selection::None;
    }

    fn redo(&mut self) {
        self.playing = false;
        self.drag = Drag::None;
        match self.history.redo(&mut self.sim, &self.mats) {
            Some(label) => self.say(format!("Redid: {label}")),
            None => self.say("Nothing to redo"),
        }
        self.selection = Selection::None;
    }

    fn selected_ids(&self) -> Vec<u64> {
        match &self.selection {
            Selection::Objects(ids) => ids.clone(),
            _ => Vec::new(),
        }
    }

    /// Forget selected objects that no longer exist (burned, broke apart...).
    fn tidy_selection(&mut self) {
        if let Selection::Objects(ids) = &self.selection {
            let alive = self.sim.physics.ids();
            let kept: Vec<u64> = ids.iter().copied().filter(|i| alive.contains(i)).collect();
            self.selection = if kept.is_empty() { Selection::None } else { Selection::Objects(kept) };
        }
    }

    fn edit_objects(&mut self, label: &str, f: impl Fn(&mut BodySpec)) {
        let ids = self.selected_ids();
        if ids.is_empty() {
            return;
        }
        self.change(label, |app| {
            for id in ids {
                app.sim.physics.edit(&mut app.sim.world, &app.mats, id, &f);
            }
        });
    }

    fn delete_selection(&mut self) {
        match self.selection.clone() {
            Selection::Objects(ids) => self.change("delete", |app| {
                for id in ids {
                    app.sim.physics.remove(&mut app.sim.world, id);
                }
            }),
            Selection::Area(r) => self.change("delete", |app| {
                edit::cut_region(&mut app.sim.world, r);
            }),
            Selection::None => return,
        }
        self.selection = Selection::None;
    }

    fn copy_selection(&mut self) -> bool {
        match &self.selection {
            Selection::Objects(ids) => {
                let specs: Vec<BodySpec> = ids.iter().filter_map(|&id| self.sim.physics.spec_of(&self.sim.world, id)).collect();
                self.say(format!("Copied {} object(s)", specs.len()));
                self.clipboard = Clipboard::Objects(specs);
                true
            }
            Selection::Area(r) => {
                self.clipboard = Clipboard::Area(edit::copy_region(&self.sim.world, *r));
                self.say("Copied an area");
                true
            }
            Selection::None => false,
        }
    }

    /// Paste the clipboard centred on (x, y).
    fn paste_at(&mut self, x: f32, y: f32) {
        match &self.clipboard {
            Clipboard::Empty => self.say("Nothing copied yet"),
            Clipboard::Objects(specs) => {
                let specs = specs.clone();
                let (cx, cy) = (
                    specs.iter().map(|s| s.x).sum::<f32>() / specs.len() as f32,
                    specs.iter().map(|s| s.y).sum::<f32>() / specs.len() as f32,
                );
                let mut new_ids = Vec::new();
                self.change("paste", |app| {
                    for mut s in specs {
                        s.x += x - cx;
                        s.y += y - cy;
                        s.vx = 0.0;
                        s.vy = 0.0;
                        s.spin = 0.0;
                        if let Some(id) = app.sim.physics.add(&mut app.sim.world, &app.mats, s) {
                            new_ids.push(id);
                        }
                    }
                });
                self.selection = Selection::Objects(new_ids);
            }
            Clipboard::Area(clip) => {
                let clip = clip.clone();
                let (x0, y0) = ((x - clip.w as f32 / 2.0).round() as i32, (y - clip.h as f32 / 2.0).round() as i32);
                self.change("paste", |app| edit::paste_clip(&mut app.sim.world, &clip, x0, y0));
                self.selection = Selection::Area(Region::new(&self.sim.world, x0, y0, x0 + clip.w as i32 - 1, y0 + clip.h as i32 - 1));
            }
        }
    }

    fn duplicate(&mut self) {
        let (cx, cy) = match &self.selection {
            Selection::Objects(ids) => {
                let specs: Vec<BodySpec> = ids.iter().filter_map(|&id| self.sim.physics.spec_of(&self.sim.world, id)).collect();
                if specs.is_empty() {
                    return;
                }
                let n = specs.len() as f32;
                (specs.iter().map(|s| s.x).sum::<f32>() / n, specs.iter().map(|s| s.y).sum::<f32>() / n)
            }
            Selection::Area(r) => ((r.x0 + r.x1) as f32 / 2.0 + 0.5, (r.y0 + r.y1) as f32 / 2.0 + 0.5),
            Selection::None => return,
        };
        let saved = std::mem::replace(&mut self.clipboard, Clipboard::Empty);
        self.copy_selection();
        let shift = match &self.selection {
            Selection::Area(r) => r.w() as f32 + 2.0,
            _ => 8.0,
        };
        self.paste_at(cx + shift, cy);
        self.clipboard = saved;
    }

    fn transform_selection(&mut self, t: Transform, label: &str) {
        match self.selection.clone() {
            Selection::Objects(_) => self.edit_objects(label, |b| edit::transform_body(b, t)),
            Selection::Area(r) => {
                let mut out = r;
                self.change(label, |app| out = edit::transform_region(&mut app.sim.world, r, t));
                self.selection = Selection::Area(out);
            }
            Selection::None => {}
        }
    }

    // -------------------------------------------------------------- files --

    fn load_scene(&mut self) {
        self.history.commit(&mut self.sim);
        self.history.touch(&mut self.sim, "load scene");
        self.load_scene_inner();
        self.history.commit(&mut self.sim);
    }

    fn load_scene_inner(&mut self) {
        let path = PathBuf::from(&self.scene_path);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let (sim, problems) = scene::load(&text, &self.mats, path.parent()).into_sim(&self.mats);
                self.sim = sim;
                self.snapshot = self.sim.snapshot();
                self.ran_since_snapshot = false;
                self.playing = false;
                self.selection = Selection::None;
                self.hover = None;
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
            self.sim.physics.materials_changed(&self.mats);
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

    // --------------------------------------------------------------- time --

    fn play(&mut self) {
        self.history.commit(&mut self.sim);
        self.snapshot = self.sim.snapshot();
        self.ran_since_snapshot = false;
        self.playing = true;
        self.sim_clock = 0.0;
    }

    fn step_once(&mut self) {
        if !self.ran_since_snapshot {
            self.snapshot = self.sim.snapshot();
        }
        let t = Instant::now();
        self.sim.step(&self.mats);
        self.sim_ms = self.sim_ms * 0.9 + t.elapsed().as_secs_f32() * 1000.0 * 0.1;
        self.ran_since_snapshot = true;
    }

    fn reset(&mut self) {
        let snap = self.snapshot.clone();
        self.change("reset", |app| app.sim = Sim::restore(&snap, &app.mats));
        self.ran_since_snapshot = false;
        self.playing = false;
        self.selection = Selection::None;
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
                let as_static = self.drop_static;
                let mut new_id = None;
                self.change("import", |app| {
                    if as_static {
                        let (x0, y0) = (cx as i32 - shape.w as i32 / 2, cy as i32 - shape.h as i32 / 2);
                        for y in 0..shape.h {
                            for x in 0..shape.w {
                                if let Some(id) = shape.cells[y * shape.w + x] {
                                    let (wx, wy) = (x0 + x as i32, y0 + y as i32);
                                    if app.sim.world.in_bounds(wx, wy) {
                                        app.sim.world.set(&app.mats, wx as usize, wy as usize, id);
                                    }
                                }
                            }
                        }
                    } else {
                        let spec = BodySpec::from_mats(shape.w, shape.h, &shape.cells, &app.mats, cx, cy);
                        new_id = app.sim.physics.add(&mut app.sim.world, &app.mats, spec);
                    }
                });
                if let Some(id) = new_id {
                    self.selection = Selection::Objects(vec![id]);
                }
                self.say(format!("Added {} ({}x{} cells)", path.file_name().unwrap_or_default().to_string_lossy(), shape.w, shape.h));
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
            Tool::Select | Tool::Object => {}
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
            if ui.add(egui::Button::new(big("Step")).min_size(Vec2::new(70.0, 34.0))).on_hover_text("Advance one frame").clicked() {
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
            ui.separator();
            let undo_tip = match self.history.undo_label() {
                Some(l) => format!("Undo {l} (Ctrl+Z)"),
                None => "Nothing to undo".into(),
            };
            if ui.add_enabled(self.history.can_undo(), egui::Button::new(big("↶ Undo"))).on_hover_text(undo_tip).clicked() {
                self.undo();
            }
            let redo_tip = match self.history.redo_label() {
                Some(l) => format!("Redo {l} (Ctrl+Y)"),
                None => "Nothing to redo".into(),
            };
            if ui.add_enabled(self.history.can_redo(), egui::Button::new(big("↷ Redo"))).on_hover_text(redo_tip).clicked() {
                self.redo();
            }
            if ui.add(egui::Button::new(big("Clear"))).on_hover_text("Empty everything (you can undo this)").clicked() {
                let (w, h) = (self.sim.world.w, self.sim.world.h);
                self.change("clear", |app| app.sim = Sim::empty(w, h, &app.mats));
                self.snapshot = self.sim.snapshot();
                self.ran_since_snapshot = false;
                self.playing = false;
                self.selection = Selection::None;
            }
            ui.separator();
            ui.toggle_value(&mut self.view.heat, big("🌡 Heat view"))
                .on_hover_text("Show temperature: blue is cold, red/yellow/white is hot (H)");
            ui.separator();
            ui.label("Speed");
            ui.add(egui::Slider::new(&mut self.speed, 0.25..=4.0).step_by(0.25).suffix("x"));
            ui.separator();
            ui.label("Scene");
            ui.add(egui::TextEdit::singleline(&mut self.scene_path).desired_width(180.0));
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
                (Tool::Select, "👆 Select", "Click or drag a box to select. Drag to move. Handles turn and resize. (V)"),
                (Tool::Paint, "🖌 Paint", "Paint the chosen material (B)"),
                (Tool::Erase, "Erase", "Turn things back into air (or right-click)"),
                (Tool::Heat, "🔥 Heat", "Make things hotter"),
                (Tool::Cool, "❄ Cool", "Make things colder"),
                (Tool::Object, "📦 Make object", "Drag a box around something you painted: it becomes a loose object that falls, tumbles and breaks"),
            ] {
                ui.selectable_value(&mut self.tool, tool, RichText::new(label).size(16.0)).on_hover_text(tip);
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

    /// A material picker. Returns the chosen material when one is picked.
    fn material_menu(&self, ui: &mut egui::Ui, label: &str) -> Option<MatId> {
        let mut picked = None;
        ui.menu_button(label, |ui| {
            let mut order: Vec<MatId> = (0..self.mats.list.len() as MatId).filter(|&i| i != AIR).collect();
            order.sort_by(|a, b| self.mats.get(*a).name.cmp(&self.mats.get(*b).name));
            for id in order {
                let m = self.mats.get(id);
                if ui.button(RichText::new(m.name.replace('_', " ")).color(color32(m.color))).clicked() {
                    picked = Some(id);
                    ui.close();
                }
            }
        });
        picked
    }

    /// Buttons shared by objects and areas.
    fn shape_buttons(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("⬌ Flip").on_hover_text("Mirror left-right (F)").clicked() {
                self.transform_selection(Transform::FlipH, "flip");
            }
            if ui.button("⬍ Flip").on_hover_text("Mirror upside down").clicked() {
                self.transform_selection(Transform::FlipV, "flip");
            }
            if ui.button("⟳ Quarter turn").clicked() {
                self.transform_selection(Transform::Turn, "turn");
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button("Bigger ×2").clicked() {
                self.transform_selection(Transform::Scale(2.0), "resize");
            }
            if ui.button("Smaller ×½").clicked() {
                self.transform_selection(Transform::Scale(0.5), "resize");
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button("⧉ Duplicate").on_hover_text("Ctrl+D").clicked() {
                self.duplicate();
            }
            if ui.button("📋 Copy").on_hover_text("Ctrl+C, then Ctrl+V to paste at the mouse").clicked() {
                self.copy_selection();
            }
            if ui.button(RichText::new("🗑 Delete").color(Color32::from_rgb(230, 90, 80))).on_hover_text("Delete key").clicked() {
                self.delete_selection();
            }
        });
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.heading("Inspector");
        self.tidy_selection();
        match self.selection.clone() {
            Selection::None => {
                ui.label(RichText::new("Nothing selected.").weak());
                ui.label(RichText::new("Pick 👆 Select (V), then click an object or drag a box around something.").size(13.0).weak());
            }
            Selection::Objects(ids) if ids.len() == 1 => self.inspect_object(ui, ids[0]),
            Selection::Objects(ids) => {
                ui.label(RichText::new(format!("{} objects", ids.len())).strong());
                let total: f32 = ids.iter().filter_map(|&id| self.sim.physics.mass(id)).sum();
                ui.label(format!("Total mass: {}", nice_mass(total)));
                ui.separator();
                if let Some(m) = self.material_menu(ui, "Change material to…") {
                    self.recolor_selection(m);
                }
                if ui.button("Freeze into scenery").on_hover_text("Stop them being loose objects: they become fixed in place").clicked() {
                    self.freeze_selection();
                }
                self.shape_buttons(ui);
            }
            Selection::Area(r) => {
                ui.label(RichText::new("Area of scenery").strong());
                ui.label(format!("{} × {} cells ({:.1} × {:.1} m)", r.w(), r.h(), r.w() as f32 * CELL, r.h() as f32 * CELL));
                let world = &self.sim.world;
                let cells = (r.y0..=r.y1).flat_map(|y| (r.x0..=r.x1).map(move |x| (x, y))).filter_map(|(x, y)| {
                    let i = world.idx(x as usize, y as usize);
                    (!world.is_owned(i)).then_some(world.mat[i])
                });
                let census = edit::census(cells, &self.mats);
                if census.is_empty() {
                    ui.label(RichText::new("Just air.").weak());
                } else {
                    ui.label("Made of:");
                    for (name, n) in census.iter().take(6) {
                        ui.label(format!("   {n} {}", name.replace('_', " ")));
                    }
                }
                ui.separator();
                if let Some(m) = self.material_menu(ui, "Change material to…") {
                    self.recolor_selection(m);
                }
                let mut t = 20.0f32;
                ui.horizontal(|ui| {
                    ui.label("Set temperature");
                    ui.add(egui::DragValue::new(&mut t).suffix(" °C").speed(5.0));
                    if ui.button("Apply").clicked() {
                        let set_to = t;
                        self.change("set temperature", |app| {
                            for y in r.y0..=r.y1 {
                                for x in r.x0..=r.x1 {
                                    let i = app.sim.world.idx(x as usize, y as usize);
                                    if !app.sim.world.is_owned(i) {
                                        app.sim.world.temp[i] = set_to;
                                    }
                                }
                            }
                        });
                    }
                });
                if ui.button("📦 Make into objects").on_hover_text("Everything solid in the area becomes loose objects").clicked() {
                    let mut n = 0;
                    self.change("make objects", |app| n = app.sim.physics.make_objects(&mut app.sim.world, &app.mats, r.x0, r.y0, r.x1, r.y1));
                    self.say(format!("Made {n} object(s)"));
                    self.selection = Selection::None;
                }
                self.shape_buttons(ui);
            }
        }
        ui.separator();
        ui.label(RichText::new("Shortcuts").strong());
        for line in [
            "Ctrl+Z undo · Ctrl+Y redo",
            "Delete · Ctrl+C / X / V · Ctrl+D",
            "Q / E turn 15° · F flip · Esc deselect",
            "Shift+click adds to the selection",
        ] {
            ui.label(RichText::new(line).size(13.0).weak());
        }
    }

    fn inspect_object(&mut self, ui: &mut egui::Ui, id: u64) {
        let Some(b) = self.sim.physics.spec_of(&self.sim.world, id) else { return };
        let census = edit::census(b.cells.iter().flatten().map(|c| c.mat), &self.mats);
        let made_of: Vec<String> = census.iter().map(|(n, c)| format!("{c} {}", n.replace('_', " "))).collect();
        ui.label(RichText::new("Object").strong());
        ui.label(format!("Made of: {}", made_of.join(", ")));
        ui.label(format!("Size: {} × {} cells ({:.1} × {:.1} m)", b.w, b.h, b.w as f32 * CELL, b.h as f32 * CELL));
        if let Some(m) = self.sim.physics.mass(id) {
            ui.label(format!("Mass: {}", nice_mass(m)));
        }
        let n = b.cell_count().max(1) as f32;
        let avg_t = b.cells.iter().flatten().map(|c| c.temp).sum::<f32>() / n;
        let burning = b.cells.iter().flatten().filter(|c| c.flags & grave_engine::world::BURNING != 0).count();
        if burning > 0 {
            ui.label(RichText::new(format!("🔥 {burning} cells burning")).color(Color32::from_rgb(255, 150, 60)));
        }
        ui.separator();

        let (mut x, mut y) = (b.x, b.y);
        let mut deg = b.angle.to_degrees();
        let (mut vx, mut vy) = (b.vx * CELL, b.vy * CELL);
        let mut t = avg_t;
        let mut changed = false;
        egui::Grid::new("obj").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label("Position");
            ui.horizontal(|ui| {
                changed |= ui.add(egui::DragValue::new(&mut x).prefix("x ").speed(0.5).max_decimals(1)).changed();
                changed |= ui.add(egui::DragValue::new(&mut y).prefix("y ").speed(0.5).max_decimals(1)).changed();
            });
            ui.end_row();
            ui.label("Angle");
            changed |= ui.add(egui::DragValue::new(&mut deg).suffix("°").speed(1.0).max_decimals(0)).changed();
            ui.end_row();
            ui.label("Speed");
            ui.horizontal(|ui| {
                changed |= ui.add(egui::DragValue::new(&mut vx).prefix("→ ").suffix(" m/s").speed(0.1).max_decimals(1)).changed();
                changed |= ui.add(egui::DragValue::new(&mut vy).prefix("↓ ").suffix(" m/s").speed(0.1).max_decimals(1)).changed();
            });
            ui.end_row();
            ui.label("Temperature");
            changed |= ui.add(egui::DragValue::new(&mut t).suffix(" °C").speed(5.0).max_decimals(0)).changed();
            ui.end_row();
        });
        if changed {
            // Gathered into one undo step until the mouse is let go.
            self.history.touch(&mut self.sim, "change object");
            let dt = t - avg_t;
            self.sim.physics.edit(&mut self.sim.world, &self.mats, id, |s| {
                s.x = x;
                s.y = y;
                s.angle = deg.to_radians();
                s.vx = vx / CELL;
                s.vy = vy / CELL;
                if dt.abs() > 0.01 {
                    for c in s.cells.iter_mut().flatten() {
                        c.temp += dt;
                    }
                }
            });
        }
        ui.separator();
        if let Some(m) = self.material_menu(ui, "Change material to…") {
            self.recolor_selection(m);
        }
        if ui.button("Freeze into scenery").on_hover_text("Stop it being a loose object: it becomes fixed in place").clicked() {
            self.freeze_selection();
        }
        self.shape_buttons(ui);
    }

    fn recolor_selection(&mut self, to: MatId) {
        let name = self.mats.get(to).name.clone();
        match self.selection.clone() {
            Selection::Objects(_) => {
                let mats = Materials::clone(&self.mats);
                self.edit_objects(&format!("change to {name}"), move |b| edit::recolor(&mut b.cells, &mats, to));
            }
            Selection::Area(r) => self.change(&format!("change to {name}"), |app| {
                let mut clip = edit::copy_region(&app.sim.world, r);
                edit::recolor(&mut clip.cells, &app.mats, to);
                edit::paste_clip(&mut app.sim.world, &clip, r.x0, r.y0);
            }),
            Selection::None => {}
        }
    }

    fn freeze_selection(&mut self) {
        let ids = self.selected_ids();
        self.change("freeze", |app| {
            for id in ids {
                if let Some(spec) = app.sim.physics.remove(&mut app.sim.world, id) {
                    edit::freeze(&mut app.sim.world, &spec);
                }
            }
        });
        self.selection = Selection::None;
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some((x, y)) = self.hover.filter(|&(x, y)| self.sim.world.in_bounds(x as i32, y as i32)) {
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
                let hint = match self.tool {
                    Tool::Select => "Click an object or drag a box to select. Drag to move, use the handles to turn and resize.",
                    _ => "Left-click to paint, right-click to erase. Space = Play/Pause, H = heat view, R = reset.",
                };
                ui.label(RichText::new(hint).weak());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "{:.0} fps · sim {:.1} ms · {} objects · {} undo steps · frame {}",
                        self.fps,
                        self.sim_ms,
                        self.sim.physics.body_count(),
                        self.history.len(),
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
        self.canvas_at = (rect.min, scale);
        let resp = ui.interact(rect, ui.scope_id().with("canvas"), Sense::click_and_drag());
        let painter = ui.painter_at(outer);
        painter.image(tex.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);

        let to_cells = |p: Pos2| -> (f32, f32) { ((p.x - rect.min.x) / scale, (p.y - rect.min.y) / scale) };
        let to_cell = |p: Pos2| -> (i32, i32) {
            let (x, y) = to_cells(p);
            (x.floor() as i32, y.floor() as i32)
        };
        let to_screen = |(x, y): (f32, f32)| Pos2::new(rect.min.x + x * scale, rect.min.y + y * scale);

        self.hover = None;
        self.hover_f = None;
        let pointer = resp.hover_pos().or(resp.interact_pointer_pos());
        if let Some(p) = resp.hover_pos() {
            let (cx, cy) = to_cell(p);
            if self.sim.world.in_bounds(cx, cy) {
                self.hover = Some((cx as usize, cy as usize));
            }
            self.hover_f = Some(to_cells(p));
            if self.tool != Tool::Select {
                let col = match self.tool {
                    Tool::Paint => color32(self.mats.get(self.brush_mat).color),
                    Tool::Erase => Color32::WHITE,
                    Tool::Heat => Color32::from_rgb(255, 120, 40),
                    Tool::Cool => Color32::from_rgb(100, 180, 255),
                    _ => SELECT_COLOR,
                };
                let r = (self.brush_size as f32 - 0.5).max(0.5) * scale;
                if self.tool != Tool::Object {
                    painter.circle_stroke(p, r, Stroke::new(1.5, col));
                }
                self.scroll_acc += ui.input(|i| i.smooth_scroll_delta.y);
                while self.scroll_acc.abs() >= 30.0 {
                    let step = self.scroll_acc.signum() as i32;
                    self.brush_size = (self.brush_size + step).clamp(1, 40);
                    self.scroll_acc -= 30.0 * step as f32;
                }
            }
        }

        let (primary, secondary, shift) = ui.input(|i| (i.pointer.primary_down(), i.pointer.secondary_down(), i.modifiers.shift));
        let pressed = resp.drag_started() || (resp.is_pointer_button_down_on() && matches!(self.drag, Drag::None) && (primary || secondary));
        let released = !primary && !secondary;

        // ---- start a drag
        if pressed && matches!(self.drag, Drag::None) {
            if let Some(p) = pointer {
                let cell = to_cell(p);
                let cf = to_cells(p);
                self.drag = match self.tool {
                    _ if secondary => {
                        self.history.touch(&mut self.sim, "erase");
                        Drag::Paint
                    }
                    Tool::Select => self.start_select_drag(p, cell, cf, shift, &to_screen),
                    Tool::Object => Drag::MakeObject(cell),
                    t => {
                        let label = match t {
                            Tool::Paint => "paint",
                            Tool::Erase => "erase",
                            Tool::Heat => "heat",
                            _ => "cool",
                        };
                        self.history.touch(&mut self.sim, label);
                        self.last_paint = None;
                        Drag::Paint
                    }
                };
            }
        }

        // ---- continue / finish it
        match self.drag {
            Drag::None => {}
            Drag::Paint => {
                if let Some(p) = pointer {
                    let (cx, cy) = to_cell(p);
                    let tool = if secondary { Tool::Erase } else { self.tool };
                    self.history.touch(&mut self.sim, "paint");
                    self.stroke(cx, cy, tool);
                }
                if released {
                    self.drag = Drag::None;
                    self.last_paint = None;
                    self.history.commit(&mut self.sim);
                }
            }
            Drag::Box(start) | Drag::MakeObject(start) => {
                let end = pointer.map(to_cell).unwrap_or(start);
                let a = to_screen((start.0.min(end.0) as f32, start.1.min(end.1) as f32));
                let b = to_screen(((start.0.max(end.0) + 1) as f32, (start.1.max(end.1) + 1) as f32));
                painter.rect_stroke(Rect::from_two_pos(a, b), 0.0, Stroke::new(2.0, SELECT_COLOR), egui::StrokeKind::Middle);
                if released {
                    let making = matches!(self.drag, Drag::MakeObject(_));
                    self.drag = Drag::None;
                    if making {
                        let mut n = 0;
                        self.change("make objects", |app| n = app.sim.physics.make_objects(&mut app.sim.world, &app.mats, start.0, start.1, end.0, end.1));
                        self.say(if n == 0 { "Nothing solid in that box".to_string() } else { format!("Made {n} object(s)") });
                    } else {
                        self.finish_box(start, end, shift);
                    }
                }
            }
            Drag::Objects(last) => {
                if let Some(p) = pointer {
                    let now = to_cells(p);
                    let (dx, dy) = (now.0 - last.0, now.1 - last.1);
                    if dx != 0.0 || dy != 0.0 {
                        self.history.touch(&mut self.sim, "move");
                        for id in self.selected_ids() {
                            self.sim.physics.edit(&mut self.sim.world, &self.mats, id, |b| {
                                b.x += dx;
                                b.y += dy;
                                b.vx = 0.0;
                                b.vy = 0.0;
                                b.spin = 0.0;
                            });
                        }
                        self.drag = Drag::Objects(now);
                    } else {
                        // Hold it still while dragging, even during Play.
                        for id in self.selected_ids() {
                            self.sim.physics.edit(&mut self.sim.world, &self.mats, id, |b| {
                                b.vx = 0.0;
                                b.vy = 0.0;
                                b.spin = 0.0;
                            });
                        }
                    }
                }
                if released {
                    self.drag = Drag::None;
                    self.history.commit(&mut self.sim);
                }
            }
            Drag::Area(start) => {
                let now = pointer.map(to_cell).unwrap_or(start);
                if let Selection::Area(r) = self.selection {
                    let m = r.moved(now.0 - start.0, now.1 - start.1);
                    let a = to_screen((m.x0 as f32, m.y0 as f32));
                    let b = to_screen(((m.x1 + 1) as f32, (m.y1 + 1) as f32));
                    painter.rect_stroke(Rect::from_two_pos(a, b), 0.0, Stroke::new(2.0, AREA_COLOR.gamma_multiply(0.6)), egui::StrokeKind::Middle);
                    if released {
                        self.drag = Drag::None;
                        let (dx, dy) = (now.0 - start.0, now.1 - start.1);
                        if dx != 0 || dy != 0 {
                            self.change("move", |app| {
                                let clip = edit::cut_region(&mut app.sim.world, r);
                                edit::paste_clip(&mut app.sim.world, &clip, r.x0 + dx, r.y0 + dy);
                            });
                            self.selection = Selection::Area(Region::new(&self.sim.world, m.x0, m.y0, m.x1, m.y1));
                        }
                    }
                } else {
                    self.drag = Drag::None;
                }
            }
            Drag::Rotate(id) => {
                if let (Some(p), Some(b)) = (pointer, self.sim.physics.spec_of(&self.sim.world, id)) {
                    let (px, py) = to_cells(p);
                    let mut angle = (px - b.x).atan2(-(py - b.y));
                    if shift {
                        let step = 15f32.to_radians();
                        angle = (angle / step).round() * step;
                    }
                    self.history.touch(&mut self.sim, "turn");
                    self.sim.physics.edit(&mut self.sim.world, &self.mats, id, |s| {
                        s.angle = angle;
                        s.vx = 0.0;
                        s.vy = 0.0;
                        s.spin = 0.0;
                    });
                }
                if released {
                    self.drag = Drag::None;
                    self.history.commit(&mut self.sim);
                }
            }
            Drag::Scale(id, start_d) => {
                if let (Some(p), Some(b)) = (pointer, self.sim.physics.spec_of(&self.sim.world, id)) {
                    let (px, py) = to_cells(p);
                    let d = ((px - b.x).powi(2) + (py - b.y).powi(2)).sqrt();
                    let f = (d / start_d.max(0.5)).clamp(0.1, 10.0);
                    let pts: Vec<Pos2> = corners(&b, f).iter().map(|&c| to_screen(c)).collect();
                    painter.add(egui::Shape::closed_line(pts, Stroke::new(2.0, SELECT_COLOR.gamma_multiply(0.6))));
                    painter.text(
                        to_screen((b.x, b.y)),
                        egui::Align2::CENTER_CENTER,
                        format!("×{f:.2}"),
                        egui::FontId::proportional(16.0),
                        SELECT_COLOR,
                    );
                }
                if released {
                    self.drag = Drag::None;
                    if let Some(f) = self.last_scale(id, start_d, pointer.map(to_cells)) {
                        if (f - 1.0).abs() > 0.02 {
                            self.change("resize", |app| {
                                app.sim.physics.edit(&mut app.sim.world, &app.mats, id, |s| edit::transform_body(s, Transform::Scale(f)));
                            });
                        }
                    }
                }
            }
        }

        self.draw_selection(&painter, &to_screen);
    }

    fn last_scale(&self, id: u64, start_d: f32, p: Option<(f32, f32)>) -> Option<f32> {
        let b = self.sim.physics.spec_of(&self.sim.world, id)?;
        let (px, py) = p?;
        let d = ((px - b.x).powi(2) + (py - b.y).powi(2)).sqrt();
        Some((d / start_d.max(0.5)).clamp(0.1, 10.0))
    }

    /// The mouse went down with the Select tool: decide what the drag does.
    fn start_select_drag(&mut self, p: Pos2, cell: (i32, i32), cf: (f32, f32), shift: bool, to_screen: &impl Fn((f32, f32)) -> Pos2) -> Drag {
        // Handles of a single selected object.
        if let Selection::Objects(ids) = &self.selection {
            if ids.len() == 1 {
                let id = ids[0];
                if let Some(b) = self.sim.physics.spec_of(&self.sim.world, id) {
                    let (rot, corner) = handles(&b);
                    if to_screen(rot).distance(p) < 12.0 {
                        return Drag::Rotate(id);
                    }
                    if to_screen(corner).distance(p) < 12.0 {
                        let d = ((cf.0 - b.x).powi(2) + (cf.1 - b.y).powi(2)).sqrt();
                        return Drag::Scale(id, d);
                    }
                }
            }
        }
        // An object under the mouse.
        if self.sim.world.in_bounds(cell.0, cell.1) {
            if let Some(id) = self.sim.physics.id_at(&self.sim.world, cell.0 as usize, cell.1 as usize) {
                let mut ids = if shift { self.selected_ids() } else { Vec::new() };
                let already = self.selected_ids().contains(&id);
                if !ids.contains(&id) {
                    ids.push(id);
                }
                if already && !shift {
                    ids = self.selected_ids();
                }
                self.selection = Selection::Objects(ids);
                return Drag::Objects(cf);
            }
        }
        // Inside the selected area: move it.
        if let Selection::Area(r) = self.selection {
            if r.contains(cell.0, cell.1) {
                return Drag::Area(cell);
            }
        }
        Drag::Box(cell)
    }

    fn finish_box(&mut self, start: (i32, i32), end: (i32, i32), shift: bool) {
        if start == end {
            if !shift {
                self.selection = Selection::None;
            }
            return;
        }
        let r = Region::new(&self.sim.world, start.0, start.1, end.0, end.1);
        let mut ids: Vec<u64> = self
            .sim
            .physics
            .ids()
            .into_iter()
            .filter(|&id| {
                self.sim.physics.spec_of(&self.sim.world, id).is_some_and(|b| r.contains(b.x.floor() as i32, b.y.floor() as i32))
            })
            .collect();
        if !ids.is_empty() {
            if shift {
                for id in self.selected_ids() {
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
            }
            self.selection = Selection::Objects(ids);
            return;
        }
        let any = (r.y0..=r.y1).any(|y| (r.x0..=r.x1).any(|x| edit::is_stuff(&self.sim.world, &self.mats, self.sim.world.idx(x as usize, y as usize))));
        self.selection = if any { Selection::Area(r) } else { Selection::None };
    }

    fn draw_selection(&self, painter: &egui::Painter, to_screen: &impl Fn((f32, f32)) -> Pos2) {
        match &self.selection {
            Selection::None => {}
            Selection::Area(r) => {
                if !matches!(self.drag, Drag::Area(..)) {
                    let a = to_screen((r.x0 as f32, r.y0 as f32));
                    let b = to_screen(((r.x1 + 1) as f32, (r.y1 + 1) as f32));
                    painter.rect_stroke(Rect::from_two_pos(a, b), 0.0, Stroke::new(2.0, AREA_COLOR), egui::StrokeKind::Middle);
                }
            }
            Selection::Objects(ids) => {
                for &id in ids {
                    let Some(b) = self.sim.physics.spec_of(&self.sim.world, id) else { continue };
                    let pts: Vec<Pos2> = corners(&b, 1.0).iter().map(|&c| to_screen(c)).collect();
                    painter.add(egui::Shape::closed_line(pts, Stroke::new(2.0, SELECT_COLOR)));
                    if ids.len() == 1 {
                        let (rot, corner) = handles(&b);
                        let top = {
                            let c = corners(&b, 1.0);
                            ((c[0].0 + c[1].0) / 2.0, (c[0].1 + c[1].1) / 2.0)
                        };
                        painter.line_segment([to_screen(top), to_screen(rot)], Stroke::new(1.5, SELECT_COLOR));
                        painter.circle(to_screen(rot), 7.0, Color32::from_rgb(40, 40, 50), Stroke::new(2.0, SELECT_COLOR));
                        let k = to_screen(corner);
                        painter.rect(Rect::from_center_size(k, Vec2::splat(12.0)), 2.0, Color32::from_rgb(40, 40, 50), Stroke::new(2.0, SELECT_COLOR), egui::StrokeKind::Middle);
                    }
                }
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (redo, undo, redo2, dup, del, esc) = ctx.input_mut(|i| {
            (
                // Ctrl+Shift+Z first, or plain Ctrl+Z would swallow it.
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z),
                i.consume_key(Modifiers::COMMAND, Key::Z),
                i.consume_key(Modifiers::COMMAND, Key::Y),
                i.consume_key(Modifiers::COMMAND, Key::D),
                i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace),
                i.key_pressed(Key::Escape),
            )
        });
        // Copy / cut / paste arrive as their own events.
        let (copy, cut, paste) = ctx.input(|i| {
            let mut c = (false, false, false);
            for e in &i.events {
                match e {
                    egui::Event::Copy => c.0 = true,
                    egui::Event::Cut => c.1 = true,
                    egui::Event::Paste(_) => c.2 = true,
                    _ => {}
                }
            }
            c
        });
        if redo {
            self.redo();
        } else if undo {
            self.undo();
        }
        if redo2 {
            self.redo();
        }
        if dup {
            self.duplicate();
        }
        if del {
            self.delete_selection();
        }
        if esc {
            self.selection = Selection::None;
        }
        if copy {
            self.copy_selection();
        }
        if cut && self.copy_selection() {
            self.delete_selection();
        }
        if paste {
            let at = self.hover_f.unwrap_or((self.sim.world.w as f32 / 2.0, self.sim.world.h as f32 / 3.0));
            self.paste_at(at.0, at.1);
        }

        let plain = ctx.input(|i| i.modifiers.is_none());
        if !plain {
            return;
        }
        let (space, h, r, q, e, f, v, b) = ctx.input(|i| {
            (
                i.key_pressed(Key::Space),
                i.key_pressed(Key::H),
                i.key_pressed(Key::R),
                i.key_pressed(Key::Q),
                i.key_pressed(Key::E),
                i.key_pressed(Key::F),
                i.key_pressed(Key::V),
                i.key_pressed(Key::B),
            )
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
        if q || e {
            let step = 15f32.to_radians() * if q { -1.0 } else { 1.0 };
            self.edit_objects("turn", |s| s.angle += step);
        }
        if f {
            self.transform_selection(Transform::FlipH, "flip");
        }
        if v {
            self.tool = Tool::Select;
        }
        if b {
            self.tool = Tool::Paint;
        }
    }
}

fn nice_mass(kg: f32) -> String {
    if kg >= 1000.0 { format!("{:.1} tonnes", kg / 1000.0) } else { format!("{kg:.0} kg") }
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
        let dropped: Vec<PathBuf> =
            ui.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        for path in dropped {
            self.import_file(&path);
        }
        self.shortcuts(&ctx);

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
        egui::Panel::right("inspector").resizable(false).exact_size(270.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.inspector(ui));
        });
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(10, 10, 14)))
            .show(ui, |ui| self.canvas(ui));

        // Inspector number drags etc. become one undo step once the mouse is let go.
        let mouse_down = ctx.input(|i| i.pointer.any_down());
        if !mouse_down && matches!(self.drag, Drag::None) {
            self.history.commit_if_idle(&mut self.sim, 0.3);
        }

        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    //! Click-through tests: real mouse and keyboard events sent to the editor.
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use eframe::egui::{Event, PointerButton};

    const SCENE: &str = "size 200 120
fill air
rect stone 0 100 200 20
rect stone 150 70 20 30
body rect wood 40 60 20 10
";

    fn editor() -> Harness<'static, Sandbox> {
        let mut h = Harness::builder()
            .with_size(Vec2::new(1500.0, 900.0))
            .allow_missing_glyphs()
            .build_eframe(|_cc| Sandbox::new(PathBuf::from("materials"), None));
        let app = h.state_mut();
        let (sim, problems) = scene::load(SCENE, &app.mats, None).into_sim(&app.mats);
        assert!(problems.is_empty(), "{problems:?}");
        app.sim = sim;
        app.selection = Selection::None;
        h.run_steps(3);
        h
    }

    /// Screen position of the middle of a cell.
    fn at(h: &Harness<Sandbox>, x: f32, y: f32) -> Pos2 {
        let (min, s) = h.state().canvas_at;
        Pos2::new(min.x + x * s, min.y + y * s)
    }

    fn press(h: &mut Harness<Sandbox>, p: Pos2, down: bool) {
        h.event(Event::PointerMoved(p));
        h.event(Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: down, modifiers: Modifiers::NONE });
        h.run_steps(2);
    }

    /// Press at `a`, move to `b` in a few steps, let go.
    fn drag(h: &mut Harness<Sandbox>, a: Pos2, b: Pos2) {
        press(h, a, true);
        for k in 1..=6 {
            let t = k as f32 / 6.0;
            h.event(Event::PointerMoved(a + (b - a) * t));
            h.run_steps(1);
        }
        press(h, b, false);
        h.run_steps(2);
    }

    fn object(h: &Harness<Sandbox>) -> BodySpec {
        let app = h.state();
        let id = app.sim.physics.ids()[0];
        app.sim.physics.spec_of(&app.sim.world, id).unwrap()
    }

    fn count(h: &Harness<Sandbox>, name: &str) -> usize {
        let app = h.state();
        let id = app.mats.id(name).unwrap();
        app.sim.world.mat.iter().filter(|&&m| m == id).count()
    }

    #[test]
    fn click_select_and_drag_an_object_then_undo() {
        let mut h = editor();
        h.key_press(Key::V);
        h.run_steps(2);
        assert!(h.state().tool == Tool::Select);
        let start = object(&h);
        { let (a, b) = (at(&h, 50.0, 65.0), at(&h, 90.0, 45.0)); drag(&mut h, a, b); }
        assert!(matches!(h.state().selection, Selection::Objects(ref ids) if ids.len() == 1), "the object is selected");
        let moved = object(&h);
        assert!((moved.x - start.x - 40.0).abs() < 2.0 && (moved.y - start.y + 20.0).abs() < 2.0, "moved to ({}, {})", moved.x, moved.y);
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run_steps(3);
        let back = object(&h);
        assert!((back.x - start.x).abs() < 0.5 && (back.y - start.y).abs() < 0.5, "undo moves it back: ({}, {})", back.x, back.y);
        h.key_press_modifiers(Modifiers::COMMAND, Key::Y);
        h.run_steps(3);
        assert!((object(&h).x - moved.x).abs() < 0.5, "redo moves it again");
    }

    #[test]
    fn rotate_with_the_handle() {
        let mut h = editor();
        h.key_press(Key::V);
        { let p = at(&h, 50.0, 65.0); press(&mut h, p, true); }
        { let p = at(&h, 50.0, 65.0); press(&mut h, p, false); }
        let b = object(&h);
        let (rot, _) = handles(&b);
        // Drag the turn handle round to the right of the object: a quarter turn.
        { let (a, b) = (at(&h, rot.0, rot.1), at(&h, b.x + 30.0, b.y)); drag(&mut h, a, b); }
        let a = object(&h).angle.to_degrees();
        assert!((a - 90.0).abs() < 5.0, "angle {a}");
    }

    #[test]
    fn box_select_scenery_delete_and_undo() {
        let mut h = editor();
        h.key_press(Key::V);
        let stone = count(&h, "stone");
        { let (a, b) = (at(&h, 148.0, 68.0), at(&h, 172.0, 99.5)); drag(&mut h, a, b); }
        assert!(matches!(h.state().selection, Selection::Area(_)), "an area is selected");
        h.key_press(Key::Delete);
        h.run_steps(3);
        assert_eq!(count(&h, "stone"), stone - 20 * 30, "the pillar is gone");
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run_steps(3);
        assert_eq!(count(&h, "stone"), stone, "undo brings it back");
    }

    #[test]
    fn paint_stroke_is_one_undo_step() {
        let mut h = editor();
        h.key_press(Key::B);
        let sand = h.state().mats.id("sand").unwrap();
        h.state_mut().brush_mat = sand;
        { let (a, b) = (at(&h, 20.0, 20.0), at(&h, 120.0, 20.0)); drag(&mut h, a, b); }
        assert!(count(&h, "sand") > 300, "painted a line of sand");
        assert_eq!(h.state().history.len(), 1, "one stroke = one undo step");
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run_steps(3);
        assert_eq!(count(&h, "sand"), 0);
    }

    #[test]
    fn copy_paste_and_duplicate_objects() {
        let mut h = editor();
        h.key_press(Key::V);
        { let p = at(&h, 50.0, 65.0); press(&mut h, p, true); }
        { let p = at(&h, 50.0, 65.0); press(&mut h, p, false); }
        h.key_press_modifiers(Modifiers::COMMAND, Key::D);
        h.run_steps(3);
        assert_eq!(h.state().sim.physics.body_count(), 2, "duplicate makes a second object");
        h.key_press(Key::Delete);
        h.run_steps(3);
        assert_eq!(h.state().sim.physics.body_count(), 1, "delete removes the selected copy");
    }

    #[test]
    fn scale_with_the_corner_handle() {
        let mut h = editor();
        h.key_press(Key::V);
        { let p = at(&h, 50.0, 65.0); press(&mut h, p, true); }
        { let p = at(&h, 50.0, 65.0); press(&mut h, p, false); }
        let b = object(&h);
        let (_, corner) = handles(&b);
        let far = (b.x + (corner.0 - b.x) * 2.0, b.y + (corner.1 - b.y) * 2.0);
        { let (a, b) = (at(&h, corner.0, corner.1), at(&h, far.0, far.1)); drag(&mut h, a, b); }
        let after = object(&h);
        assert_eq!((after.w, after.h), (40, 20), "twice the size");
    }

    #[test]
    fn change_material_from_the_inspector() {
        let mut h = editor();
        h.key_press(Key::V);
        { let p = at(&h, 50.0, 65.0); press(&mut h, p, true); }
        { let p = at(&h, 50.0, 65.0); press(&mut h, p, false); }
        h.get_by_label("Change material to…").click();
        h.run_steps(2);
        h.get_all_by_label("iron").last().unwrap().click();
        h.run_steps(3);
        let b = object(&h);
        let iron = h.state().mats.id("iron").unwrap();
        assert!(b.cells.iter().flatten().all(|c| c.mat == iron), "the whole object is iron now");
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run_steps(3);
        let wood = h.state().mats.id("wood").unwrap();
        assert!(object(&h).cells.iter().flatten().all(|c| c.mat == wood), "undo makes it wood again");
    }

    #[test]
    fn drag_an_area_of_scenery() {
        let mut h = editor();
        h.key_press(Key::V);
        { let (a, b) = (at(&h, 148.0, 68.0), at(&h, 172.0, 99.5)); drag(&mut h, a, b); }
        let stone = h.state().mats.id("stone").unwrap();
        { let (a, b) = (at(&h, 160.0, 80.0), at(&h, 120.0, 80.0)); drag(&mut h, a, b); }
        let w = &h.state().sim.world;
        assert_eq!(w.mat[w.idx(125, 80)], stone, "the pillar is now further left");
        assert_ne!(w.mat[w.idx(165, 80)], stone, "and gone from where it was");
    }
}
