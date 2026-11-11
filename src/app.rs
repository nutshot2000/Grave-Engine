//! The editor window: paint materials, select and change things, press Play.

use eframe::egui::{self, Color32, Key, Modifiers, Pos2, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Vec2};
use grave_engine::edit::{self, Clip, History, Region, Timeline, Transform};
use grave_engine::import;
use grave_engine::material::{AIR, MatId, Materials, State};
use grave_engine::physics::{BodySpec, CELL, GROUND, JointKind, Motor};
use grave_engine::render::{Renderer, View};
use grave_engine::scene;
use grave_engine::sim::{Sim, Snapshot};
use grave_engine::notes::{self, Note};
use grave_engine::template::{self, Change, Templates};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

const SIM_HZ: f64 = 60.0;
const SELECT_COLOR: Color32 = Color32::from_rgb(255, 214, 90);
const AREA_COLOR: Color32 = Color32::from_rgb(90, 200, 255);

/// The fastest the editor throws things (cells per second: 60 m/s).
const MAX_THROW: f32 = 600.0;

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
    /// Click to place copies of the chosen template.
    Place,
    /// Click two things to join them.
    Join,
}

/// Which kind of joint the Join tool makes.
#[derive(Clone, Copy, PartialEq)]
enum JoinChoice {
    Hinge,
    Wheel,
    Weld,
    Rope,
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
    /// Recording of the last Play, for rewinding.
    timeline: Timeline,
    templates: Templates,
    /// The template the Place tool puts down.
    place_template: Option<String>,
    /// Name typed for "Save as template".
    template_name: String,
    join_kind: JoinChoice,
    join_motor: bool,
    join_speed: f32,
    join_strength: f32,
    join_drive: bool,
    /// The first thing clicked with the Join tool, and where.
    join_first: Option<(u64, (f32, f32))>,
    /// The moment we rewound to (None = at the end, live).
    rewind_pos: Option<usize>,
    /// How things were when Play was last pressed. Reset goes back here.
    snapshot: Snapshot,
    ran_since_snapshot: bool,
    playing: bool,
    /// Where the pointer was lately while dragging objects (time, cell), so
    /// letting go mid-swing throws them.
    fling: Vec<(f64, (f32, f32))>,
    speed: f32,
    /// Camera settings: how quickly it catches up (per second), whether it
    /// looks ahead of moving things, and whether trigger boxes are drawn.
    cam_smooth: f32,
    cam_lead: bool,
    show_triggers: bool,
    view: View,
    renderer: Renderer,
    texture: Option<TextureHandle>,
    /// Character animation frames uploaded to the GPU: (look folder, animation, frame).
    look_textures: std::collections::HashMap<(PathBuf, &'static str, usize), TextureHandle>,
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
    /// Camera: centre (cells) and zoom (screen pixels per cell). None = show
    /// the whole world.
    cam: Option<(f32, f32, f32)>,
    /// Mouse-wheel movement not yet turned into zoom steps.
    zoom_acc: f32,
    /// How big the canvas was last frame (screen points), to work out which
    /// part of the world is on screen and draw only that.
    view_size: Vec2,
    /// Sticky notes for the open scene (see notes.rs), and the scene file
    /// they belong to (they're kept in `NAME.notes.md` next to it).
    notes: Vec<Note>,
    notes_scene: PathBuf,
    notes_stamp: Option<SystemTime>,
    /// A note being written (or edited).
    note_draft: Option<NoteDraft>,
    /// Where the right-click menu was opened (cells).
    menu_at: Option<(f32, f32)>,
    show_done_notes: bool,
    /// The material whose settings window is open.
    mat_settings: Option<MatId>,
}

/// The note box: where it goes, the text so far, and which note it is
/// (None = a new one).
struct NoteDraft {
    x: f32,
    y: f32,
    text: String,
    editing: Option<usize>,
    focused: bool,
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
            timeline: Timeline::new(),
            templates: Templates::default(),
            place_template: None,
            template_name: String::new(),
            join_kind: JoinChoice::Hinge,
            join_motor: false,
            join_speed: 1.0,
            join_strength: 5.0,
            join_drive: true,
            join_first: None,
            rewind_pos: None,
            ran_since_snapshot: false,
            playing: false,
            fling: Vec::new(),
            speed: 1.0,
            cam_smooth: 7.0,
            cam_lead: true,
            show_triggers: true,
            view: View::default(),
            renderer: Renderer::default(),
            texture: None,
            look_textures: Default::default(),
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
            cam: None,
            zoom_acc: 0.0,
            view_size: Vec2::ZERO,
            notes: Vec::new(),
            notes_scene: PathBuf::from("scenes/my_scene.txt"),
            notes_stamp: None,
            note_draft: None,
            menu_at: None,
            show_done_notes: false,
            mat_settings: None,
        };
        for p in app.mats.problems.clone() {
            app.say(p);
        }
        app.templates = Templates::load_dir("templates", &app.mats);
        for p in app.templates.problems.clone() {
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

    /// Write the notes file (right away, so notes are never lost).
    fn save_notes(&mut self) {
        let file = notes::path_for(&self.notes_scene);
        let name = self.notes_scene.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        match notes::save(&file, &name, &self.notes) {
            Ok(()) => self.notes_stamp = stamp(&file),
            Err(e) => self.say(format!("Couldn't save notes to {}: {e}", file.display())),
        }
    }

    /// Save a close-up picture of a spot, with the object (if any) outlined
    /// and a pin where the note is. Returns its path relative to the notes file.
    fn note_picture(&mut self, x: f32, y: f32, object: Option<u64>) -> Option<String> {
        let dir = notes::pictures_dir(&self.notes_scene);
        std::fs::create_dir_all(&dir).ok()?;
        let n = (1..).find(|k| !dir.join(format!("note-{k}.png")).exists())?;
        let file = dir.join(format!("note-{n}.png"));
        let w = &self.sim.world;
        let rgba = self.renderer.draw(w, &self.mats, View::default()).to_vec();
        let (cw, ch) = (96.min(w.w), 54.min(w.h));
        let x0 = (x - cw as f32 / 2.0).clamp(0.0, (w.w - cw) as f32) as usize;
        let y0 = (y - ch as f32 / 2.0).clamp(0.0, (w.h - ch) as f32) as usize;
        const S: usize = 6;
        let (ow, oh) = (cw * S, ch * S);
        let mut out = vec![0u8; ow * oh * 4];
        for oy in 0..oh {
            for ox in 0..ow {
                let i = ((y0 + oy / S) * w.w + x0 + ox / S) * 4;
                out[(oy * ow + ox) * 4..(oy * ow + ox) * 4 + 4].copy_from_slice(&rgba[i..i + 4]);
            }
        }
        let mut dot = |px: f32, py: f32, r: f32, c: [u8; 3]| {
            let (cx, cy) = ((px - x0 as f32) * S as f32, (py - y0 as f32) * S as f32);
            for yy in (cy - r) as i32..=(cy + r) as i32 {
                for xx in (cx - r) as i32..=(cx + r) as i32 {
                    if xx >= 0 && yy >= 0 && (xx as usize) < ow && (yy as usize) < oh && ((xx as f32 - cx).powi(2) + (yy as f32 - cy).powi(2)) <= r * r {
                        let o = (yy as usize * ow + xx as usize) * 4;
                        out[o..o + 3].copy_from_slice(&c);
                    }
                }
            }
        };
        // Outline the object.
        if let Some(b) = object.and_then(|id| self.sim.physics.spec_of(&self.sim.world, id)) {
            let c = corners(&b, 1.08);
            for k in 0..4 {
                let (a, e) = (c[k], c[(k + 1) % 4]);
                let steps = (((e.0 - a.0).powi(2) + (e.1 - a.1).powi(2)).sqrt() * 4.0) as usize + 1;
                for t in 0..=steps {
                    let f = t as f32 / steps as f32;
                    dot(a.0 + (e.0 - a.0) * f, a.1 + (e.1 - a.1) * f, 2.0, [255, 220, 40]);
                }
            }
        }
        // The pin.
        dot(x, y, 7.0, [255, 255, 255]);
        dot(x, y, 5.0, [230, 40, 40]);
        grave_engine::headless::write_png(&file, ow, oh, &out, 1).ok()?;
        Some(format!("{}/note-{n}.png", dir.file_name()?.to_string_lossy()))
    }

    /// Change one connected piece of scenery (all touching cells of the
    /// same material) into another material (air = delete it).
    fn recolor_piece(&mut self, x: f32, y: f32, to: MatId) {
        let w = &self.sim.world;
        let (cx, cy) = (x.floor() as i32, y.floor() as i32);
        if !w.in_bounds(cx, cy) {
            return;
        }
        let start = w.idx(cx as usize, cy as usize);
        let from = w.mat[start];
        if from == AIR || w.is_owned(start) {
            return;
        }
        let mut seen = vec![false; w.mat.len()];
        let mut stack = vec![start];
        let mut piece = Vec::new();
        seen[start] = true;
        while let Some(i) = stack.pop() {
            piece.push(i);
            if piece.len() > 400_000 {
                break;
            }
            let (x, y) = (i % w.w, i / w.w);
            let near = [(x > 0).then(|| i - 1), (x + 1 < w.w).then(|| i + 1), (y > 0).then(|| i - w.w), (y + 1 < w.h).then(|| i + w.w)];
            for j in near.into_iter().flatten() {
                if !seen[j] && w.mat[j] == from && !w.is_owned(j) {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        let name = if to == AIR { "delete piece".to_string() } else { format!("change piece to {}", self.mats.get(to).name) };
        self.change(&name, |app| {
            let ww = app.sim.world.w;
            for &i in &piece {
                if to == AIR {
                    app.sim.world.make_air(i);
                } else {
                    app.sim.world.set(&app.mats, i % ww, i / ww, to);
                }
            }
        });
        self.say(format!("{} cells changed", piece.len()));
    }

    /// The right-click menu.
    fn context_menu(&mut self, ui: &mut egui::Ui) {
        let Some((x, y)) = self.menu_at else {
            ui.close();
            return;
        };
        let (about, object) = notes::describe(&self.sim, &self.mats, x, y);
        let short = about.split(" (").next().unwrap_or(&about).to_string();
        ui.label(RichText::new(&short).strong());
        ui.separator();
        if ui.button("📝 Leave a note here…").clicked() {
            self.note_draft = Some(NoteDraft { x, y, text: String::new(), editing: None, focused: false });
            ui.close();
        }
        if ui.button("💡 Put a lamp here").on_hover_text("A lamp lighting 40 cells around (change it in 💡 Lighting). On an object, it rides along").clicked() {
            self.history.touch(&mut self.sim, "lamp");
            let mut lamp = grave_engine::light::Light::at(x, y, 40.0);
            if let Some(id) = object {
                if let Some((cx, cy, a)) = self.sim.physics.pose_of(id) {
                    let (dx, dy) = (x - cx, y - cy);
                    let (c, s) = (a.cos(), a.sin());
                    (lamp.x, lamp.y) = (c * dx + s * dy, -s * dx + c * dy);
                    lamp.on = Some(id);
                }
            }
            // A lamp in full daylight shows nothing: dim the scene the first time.
            if self.sim.lighting.lights.is_empty() && self.sim.lighting.ambient > 0.6 {
                self.sim.lighting.ambient = 0.35;
            }
            self.sim.lighting.lights.push(lamp);
            self.history.commit(&mut self.sim);
            self.say("Lamp placed: change it in 💡 Lighting");
            ui.close();
        }
        // The material right here (an object's cell, or scenery).
        let (cx, cy) = (x.floor() as i32, y.floor() as i32);
        if self.sim.world.in_bounds(cx, cy) {
            let m = self.sim.world.mat[self.sim.world.idx(cx as usize, cy as usize)];
            if m != AIR && ui.button(format!("⚙ {} settings", self.mats.get(m).name.replace('_', " "))).clicked() {
                self.mat_settings = Some(m);
                ui.close();
            }
        }
        if let Some(id) = object {
            if let Some(m) = self.material_menu(ui, "🎨 Change material to") {
                self.selection = Selection::Objects(vec![id]);
                self.recolor_selection(m);
                ui.close();
            }
            if ui.button("👆 Select it").clicked() {
                self.selection = Selection::Objects(vec![id]);
                self.tool = Tool::Select;
                ui.close();
            }
            if ui.button("📷 Follow it with the camera").clicked() {
                self.sim.follow = Some(id);
                ui.close();
            }
            if ui.button("❄ Freeze into scenery").clicked() {
                self.selection = Selection::Objects(vec![id]);
                self.freeze_selection();
                ui.close();
            }
            if ui.button("⧉ Duplicate").clicked() {
                self.selection = Selection::Objects(vec![id]);
                self.duplicate();
                ui.close();
            }
            if ui.button("🗑 Delete").clicked() {
                self.selection = Selection::Objects(vec![id]);
                self.delete_selection();
                ui.close();
            }
        } else if short != "empty space" && short != "outside the world" {
            if let Some(m) = self.material_menu(ui, "🎨 Change this piece to") {
                self.recolor_piece(x, y, m);
                ui.close();
            }
            if ui.button("🗑 Delete this piece").clicked() {
                self.recolor_piece(x, y, AIR);
                ui.close();
            }
        }
    }

    /// The note box (a small window), when writing or editing a note.
    fn note_window(&mut self, ctx: &egui::Context) {
        let Some(d) = self.note_draft.as_mut() else { return };
        let (about, object) = notes::describe(&self.sim, &self.mats, d.x, d.y);
        let mut save = false;
        let mut cancel = false;
        let mut remove = false;
        let mut done = None;
        let editing = d.editing;
        let title = if editing.is_some() { "📝 Note" } else { "📝 New note" };
        egui::Window::new(title).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 90.0)).show(ctx, |ui| {
            ui.label(RichText::new(format!("On: {about}")).size(13.0).weak());
            let edit = ui.add(
                egui::TextEdit::multiline(&mut d.text)
                    .hint_text("e.g. make this crate iron, or: add a bounce pad here")
                    .desired_rows(3)
                    .desired_width(360.0),
            );
            if !d.focused {
                edit.request_focus();
                d.focused = true;
            }
            ui.label(RichText::new("Ctrl+Enter saves. Notes go in the scene's .notes.md file, with a picture.").size(12.0).weak());
            ui.horizontal(|ui| {
                save = ui.button("💾 Save note").clicked() || ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.command);
                cancel = ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape));
                if let Some(i) = editing {
                    let is_done = self.notes.get(i).is_some_and(|n| n.done);
                    if ui.button(if is_done { "↺ Not done" } else { "✔ Done" }).clicked() {
                        done = Some(!is_done);
                    }
                    remove = ui.button("🗑 Delete").clicked();
                }
            });
        });
        let d = self.note_draft.take().unwrap();
        if save && !d.text.trim().is_empty() {
            match d.editing {
                Some(i) if i < self.notes.len() => self.notes[i].text = d.text.trim().to_string(),
                _ => {
                    let mut n = Note::new(d.x, d.y, &d.text);
                    n.about = about;
                    n.picture = self.note_picture(d.x, d.y, object);
                    n.run = self.sim.physics.run_id();
                    n.object = object.and_then(|id| self.sim.physics.spec_of(&self.sim.world, id).map(|b| (n.run, id, d.x - b.x, d.y - b.y)));
                    self.notes.push(n);
                    self.say("Note saved");
                }
            }
            self.save_notes();
        } else if let (Some(i), Some(v)) = (d.editing, done) {
            if let Some(n) = self.notes.get_mut(i) {
                n.done = v;
            }
            self.save_notes();
        } else if let (Some(i), true) = (d.editing, remove) {
            if i < self.notes.len() {
                self.notes.remove(i);
            }
            self.save_notes();
        } else if !cancel && !save {
            self.note_draft = Some(d); // still writing
        }
    }

    /// A material's settings, as sliders. Your changes go in
    /// materials/my_settings/NAME.txt over the defaults (materials/NAME.txt).
    /// The 🏞 Background menu: sky and parallax layers behind the world.
    fn background_menu(&mut self, ui: &mut egui::Ui) {
        use grave_engine::backdrop::{Backdrop, Kind, Layer, PRESETS};
        ui.set_min_width(320.0);
        ui.label(RichText::new("Ready-made").strong());
        let h = self.sim.world.h;
        ui.horizontal_wrapped(|ui| {
            for (name, lines) in PRESETS {
                if ui.button(*name).clicked() {
                    self.sim.backdrop = Backdrop::preset(lines, h);
                }
            }
            if ui.button("✖ None").clicked() {
                self.sim.backdrop = Backdrop::default();
            }
        });
        ui.separator();
        let b = &mut self.sim.backdrop;
        let mut has_sky = b.sky.is_some();
        if ui.checkbox(&mut has_sky, RichText::new("Sky").strong()).changed() {
            b.sky = has_sky.then_some(([91, 143, 208], [191, 224, 245], false));
        }
        if let Some((top, bottom, stars)) = b.sky.as_mut() {
            ui.horizontal(|ui| {
                ui.label("top");
                ui.color_edit_button_srgb(top);
                ui.label("bottom");
                ui.color_edit_button_srgb(bottom);
                ui.checkbox(stars, "stars");
            });
        }
        ui.separator();
        ui.label(RichText::new(format!("Layers, far to near ({})", b.layers.len())).strong());
        let mut remove = None;
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for (n, l) in b.layers.iter_mut().enumerate() {
                ui.push_id(n, |ui| {
                    ui.horizontal(|ui| {
                        let kind = match &l.kind {
                            Kind::Hills => "hills",
                            Kind::Mountains => "mountains",
                            Kind::City => "city",
                            Kind::Forest => "forest",
                            Kind::Clouds => "clouds",
                            Kind::Image(_) => "picture",
                        };
                        ui.label(format!("{}. {kind}", n + 1));
                        ui.color_edit_button_srgb(&mut l.color);
                        if matches!(l.kind, Kind::Mountains | Kind::City) {
                            ui.checkbox(&mut l.extra, if matches!(l.kind, Kind::Mountains) { "snow" } else { "windows" });
                        }
                        if ui.small_button("🗑").on_hover_text("Remove this layer").clicked() {
                            remove = Some(n);
                        }
                    });
                    ui.add(egui::Slider::new(&mut l.depth, 0.0..=1.0).text("depth (0 far, 1 near)"));
                    ui.add(egui::Slider::new(&mut l.y, 0.0..=h as f32).text("ground line"));
                    ui.add(egui::Slider::new(&mut l.height, 1.0..=h as f32).text("height"));
                    ui.add(egui::Slider::new(&mut l.haze, 0.0..=1.0).text("haze"));
                    let mut seed = l.seed as f32;
                    if ui.add(egui::Slider::new(&mut seed, 1.0..=99.0).step_by(1.0).text("shape")).changed() {
                        l.seed = seed as u32;
                    }
                    ui.separator();
                });
            }
        });
        if let Some(n) = remove {
            b.layers.remove(n);
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Add:");
            for (name, kind, colour) in [
                ("⛰ mountains", Kind::Mountains, [90, 96, 130]),
                ("hills", Kind::Hills, [80, 110, 90]),
                ("🏙 city", Kind::City, [35, 40, 55]),
                ("🌲 forest", Kind::Forest, [40, 70, 50]),
                ("☁ clouds", Kind::Clouds, [255, 255, 255]),
            ] {
                if ui.button(name).clicked() {
                    let hf = h as f32;
                    let (y, height) = if matches!(kind, Kind::Clouds) { (hf * 0.12, hf * 0.1) } else { (hf * 0.8, hf * 0.25) };
                    b.layers.push(Layer::new(kind, colour, 0.4, y, height));
                }
            }
        });
        ui.label(RichText::new("In a scene: `sky #top #bottom stars`, `layer KIND #colour depth D y Y height H`. Pictures: `layer image FILE depth D y Y`").size(12.0).weak());
    }

    /// The 💡 Lighting menu: overall light, sun, and each lamp.
    fn lighting_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(300.0);
        let colour = |ui: &mut egui::Ui, c: &mut [u8; 3]| ui.color_edit_button_srgb(c).changed();
        let l = &mut self.sim.lighting;
        let mut changed = false;
        ui.label(RichText::new("Overall light").strong());
        ui.horizontal(|ui| {
            changed |= ui.add(egui::Slider::new(&mut l.ambient, 0.0..=1.0).text("brightness")).on_hover_text("0 = pitch dark, 1 = full daylight").changed();
            changed |= colour(ui, &mut l.ambient_color);
        });
        ui.horizontal(|ui| {
            for (name, a, c) in [("☀ Day", 1.0, [255, 255, 255]), ("🌇 Dusk", 0.5, [255, 190, 150]), ("🌙 Night", 0.2, [96, 112, 176]), ("🕳 Cave", 0.04, [255, 255, 255])] {
                if ui.button(name).clicked() {
                    l.ambient = a;
                    l.ambient_color = c;
                    changed = true;
                }
            }
        });
        ui.separator();
        let mut sun = l.sun.is_some();
        if ui.checkbox(&mut sun, RichText::new("Sun (casts shadows)").strong()).changed() {
            l.sun = sun.then_some(grave_engine::light::Sun { angle: 30.0, strength: 0.8, color: [255, 240, 215] });
            changed = true;
        }
        if let Some(s) = l.sun.as_mut() {
            changed |= ui.add(egui::Slider::new(&mut s.angle, -75.0..=75.0).suffix("°").text("angle")).on_hover_text("From straight down: positive = sun on the left").changed();
            ui.horizontal(|ui| {
                changed |= ui.add(egui::Slider::new(&mut s.strength, 0.0..=2.0).text("strength")).changed();
                changed |= colour(ui, &mut s.color);
            });
        }
        ui.separator();
        ui.label(RichText::new(format!("Lamps ({})", l.lights.len())).strong());
        if l.lights.is_empty() {
            ui.label(RichText::new("Right-click anywhere → 💡 Put a lamp here").weak());
        }
        let mut remove = None;
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for (n, lamp) in l.lights.iter_mut().enumerate() {
                ui.push_id(n, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}{}", n + 1, if lamp.on.is_some() { " (on an object)" } else { "" }));
                        changed |= colour(ui, &mut lamp.color);
                        if ui.small_button("🗑").on_hover_text("Remove this lamp").clicked() {
                            remove = Some(n);
                        }
                    });
                    changed |= ui.add(egui::Slider::new(&mut lamp.radius, 5.0..=300.0).logarithmic(true).text("reach")).changed();
                    changed |= ui.add(egui::Slider::new(&mut lamp.strength, 0.0..=4.0).text("strength")).changed();
                    changed |= ui.add(egui::Slider::new(&mut lamp.flicker, 0.0..=1.0).text("flicker")).changed();
                    let mut cone = lamp.cone.is_some();
                    if ui.checkbox(&mut cone, "Beam (a cone)").changed() {
                        lamp.cone = cone.then_some((90.0, 30.0));
                        changed = true;
                    }
                    if let Some((d, w)) = lamp.cone.as_mut() {
                        changed |= ui.add(egui::Slider::new(d, -180.0..=180.0).suffix("°").text("points (0 right, 90 down)")).changed();
                        changed |= ui.add(egui::Slider::new(w, 2.0..=90.0).suffix("°").text("width")).changed();
                    }
                    ui.separator();
                });
            }
        });
        if let Some(n) = remove {
            l.lights.remove(n);
            changed = true;
        }
        if changed {
            self.ran_since_snapshot = true;
        }
    }

    /// The 🎬 Camera menu: how close, how smooth, looking ahead, effects.
    fn camera_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(260.0);
        ui.label(RichText::new("Following").strong());
        let mut close = if self.sim.zoom > 0.0 { self.sim.zoom } else { 3.0 };
        if ui.add(egui::Slider::new(&mut close, 1.0..=20.0).logarithmic(true).text("how close")).on_hover_text("How much closer than the whole world the following camera is").changed() {
            self.sim.zoom = close;
        }
        ui.add(egui::Slider::new(&mut self.cam_smooth, 1.0..=20.0).logarithmic(true).text("snappiness"))
            .on_hover_text("How quickly the camera catches up: low is lazy and cinematic, high is tight");
        ui.checkbox(&mut self.cam_lead, "Look ahead").on_hover_text("Look a little ahead of fast-moving things");
        if ui.button("📷 Follow the selected object").clicked() {
            self.start_following();
            ui.close();
        }
        ui.separator();
        ui.label(RichText::new("Effects (during Play)").strong());
        ui.label(RichText::new("Triggers can do these too: slowmo, zoom, focus, shake (see the trigger lines in a scene)").size(12.0).weak());
        if ui.button("🐢 Slow motion for 2 seconds").clicked() {
            self.sim.camera.slow_motion(0.25, 2.0);
            self.playing = true;
        }
        if ui.button("🔍 Zoom in for 2 seconds").clicked() {
            self.sim.camera.zoom_in(2.0, 2.0);
            self.playing = true;
        }
        if ui.button("💥 Shake").clicked() {
            self.sim.camera.shake(4.0);
            self.playing = true;
        }
        if ui.button("↺ Camera back to normal").clicked() {
            self.sim.camera.reset();
        }
        ui.separator();
        ui.checkbox(&mut self.show_triggers, "Show trigger boxes").on_hover_text("Draw where the scene's triggers are and what they do");
    }

    /// Outline each trigger box, with what it does.
    fn draw_triggers(&self, painter: &egui::Painter, origin: Pos2, scale: f32) {
        for t in &self.sim.triggers {
            let r = Rect::from_min_size(origin + Vec2::new(t.x * scale, t.y * scale), Vec2::new(t.w * scale, t.h * scale));
            let colour = if t.fired && !t.repeat { Color32::from_rgba_unmultiplied(150, 150, 150, 120) } else { Color32::from_rgba_unmultiplied(255, 210, 60, 200) };
            painter.rect_stroke(r, 0.0, egui::Stroke::new(1.5, colour), egui::StrokeKind::Inside);
            let line = t.to_line("");
            let what = line.split_once(" : ").map_or("", |p| p.1);
            let what: String = what.chars().take(48).collect();
            painter.text(r.left_top() + Vec2::new(3.0, 2.0), egui::Align2::LEFT_TOP, format!("⚡ {what}"), egui::FontId::proportional(11.0), colour);
        }
    }

    fn material_settings_window(&mut self, ctx: &egui::Context) {
        let Some(id) = self.mat_settings else { return };
        if id as usize >= self.mats.list.len() {
            self.mat_settings = None;
            return;
        }
        let cur = self.mats.get(id).clone();
        let def = self.mats.default_of(id).clone();
        let changed = |key: &str| cur.custom.iter().any(|k| k == key);
        // (key in the file, value) to set, or None to go back to the default.
        let edits: std::cell::RefCell<Vec<(String, String, Option<String>)>> = Default::default();
        let mut reset_all = false;
        let mut open = true;
        let name = cur.name.replace('_', " ");
        egui::Window::new(format!("⚙ {name} settings")).open(&mut open).default_width(380.0).resizable(true).show(ctx, |ui| {
            ui.label(RichText::new(&cur.about).italics());
            ui.label(
                RichText::new(format!(
                    "Changes apply straight away. They're kept in materials/my_settings/{}.txt; the defaults stay in materials/{}.txt. Hover a slider to see its default.",
                    cur.name, cur.name
                ))
                .size(12.0)
                .weak(),
            );
            egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                let num = |ui: &mut egui::Ui, key: &str, label: &str, help: &str, now: f32, was: f32, range: std::ops::RangeInclusive<f32>, log: bool| {
                    ui.horizontal(|ui| {
                        ui.add_sized([150.0, 20.0], egui::Label::new(label)).on_hover_text(help);
                        let mut v = now;
                        let slider = egui::Slider::new(&mut v, range).logarithmic(log);
                        if ui.add(slider).on_hover_text(format!("{help}\nDefault: {}", nice_num(was))).changed() {
                            let reset = (v - was).abs() <= f32::EPSILON.max(was.abs() * 1e-4);
                            edits.borrow_mut().push((key.to_string(), key.to_string(), (!reset).then(|| nice_num(v))));
                        }
                        if changed(key) && ui.small_button("↺").on_hover_text(format!("Back to the default: {}", nice_num(was))).clicked() {
                            edits.borrow_mut().push((key.to_string(), key.to_string(), None));
                        }
                    });
                };
                ui.heading("Look");
                ui.horizontal(|ui| {
                    ui.add_sized([150.0, 20.0], egui::Label::new("Colour")).on_hover_text("Its colour (when it has no texture, and for its button)");
                    let mut c = cur.color;
                    if ui.color_edit_button_srgb(&mut c).changed() {
                        let v = format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
                        edits.borrow_mut().push(("color".into(), "color".into(), (c != def.color).then_some(v)));
                    }
                    if changed("color") && ui.small_button("↺").clicked() {
                        edits.borrow_mut().push(("color".into(), "color".into(), None));
                    }
                });
                num(ui, "color_variation", "Speckle", "How much each cell's shade varies", cur.color_variation, def.color_variation, 0.0..=1.0, false);
                num(ui, "glow", "Glow", "How much light it gives off", cur.glow, def.glow, 0.0..=2.0, false);

                ui.heading("Weight and movement");
                num(ui, "density", "Weight (kg/m³)", "Heavier things sink below lighter ones. Water is 1000", cur.density, def.density, 1.0..=20000.0, true);
                num(ui, "friction", "Grip", "How much things grip it (ice 0.02, rubber 1.2)", cur.friction, def.friction, 0.0..=2.0, false);
                num(ui, "bounciness", "Bounce", "How much things bounce off it (0 none, 1 perfect)", cur.bounciness, def.bounciness, 0.0..=1.0, false);
                num(ui, "launch", "Bounce pad (m/s)", "Flings anything landing on it upwards this fast (0 = off)", cur.launch, def.launch, 0.0..=40.0, false);
                if cur.state != State::Gas {
                    num(ui, "rolling_resistance", "Slows wheels", "How much it slows wheels rolling on it, per second (sand 0.6, mud 1.8, hard ground 0)", cur.rolling_resistance, def.rolling_resistance, 0.0..=3.0, false);
                }
                if matches!(cur.state, State::Powder | State::Liquid) || cur.tyres_dig {
                    num(ui, "clump", "Thrown in clumps of", "Tyres throw it in clumps of this many cells (dirt and mud 3-4, sand 1)", cur.clump as f32, def.clump as f32, 1.0..=8.0, false);
                }
                if cur.state == State::Solid {
                    ui.horizontal(|ui| {
                        ui.add_sized([150.0, 20.0], egui::Label::new("Tyres dig it up")).on_hover_text("Driven tyres chew up its top layer and throw it (dirt)");
                        let mut v = cur.tyres_dig;
                        if ui.checkbox(&mut v, "").on_hover_text(format!("Default: {}", if def.tyres_dig { "yes" } else { "no" })).changed() {
                            edits.borrow_mut().push(("tyres_dig".into(), "tyres_dig".into(), (v != def.tyres_dig).then(|| if v { "yes" } else { "no" }.to_string())));
                        }
                    });
                }
                if cur.state == State::Powder {
                    num(ui, "slides", "Slides down piles", "How readily it slides down the side of a pile (1 = dry sand)", cur.slides, def.slides, 0.0..=1.0, false);
                }
                if matches!(cur.state, State::Liquid | State::Gas) {
                    num(ui, "flows", "Flows", "How far it spreads sideways each frame", cur.flows as f32, def.flows as f32, 0.0..=16.0, false);
                }

                ui.heading("Heat");
                num(ui, "temperature", "Starts at (°C)", "Its temperature when painted", cur.temperature, def.temperature, -100.0..=3000.0, false);
                num(ui, "conducts_heat", "Conducts heat", "How quickly heat passes through it (0..1)", cur.conducts_heat, def.conducts_heat, 0.0..=1.0, false);
                num(ui, "heat_capacity", "Heat capacity", "How much heat it takes to warm up (water 4)", cur.heat_capacity, def.heat_capacity, 0.05..=10.0, true);
                num(ui, "cools_off", "Cools off", "How quickly it drifts back to room temperature by itself", cur.cools_off, def.cools_off, 0.0..=0.1, false);
                for (key, label, now, was) in [
                    ("melts_at", "Melts at (°C)", cur.melts.map(|c| c.at), def.melts.map(|c| c.at)),
                    ("boils_at", "Boils at (°C)", cur.boils.map(|c| c.at), def.boils.map(|c| c.at)),
                    ("freezes_at", "Freezes at (°C)", cur.freezes.map(|c| c.at), def.freezes.map(|c| c.at)),
                    ("condenses_at", "Condenses at (°C)", cur.condenses.map(|c| c.at), def.condenses.map(|c| c.at)),
                ] {
                    if let Some(now) = now.filter(|v| v.is_finite()) {
                        num(ui, key, label, "Turns into something else past this temperature", now, was.unwrap_or(now), -273.0..=4000.0, false);
                    }
                }
                if let Some(now) = cur.softens_at {
                    num(ui, "softens_at", "Softens at (°C)", "Past this it gets weaker the hotter it is (cracks, bends and sags more), down to nothing at its melting point", now, def.softens_at.unwrap_or(now), -100.0..=3000.0, false);
                }

                if cur.catches_fire_at.is_some() {
                    ui.heading("Fire");
                    let (now, was) = (cur.catches_fire_at.unwrap(), def.catches_fire_at.unwrap_or(cur.catches_fire_at.unwrap()));
                    num(ui, "catches_fire_at", "Catches fire at (°C)", "Bursts into flame past this temperature, if it touches air", now, was, 0.0..=2000.0, false);
                    num(ui, "burns_for", "Burns for (frames)", "How long it burns before it's gone (60 = a second)", cur.burns_for as f32, def.burns_for as f32, 1.0..=6000.0, true);
                    num(ui, "burn_temperature", "Burns at (°C)", "How hot its flames are", cur.burn_temperature, def.burn_temperature, 100.0..=3000.0, false);
                    num(ui, "burns_into_chance", "Leaves ash", "Chance each burnt cell leaves what it burns into (ash)", cur.burns_into_chance, def.burns_into_chance, 0.0..=1.0, false);
                    num(ui, "gives_off_chance", "Smoke and flames", "How often burning puffs out fire or smoke", cur.gives_off_chance, def.gives_off_chance, 0.0..=1.0, false);
                    if cur.sparks.is_some() {
                        num(ui, "sparks_chance", "Sparks", "How often it throws glowing embers while burning (they drift, land, and can start new fires)", cur.sparks_chance, def.sparks_chance, 0.0..=0.05, false);
                    }
                }

                ui.heading("Strength");
                if cur.state == State::Solid {
                    num(ui, "strength", "Overhang strength", "How far it can stick out from a support before snapping", cur.strength, def.strength, 0.0..=500.0, false);
                }
                if let Some(t) = cur.toughness {
                    num(ui, "toughness", "Toughness (kN)", "How hard a hit it takes to crack it", t, def.toughness.unwrap_or(t), 10.0..=100_000.0, true);
                }
                if let Some(y) = cur.yield_strength {
                    num(ui, "yield", "Bends at (MPa)", "Metals: how strong it is before it bends for good (iron 200, steel 350, copper 70). Thicker parts take more", y, def.yield_strength.unwrap_or(y), 1.0..=2000.0, true);
                }
                if cur.explosion_size > 0.0 {
                    ui.heading("Explosion");
                    num(ui, "explosion_size", "Blast radius (cells)", "How far its explosion reaches", cur.explosion_size, def.explosion_size, 1.0..=64.0, false);
                    num(ui, "explosion_power", "Blast power (kN)", "How hard the blast hits at its centre", cur.explosion_power, def.explosion_power, 100.0..=200_000.0, true);
                }

                if !cur.reactions.is_empty() {
                    ui.heading("Reactions");
                    for r in &cur.reactions {
                        let (with, me, other) = (self.mats.get(r.with).name.clone(), self.mats.get(r.me_into).name.clone(), self.mats.get(r.other_into).name.clone());
                        let was = def.reactions.iter().find(|d| d.with == r.with).map_or(r.chance, |d| d.chance);
                        let key = format!("reaction:{with}");
                        ui.horizontal(|ui| {
                            ui.add_sized([150.0, 20.0], egui::Label::new(format!("With {}", with.replace('_', " "))))
                                .on_hover_text(format!("Touching {with}: this becomes {me} and the {with} becomes {other}. How likely, each frame."));
                            let mut c = r.chance;
                            if ui.add(egui::Slider::new(&mut c, 0.0001..=1.0).logarithmic(true)).on_hover_text(format!("Default: {}", nice_num(was))).changed() {
                                let value = format!("{with} -> {me} + {other}, chance {}", nice_num(c));
                                edits.borrow_mut().push((key.clone(), "reaction".into(), ((c - was).abs() > 1e-6).then_some(value)));
                            }
                            if changed(&key) && ui.small_button("↺").on_hover_text(format!("Back to the default: {}", nice_num(was))).clicked() {
                                edits.borrow_mut().push((key.clone(), "reaction".into(), None));
                            }
                        });
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                let any = !cur.custom.is_empty();
                if ui.add_enabled(any, egui::Button::new("↺ Reset all to default")).on_hover_text("Forget all your changes to this material").clicked() {
                    reset_all = true;
                }
                if any {
                    ui.label(RichText::new(format!("{} changed", cur.custom.len())).size(13.0).weak());
                } else {
                    ui.label(RichText::new("All defaults").size(13.0).weak());
                }
            });
        });
        if !open {
            self.mat_settings = None;
        }
        let edits = edits.into_inner();
        if edits.is_empty() && !reset_all {
            return;
        }
        let mut pairs = if reset_all { Vec::new() } else { self.mats.my_settings(&cur.name) };
        for (ckey, file_key, value) in edits {
            pairs.retain(|(k, v)| grave_engine::material::custom_key(k, v) != ckey);
            if let Some(v) = value {
                pairs.push((file_key, v));
            }
        }
        match self.mats.save_my_settings(&cur.name, &pairs) {
            Ok(()) => {
                self.mats.reload();
                self.sim.physics.materials_changed(&self.mats);
                self.renderer.reload_textures();
                if reset_all {
                    self.say(format!("{name}: back to the defaults"));
                }
            }
            Err(e) => self.say(format!("Couldn't save the settings: {e}")),
        }
    }

    /// The notes list (bottom of the inspector).
    fn notes_panel(&mut self, ui: &mut egui::Ui) {
        let open = self.notes.iter().filter(|n| !n.done).count();
        ui.add_space(8.0);
        ui.separator();
        egui::CollapsingHeader::new(RichText::new(format!("📝 Notes ({open})")).strong()).default_open(true).show(ui, |ui| {
            ui.label(RichText::new("Right-click anything (👆 Select tool) to leave a note on it.").size(12.0).weak());
            if self.notes.is_empty() {
                return;
            }
            ui.checkbox(&mut self.show_done_notes, "Show done notes");
            let mut changed = false;
            let mut open_note = None;
            for (i, n) in self.notes.iter_mut().enumerate() {
                if n.done && !self.show_done_notes {
                    continue;
                }
                ui.horizontal(|ui| {
                    changed |= ui.checkbox(&mut n.done, "").on_hover_text("Done").changed();
                    let text: String = n.text.chars().take(34).collect();
                    let text = if n.text.chars().count() > 34 { format!("{text}…") } else { text };
                    let label = if n.done { RichText::new(text).strikethrough().weak() } else { RichText::new(text) };
                    if ui.link(label).on_hover_text(format!("{}\n\nOn: {}", n.text, n.about)).clicked() {
                        open_note = Some(i);
                    }
                });
            }
            if changed {
                self.save_notes();
            }
            if let Some(i) = open_note {
                let n = &self.notes[i];
                self.cam = Some((n.x, n.y, self.canvas_at.1.max(self.cam.map_or(0.0, |c| c.2))));
                self.note_draft = Some(NoteDraft { x: n.x, y: n.y, text: n.text.clone(), editing: Some(i), focused: false });
            }
        });
    }

    /// Note pins on the canvas. Returns the note under the pointer, if any.
    fn draw_notes(&self, painter: &egui::Painter, to_screen: &impl Fn((f32, f32)) -> Pos2, pointer: Option<Pos2>) -> Option<usize> {
        let mut hovered = None;
        for (i, n) in self.notes.iter().enumerate() {
            if n.done && !self.show_done_notes {
                continue;
            }
            let p = to_screen((n.x, n.y));
            let pin = Rect::from_center_size(p + Vec2::new(0.0, -12.0), Vec2::new(16.0, 16.0));
            let fill = if n.done { Color32::from_rgb(120, 160, 120) } else { Color32::from_rgb(255, 214, 64) };
            painter.line_segment([p, pin.center_bottom()], Stroke::new(2.0, Color32::from_rgb(60, 50, 20)));
            painter.circle_filled(p, 3.0, Color32::from_rgb(220, 40, 40));
            painter.rect_filled(pin, 2.0, fill);
            painter.rect_stroke(pin, 2.0, Stroke::new(1.0, Color32::from_rgb(60, 50, 20)), egui::StrokeKind::Middle);
            painter.text(pin.center(), egui::Align2::CENTER_CENTER, if n.done { "✔" } else { "✎" }, egui::FontId::proportional(11.0), Color32::from_rgb(60, 50, 20));
            let near = pointer.is_some_and(|q| pin.expand(4.0).contains(q));
            if near {
                hovered = Some(i);
            }
            // Short text next to the pin; the whole note on hover.
            let full = near;
            let text: String = if full { n.text.clone() } else { n.text.chars().take(28).collect::<String>() + if n.text.chars().count() > 28 { "…" } else { "" } };
            let galley = painter.layout(text, egui::FontId::proportional(13.0), Color32::from_rgb(30, 26, 10), if full { 260.0 } else { f32::INFINITY });
            let at = pin.right_top() + Vec2::new(4.0, 0.0);
            let bg = Rect::from_min_size(at, galley.size() + Vec2::new(8.0, 4.0));
            painter.rect_filled(bg, 3.0, fill.gamma_multiply(if full { 1.0 } else { 0.85 }));
            painter.galley(at + Vec2::new(4.0, 2.0), galley, Color32::BLACK);
        }
        hovered
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

    fn fit_view(&mut self) {
        self.cam = None;
        self.sim.follow = None;
    }

    /// Follow the selected object, or else whatever a `drive` motor is
    /// attached to (the car).
    fn start_following(&mut self) {
        let pick = self.selected_ids().first().copied().or_else(|| {
            self.sim
                .physics
                .joints()
                .into_iter()
                .find(|j| j.motor.as_ref().is_some_and(|m| m.drive))
                .map(|j| j.a)
                .filter(|&a| a != GROUND)
        });
        match pick {
            Some(id) => {
                self.sim.follow = Some(id);
                if self.sim.zoom <= 0.0 {
                    self.sim.zoom = 3.0;
                }
            }
            None => self.say("Select an object to follow first"),
        }
    }

    /// Selected copies of templates remember this as one of their own changes.
    fn note_change(&mut self, c: Change) {
        for id in self.selected_ids() {
            if let Some(inst) = self.sim.instances.get_mut(&id) {
                inst.changes.push(c.clone());
            }
        }
    }

    fn transform_selection(&mut self, t: Transform, label: &str) {
        match self.selection.clone() {
            Selection::Objects(ids) => {
                let mats = Materials::clone(&self.mats);
                self.change(label, |app| {
                    for id in ids {
                        app.sim.physics.edit(&mut app.sim.world, &mats, id, |b| edit::transform_body(b, t));
                    }
                    app.note_change(Change::Transform(t));
                });
            }
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
                self.timeline.clear();
                self.rewind_pos = None;
                self.snapshot = self.sim.snapshot();
                self.ran_since_snapshot = false;
                self.playing = false;
                self.selection = Selection::None;
                self.hover = None;
                self.cam = None;
                self.scene_stamp = stamp(&path);
                self.notes_scene = path.clone();
                self.notes = notes::load(&notes::path_for(&path));
                self.notes_stamp = stamp(&notes::path_for(&path));
                self.say(format!("Loaded {}", path.display()));
                if self.notes.iter().any(|n| !n.done) {
                    self.say(format!("{} note(s) on this scene: see 📝 Notes", self.notes.iter().filter(|n| !n.done).count()));
                }
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
            self.renderer.reload_textures();
            if self.mats.problems.is_empty() {
                self.say("Materials reloaded");
            } else {
                for p in self.mats.problems.clone() {
                    self.say(p);
                }
            }
        }
        if self.templates.changed_on_disk() {
            self.templates.reload(&self.mats);
            for p in self.templates.problems.clone() {
                self.say(p);
            }
            let n = template::refresh(&mut self.sim, &self.templates, &self.mats);
            self.say(format!("Templates reloaded: {n} cop{} updated", if n == 1 { "y" } else { "ies" }));
        }
        let notes_file = notes::path_for(&self.notes_scene);
        let now = stamp(&notes_file);
        if now.is_some() && now != self.notes_stamp {
            self.notes = notes::load(&notes_file);
            self.notes_stamp = now;
            self.say("Notes updated");
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

    /// About to simulate: if we'd rewound, the old future is replaced; if the
    /// world was changed since the recording ended, start a new recording.
    fn continue_from_here(&mut self) {
        match self.rewind_pos.take() {
            Some(i) => self.timeline.truncate_after(i, &mut self.sim),
            None => {
                if self.timeline.last_frame() != Some(self.sim.world.frame) {
                    self.timeline.clear();
                }
            }
        }
        if self.timeline.is_empty() {
            self.timeline.record(&mut self.sim);
        }
    }

    fn play(&mut self) {
        self.history.commit(&mut self.sim);
        self.continue_from_here();
        self.snapshot = self.sim.snapshot();
        self.ran_since_snapshot = false;
        self.playing = true;
        self.sim_clock = 0.0;
    }

    fn step_once(&mut self) {
        if self.rewind_pos.is_some() || self.timeline.is_empty() {
            self.continue_from_here();
        }
        if !self.ran_since_snapshot {
            self.snapshot = self.sim.snapshot();
        }
        let t = Instant::now();
        self.sim.step(&self.mats);
        self.timeline.record(&mut self.sim);
        self.sim_ms = self.sim_ms * 0.9 + t.elapsed().as_secs_f32() * 1000.0 * 0.1;
        self.ran_since_snapshot = true;
    }

    fn reset(&mut self) {
        let snap = self.snapshot.clone();
        self.change("reset", |app| app.sim = Sim::restore(&snap, &app.mats));
        self.timeline.clear();
        self.rewind_pos = None;
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
            Tool::Select | Tool::Object | Tool::Place | Tool::Join => {}
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
                self.timeline.clear();
                self.rewind_pos = None;
                self.snapshot = self.sim.snapshot();
                self.ran_since_snapshot = false;
                self.playing = false;
                self.selection = Selection::None;
            }
            ui.separator();
            if ui.button(big("⤢ Fit")).on_hover_text("Show the whole world (Home). Mouse wheel zooms, middle-drag pans.").clicked() {
                self.fit_view();
            }
            let following = self.sim.follow.is_some();
            let mut on = following;
            if ui.toggle_value(&mut on, big("📷 Follow")).on_hover_text("Keep the camera on the selected object (or the car)").changed() {
                if on {
                    self.start_following();
                } else {
                    self.sim.follow = None;
                }
            }
            ui.separator();
            ui.toggle_value(&mut self.view.heat, big("🌡 Heat view"))
                .on_hover_text("Show temperature: blue is cold, red/yellow/white is hot (H)");
            ui.separator();
            ui.label("Speed");
            ui.add(egui::Slider::new(&mut self.speed, 0.05..=4.0).logarithmic(true).suffix("x"))
                .on_hover_text("How fast time runs. Below 1 is slow motion");
            let mut slow = self.speed < 0.99;
            if ui.toggle_value(&mut slow, big("🐢")).on_hover_text("Slow motion: a quarter speed (click again for normal)").changed() {
                self.speed = if slow { 0.25 } else { 1.0 };
            }
            ui.menu_button(big("🎬 Camera"), |ui| self.camera_menu(ui));
            ui.menu_button(big("💡 Lighting"), |ui| self.lighting_menu(ui));
            ui.menu_button(big("🏞 Background"), |ui| self.background_menu(ui));
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
                (Tool::Place, "🧩 Place", "Click to place copies of the chosen template"),
                (Tool::Join, "🔗 Join", "Click two things to join them (empty space = the ground)"),
            ] {
                ui.selectable_value(&mut self.tool, tool, RichText::new(label).size(16.0)).on_hover_text(tip);
            }
        });
        if self.tool == Tool::Join {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for (k, label, tip) in [
                    (JoinChoice::Hinge, "Hinge", "A pin both turn around: doors, pendulums, seesaws"),
                    (JoinChoice::Wheel, "Wheel", "An axle with a suspension spring: click the body, then the wheel"),
                    (JoinChoice::Weld, "Weld", "Glue them rigidly together"),
                    (JoinChoice::Rope, "Rope", "A rope from the first click to the second"),
                ] {
                    ui.selectable_value(&mut self.join_kind, k, label).on_hover_text(tip);
                }
            });
            if matches!(self.join_kind, JoinChoice::Hinge | JoinChoice::Wheel) {
                ui.checkbox(&mut self.join_motor, "With a motor");
                if self.join_motor {
                    ui.add(egui::Slider::new(&mut self.join_speed, -5.0..=5.0).text("turns/s"));
                    ui.add(egui::Slider::new(&mut self.join_strength, 0.5..=50.0).logarithmic(true).text("strength"));
                    ui.checkbox(&mut self.join_drive, "Drive with ← → / A D");
                }
            }
            ui.label(RichText::new(match self.join_first {
                None => "Click the first thing (or empty space for the ground).",
                Some(_) => "Now click the second thing.",
            }).size(13.0).weak());
        }
        ui.add_space(4.0);
        ui.label("Brush size (Shift + mouse wheel)");
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
                tip += "\n\nRight-click: its settings (sliders)";
                let r = ui.add(b).on_hover_text(tip);
                if r.clicked() {
                    self.brush_mat = id;
                    self.tool = Tool::Paint;
                }
                if r.secondary_clicked() {
                    self.mat_settings = Some(id);
                }
            }
        });
        if ui.button(format!("⚙ {} settings", self.mats.get(self.brush_mat).name.replace('_', " "))).on_hover_text("Play with this material's settings (or right-click any material)").clicked() {
            self.mat_settings = Some(self.brush_mat);
        }
        ui.separator();
        ui.label(RichText::new("Templates").strong());
        if self.templates.list.is_empty() {
            ui.label(RichText::new("None yet: select an object and press Save as template.").size(13.0).weak());
        }
        ui.horizontal_wrapped(|ui| {
            let names: Vec<String> = self.templates.list.iter().map(|t| t.name.clone()).collect();
            for name in names {
                let on = self.tool == Tool::Place && self.place_template.as_deref() == Some(name.as_str());
                if ui.selectable_label(on, name.replace('_', " ")).on_hover_text("Click, then click in the world to place copies").clicked() {
                    self.place_template = Some(name);
                    self.tool = Tool::Place;
                }
            }
        });
        ui.separator();
        ui.label(RichText::new("Import").strong());
        ui.label(RichText::new("Drop an image or 3D model onto the world to add it (.png .jpg .glb .obj .fbx .blend).").size(13.0));
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
                                        app.sim.world.wake_at(i);
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
        if self.sim.follow == Some(id) {
            if ui.button("📷 Stop following").clicked() {
                self.sim.follow = None;
            }
        } else if ui.button("📷 Follow with the camera").clicked() {
            self.sim.follow = Some(id);
            if self.sim.zoom <= 0.0 {
                self.sim.zoom = 3.0;
            }
        }
        let joints = self.sim.physics.joints_of(id);
        if !joints.is_empty() {
            ui.separator();
            let names: Vec<&str> = joints
                .iter()
                .map(|j| match j.kind {
                    JointKind::Hinge => "hinge",
                    JointKind::Wheel { .. } => "wheel",
                    JointKind::Weld => "weld",
                    JointKind::Rope { .. } => "rope",
                })
                .collect();
            ui.label(format!("🔗 Joints: {}", names.join(", ")));
            if ui.button("Remove its joints").clicked() {
                self.change("remove joints", |app| app.sim.physics.unjoin(id));
            }
        }
        ui.separator();
        self.template_section(ui, id, &b);
    }

    fn template_section(&mut self, ui: &mut egui::Ui, id: u64, b: &BodySpec) {
        match self.sim.instances.get(&id).cloned() {
            Some(inst) => {
                ui.label(RichText::new(format!("🧩 Copy of template \"{}\"", inst.template.replace('_', " "))).strong());
                if inst.changes.is_empty() {
                    ui.label(RichText::new("Exactly like the template.").weak());
                } else {
                    ui.label("Its own changes:");
                    for c in &inst.changes {
                        ui.label(format!("   • {}", c.describe()));
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Reset to template").on_hover_text("Undo this copy's own changes").clicked() {
                        if let Some(t) = self.templates.get(&inst.template).cloned() {
                            self.change("reset to template", |app| {
                                app.sim.physics.edit(&mut app.sim.world, &app.mats, id, |s| {
                                    s.w = t.shape.w;
                                    s.h = t.shape.h;
                                    s.cells = t.shape.cells.clone();
                                });
                                if let Some(i) = app.sim.instances.get_mut(&id) {
                                    i.changes.clear();
                                }
                            });
                        }
                    }
                    if ui.button("Update template from this").on_hover_text("Make the template look like this copy: every copy updates").clicked() {
                        let dir = self.templates.dir().to_path_buf();
                        match template::save(&dir, &inst.template, b, &self.mats) {
                            Ok(_) => {
                                self.change("update template", |app| {
                                    if let Some(i) = app.sim.instances.get_mut(&id) {
                                        i.changes.clear();
                                    }
                                });
                                self.say(format!("Template {} updated", inst.template));
                            }
                            Err(e) => self.say(e),
                        }
                    }
                    if ui.button("Unlink").on_hover_text("Make it a normal object (template changes won't affect it)").clicked() {
                        self.change("unlink", |app| {
                            app.sim.instances.remove(&id);
                        });
                    }
                });
            }
            None => {
                ui.label(RichText::new("Save as template").strong());
                ui.label(RichText::new("Reuse this object: place copies from the Templates list.").size(13.0).weak());
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.template_name).hint_text("name, e.g. crate").desired_width(140.0));
                    if ui.button("💾 Save template").clicked() {
                        let dir = self.templates.dir().to_path_buf();
                        let name = template::clean_name(&self.template_name);
                        match name.ok_or("Type a name first".to_string()).and_then(|n| template::save(&dir, &n, b, &self.mats).map(|_| n)) {
                            Ok(n) => {
                                self.templates.reload(&self.mats);
                                let link = n.clone();
                                self.change("save as template", |app| {
                                    app.sim.instances.insert(id, template::Instance { template: link, changes: Vec::new() });
                                });
                                self.say(format!("Saved template {n}"));
                                self.template_name.clear();
                            }
                            Err(e) => self.say(e),
                        }
                    }
                });
            }
        }
    }

    fn recolor_selection(&mut self, to: MatId) {
        let name = self.mats.get(to).name.clone();
        match self.selection.clone() {
            Selection::Objects(ids) => {
                let mats = Materials::clone(&self.mats);
                let mname = name.clone();
                self.change(&format!("change to {name}"), |app| {
                    for id in ids {
                        app.sim.physics.edit(&mut app.sim.world, &mats, id, |b| edit::recolor(&mut b.cells, &mats, to));
                    }
                    app.note_change(Change::Material(mname));
                });
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

    /// The rewind slider: drag back through what happened during Play.
    fn timeline_bar(&mut self, ui: &mut egui::Ui) {
        let n = self.timeline.len();
        ui.horizontal(|ui| {
            ui.label(RichText::new("⏪ Rewind").strong());
            if n < 2 {
                ui.label(RichText::new("Press Play: everything that happens is recorded, then you can drag back to any moment here.").weak());
                return;
            }
            let last = n - 1;
            let mut pos = self.rewind_pos.unwrap_or(last);
            let end_frame = self.timeline.frame(last).unwrap_or(0);
            let width = (ui.available_width() - 260.0).max(120.0);
            ui.spacing_mut().slider_width = width;
            let resp = ui.add(egui::Slider::new(&mut pos, 0..=last).show_value(false));
            let secs = (self.timeline.frame(pos).unwrap_or(0) as f32 - end_frame as f32) / SIM_HZ as f32;
            ui.label(if pos == last { "now".to_string() } else { format!("{secs:.1} s") });
            if resp.changed() {
                self.playing = false;
                self.drag = Drag::None;
                self.history.commit(&mut self.sim);
                if self.timeline.restore(pos, &mut self.sim, &self.mats) {
                    self.rewind_pos = if pos == last { None } else { Some(pos) };
                    self.selection = Selection::None;
                }
            }
            if self.rewind_pos.is_some() {
                ui.label(RichText::new("Play carries on from here").weak().size(13.0));
            }
        });
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
                    Tool::Join => "Click the first thing, then the second (empty space = the ground). While playing, drive with ← → or A D.",
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
        self.renderer.light(&self.sim);
        // Backdrop layers slide by at their depth as the camera moves.
        let (w0, h0) = (self.sim.world.w as f32, self.sim.world.h as f32);
        let (cx0, cy0) = self.cam.map_or((w0 / 2.0, h0 / 2.0), |c| (c.0, c.1));
        self.renderer.set_camera(cx0, cy0);
        // Draw only the part of the world that's on screen (where the camera
        // was last frame, plus a margin): big worlds stay fast.
        let win = match self.cam {
            Some((x, y, s)) if self.view_size.x > 0.0 && s > 0.0 => {
                let (hw, hh) = (self.view_size.x / s / 2.0 + 12.0, self.view_size.y / s / 2.0 + 12.0);
                let (x0, y0) = ((x - hw).max(0.0) as usize, (y - hh).max(0.0) as usize);
                grave_engine::render::clip(&self.sim.world, (x0, y0, (hw * 2.0).ceil() as usize + 1, (hh * 2.0).ceil() as usize + 1))
            }
            _ => (0, 0, w, h),
        };
        let rgba = self.renderer.draw_window(&self.sim.world, &self.mats, self.view, win);
        let image = egui::ColorImage::from_rgba_unmultiplied([win.2, win.3], rgba);
        match &mut self.texture {
            Some(t) if t.size() == [win.2, win.3] => t.set(image, TextureOptions::NEAREST),
            _ => self.texture = Some(ui.ctx().load_texture("grid", image, TextureOptions::NEAREST)),
        }
        let tex = self.texture.as_ref().unwrap();

        // The camera: by default the whole world fits; zoomed in, it shows part
        // of it (and can follow an object).
        let avail = ui.available_size();
        let fit = (avail.x / w as f32).min(avail.y / h as f32).max(0.05);
        let (outer, _) = ui.allocate_exact_size(avail, Sense::hover());
        self.view_size = outer.size();
        // What to look at: a camera effect's focus (from a trigger), the
        // followed object, or the player; looking ahead of where it's going.
        let fx = &self.sim.camera;
        let target: Option<((f32, f32), (f32, f32))> = if let Some(p) = fx.focus_point(&self.sim).filter(|_| self.playing) {
            Some((p, (0.0, 0.0)))
        } else if let Some(b) = self.sim.follow.and_then(|id| self.sim.physics.spec_of(&self.sim.world, id)) {
            Some(((b.x, b.y), (b.vx, b.vy)))
        } else if self.playing {
            self.sim.physics.actor_status().first().map(|p| ((p.x, p.y), (0.0, 0.0)))
        } else {
            None
        };
        let fx_zoom = if self.playing { fx.zoom.map_or(1.0, |z| z.value) } else { 1.0 };
        if let Some(((x, y), (vx, vy))) = target {
            let zoom = if self.sim.zoom > 0.0 { self.sim.zoom } else { 3.0 } * fx_zoom;
            let lead = if self.cam_lead && self.playing { 0.3 } else { 0.0 };
            let (tx, ty) = (x + vx * lead, y + vy * lead * 0.5);
            let cam = self.cam.get_or_insert((tx, ty, fit * zoom));
            let dt = ui.input(|i| i.stable_dt).clamp(0.0, 0.1);
            let k = 1.0 - (-dt * if self.playing { self.cam_smooth } else { 25.0 }).exp();
            cam.0 += (tx - cam.0) * k;
            cam.1 += (ty - cam.1) * k;
            cam.2 += (fit * zoom - cam.2) * (1.0 - (-dt * 4.0).exp());
        }
        let (mut cx, mut cy, scale) = match self.cam {
            Some((x, y, s)) => (x, y, s.clamp(fit, fit * 40.0)),
            None => (w as f32 / 2.0, h as f32 / 2.0, fit),
        };
        // Don't scroll past the edges of the world.
        let (hw, hh) = (outer.width() / scale / 2.0, outer.height() / scale / 2.0);
        cx = if hw * 2.0 >= w as f32 { w as f32 / 2.0 } else { cx.clamp(hw, w as f32 - hw) };
        cy = if hh * 2.0 >= h as f32 { h as f32 / 2.0 } else { cy.clamp(hh, h as f32 - hh) };
        if self.cam.is_some() {
            self.cam = Some((cx, cy, scale));
        }
        // Shaking (a camera effect) nudges the picture, not the camera.
        let shake = if self.playing { self.sim.camera.shake_offset() } else { (0.0, 0.0) };
        let rect = Rect::from_min_size(outer.center() - Vec2::new((cx + shake.0) * scale, (cy + shake.1) * scale), Vec2::new(w as f32 * scale, h as f32 * scale));
        self.canvas_at = (rect.min, scale);
        let resp = ui.interact(outer, ui.scope_id().with("canvas"), Sense::click_and_drag());
        let painter = ui.painter_at(outer);
        let shown = Rect::from_min_size(rect.min + Vec2::new(win.0 as f32 * scale, win.1 as f32 * scale), Vec2::new(win.2 as f32 * scale, win.3 as f32 * scale));
        painter.image(tex.id(), shown, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        self.draw_characters(ui, &painter, rect.min, scale);
        if self.show_triggers && !self.sim.triggers.is_empty() {
            self.draw_triggers(&painter, rect.min, scale);
        }

        // Mouse wheel zooms around the pointer (Shift + wheel: brush size).
        // Middle mouse drag pans.
        if let Some(p) = resp.hover_pos() {
            let (shift, wheel, middle, delta) = ui.input(|i| {
                (i.modifiers.shift, i.smooth_scroll_delta.y + i.smooth_scroll_delta.x, i.pointer.middle_down(), i.pointer.delta())
            });
            if shift {
                self.scroll_acc += wheel;
                while self.scroll_acc.abs() >= 30.0 {
                    let step = self.scroll_acc.signum() as i32;
                    self.brush_size = (self.brush_size + step).clamp(1, 40);
                    self.scroll_acc -= 30.0 * step as f32;
                }
            } else if wheel != 0.0 {
                self.zoom_acc += wheel;
                while self.zoom_acc.abs() >= 30.0 {
                    let step = self.zoom_acc.signum();
                    self.zoom_acc -= 30.0 * step;
                    let (_, _, s0) = self.cam.unwrap_or((cx, cy, fit));
                    let s1 = (s0 * 1.2f32.powf(step)).clamp(fit, fit * 40.0);
                    // Keep the cell under the pointer where it is.
                    let under = ((p.x - rect.min.x) / scale, (p.y - rect.min.y) / scale);
                    let off = p - outer.center();
                    self.cam = Some((under.0 - off.x / s1, under.1 - off.y / s1, s1));
                    if self.sim.follow.is_some() {
                        self.sim.zoom = s1 / fit;
                    }
                }
            }
            if middle && delta != Vec2::ZERO {
                let (x, y, s0) = self.cam.unwrap_or((cx, cy, scale));
                self.cam = Some((x - delta.x / s0, y - delta.y / s0, s0));
                self.sim.follow = None;
            }
        }

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
                if self.tool == Tool::Place {
                    if let Some(t) = self.place_template.as_ref().and_then(|n| self.templates.get(n)) {
                        // Ghost outline of where the copy will go.
                        let (cx, cy) = to_cells(p);
                        let ghost = BodySpec { x: cx, y: cy, angle: 0.0, ..t.shape.clone() };
                        let pts: Vec<Pos2> = corners(&ghost, 1.0).iter().map(|&c| to_screen(c)).collect();
                        painter.add(egui::Shape::closed_line(pts, Stroke::new(1.5, SELECT_COLOR.gamma_multiply(0.7))));
                    }
                } else if self.tool == Tool::Join {
                    if let Some((_, at)) = self.join_first {
                        painter.line_segment([to_screen(at), p], Stroke::new(1.5, SELECT_COLOR));
                    }
                } else if self.tool != Tool::Object {
                    painter.circle_stroke(p, r, Stroke::new(1.5, col));
                }
            }
        }

        // Note pins, and the right-click menu (Select tool).
        let pin_hover = self.draw_notes(&painter, &to_screen, pointer);
        if self.tool == Tool::Select {
            if resp.secondary_clicked() {
                self.menu_at = pointer.map(to_cells);
            }
            resp.context_menu(|ui| self.context_menu(ui));
        }
        let (primary, secondary, shift) = ui.input(|i| (i.pointer.primary_down(), i.pointer.secondary_down(), i.modifiers.shift));
        if let (Some(i), true) = (pin_hover, resp.clicked()) {
            let n = &self.notes[i];
            self.note_draft = Some(NoteDraft { x: n.x, y: n.y, text: n.text.clone(), editing: Some(i), focused: false });
        }
        let pressed = pin_hover.is_none()
            && (resp.drag_started() || (resp.is_pointer_button_down_on() && matches!(self.drag, Drag::None) && (primary || secondary)));
        let released = !primary && !secondary;

        // ---- start a drag
        if pressed && matches!(self.drag, Drag::None) {
            if let Some(p) = pointer {
                let cell = to_cell(p);
                let cf = to_cells(p);
                self.drag = match self.tool {
                    // (Right-click in Select opens the menu instead.)
                    Tool::Select if secondary => Drag::None,
                    _ if secondary => {
                        self.history.touch(&mut self.sim, "erase");
                        Drag::Paint
                    }
                    Tool::Select => self.start_select_drag(p, cell, cf, shift, &to_screen),
                    Tool::Object => Drag::MakeObject(cell),
                    Tool::Join => {
                        let hit = if self.sim.world.in_bounds(cell.0, cell.1) {
                            self.sim.physics.id_at(&self.sim.world, cell.0 as usize, cell.1 as usize)
                        } else {
                            None
                        };
                        let this = hit.unwrap_or(GROUND);
                        match self.join_first.take() {
                            None => {
                                self.join_first = Some((this, cf));
                                self.say(if this == GROUND { "First: the ground. Now click the second thing." } else { "Now click the second thing." });
                            }
                            Some((first, at)) if first == this => {
                                self.say("That's the same thing: click something else");
                                self.join_first = Some((first, at));
                            }
                            Some((first, at)) => {
                                let motor = (self.join_motor && matches!(self.join_kind, JoinChoice::Hinge | JoinChoice::Wheel)).then(|| Motor {
                                    speed: self.join_speed,
                                    strength: self.join_strength,
                                    drive: self.join_drive,
                                });
                                let centre = self.sim.physics.spec_of(&self.sim.world, this).map(|b| (b.x, b.y)).unwrap_or(cf);
                                let (kind, point, point_b) = match self.join_kind {
                                    JoinChoice::Hinge => (JointKind::Hinge, cf, None),
                                    JoinChoice::Wheel => (JointKind::Wheel { stiffness: 60.0, damping: 12.0, travel: 3.0 }, centre, None),
                                    JoinChoice::Weld => (JointKind::Weld, cf, None),
                                    JoinChoice::Rope => {
                                        let len = ((cf.0 - at.0).powi(2) + (cf.1 - at.1).powi(2)).sqrt();
                                        (JointKind::Rope { length: len }, at, Some(cf))
                                    }
                                };
                                let mut ok = false;
                                self.change("join", |app| ok = app.sim.physics.join(&app.mats, first, this, kind, point, point_b, motor));
                                self.say(if ok { "Joined" } else { "Couldn't join those" });
                            }
                        }
                        Drag::Paint
                    }
                    Tool::Place => {
                        let name = self.place_template.clone();
                        match name.and_then(|n| self.templates.get(&n).cloned()) {
                            Some(t) => {
                                let mut new_id = None;
                                self.change(&format!("place {}", t.name), |app| {
                                    new_id = template::place(&mut app.sim, &t, cf.0, cf.1, &app.mats);
                                });
                                if let Some(id) = new_id {
                                    self.selection = Selection::Objects(vec![id]);
                                }
                            }
                            None => self.say("Pick a template in the Templates list first"),
                        }
                        Drag::Paint
                    }
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
            Drag::Paint if matches!(self.tool, Tool::Place | Tool::Join) && !secondary => {
                if released {
                    self.drag = Drag::None;
                }
            }
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
                // Time from summed frame lengths (steady even if the clock isn't).
                let time = self.fling.last().map_or(0.0, |f| f.0) + ui.input(|i| i.stable_dt.clamp(0.001, 0.1)) as f64;
                if let Some(p) = pointer {
                    let now = to_cells(p);
                    self.fling.push((time, now));
                    if self.fling.len() > 6 {
                        self.fling.remove(0); // the last few frames (~0.1 s)
                    }
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
                    // Let go while swinging (during Play): it flies off at
                    // the pointer's speed. Throw things at things!
                    if let (true, Some(&(t0, p0)), Some(&(t1, p1))) = (self.playing, self.fling.first(), self.fling.last()) {
                        let dt = (t1 - t0) as f32;
                        if dt > 0.02 {
                            let (vx, vy) = ((p1.0 - p0.0) / dt, (p1.1 - p0.1) / dt);
                            let speed = vx.hypot(vy);
                            if speed > 15.0 {
                                let k = speed.min(MAX_THROW) / speed;
                                for id in self.selected_ids() {
                                    self.sim.physics.edit(&mut self.sim.world, &self.mats, id, |b| {
                                        b.vx = vx * k;
                                        b.vy = vy * k;
                                    });
                                }
                                self.say(format!("Thrown at {:.0} m/s", speed.min(MAX_THROW) * 0.1));
                            }
                        }
                    }
                    self.fling.clear();
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
        self.draw_joints(&painter, &to_screen);
        self.draw_hud(&painter, outer);
    }

    /// Characters with a look are drawn as their (sharp, full resolution)
    /// animation frames, on top of the world.
    fn draw_characters(&mut self, ui: &egui::Ui, painter: &egui::Painter, origin: Pos2, scale: f32) {
        let body_h = grave_engine::physics::actor_height();
        for a in self.sim.physics.actor_status() {
            let Some(look) = &a.look else { continue };
            let (frame, index) = look.frame(a.anim, a.anim_time);
            let key = (look.dir.clone(), a.anim, index);
            let tex = self.look_textures.entry(key).or_insert_with(|| {
                let img = egui::ColorImage::from_rgba_unmultiplied([frame.w, frame.h], &frame.rgba);
                ui.ctx().load_texture(format!("{}/{}/{index}", look.name, a.anim), img, TextureOptions::LINEAR)
            });
            let cpp = look.cells_per_px(body_h);
            let feet = a.y + body_h as f32 / 2.0;
            let left = a.x - frame.w as f32 * cpp / 2.0;
            let top = feet - look.feet * cpp;
            let r = Rect::from_min_size(
                origin + Vec2::new(left * scale, top * scale),
                Vec2::new(frame.w as f32 * cpp * scale, frame.h as f32 * cpp * scale),
            );
            let mirror = (a.facing < 0.0) == look.faces_right;
            let uv = if mirror {
                Rect::from_min_max(Pos2::new(1.0, 0.0), Pos2::new(0.0, 1.0))
            } else {
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))
            };
            // Hurt: flash red.
            let tint = if a.hurt_by.is_some() && !a.dead && (self.sim.world.frame / 5) % 2 == 0 {
                Color32::from_rgb(255, 150, 150)
            } else {
                Color32::WHITE
            };
            painter.image(tex.id(), r, uv, tint);
        }
    }

    /// The player's health and breath, top left of the view.
    fn draw_hud(&self, painter: &egui::Painter, outer: Rect) {
        let Some(p) = self.sim.physics.actor_status().into_iter().next() else { return };
        let at = outer.min + Vec2::new(12.0, 12.0);
        let bar = |y: f32, frac: f32, col: Color32, label: &str| {
            let r = Rect::from_min_size(at + Vec2::new(0.0, y), Vec2::new(160.0, 12.0));
            painter.rect_filled(r, 3.0, Color32::from_black_alpha(160));
            let mut fill = r.shrink(2.0);
            fill.set_width(fill.width() * frac.clamp(0.0, 1.0));
            painter.rect_filled(fill, 2.0, col);
            painter.text(r.right_center() + Vec2::new(8.0, 0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.0), Color32::WHITE);
        };
        let hurting = p.hurt_by.is_some() && (self.sim.world.frame / 6) % 2 == 0;
        let health_col = if hurting { Color32::from_rgb(255, 230, 120) } else { Color32::from_rgb(220, 60, 60) };
        bar(0.0, p.health / 100.0, health_col, &format!("health {:.0}", p.health));
        let mut y = 16.0;
        if p.breath < 10.0 {
            bar(y, p.breath / 10.0, Color32::from_rgb(90, 170, 255), "breath");
            y += 16.0;
        }
        let text = if p.dead {
            Some(format!("You {}. Back in a moment...", p.hurt_by.unwrap_or("died")))
        } else if let Some(h) = p.hurt_by {
            Some(format!("Ouch! ({h}, body {:.0}°C)", p.temp))
        } else if !self.playing {
            Some("Press Play, then A / D to walk and W to jump".to_string())
        } else {
            None
        };
        if let Some(t) = text {
            painter.text(at + Vec2::new(0.0, y + 4.0), egui::Align2::LEFT_TOP, t, egui::FontId::proportional(14.0), Color32::WHITE);
        }
    }

    /// Joints drawn on top of the world: pins, axles, welds, ropes.
    fn draw_joints(&self, painter: &egui::Painter, to_screen: &impl Fn((f32, f32)) -> Pos2) {
        let col = Color32::from_rgb(120, 230, 160);
        for j in self.sim.physics.joints() {
            let Some((ea, eb)) = self.sim.physics.joint_ends(&j) else { continue };
            let (pa, pb) = (to_screen(ea), to_screen(eb));
            match j.kind {
                JointKind::Hinge => {
                    painter.circle_stroke(pb, 4.0, Stroke::new(2.0, col));
                }
                JointKind::Wheel { .. } => {
                    painter.circle_stroke(pb, 3.0, Stroke::new(2.0, col));
                    painter.circle_filled(pb, 1.5, col);
                }
                JointKind::Weld => {
                    painter.rect_stroke(Rect::from_center_size(pb, Vec2::splat(6.0)), 0.0, Stroke::new(2.0, col), egui::StrokeKind::Middle);
                }
                JointKind::Rope { .. } => {
                    painter.line_segment([pa, pb], Stroke::new(1.5, Color32::from_rgb(200, 170, 120)));
                }
            }
            if j.motor.is_some() {
                painter.circle_stroke(pb, 7.0, Stroke::new(1.0, col.gamma_multiply(0.6)));
            }
        }
        if let (Tool::Join, Some((_, at))) = (self.tool, self.join_first) {
            painter.circle_filled(to_screen(at), 4.0, SELECT_COLOR);
        }
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
            self.join_first = None;
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
        if ctx.input(|i| i.key_pressed(Key::Home)) {
            self.fit_view();
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

/// A number for a settings file: no more decimals than it needs.
fn nice_num(v: f32) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
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

        // Driving: → / D ahead, ← / A reverse (motors marked `drive`).
        let typing = ctx.egui_wants_keyboard_input();
        let throttle = if typing || !self.playing {
            0.0
        } else {
            ctx.input(|i| {
                let fwd = i.key_down(Key::ArrowRight) || (i.key_down(Key::D) && i.modifiers.is_none());
                let back = i.key_down(Key::ArrowLeft) || (i.key_down(Key::A) && i.modifiers.is_none());
                fwd as i32 as f32 - back as i32 as f32
            })
        };
        let has_player = self.sim.physics.actor_count() > 0;
        self.sim.physics.set_throttle(if has_player { 0.0 } else { throttle });
        let jump = !typing
            && self.playing
            && ctx.input(|i| i.key_down(Key::ArrowUp) || (i.key_down(Key::W) && i.modifiers.is_none()));
        self.sim.physics.set_controls(grave_engine::physics::Controls { walk: if has_player { throttle } else { 0.0 }, jump });

        // Run the simulation at a steady 60 steps per second (times speed).
        if self.playing {
            // Slow motion from a trigger (or the game) slows time on top.
            self.sim_clock += dt * SIM_HZ * self.speed as f64 * self.sim.camera.time_scale() as f64;
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
        notes::follow(&self.sim, &mut self.notes);
        egui::Panel::right("inspector").resizable(false).exact_size(270.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.inspector(ui);
                self.notes_panel(ui);
            });
        });
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::bottom("timeline").show(ui, |ui| self.timeline_bar(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(10, 10, 14)))
            .show(ui, |ui| self.canvas(ui));
        self.note_window(&ctx);
        self.material_settings_window(&ctx);

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
    fn right_click_puts_a_lamp_and_the_lighting_menu_shows_it() {
        let mut h = editor();
        h.state_mut().tool = Tool::Select;
        h.run_steps(2);
        let p = at(&h, 30.0, 20.0);
        right_click(&mut h, p);
        h.run_steps(2);
        h.get_by_label_contains("Put a lamp here").click();
        h.run_steps(2);
        let l = &h.state().sim.lighting;
        assert_eq!(l.lights.len(), 1, "a lamp");
        assert!((l.lights[0].x - 30.0).abs() < 1.0 && (l.lights[0].y - 20.0).abs() < 1.0);
        assert!(l.ambient < 0.6, "the scene dims so the lamp shows");
        h.get_by_label("💡 Lighting").click();
        h.run_steps(2);
        assert!(h.query_by_label_contains("Lamps (1)").is_some() && h.query_by_label_contains("Sun (casts shadows)").is_some());
        // Undo takes the lamp away again.
        h.key_press(Key::Escape);
        h.run_steps(2);
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run_steps(3);
        assert!(h.state().sim.lighting.lights.is_empty());
    }

    #[test]
    fn swing_an_object_and_let_go_to_throw_it() {
        let mut h = editor();
        h.key_press(Key::V);
        h.run_steps(2);
        h.state_mut().playing = true;
        { let (a, b) = (at(&h, 50.0, 65.0), at(&h, 90.0, 55.0)); drag(&mut h, a, b); }
        let thrown = object(&h);
        assert!(thrown.vx > 20.0 && thrown.vy < 0.0, "it flies off the way it was swung: ({}, {})", thrown.vx, thrown.vy);
        // Paused, a drag just moves it.
        h.state_mut().playing = false;
        { let p = object(&h); let (a, b) = (at(&h, p.x, p.y), at(&h, p.x + 30.0, p.y)); drag(&mut h, a, b); }
        assert_eq!(object(&h).vx, 0.0, "while paused it's only moved");
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

    #[test]
    fn rewind_with_the_slider() {
        let mut h = editor();
        h.state_mut().play();
        for _ in 0..120 {
            h.state_mut().step_once();
        }
        h.state_mut().playing = false;
        h.run_steps(2);
        let end = object(&h).y;
        assert!(h.state().timeline.len() > 10);
        // Drag the slider all the way to the start.
        // The rewind slider is the widest one.
        let r = h
            .get_all_by_role(egui::accesskit::Role::Slider)
            .map(|n| n.rect())
            .max_by(|a, b| a.width().total_cmp(&b.width()))
            .unwrap();
        let (a, b) = (r.right_center() - Vec2::new(2.0, 0.0), r.left_center());
        drag(&mut h, a, b);
        assert_eq!(h.state().rewind_pos, Some(0));
        let start = object(&h).y;
        assert!(start < end - 5.0, "back at the start the object is higher up: start {start}, end {end}");
        // Playing again carries on from the start and replaces the rest.
        h.state_mut().play();
        h.state_mut().step_once();
        assert!(h.state().timeline.len() <= 3);
    }

    #[test]
    fn save_a_template_then_place_copies() {
        let mut h = editor();
        let dir = std::env::temp_dir().join(format!("grave_ui_templates_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mats = Materials::clone(&h.state().mats);
        h.state_mut().templates = Templates::load_dir(&dir, &mats);
        h.key_press(Key::V);
        { let p = at(&h, 50.0, 65.0); press(&mut h, p, true); }
        { let p = at(&h, 50.0, 65.0); press(&mut h, p, false); }
        h.state_mut().template_name = "box".into();
        h.get_by_label("💾 Save template").click();
        h.run_steps(3);
        assert!(dir.join("box.txt").exists(), "the template file was written");
        assert_eq!(h.state().sim.instances.len(), 1, "the object is now a copy of it");
        // Place two more copies.
        h.state_mut().place_template = Some("box".into());
        h.state_mut().tool = Tool::Place;
        for x in [100.0, 130.0] {
            let p = at(&h, x, 30.0);
            press(&mut h, p, true);
            press(&mut h, p, false);
        }
        assert_eq!(h.state().sim.physics.body_count(), 3);
        assert_eq!(h.state().sim.instances.len(), 3);
        // Make the last one iron: only that copy changes, and it remembers it.
        let iron = h.state().mats.id("iron").unwrap();
        h.state_mut().recolor_selection(iron);
        let changed = h.state().sim.instances.values().filter(|i| !i.changes.is_empty()).count();
        assert_eq!(changed, 1);
    }

    #[test]
    fn join_two_objects_with_the_join_tool() {
        let mut h = editor();
        // A second object to join to the plank.
        let mats = Materials::clone(&h.state().mats);
        {
            let app = h.state_mut();
            let spec = BodySpec::from_mats(6, 6, &vec![Some(mats.id("iron").unwrap()); 36], &mats, 80.0, 65.0);
            app.sim.physics.add(&mut app.sim.world, &mats, spec);
        }
        h.state_mut().tool = Tool::Join;
        h.state_mut().join_kind = JoinChoice::Weld;
        for (x, y) in [(55.0, 65.0), (80.0, 65.0)] {
            let p = at(&h, x, y);
            press(&mut h, p, true);
            press(&mut h, p, false);
        }
        assert_eq!(h.state().sim.physics.joints().len(), 1, "welded");
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run_steps(3);
        assert_eq!(h.state().sim.physics.joints().len(), 0, "undo removes it");
    }

    /// Right-click with the button only (no drag).
    fn right_click(h: &mut Harness<Sandbox>, p: Pos2) {
        h.event(Event::PointerMoved(p));
        h.event(Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE });
        h.run_steps(1);
        h.event(Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE });
        h.run_steps(2);
    }

    #[test]
    fn right_click_leaves_notes_on_objects_and_ground() {
        let mut h = editor();
        let dir = std::env::temp_dir().join(format!("grave_notes_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        h.state_mut().notes_scene = dir.join("level.txt");
        h.state_mut().tool = Tool::Select;
        h.run_steps(2);

        // On the wooden object.
        let b = object(&h);
        let p = at(&h, b.x, b.y);
        right_click(&mut h, p);
        h.get_by_label_contains("Leave a note here").click();
        h.run_steps(2);
        h.get_by_role(egui::accesskit::Role::MultilineTextInput).type_text("make this crate iron");
        h.run_steps(1);
        h.get_by_label_contains("Save note").click();
        h.run_steps(3);
        // On plain ground.
        let p = at(&h, 100.0, 105.0);
        right_click(&mut h, p);
        h.get_by_label_contains("Leave a note here").click();
        h.run_steps(2);
        h.get_by_role(egui::accesskit::Role::MultilineTextInput).type_text("add a bounce pad here");
        h.run_steps(1);
        h.get_by_label_contains("Save note").click();
        h.run_steps(3);

        let text = std::fs::read_to_string(dir.join("level.notes.md")).expect("notes file written");
        let saved = grave_engine::notes::parse(&text);
        assert_eq!(saved.len(), 2, "{text}");
        assert_eq!(saved[0].text, "make this crate iron");
        assert!(saved[0].about.contains("object #1") && saved[0].about.contains("wood"), "{}", saved[0].about);
        assert_eq!(saved[1].text, "add a bounce pad here");
        assert!(saved[1].about.contains("stone"), "{}", saved[1].about);
        for n in &saved {
            let pic = dir.join(n.picture.as_ref().expect("a picture"));
            assert!(pic.is_file(), "{}", pic.display());
        }
        // The note follows its object when it moves.
        let id = h.state().sim.physics.ids()[0];
        {
            let app = h.state_mut();
            let mats = Materials::clone(&app.mats);
            app.sim.physics.edit(&mut app.sim.world, &mats, id, |b| b.x += 30.0);
        }
        h.run_steps(2);
        assert!((h.state().notes[0].x - (b.x + 30.0)).abs() < 1.0, "note moved with the crate");

        // The right-click menu also changes materials.
        let p = at(&h, b.x + 30.0, b.y);
        right_click(&mut h, p);
        h.get_by_label_contains("Change material to").click();
        h.run_steps(2);
        // (The palette has an "iron" button too: the menu's is drawn last.)
        h.get_all_by_label("iron").last().unwrap().click();
        h.run_steps(3);
        assert!(count(&h, "iron") > 100, "the crate is iron now");
        if std::env::var("GRAVE_KEEP_TEST_NOTES").is_err() {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn right_click_a_material_opens_its_settings() {
        let mut h = editor();
        // Right-click a material in the list (one near the top, so it's on screen).
        let b = h.get_all_by_label("brick").next().unwrap().rect().center();
        right_click(&mut h, b);
        h.run_steps(2);
        assert_eq!(h.state().mat_settings, h.state().mats.id("brick"));
        assert!(h.query_by_label_contains("brick settings").is_some(), "the window is open");
        assert!(h.query_by_label_contains("Reset all to default").is_some());
        // Or the ⚙ button for the brush material.
        h.state_mut().mat_settings = None;
        h.state_mut().brush_mat = h.state().mats.id("sand").unwrap();
        h.run_steps(2);
        h.get_by_label_contains("⚙ sand settings").click();
        h.run_steps(2);
        assert_eq!(h.state().mat_settings, h.state().mats.id("sand"));
        assert!(h.query_by_label_contains("With water").is_some(), "sand's thirst for water is a setting");
        assert!(h.query_by_label_contains("Slows wheels").is_some(), "and how it slows wheels");
        // Metals: softening and bending are settings too.
        h.state_mut().mat_settings = h.state().mats.id("steel");
        h.run_steps(2);
        assert!(h.query_by_label_contains("Softens at").is_some() && h.query_by_label_contains("Bends at").is_some());
    }

    #[test]
    fn keys_drive_the_player() {
        let mut h = editor();
        {
            let app = h.state_mut();
            let mats = Materials::clone(&app.mats);
            let (sim, _) = scene::load("size 200 120
fill air
rect stone 0 100 200 20
player 50 100
", &mats, None).into_sim(&mats);
            app.sim = sim;
            app.playing = true;
        }
        h.run_steps(2);
        let key = |key, pressed| Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
        h.event(key(Key::D, true));
        h.event(key(Key::W, true));
        h.run_steps(2);
        let c = h.state().sim.physics.controls();
        assert_eq!((c.walk, c.jump), (1.0, true), "D walks right, W jumps");
        h.event(key(Key::D, false));
        h.event(key(Key::W, false));
        h.event(key(Key::A, true));
        h.run_steps(2);
        assert_eq!(h.state().sim.physics.controls().walk, -1.0, "A walks left");
        assert_eq!(h.state().sim.physics.throttle(), 0.0, "cars don't move while there's a player");
    }

    #[test]
    fn wheel_zooms_and_follow_keeps_the_object_in_view() {
        let mut h = editor();
        let fit = h.state().canvas_at.1;
        let p = at(&h, 100.0, 60.0);
        h.event(Event::PointerMoved(p));
        h.event(Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: Vec2::new(0.0, 120.0), modifiers: Modifiers::NONE, phase: egui::TouchPhase::Move, source: egui::MouseWheelSource::Unknown });
        h.run_steps(4);
        assert!(h.state().canvas_at.1 > fit * 1.5, "zoomed in: {} -> {}", fit, h.state().canvas_at.1);
        // Follow the plank and push it to the far right: the camera goes with it.
        let id = h.state().sim.physics.ids()[0];
        h.state_mut().sim.follow = Some(id);
        {
            let app = h.state_mut();
            let mats = Materials::clone(&app.mats);
            app.sim.physics.edit(&mut app.sim.world, &mats, id, |b| b.x = 170.0);
        }
        h.run_steps(40);
        let (min, s) = h.state().canvas_at;
        let screen_x = min.x + 170.0 * s;
        let centre = h.state().canvas_at.0.x; // world left edge on screen
        assert!(screen_x > centre, "sanity");
        // The object should be near the middle of the canvas, not off to the side.
        let b = object(&h);
        let sx = min.x + b.x * s;
        let left = h.state().cam.unwrap();
        assert!((b.x - left.0).abs() < 25.0, "camera centre {} should be near the object at {} (screen x {sx})", left.0, b.x);
        // Fit shows everything again.
        h.key_press(Key::Home);
        h.run_steps(2);
        assert!(h.state().cam.is_none() && h.state().sim.follow.is_none());
    }
}
