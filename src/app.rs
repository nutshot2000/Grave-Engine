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
mod studio_ui;
mod animation_ui;
use crate::studio::{Studio,Tab};
use grave_engine::assemblies;

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
    Assembly(assemblies::Clip),
    Empty,
    Objects(Vec<BodySpec>),
    Area(Clip),
}

struct Sandbox {
    animation_editor: animation_ui::AnimationEditor,
    studio: Studio,
    workshop:crate::workshop::Workshop,
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
    /// Edit > Generate caves: tile size, seed, how open, how many pools.
    cave_gen: (usize, u32, f32, f32, f32, f32),
    /// Crystal/torch density and cave ambient light.
    cave_atmosphere: (f32, f32, f32),
    /// Generate caves with the hand-made tiles in tiles/caves.
    cave_tiles: bool,
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
    look_textures: std::collections::HashMap<(PathBuf, String, usize), TextureHandle>,
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
    /// Asset files the scene places (`asset FILE ...`): when one changes on
    /// disk (saved in the workshop, or edited by an AI tool), the scene reloads.
    scene_assets: Vec<(PathBuf, Option<SystemTime>)>,
    messages: Vec<(Instant, String)>,
    /// Follow mode: when an AI agent (or anything) changes a scene or asset
    /// file, open it so you can watch. Toggle off to work on your own.
    follow_ai: bool,
    /// Scene and workshop asset files and when they last changed.
    watched: std::collections::HashMap<PathBuf, SystemTime>,
    watch_check: Instant,
    /// `live/agent.txt`: what the AI says it's doing (and when we read it).
    agent_stamp: Option<SystemTime>,
    agent_note: Option<(Instant, String)>,
    /// Something the AI is working on that we didn't jump to (follow off, or busy).
    follow_pending: Option<PathBuf>,
    /// What the AI has done lately (the 🤖 Activity panel), and whether it's showing.
    activity: Vec<Activity>,
    show_activity: bool,
    /// Sound from the materials (see sound.rs): the 🔊 button mutes it.
    sound: grave_engine::sound::Player,
    /// Areas of the scene that just changed on disk (cells: x0, y0, x1, y1),
    /// outlined for a few seconds so you can see what's new.
    highlights: Vec<(usize, usize, usize, usize)>,
    highlight_since: Instant,
    /// Where the window is (x, y, width, height, maximised), to come back there after an update.
    window_at: Option<[f32; 5]>,
    /// "Updated: what changed" after an engine update (until when it shows).
    update_note: Option<(Instant, String)>,
    last_file_check: Instant,
    sim_clock: f64,
    last_frame: Instant,
    fps: f32,
    sim_ms: f32,
    /// Play controls: keyboard and game controller (assets/input/editor.input).
    input: grave_engine::input::Input,
    /// How long drawing the visible world takes (ms, smoothed).
    draw_ms: f32,
    /// Where the world is drawn on screen (top-left) and how big a cell is.
    canvas_at: (Pos2, f32),
    /// Camera: centre (cells) and zoom (screen pixels per cell). None = show
    /// the whole world.
    cam: Option<(f32, f32, f32)>,
    /// An endless map streaming as the player explores (endless.rs), and
    /// where on the map the Reset point is.
    endless: Option<grave_engine::endless::Endless>,
    endless_reset_origin: (i64, i64),
    /// Mouse-wheel movement not yet turned into zoom steps.
    zoom_acc: f32,
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
    /// The real program file (the editor runs from a copy) and when it was
    /// last built, to notice new builds; and since when a new one is waiting.
    original_exe: Option<(PathBuf, Option<SystemTime>)>,
    new_build_since: Option<Instant>,
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

/// The editor's sound player, at the volume it had before an update restarted it.
fn editor_sound() -> grave_engine::sound::Player {
    if cfg!(test) {
        return grave_engine::sound::Player::silent();
    }
    let mut p = grave_engine::sound::Player::new();
    if let Ok(v) = std::env::var("GRAVE_SOUND") {
        let mut w = v.split_whitespace();
        if let Some(vol) = w.next().and_then(|n| n.parse::<f32>().ok()) {
            p.volume = vol.clamp(0.0, 1.0);
        }
        p.muted = w.next() == Some("1");
    }
    p
}

/// How the File menu groups the scenes.
const SCENE_GROUPS: [&str; 4] = ["Games & levels", "Showcases", "Labs & tests", "Other"];

/// Which group a scene file belongs in (by name; new ones land in "Other").
fn scene_group(path: &Path) -> &'static str {
    let name = path.file_stem().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
    if has(&["stunt-yard", "mountain-run", "ember-rise", "racer", "red-mesa"]) {
        "Games & levels"
    } else if has(&["lab", "test", "kimodo", "player", "forces", "objects", "elements", "templates", "imports", "imported", "machine", "studio", "workshop", "props"]) {
        "Labs & tests"
    } else if has(&["house", "shop", "lane", "creek", "fire", "beach", "splash", "waterwheel", "millbrook", "heron", "oil", "weather"]) {
        "Showcases"
    } else {
        "Other"
    }
}

/// The asset files a scene places (`asset FILE ...`), and when each last changed.
fn scene_assets(text: &str, scene: &Path) -> Vec<(PathBuf, Option<SystemTime>)> {
    text.lines()
        .filter_map(|l| l.trim().strip_prefix("asset ")?.split_whitespace().next())
        .map(|f| scene.parent().map(|d| d.join(f)).filter(|p| p.exists()).unwrap_or_else(|| PathBuf::from(f)))
        .map(|p| {
            let s = stamp(&p);
            (p, s)
        })
        .collect()
}

/// Which Stunt Yard course (its `--level=` number) a scene file is, if any.
fn game_level(scene_path: &str) -> Option<usize> {
    let name = std::path::Path::new(scene_path).file_name()?.to_str()?;
    match name {
        "stunt-yard.txt" => Some(0),
        "mountain-run.txt" => Some(1),
        _ => None,
    }
}

/// Default play controls, built in; assets/input/editor.input is read at
/// start-up, so editing it remaps them without rebuilding.
const EDITOR_INPUT: &str = include_str!("../assets/input/editor.input");
/// Environment variables for running from a copy and restarting into a new
/// build: where the real program is, and what to come back to.
pub const ORIGINAL_EXE: &str = "GRAVE_EDITOR_ORIGINAL";
const RESUME_SCENE: &str = "GRAVE_RESUME_SCENE_PATH";
const RESUME_NOTES: &str = "GRAVE_RESUME_NOTES_SCENE";
const RESUME_CAM: &str = "GRAVE_RESUME_CAMERA";
const RESUME_WINDOW: &str = "GRAVE_RESUME_WINDOW";
/// Where Reset goes back to (the scene before it was played), kept across an update.
const RESUME_RESET: &str = "GRAVE_RESUME_RESET";

/// One thing that happened with the AI: when (seconds since 1970), what, and
/// the file it was about (to jump back to it).
#[derive(Clone, Debug)]
struct Activity {
    at: u64,
    text: String,
    file: Option<PathBuf>,
}

const ACTIVITY_FILE: &str = "live/activity.txt";

fn now_secs() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// The activity log, kept on disk so it survives updates (newest last, at most 200).
fn load_activity() -> Vec<Activity> {
    if cfg!(test) {
        return Vec::new();
    }
    std::fs::read_to_string(ACTIVITY_FILE)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut it = l.splitn(3, '|');
            let at = it.next()?.parse().ok()?;
            let file = it.next().filter(|f| !f.is_empty()).map(PathBuf::from);
            Some(Activity { at, file, text: it.next()?.to_string() })
        })
        .collect()
}

/// How long ago, in words.
fn ago(at: u64) -> String {
    let s = now_secs().saturating_sub(at);
    match s {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", s / 60),
        _ => format!("{} h ago", s / 3600),
    }
}

pub fn run(materials: PathBuf, scene_file: Option<PathBuf>) -> Result<(), String> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Grave Engine")
        .with_inner_size([1500.0, 900.0])
        .with_min_inner_size([900.0, 550.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")) {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    // Restarted into a new build: the window comes back where it was.
    if let Some(v) = std::env::var(RESUME_WINDOW).ok().map(|s| s.split(',').filter_map(|x| x.parse::<f32>().ok()).collect::<Vec<_>>())
        && v.len() == 5
    {
        viewport = viewport.with_position([v[0], v[1]]).with_inner_size([v[2].max(900.0), v[3].max(550.0)]).with_maximized(v[4] > 0.5);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native(
        "Grave Engine",
        options,
        Box::new(move |cc| {
            crate::studio::theme(&cc.egui_ctx);
            cc.egui_ctx.global_style_mut(|s| {
                s.spacing.button_padding = Vec2::new(10.0, 6.0);
                s.spacing.item_spacing = Vec2::new(8.0, 6.0);
                for (style, font) in s.text_styles.iter_mut() {
                    if matches!(style, egui::TextStyle::Body | egui::TextStyle::Button) {
                        font.size = 14.0;
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
        let mut workshop=crate::workshop::Workshop::new(&mats);workshop.open=std::env::args().any(|a|a=="--workshop");
        let mut app = Sandbox {
            studio: Studio::default(),
            workshop,
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
            cave_gen: (64, 1, 0.6, 0.2, 0.65, 0.75),
            cave_atmosphere: (0.7, 0.3, 0.32),
            cave_tiles: true,
            endless: None,
            endless_reset_origin: (0, 0),
            playing: false,
            fling: Vec::new(),
            speed: 1.0,
            cam_smooth: 7.0,
            cam_lead: true,
            show_triggers: false,
            view: View::default(),
            renderer: Renderer::default(),
            texture: None,
            look_textures: Default::default(),
            tool: Tool::Select,
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
            scene_assets: Vec::new(),
            messages: Vec::new(),
            follow_ai: std::env::var("GRAVE_FOLLOW_AI").map_or(true, |v| v != "0"),
            watched: std::collections::HashMap::new(),
            watch_check: Instant::now(),
            agent_stamp: None,
            agent_note: None,
            follow_pending: None,
            activity: load_activity(),
            show_activity: false,
            sound: editor_sound(),
            highlights: Vec::new(),
            highlight_since: Instant::now(),
            window_at: None,
            update_note: None,
            last_file_check: Instant::now(),
            sim_clock: 0.0,
            last_frame: Instant::now(),
            fps: 60.0,
            sim_ms: 0.0,
            input: grave_engine::input::Input::load_or("assets/input/editor.input", EDITOR_INPUT).unwrap_or_else(|e| {
                eprintln!("Controls file problem, using the defaults: {e}");
                let mut i = grave_engine::input::Input::parse(EDITOR_INPUT).expect("default controls");
                i.connect_controllers();
                i
            }),
            draw_ms: 0.0,
            canvas_at: (Pos2::ZERO, 1.0),
            cam: None,
            zoom_acc: 0.0,
            notes: Vec::new(),
            notes_scene: PathBuf::from("scenes/my_scene.txt"),
            notes_stamp: None,
            note_draft: None,
            menu_at: None,
            show_done_notes: false,
            mat_settings: None,
            animation_editor: Default::default(),
            original_exe: std::env::var_os(ORIGINAL_EXE).map(PathBuf::from).map(|p| {
                let s = stamp(&p);
                (p, s)
            }),
            new_build_since: None,
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
            // Keep the file it was opened with: Save writes back to it, and
            // when it changes on disk (an AI agent editing it), it reloads.
            app.scene_path = p.display().to_string();
            app.load_scene_inner();
        }
        // Restarted into a new build: pick up where we were (the scene as
        // it was, under its own file name, still watched for changes).
        // Restarted mid endless run: carry on where the map is.
        if let (Ok(path), Ok(at)) = (std::env::var(RESUME_SCENE), std::env::var("GRAVE_RESUME_ENDLESS")) {
            let v: Vec<i64> = at.split_whitespace().filter_map(|n| n.parse().ok()).collect();
            let p = Path::new(&path);
            let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
            if let (Ok(text), [ox, oy, rx, ry]) = (std::fs::read_to_string(p), v.as_slice()) {
                app.endless = grave_engine::endless::Endless::from_scene(&text, &app.mats, p.parent(), &name);
                if let Some(e) = &mut app.endless {
                    e.resume((*ox, *oy));
                }
                app.endless_reset_origin = (*rx, *ry);
            }
        }
        if let Ok(path) = std::env::var(RESUME_SCENE) {
            app.scene_path = path;
            app.scene_stamp = stamp(Path::new(&app.scene_path));
            let file = PathBuf::from(&app.scene_path);
            app.scene_assets = scene_assets(&std::fs::read_to_string(&file).unwrap_or_default(), &file);
        }
        if let Ok(notes_scene) = std::env::var(RESUME_NOTES) {
            app.notes_scene = PathBuf::from(notes_scene);
            app.notes = notes::load(&notes::path_for(&app.notes_scene));
            app.notes_stamp = stamp(&notes::path_for(&app.notes_scene));
        }
        if let Some(c) = std::env::var(RESUME_CAM).ok().and_then(|c| {
            let v: Vec<f32> = c.split(',').filter_map(|x| x.parse().ok()).collect();
            (v.len() == 3).then(|| (v[0], v[1], v[2]))
        }) {
            app.cam = Some(c);
        }
        if let Some(text) = std::env::var_os(RESUME_RESET).and_then(|p| std::fs::read_to_string(p).ok()) {
            let (reset, _) = scene::load(&text, &app.mats, Path::new(&app.scene_path).parent()).into_sim(&app.mats);
            let mut reset = reset;
            app.snapshot = reset.snapshot();
            // The scene on screen has been played since that point: pressing
            // Play mustn't take a new reset point from it (that left Reset
            // stuck on the end of the run after an update).
            app.ran_since_snapshot = true;
        }
        if std::env::var_os(RESUME_SCENE).is_some() {
            // What the AI said the update was (`update TEXT` in live/agent.txt).
            let what = std::fs::read_to_string("live/agent.txt")
                .ok()
                .and_then(|t| t.lines().rev().find_map(|l| l.trim().strip_prefix("update ").map(|s| s.trim().to_string())));
            let text = match &what {
                Some(w) => format!("Engine updated: {w}"),
                None => "Engine updated to the new build".to_string(),
            };
            app.say(text.clone());
            app.log_activity(text.clone(), None);
            app.update_note = Some((Instant::now(), text));
        }
        app
    }

    /// Restart into a new build of the engine, keeping the scene (as it is
    /// now), its file name, its notes and the camera. Undo history is lost.
    fn restart_into_new_build(&mut self) {
        let Some((exe, _)) = self.original_exe.clone() else { return };
        let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) else { return };
        // (Next to the copies, not among them: the new start tidies that folder.)
        let resume = dir.parent().unwrap_or(&dir).join("editor-resume.txt");
        let snap = self.sim.snapshot();
        if std::fs::write(&resume, scene::save(&snap, &self.mats)).is_err() {
            self.say("Couldn't save the scene to restart: keep working, then reopen the editor");
            self.original_exe = None;
            return;
        }
        // Also the point Reset goes back to: the scene before it was played,
        // not the state it happened to be in when the update came.
        let reset = dir.parent().unwrap_or(&dir).join("editor-reset.txt");
        let reset_ok = std::fs::write(&reset, scene::save(&self.snapshot, &self.mats)).is_ok();
        let mut cmd = std::process::Command::new(&exe);
        cmd.arg(&resume).env_remove(ORIGINAL_EXE).env(RESUME_SCENE, &self.scene_path).env(RESUME_NOTES, &self.notes_scene);
        if reset_ok {
            cmd.env(RESUME_RESET, &reset);
        }
        if let Some(dir) = &self.mats.dir {
            cmd.arg("--materials").arg(dir);
        }
        if let Some((x, y, s)) = self.cam {
            cmd.env(RESUME_CAM, format!("{x},{y},{s}"));
        }
        if let Some(w) = self.window_at {
            cmd.env(RESUME_WINDOW, w.map(|v| v.to_string()).join(","));
        }
        cmd.env("GRAVE_FOLLOW_AI", if self.follow_ai { "1" } else { "0" });
        if let Some(e) = &self.endless {
            cmd.env("GRAVE_RESUME_ENDLESS", format!("{} {} {} {}", e.origin.0, e.origin.1, self.endless_reset_origin.0, self.endless_reset_origin.1));
        }
        cmd.env("GRAVE_SOUND", format!("{} {}", self.sound.volume, u8::from(self.sound.muted)));
        match cmd.spawn() {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                self.say(format!("Couldn't start the new build: {e}"));
                self.original_exe = None;
            }
        }
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
            if let Some(id) = object
                && let Some((cx, cy, a)) = self.sim.physics.pose_of(id) {
                    let (dx, dy) = (x - cx, y - cy);
                    let (c, s) = (a.cos(), a.sin());
                    (lamp.x, lamp.y) = (c * dx + s * dy, -s * dx + c * dy);
                    lamp.on = Some(id);
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
    /// Show a menu that changes the scene's look (lighting, background), and
    /// record any change as an undoable step.
    fn look_menu(&mut self, ui: &mut egui::Ui, label: &str, menu: fn(&mut Self, &mut egui::Ui)) {
        let before = (self.sim.lighting.clone(), self.sim.backdrop.clone(), self.sim.weather.clone());
        menu(self, ui);
        if self.sim.lighting != before.0 || self.sim.backdrop != before.1 || self.sim.weather != before.2 {
            let after = (std::mem::replace(&mut self.sim.lighting, before.0), std::mem::replace(&mut self.sim.backdrop, before.1), std::mem::replace(&mut self.sim.weather, before.2));
            self.history.touch(&mut self.sim, label); // (remembers how it was)
            self.sim.lighting = after.0;
            self.sim.backdrop = after.1;
            self.sim.weather = after.2;
        }
    }

    /// The ☁ Weather menu: how heavy each kind is, presets, strike now.
    /// Edit > Generate caves (levelgen.rs): the whole world becomes caves.
    fn generate_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(280.0);
        ui.label(RichText::new("Caves from herringbone tiles").strong());
        ui.label(RichText::new("Replaces the ground everywhere (objects and characters stay). Undo brings it back.").small());
        let (mut tile, mut seed, mut open, mut pools, mut caverns, mut dead_ends) = self.cave_gen;
        let (mut crystals, mut torches, mut cave_light) = self.cave_atmosphere;
        egui::Grid::new("gen").num_columns(2).show(ui, |ui| {
            ui.label("Tile size").on_hover_text("How big each tile is, in cells: bigger = bigger caves");
            ui.add(egui::DragValue::new(&mut tile).range(16..=200).suffix(" cells"));
            ui.end_row();
            ui.label("Seed").on_hover_text("The same seed always makes the same caves");
            ui.add(egui::DragValue::new(&mut seed));
            ui.end_row();
            ui.label("Openness").on_hover_text("How many tile edges have a passage through them");
            ui.add(egui::Slider::new(&mut open, 0.2..=0.95).show_value(false));
            ui.end_row();
            ui.label("Huge caverns").on_hover_text("Occasional open chambers spanning several tiles; zero turns them off");
            ui.add(egui::Slider::new(&mut caverns, 0.0..=1.0).show_value(false));
            ui.end_row();
            ui.label("Dead ends").on_hover_text("More blind side passages with rock beyond their tips");
            ui.add(egui::Slider::new(&mut dead_ends, 0.0..=1.0).show_value(false));
            ui.end_row();
            ui.label("Crystals").on_hover_text("Glowing cyan/violet clusters around the large caverns");
            ui.add(egui::Slider::new(&mut crystals, 0.0..=1.0).show_value(false));
            ui.end_row();
            ui.label("Torches").on_hover_text("Occasional warm, cool-to-touch decorative torch lights");
            ui.add(egui::Slider::new(&mut torches, 0.0..=1.0).show_value(false));
            ui.end_row();
            ui.label("Cave brightness").on_hover_text("Ambient light between glowing sources; 1 is fully lit");
            ui.add(egui::Slider::new(&mut cave_light, 0.08..=1.0));
            ui.end_row();
            ui.label("Pools").on_hover_text("How often a cavern has water in it");
            ui.add(egui::Slider::new(&mut pools, 0.0..=1.0).show_value(false));
            ui.end_row();
        });
        ui.checkbox(&mut self.cave_tiles, "Hand-made tiles (tiles/caves)").on_hover_text("Mix in the hand-drawn tiles: halls, vaults, shafts, bridges, wells...");
        let mut go = false;
        ui.horizontal(|ui| {
            go |= ui.button("Generate").clicked();
            if ui.button("🎲 Reroll").on_hover_text("New caves with a new seed").clicked() {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345) % 100_000;
                go = true;
            }
        });
        self.cave_gen = (tile, seed, open, pools, caverns, dead_ends);
        self.cave_atmosphere = (crystals, torches, cave_light);
        if go {
            let set = grave_engine::levelgen::Settings { tile, seed, open, pools, caverns, dead_ends, crystals, torches, ..grave_engine::levelgen::Settings::new(&self.mats) };
            let tiles = if self.cave_tiles { grave_engine::levelgen::Tile::load_dir(Path::new("tiles/caves"), &self.mats).0 } else { Vec::new() };
            self.change("generate caves", |app| {
                let (w, h) = (app.sim.world.w, app.sim.world.h);
                grave_engine::levelgen::generate(&mut app.sim.world, &app.mats, (0, 0, w, h), &set, &tiles);
                app.sim.world.wake_all();
                app.sim.lighting.ambient = cave_light;
                app.sim.lighting.ambient_color = [200, 214, 240];
                app.sim.lighting.fade = None;
            });
            self.playing = false;
            self.ran_since_snapshot = false;
            self.timeline.clear();
            self.say(format!("Generated caves (tile {tile}, seed {seed}). Reroll for different ones."));
        }
    }

    fn weather_menu(&mut self, ui: &mut egui::Ui) {
        use grave_engine::weather::{Kind, Weather};
        ui.set_min_width(300.0);
        ui.label(RichText::new("Outside temperature").strong());
        let mut degrees=self.sim.world.ambient;
        if ui.add(egui::DragValue::new(&mut degrees).range(-100.0..=100.0).speed(0.5).suffix(" °C")).on_hover_text("Outdoor air temperature. Above freezing, falling snow and settled drifts can melt.").changed() {
            self.change("outside temperature",|app|{let _=app.sim.set_outside_temperature(degrees,&app.mats,false);});
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("Cold start (-8°C)").on_hover_text("Chill ordinary air, ground and props for a snow test. Fires, heaters and deliberately hot/cold materials keep their temperatures.").clicked() {
                self.change("cold start",|app|{let _=app.sim.set_outside_temperature(-8.,&app.mats,true);});
                self.playing=false;self.ran_since_snapshot=false;self.timeline.clear();
            }
            if ui.button("Match scene temperature").on_hover_text("Apply the chosen outside temperature to ordinary air, ground and props now; otherwise warm surfaces cool gradually.").clicked() {
                let degrees=self.sim.world.ambient;
                self.change("match scene temperature",|app|{let _=app.sim.set_outside_temperature(degrees,&app.mats,true);});
                self.playing=false;self.ran_since_snapshot=false;self.timeline.clear();
            }
        });
        ui.label(RichText::new("Warm ground takes time to cool. Use Cold start to test snow building up immediately.").small());
        if self.sim.weather.snow>0. && self.sim.world.ambient>0. {ui.colored_label(Color32::from_rgb(210,140,45),"Above freezing: snow can melt into water.");}
        ui.separator();
        let w = &mut self.sim.weather;
        ui.checkbox(&mut w.on, RichText::new("Weather on").strong()).on_hover_text("Switch it all off without losing the settings");
        ui.horizontal_wrapped(|ui| {
            for (name, preset) in Weather::presets() {
                if ui.button(name).clicked() {
                    let on = w.on;
                    *w = Weather { on, ..preset };
                }
            }
        });
        ui.separator();
        for (kind, label, help) in [
            (Kind::Snow, "❄ Snow", "Flakes land and pile up: drifts, snow on roofs. 1 is a blizzard"),
            (Kind::Rain, "🌧 Rain", "Real water: fills puddles, puts fires out, wets the ground"),
            (Kind::Hail, "🧊 Hail", "Heavy ice pellets that pile up, then melt"),
            (Kind::Dust, "🏜 Sandstorm", "Blown dust from upwind, with a tan haze"),
            (Kind::Fog, "🌫 Fog", "Hides the distance and softens everything"),
            (Kind::Gusts, "💨 Gusty wind", "The wind rises and falls in waves (on top of the scene's wind)"),
        ] {
            let mut v = w.get(kind);
            if ui.add(egui::Slider::new(&mut v, 0.0..=1.0).text(label)).on_hover_text(help).changed() {
                w.set(kind, v, 0.0);
            }
        }
        let mut strikes = w.lightning;
        if ui.add(egui::Slider::new(&mut strikes, 0.0..=30.0).text("⚡ Lightning (a minute)")).on_hover_text("How often it strikes: it picks the tallest thing in view, flickers a warning, then hits: fire, a blast, glass from sand").changed() {
            w.set(Kind::Lightning, strikes, 0.0);
        }
        ui.separator();
        if ui.button("⚡ Strike now").on_hover_text("Call a strike at the tallest thing in view (during Play)").clicked() {
            self.sim.weather_run.strike_now = true;
            self.playing = true;
        }
        ui.label(RichText::new("Weather falls around the part of the world you're looking at. Triggers can change it: `weather snow 0.5 6`").size(12.0).weak());
    }

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
        let _ = changed; // (recorded for undo by `look_menu`)
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
                num(ui, "texture_blend", "Texture join", "Soften joins with other materials that also enable this, in cells; visual only", cur.texture_blend, def.texture_blend, 0.0..=8.0, false);
                num(ui, "glow", "Glow", "How much light it gives off", cur.glow, def.glow, 0.0..=2.0, false);
                num(ui, "light_radius", "Light reach (cells)", "Radius of shadow-casting light from each connected material patch", cur.light_radius, def.light_radius, 0.0..=240.0, false);
                num(ui, "light_strength", "Light brightness", "Brightness of each material light; uses its surface colour", cur.light_strength, def.light_strength, 0.0..=5.0, false);
                num(ui, "light_flicker", "Light flicker", "Steady at zero; torch-like at higher values", cur.light_flicker, def.light_flicker, 0.0..=1.0, false);

                ui.heading("Weight and movement");
                num(ui, "density", "Weight (kg/m³)", "Heavier things sink below lighter ones. Water is 1000", cur.density, def.density, 1.0..=20000.0, true);
                num(ui, "friction", "Grip", "How much things grip it (ice 0.02, rubber 1.2)", cur.friction, def.friction, 0.0..=2.0, false);
                num(ui, "bounciness", "Bounce", "How much things bounce off it (0 none, 1 perfect)", cur.bounciness, def.bounciness, 0.0..=1.0, false);
                num(ui, "launch", "Bounce pad (m/s)", "Flings anything landing on it upwards this fast (0 = off)", cur.launch, def.launch, 0.0..=40.0, false);
                if cur.state != State::Gas {
                    num(ui, "rolling_resistance", "Slows wheels", "How much it slows wheels rolling on it, per second (sand 0.6, mud 1.8, hard ground 0)", cur.rolling_resistance, def.rolling_resistance, 0.0..=3.0, false);
                }
                if cur.leaves_film() {
                    num(ui, "coat_grip", "Film grip", "Ground wet with it keeps this share of its grip (water 0.7, oil 0.12)", cur.coat_grip, def.coat_grip, 0.0..=1.0, false);
                    num(ui, "coat_lasts", "Film dries in (s)", "How long a film of it stays on the ground (water 45, oil 900)", cur.coat_lasts, def.coat_lasts, 1.0..=3600.0, true);
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
                self.sim.materials_changed(&self.mats);
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

    /// Add to the AI activity log (and its file, so it survives updates).
    fn log_activity(&mut self, text: impl Into<String>, file: Option<PathBuf>) {
        let a = Activity { at: now_secs(), text: text.into(), file };
        self.activity.push(a);
        if self.activity.len() > 200 {
            self.activity.remove(0);
        }
        if cfg!(test) {
            return;
        }
        let _ = std::fs::create_dir_all("live");
        let body: String = self
            .activity
            .iter()
            .map(|a| format!("{}|{}|{}\n", a.at, a.file.as_ref().map(|f| f.display().to_string()).unwrap_or_default(), a.text.replace('\n', " ")))
            .collect();
        let _ = std::fs::write(ACTIVITY_FILE, body);
    }

    /// Reload the scene from disk and outline the areas that changed.
    fn reload_highlighting(&mut self, why: String) {
        let (w, h) = (self.sim.world.w, self.sim.world.h);
        let before = self.sim.world.mat.clone();
        self.say(why.clone());
        self.load_scene();
        self.highlights.clear();
        if (self.sim.world.w, self.sim.world.h) == (w, h) {
            // Changed cells, gathered into 8-cell blocks, merged along rows.
            const B: usize = 8;
            let (bw, bh) = (w.div_ceil(B), h.div_ceil(B));
            let mut hit = vec![false; bw * bh];
            for (i, (a, b)) in before.iter().zip(&self.sim.world.mat).enumerate() {
                if a != b {
                    hit[(i / w / B) * bw + (i % w) / B] = true;
                }
            }
            for by in 0..bh {
                let mut bx = 0;
                while bx < bw {
                    if hit[by * bw + bx] {
                        let start = bx;
                        while bx < bw && hit[by * bw + bx] {
                            bx += 1;
                        }
                        let rect = (start * B, by * B, (bx * B).min(w), ((by + 1) * B).min(h));
                        // Join to the rectangle just above if it spans the same columns.
                        if let Some(r) = self.highlights.iter_mut().find(|r| r.0 == rect.0 && r.2 == rect.2 && r.3 == rect.1) {
                            r.3 = rect.3;
                        } else {
                            self.highlights.push(rect);
                        }
                    } else {
                        bx += 1;
                    }
                }
            }
            self.highlights.truncate(40);
        }
        self.highlight_since = Instant::now();
        let n = self.highlights.len();
        let path = PathBuf::from(&self.scene_path);
        let detail = if n > 0 {
            format!("{n} area{} changed", if n == 1 { "" } else { "s" })
        } else if (self.sim.world.w, self.sim.world.h) != (w, h) {
            format!("new size {}x{}", self.sim.world.w, self.sim.world.h)
        } else {
            format!("no difference found in {}x{} cells", w, h)
        };
        self.log_activity(format!("{why} ({detail})"), Some(path));
    }

    /// The 🤖 AI activity panel: newest first, each with a button to open its file again.
    fn activity_window(&mut self, ctx: &egui::Context) {
        if !self.show_activity {
            return;
        }
        let mut open = true;
        let mut go = None;
        let mut clear = false;
        egui::Window::new("🤖 AI activity")
            .open(&mut open)
            .default_width(380.0)
            .default_pos(ctx.content_rect().right_top() + Vec2::new(-400.0, 90.0))
            .show(ctx, |ui| {
                ui.label(RichText::new("What the AI has done lately. Open jumps back to that scene or asset.").small().weak());
                if self.activity.is_empty() {
                    ui.label("Nothing yet.");
                }
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    for a in self.activity.iter().rev().take(80) {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(ago(a.at)).small().weak());
                            ui.label(&a.text);
                            if let Some(f) = &a.file
                                && ui.small_button("Open").on_hover_text(f.display().to_string()).clicked()
                            {
                                go = Some(f.clone());
                            }
                        });
                    }
                });
                if ui.small_button("Clear the list").clicked() {
                    clear = true;
                }
            });
        self.show_activity = open;
        if clear {
            self.activity.clear();
            let _ = std::fs::write(ACTIVITY_FILE, "");
        }
        if let Some(f) = go {
            self.follow_to(&f);
        }
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
            Some(label) => { if label == "animation graph" { self.sync_animation_reset(); } self.say(format!("Undid: {label}")); },
            None => self.say("Nothing to undo"),
        }
        self.selection = Selection::None;
    }

    fn redo(&mut self) {
        self.playing = false;
        self.drag = Drag::None;
        match self.history.redo(&mut self.sim, &self.mats) {
            Some(label) => { if label == "animation graph" { self.sync_animation_reset(); } self.say(format!("Redid: {label}")); },
            None => self.say("Nothing to redo"),
        }
        self.selection = Selection::None;
    }

    fn selected_ids(&self) -> Vec<u64> {
        match &self.selection {
            Selection::Objects(ids) => if self.studio.whole_assets {assemblies::expand_front(&self.sim,ids)} else {ids.clone()},
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
        if self.studio.whole_assets && !assemblies::selected_names(&self.sim,&ids).is_empty() {
            if let Some(first)=self.sim.physics.spec_of(&self.sim.world,ids[0]) {
                let mut next=first.clone();f(&mut next);let d=(next.x-first.x,next.y-first.y);let angle=next.angle-first.angle;
                if d.0!=0. || d.1!=0. || angle!=0. {
                    self.change(label,|app|assemblies::transform(&mut app.sim,&app.mats,&ids,true,d,angle,(first.x,first.y)));return;
                }
            }
        }
        self.change(label, |app| {
            let members=assemblies::members(&app.sim,&ids,app.studio.whole_assets);
            for (z,ids) in members.into_iter().enumerate() {let p=assemblies::plane_mut(&mut app.sim,z);for id in ids {p.physics.edit(&mut p.world,&app.mats,id,&f);}}
        });
    }

    fn delete_selection(&mut self) {
        match self.selection.clone() {
            Selection::Objects(ids) => self.change("delete", |app| {assemblies::remove(&mut app.sim,&ids,app.studio.whole_assets);}),
            Selection::Area(r) => self.change("delete", |app| {
                edit::cut_region(&mut app.sim.world, r);
            }),
            Selection::None => return,
        }
        self.selection = Selection::None;
    }

    fn copy_selection(&mut self) -> bool {
        let ids=self.selected_ids();
        if self.studio.whole_assets && !assemblies::selected_names(&self.sim,&ids).is_empty() {self.clipboard=Clipboard::Assembly(assemblies::Clip::capture(&self.sim,&ids,true));self.say("Copied the complete layered asset");return true;}
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
            Clipboard::Assembly(clip)=>{let clip=clip.clone();let mut ids=Vec::new();self.change("paste layered asset",|app|{ids=clip.paste(&mut app.sim,&app.mats,x,y);});self.selection=Selection::Objects(ids);},
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
        let ids=self.selected_ids();
        if self.studio.whole_assets&&!assemblies::selected_names(&self.sim,&ids).is_empty() {
            if matches!(t,Transform::Turn) {self.edit_objects(label,|b|b.angle+=std::f32::consts::FRAC_PI_2);}
            else {self.say("Turn off whole-asset selection to reshape an individual part.");}
            return;
        }
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

    /// Open Stunt Yard on a course: the built game next to this program if
    /// there is one, otherwise build and run it with cargo.
    fn play_as_game(&mut self, level: usize) {
        let arg = format!("--level={level}");
        let name = if cfg!(windows) { "stunt-yard.exe" } else { "stunt-yard" };
        // (The editor runs from a copy: the games are next to the real program.)
        let home = self.original_exe.as_ref().map(|(p, _)| p.clone()).or_else(|| std::env::current_exe().ok());
        let sibling = home.and_then(|e| e.parent().map(|d| d.join(name))).filter(|p| p.exists());
        let started = match sibling {
            Some(exe) => std::process::Command::new(exe).arg(&arg).spawn(),
            None => std::process::Command::new("cargo").args(["run", "--release", "--bin", "stunt-yard", "--", &arg]).spawn(),
        };
        match started {
            Ok(_) => self.say("Opening the game (it may take a moment to build the first time)".to_string()),
            Err(e) => self.say(format!("Couldn't start the game: {e}")),
        }
    }

    /// Overhead scenes use their own altitude-aware viewport, without replacing
    /// the current side-view scene or any unsaved workshop draft.
    fn open_overhead_game(&mut self, binary: &str) {
        let name = if cfg!(windows) { format!("{binary}.exe") } else { binary.to_string() };
        let sibling = std::env::current_exe().ok().and_then(|p| p.parent().map(|p|p.join(name))).filter(|p|p.exists());
        let mut command = match sibling {Some(p)=>std::process::Command::new(p),None=>{let mut c=std::process::Command::new("cargo");c.args(["run","--release","--bin",binary]);c}};
        #[cfg(windows)] {use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
        match command.spawn(){Ok(_)=>self.say(format!("Opening {binary}")),Err(e)=>self.say(format!("Couldn't open flight lab: {e}"))}
    }

    fn load_scene_inner(&mut self) {
        let path = PathBuf::from(&self.scene_path);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let (sim, problems) = scene::load(&text, &self.mats, path.parent()).into_sim(&self.mats);
                self.sim = sim;
                let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                self.endless = grave_engine::endless::Endless::from_scene(&text, &self.mats, path.parent(), &name);
                if let Some(e) = &mut self.endless {
                    e.new_run();
                }
                self.endless_reset_origin = (0, 0);
                self.timeline.clear();
                self.rewind_pos = None;
                self.snapshot = self.sim.snapshot();
                self.ran_since_snapshot = false;
                self.playing = false;
                self.selection = Selection::None;
                self.hover = None;
                self.cam = None;
                self.scene_stamp = stamp(&path);
                self.scene_assets = scene_assets(&text, &path);
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
    /// Follow mode. Watches `scenes/*.txt`, `assets/workshop/*.asset` and the
    /// agent note file `live/agent.txt` (lines: `say TEXT`, `open PATH`,
    /// `look X Y`). A changed file that isn't already on screen is opened:
    /// a scene in the editor (one Undo goes back), an asset in the workshop.
    /// With follow off (or while you're playing, dragging or writing a note)
    /// it's offered instead: "Go there".
    fn follow_files(&mut self) {
        // (Tests must not react to what an AI happens to be doing in this folder.)
        if cfg!(test) {
            return;
        }
        if self.watch_check.elapsed().as_secs_f32() < 0.5 {
            return;
        }
        self.watch_check = Instant::now();
        let first = self.watched.is_empty();
        let mut changed: Vec<(SystemTime, PathBuf)> = Vec::new();
        for (dir, ext) in [("scenes", "txt"), ("assets/workshop", "asset")] {
            let Ok(rd) = std::fs::read_dir(dir) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().is_none_or(|x| x != ext) || p.to_string_lossy().contains(".local.") {
                    continue;
                }
                let Some(t) = stamp(&p) else { continue };
                if self.watched.insert(p.clone(), t).is_some_and(|old| old != t) {
                    changed.push((t, p));
                }
            }
        }
        // What the agent says it's doing.
        let note_file = Path::new("live/agent.txt");
        let now = stamp(note_file);
        let mut look = None;
        if now.is_some() && now != self.agent_stamp {
            let fresh = self.agent_stamp.is_some() || now.and_then(|t| t.elapsed().ok()).is_some_and(|e| e.as_secs() < 30);
            self.agent_stamp = now;
            if fresh && let Ok(text) = std::fs::read_to_string(note_file) {
                for line in text.lines() {
                    let line = line.trim();
                    if let Some(msg) = line.strip_prefix("say ") {
                        self.agent_note = Some((Instant::now(), msg.trim().to_string()));
                        self.log_activity(format!("🤖 {}", msg.trim()), None);
                    } else if let Some(p) = line.strip_prefix("open ") {
                        changed.push((SystemTime::now(), PathBuf::from(p.trim())));
                    } else if let Some(xy) = line.strip_prefix("look ") {
                        let v: Vec<f32> = xy.split_whitespace().filter_map(|n| n.parse().ok()).collect();
                        if v.len() >= 2 {
                            look = Some((v[0], v[1]));
                        }
                    }
                }
            }
        }
        if first {
            return;
        }
        let same = |a: &Path, b: &Path| match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
            (Ok(a), Ok(b)) => a == b,
            _ => a == b,
        };
        changed.sort();
        if let Some((_, path)) = changed.pop() {
            let shown = same(&path, Path::new(&self.scene_path))
                || self.scene_assets.iter().any(|(p, _)| same(p, &path))
                || (self.workshop.open && self.workshop.current_file().is_some_and(|w| same(w, &path)));
            if !shown {
                self.follow_pending = Some(path);
            }
        }
        let free = !self.playing && matches!(self.drag, Drag::None) && self.note_draft.is_none();
        if self.follow_ai && free {
            if let Some(path) = self.follow_pending.take() {
                self.follow_to(&path);
            }
            if let Some((x, y)) = look {
                self.cam = Some((x, y, self.canvas_at.1.max(self.cam.map_or(0.0, |c| c.2))));
            }
        }
    }

    /// Show a file the AI is working on: a scene in the editor, an asset in the workshop.
    fn follow_to(&mut self, path: &Path) {
        if path.extension().is_some_and(|x| x == "asset") {
            if self.workshop.is_dirty() {
                self.say(format!("The AI changed {}; your workshop draft has unsaved changes, so it wasn't opened", path.display()));
                return;
            }
            self.workshop.load(path, &self.mats);
            self.workshop.open = true;
            self.say(format!("Following the AI: {}", path.display()));
            self.log_activity(format!("Opened {} in the workshop", path.display()), Some(path.to_path_buf()));
        } else {
            if self.workshop.open && self.workshop.is_dirty() {
                self.say(format!("The AI is working on {}; leave the workshop to follow", path.display()));
                return;
            }
            self.workshop.open = false;
            self.scene_path = path.display().to_string();
            self.load_scene();
            self.say(format!("Following the AI: {} (Undo goes back)", path.display()));
            self.log_activity(format!("Opened {}", path.display()), Some(path.to_path_buf()));
        }
    }

    fn check_files(&mut self) {
        self.follow_files();
        if self.last_file_check.elapsed().as_secs_f32() < 0.25 {
            return;
        }
        self.last_file_check = Instant::now();
        // A new build of the engine? (Wait until the file has stopped
        // changing for a moment: the build is still writing it.)
        if let Some((path, seen)) = &self.original_exe {
            let now = stamp(path);
            if now.is_some() && now != *seen {
                let quiet = now.and_then(|t| t.elapsed().ok()).is_some_and(|e| e.as_secs_f32() > 1.5);
                if quiet && self.new_build_since.is_none() {
                    self.new_build_since = Some(Instant::now());
                    self.say("The engine was rebuilt: restarting into the new build when you're not playing (or press Update)");
                }
            }
        }
        // Restart when nothing would be lost: not playing, not dragging, not
        // typing a note, not in the asset workshop (its draft isn't a scene).
        if self.new_build_since.is_some() && !self.playing && matches!(self.drag, Drag::None) && self.note_draft.is_none() && !self.workshop.open {
            self.restart_into_new_build();
        }
        if self.mats.changed_on_disk() {
            self.mats.reload();
            self.sim.materials_changed(&self.mats);
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
                self.reload_highlighting(format!("{} changed", path.display()));
            } else if !self.playing
                && let Some((p, _)) = self.scene_assets.iter().find(|(p, s)| stamp(p).is_some_and(|now| Some(now) != *s))
            {
                let why = format!("{} changed (the scene uses it)", p.display());
                self.reload_highlighting(why);
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
        self.endless_reset_origin = self.endless.as_ref().map_or((0, 0), |e| e.origin);
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
            self.endless_reset_origin = self.endless.as_ref().map_or((0, 0), |e| e.origin);
        }
        let t = Instant::now();
        self.sim.step(&self.mats);
        self.timeline.record(&mut self.sim);
        self.stream_endless();
        self.sim_ms = self.sim_ms * 0.9 + t.elapsed().as_secs_f32() * 1000.0 * 0.1;
        self.ran_since_snapshot = true;
    }

    /// Endless maps: keep the player away from the world's edges by
    /// scrolling the map (endless.rs). Cameras move with it; undo and
    /// rewind start again (they'd show the old window).
    fn stream_endless(&mut self) {
        let Some(e) = &mut self.endless else { return };
        let focus = match self.sim.physics.actor_status().first() {
            Some(a) if !a.dead => (a.x, a.y),
            _ => match self.sim.follow.and_then(|id| self.sim.physics.pose_of(id)) {
                Some((x, y, _)) => (x, y),
                None => return,
            },
        };
        let Some((dx, dy)) = e.update(&mut self.sim, &self.mats, focus) else { return };
        if let Some(c) = self.cam.as_mut() {
            c.0 += dx as f32;
            c.1 += dy as f32;
        }
        for p in e.problems.drain(..).collect::<Vec<_>>() {
            self.say(p);
        }
        self.timeline.clear();
        self.timeline.record(&mut self.sim);
        self.history = History::new();
        self.selection = Selection::None;
    }

    fn reset(&mut self) {
        let snap = self.snapshot.clone();
        self.change("reset", |app| app.sim = Sim::restore(&snap, &app.mats));
        // Endless: back to where the run started on the map, all fresh.
        let origin = self.endless_reset_origin;
        if let Some(e) = &mut self.endless {
            e.new_run();
            e.origin = origin;
        }
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
        // What the AI is doing, and the follow switch.
        let note = self.agent_note.as_ref().filter(|(t, _)| t.elapsed().as_secs() < 120).map(|(_, m)| m.clone());
        if note.is_some() || self.follow_pending.is_some() {
            ui.horizontal(|ui| {
                if let Some(m) = &note {
                    ui.label(RichText::new(format!("🤖 {m}")).strong().color(Color32::from_rgb(140, 200, 255)));
                }
                if let Some(p) = self.follow_pending.clone() {
                    ui.label(RichText::new(format!("AI is working on {}", p.display())).weak());
                    if ui.button("Go there").clicked() {
                        self.follow_pending = None;
                        self.follow_to(&p);
                    }
                    if ui.small_button("×").clicked() {
                        self.follow_pending = None;
                    }
                }
            });
        }
        if let Some((t, text)) = self.update_note.clone() {
            if t.elapsed().as_secs() < 20 {
                ui.label(RichText::new(format!("✨ {text}")).strong().color(Color32::from_rgb(140, 230, 140)));
            } else {
                self.update_note = None;
            }
        }
        if self.new_build_since.is_some() {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔄 A new build of the engine is ready.").strong().color(Color32::from_rgb(255, 214, 64)));
                if ui.button("Update now (keeps your scene)").clicked() {
                    self.restart_into_new_build();
                }
            });
        }
        // Row 1: the menu bar. Files, editing, view, how the world looks, tools.
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("GRAVE").size(20.).strong().color(Color32::from_rgb(105, 221, 204)))
                .on_hover_text("Grave Engine level editor");
            ui.add_space(4.0);
            ui.menu_button("📁 File", |ui| self.file_menu(ui)).response.on_hover_text("Open, save and reload scenes");
            ui.menu_button("✏ Edit", |ui| {
                if ui.add_enabled(self.history.can_undo(), egui::Button::new("↶ Undo   Ctrl+Z")).clicked() { self.undo(); ui.close(); }
                if ui.add_enabled(self.history.can_redo(), egui::Button::new("↷ Redo   Ctrl+Y")).clicked() { self.redo(); ui.close(); }
                ui.separator();
                if ui.button("🗑 Clear the scene").on_hover_text("Empty the world (Undo brings it back)").clicked() {
                    let (w, h) = (self.sim.world.w, self.sim.world.h);
                    self.change("clear", |app| app.sim = Sim::empty(w, h, &app.mats));
                    self.playing = false;
                    self.selection = Selection::None;
                    self.timeline.clear();
                    ui.close();
                }
                ui.separator();
                ui.menu_button("🗺 Generate caves", |ui| self.generate_menu(ui)).response.on_hover_text("Fill the world with caves made from herringbone tiles (like Noita)");
            }).response.on_hover_text("Undo, redo, clear, generate caves");
            ui.menu_button("👁 View", |ui| {
                if ui.button("⤢ Fit the whole world").clicked() { self.fit_view(); ui.close(); }
                let mut follow = self.sim.follow.is_some();
                if ui.checkbox(&mut follow, "📷 Camera follows the selected object").changed() {
                    if follow { self.start_following(); } else { self.sim.follow = None; }
                }
                ui.checkbox(&mut self.view.heat, "🌡 Heat view (temperatures as colours)");
                ui.checkbox(&mut self.studio.inspector_open, "🔍 Inspector panel (right)");
                ui.checkbox(&mut self.show_activity, "🤖 AI activity panel");
                ui.separator();
                ui.horizontal(|ui| {
                    let mut on = !self.sound.muted;
                    if ui.checkbox(&mut on, "🔊 Sound").changed() {
                        self.sound.muted = !on;
                    }
                    ui.add(egui::Slider::new(&mut self.sound.volume, 0.0..=1.0).show_value(false)).on_hover_text("Volume");
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.studio.snap, "Snap to grid");
                    ui.add(egui::DragValue::new(&mut self.studio.grid).range(1.0..=20.).suffix(" cells"));
                });
            }).response.on_hover_text("Camera, heat view, panels, snapping");
            ui.separator();
            ui.label(RichText::new("World").weak()).on_hover_text("How the world looks and feels: these settings are saved with the scene");
            ui.menu_button("🎬 Camera", |ui| self.camera_menu(ui)).response.on_hover_text("Slow motion, zoom, shake, follow smoothing");
            ui.menu_button("💡 Lighting", |ui| self.look_menu(ui, "lighting", Self::lighting_menu)).response.on_hover_text("Day and night, the sun, lamps");
            ui.menu_button("🏞 Background", |ui| self.look_menu(ui, "background", Self::background_menu)).response.on_hover_text("Sky colours and the layers of scenery behind the world");
            ui.menu_button("☁ Weather", |ui| self.look_menu(ui, "weather", Self::weather_menu)).response.on_hover_text("Snow, rain, hail, sandstorms, fog, wind gusts, lightning");
            ui.separator();
            if ui.button("Asset workshop").on_hover_text("Draw props and machines from real materials, with moving parts").clicked() {
                self.playing = false;
                self.workshop.open = true;
            }
            if ui.button("Animations").on_hover_text("The character's animation states and blends").clicked() {
                self.animation_editor.select(&self.sim, 0);
                self.animation_editor.open = true;
            }
            if let Some(level) = game_level(&self.scene_path)
                && ui.button("🎮 Play as game").on_hover_text("Open this course in its game, with controls and score (plays the saved file)").clicked()
            {
                self.play_as_game(level);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.toggle_value(&mut self.follow_ai, "👁 Follow AI").on_hover_text(
                    "On: when an AI agent changes a scene or asset, it opens here so you can watch (Undo goes back).\nOff: work on your own; you'll get a \"Go there\" button instead.",
                );
                let fresh = self.activity.iter().filter(|a| now_secs().saturating_sub(a.at) < 600).count();
                ui.toggle_value(&mut self.show_activity, if fresh > 0 { format!("🤖 Activity ({fresh})") } else { "🤖 Activity".to_string() })
                    .on_hover_text("What the AI has been doing, with buttons to jump back to each scene or asset");
            });
        });
        ui.separator();
        // Row 2: the toolbar. Running the scene, undo, speed, camera.
        ui.horizontal_wrapped(|ui| {
            let testing = self.studio.tab == Tab::Tests;
            let running = if testing { self.studio.test_running } else { self.playing };
            if ui
                .add(egui::Button::new(RichText::new(if running { "⏸ Pause" } else { "▶ Play" }).strong()).fill(Color32::from_rgb(35, 105, 94)).min_size(Vec2::new(96., 28.)))
                .on_hover_text("Run the simulation (Space)")
                .clicked()
            {
                if testing { self.studio.test_running = !running; } else if running { self.playing = false; } else { self.play(); }
            }
            if ui.button("⏭ Step").on_hover_text("Advance one frame").clicked() {
                if testing {
                    if let Some(s) = &mut self.studio.test { s.step(&self.mats); self.studio.test_frame += 1; self.studio.test_running = false; }
                } else {
                    self.playing = false;
                    self.step_once();
                }
            }
            if ui.button("↺ Reset").on_hover_text("Back to how the scene was before you pressed Play (R)").clicked() {
                if testing { self.start_studio_test(); self.studio.test_running = false; } else { self.reset(); }
            }
            ui.separator();
            if ui.add_enabled(self.history.can_undo(), egui::Button::new("↶ Undo")).on_hover_text("Undo (Ctrl+Z)").clicked() { self.undo(); }
            if ui.add_enabled(self.history.can_redo(), egui::Button::new("↷ Redo")).on_hover_text("Redo (Ctrl+Y)").clicked() { self.redo(); }
            ui.separator();
            ui.add_sized([120., 24.], egui::Slider::new(&mut self.speed, 0.05..=4.).logarithmic(true).suffix("x"))
                .on_hover_text("Simulation speed: below 1 is slow motion");
            ui.separator();
            if ui.button("⤢ Fit").on_hover_text("Show the whole world").clicked() { self.fit_view(); }
            let mut follow = self.sim.follow.is_some();
            if ui.toggle_value(&mut follow, "📷 Follow").on_hover_text("The camera follows the selected object").changed() {
                if follow { self.start_following(); } else { self.sim.follow = None; }
            }
            ui.toggle_value(&mut self.view.heat, "🌡 Heat").on_hover_text("Show temperatures as colours (H)");
            let mut on = !self.sound.muted;
            let label = if on { "🔊 Sound" } else { "🔇 Sound" };
            if ui
                .toggle_value(&mut on, label)
                .on_hover_text("Sound made by the materials themselves: metal rings, wood knocks, glass chimes, stone thuds, water splashes, fire crackles, blasts boom.
Click to mute. The volume is in the View menu.")
                .changed()
            {
                self.sound.muted = !on;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let name = Path::new(&self.scene_path).file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                ui.label(RichText::new(name).strong()).on_hover_text(format!("The open scene: {}", self.scene_path));
            });
        });
    }

    /// The File menu: the scene's file, save and reload, and every scene to
    /// open, grouped.
    fn file_menu(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Scene file").weak());
        ui.add(egui::TextEdit::singleline(&mut self.scene_path).desired_width(260.)).on_hover_text("Where Save writes (and Reload reads)");
        if ui.button("💾 Save   Ctrl+S").on_hover_text("Write the scene to its file").clicked() { self.save_scene(); ui.close(); }
        if ui.button("📂 Reload from file").on_hover_text("Throw away changes and load the file again").clicked() { self.load_scene(); ui.close(); }
        ui.separator();
        ui.label(RichText::new("Open a scene").weak());
        let mut files: Vec<PathBuf> = std::fs::read_dir("scenes")
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|s| s == "txt"))
            .collect();
        files.sort();
        for group in SCENE_GROUPS {
            ui.menu_button(group, |ui| {
                if group == "Games & levels" {
                    if ui.button("Dust Hawk — Operation Red Mesa (2.5D)").clicked() { self.open_overhead_game("dust-hawk"); ui.close(); }
                    if ui.button("Flight & destruction lab (2.5D)").clicked() { self.open_overhead_game("flight-test"); ui.close(); }
                    ui.separator();
                }
                egui::ScrollArea::vertical().max_height(420.).show(ui, |ui| {
                    for f in files.iter().filter(|f| scene_group(f) == group) {
                        let name = f.file_stem().unwrap_or_default().to_string_lossy().replace('-', " ");
                        if ui.button(name).on_hover_text(f.display().to_string()).clicked() {
                            self.scene_path = f.display().to_string();
                            self.load_scene();
                            ui.close();
                        }
                    }
                });
            });
        }
    }

    fn palette(&mut self, ui: &mut egui::Ui) {
        self.workspace(ui);
        ui.add_space(2.0);
        // Tools: a tidy grid, every one explained on hover.
        ui.label(RichText::new("TOOLS").small().strong().weak());
        egui::Grid::new("tool_grid").num_columns(2).spacing(Vec2::new(4.0, 4.0)).show(ui, |ui| {
            let tools = [
                (Tool::Select, "👆 Select", "Click or drag a box to select. Drag to move; the handles turn and resize. (V)"),
                (Tool::Paint, "🖌 Paint", "Paint the chosen material (B). Shift + mouse wheel changes the brush size."),
                (Tool::Erase, "🧽 Erase", "Turn things back into air (or right-click with any tool)"),
                (Tool::Heat, "🔥 Heat", "Make things hotter: light fires, melt metal"),
                (Tool::Cool, "❄ Cool", "Make things colder: freeze water, put out fires"),
                (Tool::Object, "📦 Make object", "Drag a box round something you painted: it becomes a loose object that falls, tumbles and breaks"),
                (Tool::Place, "🧩 Place", "Click to place copies of the chosen template"),
                (Tool::Join, "🔗 Join", "Join two things: hinge, wheel, weld or rope (empty space = the ground)"),
            ];
            for (n, (tool, label, tip)) in tools.into_iter().enumerate() {
                let on = self.tool == tool;
                let b = egui::Button::new(RichText::new(label).size(14.0)).selected(on).min_size(Vec2::new((ui.available_width() / 2.0 - 4.0).max(100.0), 26.0));
                if ui.add(b).on_hover_text(tip).clicked() {
                    self.tool = tool;
                    self.studio.placement = None;
                    if tool != Tool::Select {
                        self.studio.tab = Tab::Materials;
                    }
                }
                if n % 2 == 1 {
                    ui.end_row();
                }
            }
        });
        match self.studio.tab {
            Tab::Assets=>{self.asset_browser(ui);return;},Tab::Build=>{egui::ScrollArea::vertical().id_salt("build_controls").show(ui,|ui|self.builder_panel(ui));return;},Tab::Events=>{egui::ScrollArea::vertical().id_salt("event_controls").show(ui,|ui|self.event_panel(ui));return;},Tab::Lights=>{egui::ScrollArea::vertical().id_salt("light_controls").show(ui,|ui|self.lights_panel(ui));return;},Tab::Tests=>{egui::ScrollArea::vertical().id_salt("test_controls").show(ui,|ui|self.tests_panel(ui));return;},Tab::Materials=>{}
        }
        if self.tool == Tool::Join {
            ui.add_space(4.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
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
                    ui.checkbox(&mut self.join_motor, "With a motor").on_hover_text("It turns by itself");
                    if self.join_motor {
                        ui.add(egui::Slider::new(&mut self.join_speed, -5.0..=5.0).text("turns/s")).on_hover_text("Minus turns the other way");
                        ui.add(egui::Slider::new(&mut self.join_strength, 0.5..=50.0).logarithmic(true).text("strength")).on_hover_text("How hard it can push, in kN·m");
                        ui.checkbox(&mut self.join_drive, "Drive with ← → / A D").on_hover_text("Only turns while you hold the keys (vehicles)");
                    }
                }
                ui.label(RichText::new(match self.join_first {
                    None => "Click the first thing (or empty space for the ground).",
                    Some(_) => "Now click the second thing.",
                }).size(13.0).weak());
            });
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Brush").on_hover_text("How big the paint, erase, heat and cool brushes are (Shift + mouse wheel)");
            ui.add(egui::Slider::new(&mut self.brush_size, 1..=40).suffix(" cells"));
        });
        ui.separator();
        // Materials, grouped. The search box shows every match at once.
        ui.horizontal(|ui| {
            ui.label(RichText::new("MATERIALS").small().strong().weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button(format!("⚙ {} settings", self.mats.get(self.brush_mat).name.replace('_', " "))).on_hover_text("Play with this material's settings (or right-click any material)").clicked() {
                    self.mat_settings = Some(self.brush_mat);
                }
            });
        });
        ui.add(egui::TextEdit::singleline(&mut self.studio.material_filter).hint_text("🔍 Find a material"));
        let filter = self.studio.material_filter.to_lowercase();
        egui::ScrollArea::vertical().id_salt("materials_list").max_height((ui.available_height() - 150.0).max(160.0)).show(ui, |ui| {
            for group in grave_engine::material::GROUPS.iter().copied().chain(std::iter::once("Other")) {
                let mut ids: Vec<MatId> = (0..self.mats.list.len() as MatId)
                    .filter(|&i| i != AIR)
                    .filter(|&i| {
                        let g = self.mats.get(i).group();
                        if group == "Other" { !grave_engine::material::GROUPS.contains(&g) } else { g == group }
                    })
                    .filter(|&i| filter.is_empty() || self.mats.get(i).name.contains(&filter))
                    .collect();
                if ids.is_empty() {
                    continue;
                }
                ids.sort_by(|a, b| self.mats.get(*a).name.cmp(&self.mats.get(*b).name));
                let holds_brush = ids.contains(&self.brush_mat);
                egui::CollapsingHeader::new(RichText::new(format!("{group}  ({})", ids.len())).strong())
                    .id_salt(("mat_group", group))
                    .default_open(holds_brush || group == "Earth, sand & snow")
                    .open(if filter.is_empty() { None } else { Some(true) })
                    .show(ui, |ui| {
                        for id in ids {
                            self.material_button(ui, id);
                        }
                    });
            }
        });
        ui.separator();
        egui::CollapsingHeader::new(RichText::new("Templates").strong()).id_salt("templates_section").default_open(false).show(ui, |ui| {
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
        });
        egui::CollapsingHeader::new(RichText::new("Import").strong()).id_salt("import_section").default_open(false).show(ui, |ui| {
            ui.label(RichText::new("Drop an image or 3D model onto the world to add it (.png .jpg .glb .obj .fbx .blend).").size(13.0));
            ui.checkbox(&mut self.drop_static, "Drop as fixed scenery").on_hover_text("Off: it arrives as a loose object that can fall and break");
        });
        egui::CollapsingHeader::new(RichText::new("Messages").strong()).id_salt("messages_section").default_open(true).show(ui, |ui| {
            for (when, msg) in self.messages.iter().rev().take(5) {
                let age = when.elapsed().as_secs_f32();
                let alpha = (1.0 - (age - 8.0) / 4.0).clamp(0.35, 1.0);
                let col = ui.visuals().text_color().gamma_multiply(alpha);
                ui.label(RichText::new(msg).size(13.0).color(col));
            }
        });
    }

    /// One material in the list: a colour swatch and its name; hover for what
    /// it does, click to paint with it, right-click for its settings.
    fn material_button(&mut self, ui: &mut egui::Ui, id: MatId) {
        let m = self.mats.get(id);
        let selected = self.brush_mat == id && self.tool == Tool::Paint;
        let c = color32(m.color);
        let text_col = ui.visuals().text_color();
        let label = RichText::new(m.name.replace('_', " ")).size(14.0).color(text_col);
        let mut b = egui::Button::new(label).min_size(Vec2::new(ui.available_width(), 24.0));
        if selected {
            b = b.stroke(Stroke::new(2.0, Color32::WHITE)).selected(true);
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
        tip += "\n\nClick: paint with it.  Right-click: its settings (sliders).";
        let r = ui.add(b).on_hover_text(tip);
        ui.painter().rect_filled(Rect::from_min_size(r.rect.min + Vec2::new(8., 6.), Vec2::splat(12.)), 3., c);
        ui.painter().rect_stroke(Rect::from_min_size(r.rect.min + Vec2::new(8., 6.), Vec2::splat(12.)), 3., Stroke::new(1.0, Color32::from_black_alpha(140)), egui::StrokeKind::Inside);
        if r.clicked() {
            self.brush_mat = id;
            self.tool = Tool::Paint;
        }
        if r.secondary_clicked() {
            self.mat_settings = Some(id);
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
            let moved=x!=b.x || y!=b.y || (deg.to_radians()-b.angle).abs()>0.0001;
            if moved {assemblies::transform(&mut self.sim,&self.mats,&[id],self.studio.whole_assets,(x-b.x,y-b.y),deg.to_radians()-b.angle,(b.x,b.y));}
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
                    if ui.button("Reset to template").on_hover_text("Undo this copy's own changes").clicked()
                        && let Some(t) = self.templates.get(&inst.template).cloned() {
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
            let groups=assemblies::members(&app.sim,&ids,app.studio.whole_assets);
            for (z,ids) in groups.into_iter().enumerate() {let p=assemblies::plane_mut(&mut app.sim,z);for id in ids {
                if let Some(spec)=p.physics.remove(&mut p.world,id) {edit::freeze(&mut p.world,&spec);}
            }}
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
                        "{:.0} fps · sim {:.1} ms · draw {:.1} ms · {} objects · {} undo steps · frame {}",
                        self.fps,
                        self.sim_ms,
                        self.draw_ms,
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

        // The camera: by default the whole world fits; zoomed in, it shows part
        // of it (and can follow an object).
        let avail = ui.available_size();
        let fit = (avail.x / w as f32).min(avail.y / h as f32).max(0.05);
        let (outer, _) = ui.allocate_exact_size(avail, Sense::hover());
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
        resp.widget_info(||egui::WidgetInfo::labeled(egui::Role::Image,true,"Level canvas"));
        let painter = ui.painter_at(outer);
        // Draw only the part of the world on screen now (this frame's camera
        // and shake, plus a margin): big worlds stay fast.
        self.renderer.light(&self.sim);
        self.renderer.set_camera(cx, cy); // backdrop layers slide by at their depth
        let half = (outer.width() / scale / 2.0 + shake.0.abs(), outer.height() / scale / 2.0 + shake.1.abs());
        // Weather falls around what's on screen.
        self.sim.weather_view = Some((cx - half.0 - 25.0, cy - half.1, cx + half.0 + 25.0, cy + half.1));
        let win = grave_engine::render::view_window(&self.sim.world, (cx, cy), half, 4.0);
        let drawn = Instant::now();
        let rgba = self.renderer.draw_window(&self.sim.world, &self.mats, self.view, win);
        self.draw_ms = self.draw_ms * 0.9 + drawn.elapsed().as_secs_f32() * 1000.0 * 0.1;
        let image = egui::ColorImage::from_rgba_unmultiplied([win.2, win.3], rgba);
        match &mut self.texture {
            Some(t) if t.size() == [win.2, win.3] => t.set(image, TextureOptions::NEAREST),
            _ => self.texture = Some(ui.ctx().load_texture("grid", image, TextureOptions::NEAREST)),
        }
        let tex = self.texture.as_ref().unwrap();
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
        let pointer = if self.studio.placement.is_some() {ui.input(|i|i.pointer.latest_pos()).filter(|p|outer.contains(*p))} else {resp.hover_pos().or(resp.interact_pointer_pos())};
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
        // Fresh changes from disk: a fading outline for a few seconds.
        let age = self.highlight_since.elapsed().as_secs_f32();
        if !self.highlights.is_empty() && age < 12.0 {
            let a = ((1.0 - age / 12.0).sqrt() * 255.0) as u8;
            let pulse = 2.0 + (age * 6.0).sin().abs() * 1.5;
            for &(x0, y0, x1, y1) in &self.highlights {
                let r = Rect::from_min_max(to_screen((x0 as f32, y0 as f32)), to_screen((x1 as f32, y1 as f32)));
                painter.rect_stroke(r, 2.0, Stroke::new(pulse, Color32::from_rgba_unmultiplied(255, 220, 60, a)), egui::StrokeKind::Outside);
            }
        } else if !self.highlights.is_empty() {
            self.highlights.clear();
        }
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

        if self.studio_canvas_input(ui,&resp,pointer,&to_cells,&to_screen) {return;}

        // ---- start a drag
        if pressed && matches!(self.drag, Drag::None)
            && let Some(p) = pointer {
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
                                let motor = (self.join_motor && matches!(self.join_kind, JoinChoice::Hinge | JoinChoice::Wheel)).then_some(Motor {
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
                    let now = if self.playing {to_cells(p)} else {self.studio.snapped(to_cells(p))};
                    self.fling.push((time, now));
                    if self.fling.len() > 6 {
                        self.fling.remove(0); // the last few frames (~0.1 s)
                    }
                    let (dx, dy) = (now.0 - last.0, now.1 - last.1);
                    if dx != 0.0 || dy != 0.0 {
                        self.history.touch(&mut self.sim, "move");
                        let ids=self.selected_ids();
                        assemblies::transform(&mut self.sim,&self.mats,&ids,self.studio.whole_assets,(dx,dy),0.,(0.,0.));
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
                    assemblies::transform(&mut self.sim,&self.mats,&[id],self.studio.whole_assets,(0.,0.),angle-b.angle,(b.x,b.y));
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
                    if let Some(f) = self.last_scale(id, start_d, pointer.map(to_cells))
                        && (f - 1.0).abs() > 0.02 {
                            self.change("resize", |app| {
                                app.sim.physics.edit(&mut app.sim.world, &app.mats, id, |s| edit::transform_body(s, Transform::Scale(f)));
                            });
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
            // Soaked (water, oil) or on fire: an icon over the head says so.
            if let Some(s) = a.soak
                && !a.dead
            {
                let head = origin + Vec2::new(a.x * scale, (a.y - body_h as f32 / 2.0 - 3.5) * scale);
                let r = (scale * 1.6).clamp(5.0, 11.0);
                if s.burning {
                    let f = (self.sim.world.frame / 3 % 2) as u8;
                    painter.circle_filled(head, r * 1.15, Color32::from_rgb(255, 120 + 60 * f, 20));
                    painter.text(head, egui::Align2::CENTER_CENTER, "🔥", egui::FontId::proportional(r * 1.6), Color32::from_rgb(255, 240, 200));
                } else {
                    let c = self.mats.get(s.liquid).color;
                    let alpha = (90.0 + 165.0 * s.amount) as u8;
                    // A drop: a circle with a point on top.
                    let col = Color32::from_rgba_unmultiplied(c[0].saturating_add(40), c[1].saturating_add(40), c[2].saturating_add(40), alpha);
                    painter.add(egui::Shape::convex_polygon(vec![head + Vec2::new(0.0, -r * 1.7), head + Vec2::new(r * 0.86, -r * 0.5), head + Vec2::new(-r * 0.86, -r * 0.5)], col, egui::Stroke::NONE));
                    painter.circle(head, r, col, egui::Stroke::new(1.0, Color32::from_white_alpha(alpha / 2)));
                }
            }
            let Some(look) = &a.look else { continue };
            let (frame, label, index) = look.sample(&a.anim, a.anim_time, a.blend.as_ref());
            let key = (look.dir.clone(), label.to_string(), index);
            let tex = self.look_textures.entry(key).or_insert_with(|| {
                let img = egui::ColorImage::from_rgba_unmultiplied([frame.w, frame.h], &frame.rgba);
                ui.ctx().load_texture(format!("{}/{}/{index}", look.name, a.anim), img, TextureOptions::LINEAR)
            });
            let base_cpp = look.cells_per_px(body_h);
            let cpp = base_cpp * frame.scale;
            let feet = a.y + body_h as f32 / 2.0 + a.visual_offset.1;
            let left = a.x + a.visual_offset.0 - frame.w as f32 * cpp / 2.0;
            let top = feet - look.feet * base_cpp;
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
            let tint = if a.hurt_by.is_some() && !a.dead && (self.sim.world.frame / 5).is_multiple_of(2) {
                Color32::from_rgb(255, 150, 150)
            } else if let Some(s) = a.soak {
                // Soaked: tinted towards the liquid and darker; on fire, flickering orange.
                if s.burning {
                    let f = (self.sim.world.frame / 3 % 2) as f32;
                    Color32::from_rgb(255, (170.0 + 40.0 * f) as u8, 110)
                } else {
                    let c = self.mats.get(s.liquid).color;
                    let k = 0.45 * s.amount;
                    let mix = |v: u8| ((255.0 * (1.0 - k) + v as f32 * k) * (1.0 - 0.2 * s.amount)) as u8;
                    Color32::from_rgb(mix(c[0]), mix(c[1]), mix(c[2]))
                }
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
        let hurting = p.hurt_by.is_some() && (self.sim.world.frame / 6).is_multiple_of(2);
        let health_col = if hurting { Color32::from_rgb(255, 230, 120) } else { Color32::from_rgb(220, 60, 60) };
        bar(0.0, p.health / 100.0, health_col, &format!("health {:.0}", p.health));
        let mut y = 16.0;
        if p.breath < 10.0 {
            bar(y, p.breath / 10.0, Color32::from_rgb(90, 170, 255), "breath");
            y += 16.0;
        }
        // Soaked: what in, how much, and what it means (players need telling).
        if let Some(s) = p.soak {
            let m = self.mats.get(s.liquid);
            let (col, label) = if s.burning {
                (Color32::from_rgb(255, 140, 30), "ON FIRE! Jump in water".to_string())
            } else if m.catches_fire_at.is_some() {
                (Color32::from_rgb(m.color[0].saturating_add(60), m.color[1].saturating_add(50), m.color[2].saturating_add(30)), format!("covered in {}: catches fire easily", m.name))
            } else if m.puts_out_fire {
                (Color32::from_rgb(m.color[0].saturating_add(40), m.color[1].saturating_add(60), m.color[2].saturating_add(40)), format!("{}-soaked: heat hurts less", m.name))
            } else {
                (Color32::from_rgb(m.color[0], m.color[1], m.color[2]), format!("covered in {}", m.name))
            };
            bar(y, s.amount, col, &label);
            y += 16.0;
        }
        let text = if p.dead {
            Some(format!("You {}. Back in a moment...", p.hurt_by.unwrap_or("died")))
        } else if p.hanging {
            Some("Hanging: release W/Up, then press again to climb. S/Down drops.".to_string())
        } else if p.rolling {
            Some("Rolling! C holds a crouch afterwards.".to_string())
        } else if p.crouched {
            Some(if p.stand_blocked { "Low ceiling: keep moving to stand up safely." } else { "Crouching: release C to stand. X rolls." }.to_string())
        } else if p.climbing {
            Some("Climbing onto the ledge. S/Down lets go.".to_string())
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
            if self.tool != Tool::Join && !matches!(j.kind,JointKind::Rope {..}) {continue;}
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
        if let Selection::Objects(ids) = &self.selection
            && ids.len() == 1 {
                let id = ids[0];
                if let Some(b) = self.sim.physics.spec_of(&self.sim.world, id) {
                    let (rot, corner) = handles(&b);
                    if to_screen(rot).distance(p) < 12.0 {
                        return Drag::Rotate(id);
                    }
                    if !(self.studio.whole_assets && self.sim.physics.assembly(id).is_some()) && to_screen(corner).distance(p) < 12.0 {
                        let d = ((cf.0 - b.x).powi(2) + (cf.1 - b.y).powi(2)).sqrt();
                        return Drag::Scale(id, d);
                    }
                }
            }
        // An object under the mouse.
        if self.sim.world.in_bounds(cell.0, cell.1)
            && let Some(id) = self.sim.physics.id_at(&self.sim.world, cell.0 as usize, cell.1 as usize) {
                let mut ids = if shift { self.selected_ids() } else { Vec::new() };
                let already = self.selected_ids().contains(&id);
                if !ids.contains(&id) {
                    ids.push(id);
                }
                if already && !shift {
                    ids = self.selected_ids();
                }
                self.selection = Selection::Objects(ids);
                self.fling.clear(); // a new swing starts here
                return Drag::Objects(cf);
            }
        // Inside the selected area: move it.
        if let Selection::Area(r) = self.selection
            && r.contains(cell.0, cell.1) {
                return Drag::Area(cell);
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
            Selection::Objects(raw_ids) => {
                let ids=self.selected_ids();
                for &id in &ids {
                    let Some(b) = self.sim.physics.spec_of(&self.sim.world, id) else { continue };
                    let pts: Vec<Pos2> = corners(&b, 1.0).iter().map(|&c| to_screen(c)).collect();
                    painter.add(egui::Shape::closed_line(pts, Stroke::new(2.0, SELECT_COLOR)));
                    if raw_ids.len() == 1 && id==raw_ids[0] {
                        let (rot, corner) = handles(&b);
                        let top = {
                            let c = corners(&b, 1.0);
                            ((c[0].0 + c[1].0) / 2.0, (c[0].1 + c[1].1) / 2.0)
                        };
                        painter.line_segment([to_screen(top), to_screen(rot)], Stroke::new(1.5, SELECT_COLOR));
                        painter.circle(to_screen(rot), 7.0, Color32::from_rgb(40, 40, 50), Stroke::new(2.0, SELECT_COLOR));
                        if !(self.studio.whole_assets && self.sim.physics.assembly(id).is_some()) {
                            let k = to_screen(corner);
                            painter.rect(Rect::from_center_size(k, Vec2::splat(12.0)), 2.0, Color32::from_rgb(40, 40, 50), Stroke::new(2.0, SELECT_COLOR), egui::StrokeKind::Middle);
                        }
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
            self.studio.placement=None;
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
        if space && self.studio.tab==Tab::Tests {self.studio.test_running=!self.studio.test_running;}
        else if space {
            if self.playing { self.playing = false } else { self.play() }
        }
        if h {
            self.view.heat = !self.view.heat;
        }
        if r && self.studio.tab==Tab::Tests {self.start_studio_test();}
        else if r {
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
        if self.workshop.open {
            let mut action=None;
            egui::CentralPanel::default().show(ui,|ui|{action=self.workshop.ui(ui,&self.mats,dt);});
            if let Some(crate::workshop::Action::Place(asset))=action {
                self.playing=false;
                let (x,y)=self.cam.map_or((self.sim.world.w as f32/2.,self.sim.world.h as f32/2.),|(x,y,_)|(x,y));
                self.change("place material asset",|app|{
                    match asset.instantiate(&mut app.sim,&app.mats,x,y){
                        Ok(parts)=>{app.selection=Selection::Objects(parts.iter().map(|(_,id)|*id).collect());app.tool=Tool::Select;app.say(format!("Placed {} with {} named parts. Drag the selection to position it.",asset.name,parts.len()));app.workshop.open=false;},
                        Err(e)=>app.say(e),
                    }
                });
            }
            self.studio.needs_refresh=true;
            // Follow mode works here too (an asset the AI changes opens in the workshop).
            self.follow_files();
            return;
        }

        if dt > 0.0 {
            self.fps = self.fps * 0.95 + (1.0 / dt as f32) * 0.05;
        }

        self.check_files();
        // Where the window is, so an engine update brings it back in the same place.
        self.window_at = ctx.input(|i| {
            let v = i.viewport();
            v.outer_rect.zip(v.inner_rect).map(|(o, inner)| [o.min.x, o.min.y, inner.width(), inner.height(), v.maximized.unwrap_or(false) as u8 as f32])
        });
        self.activity_window(&ctx);
        let dropped: Vec<PathBuf> =
            ui.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        for path in dropped {
            self.import_file(&path);
        }
        self.shortcuts(&ctx);

        // Driving and the player: keyboard or game controller, through the
        // engine's controls (assets/input/editor.input). → / D or the left
        // stick drive motors marked `drive`, or walk the player.
        let typing = ctx.egui_wants_keyboard_input();
        self.input.update(&ctx);
        let live = !typing && self.playing && !ctx.input(|i| i.modifiers.shift);
        let throttle = if live { self.input.axis("walk") } else { 0.0 };
        let has_player = self.sim.physics.actor_count() > 0;
        self.sim.physics.set_throttle(if has_player { 0.0 } else { throttle });
        let jump = live && self.input.down("jump");
        let drop = live && self.input.down("drop");
        let crouch = live && self.input.down("crouch");
        let roll = live && self.input.down("roll");
        self.sim.physics.set_controls(grave_engine::physics::Controls { crouch, roll, walk: if has_player { throttle } else { 0.0 }, jump, drop });

        // Run the simulation at a steady 60 steps per second (times speed).
        if self.playing {
            // Slow motion from a trigger (or the game) slows time on top.
            self.sim_clock += dt * SIM_HZ * self.speed as f64 * self.sim.camera.time_scale() as f64;
            let mut steps = 0;
            let step_started=Instant::now();
            while self.sim_clock >= 1.0 && steps < 4 && (steps==0 || step_started.elapsed().as_secs_f64()<0.012) {
                self.step_once();
                self.sim_clock -= 1.0;
                steps += 1;
            }
            // Heavy destruction must not turn a delayed UI frame into six or
            // eight further physics steps before accepting mouse input again.
            if self.sim_clock>=1.0 {self.sim_clock=self.sim_clock.min(1.0);}
        }

        egui::Panel::top("top").show(ui, |ui| {
            ui.add_space(4.0);
            self.top_bar(ui);
            ui.add_space(2.0);
        });
        egui::Panel::left("palette").resizable(true).default_size(280.0).min_size(230.0).max_size(420.0).show(ui, |ui| self.palette(ui));
        notes::follow(&self.sim, &mut self.notes);
        if self.studio.inspector_open {egui::Panel::right("inspector").resizable(true).default_size(270.0).min_size(230.0).max_size(440.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.studio_inspector(ui);
            });
        });
        }
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::bottom("timeline").show(ui, |ui| self.timeline_bar(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(10, 10, 14)))
            .show(ui, |ui| {if self.studio.tab==Tab::Tests {self.studio_test_canvas(ui,dt);} else {self.canvas(ui);}});
        // Hear what just happened, around what's on screen (paused = quiet).
        if let Some((x0, y0, x1, y1)) = self.sim.weather_view {
            let speed = if self.playing && self.studio.tab != Tab::Tests { self.speed * self.sim.camera.time_scale() } else { 0.0 };
            self.sound.listen(&mut self.sim, &self.mats, (x0 + 25.0, y0, x1 - 25.0, y1), speed);
        }
        self.note_window(&ctx);
        self.material_settings_window(&ctx);
        self.animation_window(&ctx, dt as f32);

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

    #[test]
    fn a_scene_changed_on_disk_reloads_and_outlines_what_changed() {
        let dir = std::env::temp_dir().join(format!("grave-outline-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("s.txt");
        std::fs::write(&file, "size 120 80
fill air
rect stone 0 70 120 10
").unwrap();
        let mut app = Sandbox::new(PathBuf::from("materials"), Some(file.clone()));
        std::fs::write(&file, "size 120 80
fill air
rect stone 0 70 120 10
rect wood 90 50 12 20
").unwrap();
        app.reload_highlighting("changed".into());
        println!("{:?}", app.highlights);
        assert!(!app.highlights.is_empty(), "the new wood is outlined");
        let (x0, y0, x1, y1) = app.highlights.iter().fold((usize::MAX, usize::MAX, 0, 0), |a, r| (a.0.min(r.0), a.1.min(r.1), a.2.max(r.2), a.3.max(r.3)));
        assert!(x0 >= 80 && x1 <= 110 && y0 >= 40 && y1 <= 72, "only around the change: {:?}", app.highlights);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn course_scenes_can_be_played_as_a_game() {
        assert_eq!(game_level("scenes/mountain-run.txt"), Some(1));
        assert_eq!(game_level("scenes/stunt-yard.txt"), Some(0));
        assert_eq!(game_level("scenes/weather-house.txt"), None);
    }

    #[test]
    fn animation_editor_authors_and_applies_a_reusable_state() {
        let mut h = editor();
        let app = h.state_mut();
        let (sim, errors) = scene::load("size 100 80\nrect stone 0 70 100 10\nplayer 20 70 look hero\n", &app.mats, None).into_sim(&app.mats);
        assert!(errors.is_empty()); app.sim = sim; app.snapshot = app.sim.snapshot();
        h.run_steps(2);
        h.get_by_label("Animations").click(); h.run_steps(2);
        h.get_by_label("Add state").click(); h.run_steps(2);
        h.get_by_label("Apply to character").click(); h.run_steps(2);
        assert_eq!(h.state().sim.physics.export_actors()[0].animation.as_ref().unwrap().graph.states.len(), 10);
        assert_eq!(h.state().snapshot.actors[0].animation.as_ref().unwrap().graph.states.len(), 10);
        h.state_mut().undo();
        assert_eq!(h.state().snapshot.actors[0].animation.as_ref().unwrap().graph.states.len(), 9);
        h.state_mut().redo();
        h.state_mut().reset();
        assert_eq!(h.state().sim.physics.export_actors()[0].animation.as_ref().unwrap().graph.states.len(), 10);
    }

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
        app.studio.tab=Tab::Materials;app.studio.snap=false;app.tool=Tool::Paint;
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
    fn lighting_changes_can_be_undone_and_keep_the_reset_point() {
        let mut h = editor();
        h.get_by_label("💡 Lighting").click();
        h.run_steps(2);
        h.get_by_label_contains("Night").click();
        h.run_steps(2);
        assert!(h.state().sim.lighting.ambient < 0.5, "night");
        assert!(!h.state().ran_since_snapshot, "a lighting change isn't the sim running");
        h.key_press(Key::Escape);
        h.run_steps(2);
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run_steps(3);
        assert_eq!(h.state().sim.lighting.ambient, 1.0, "undo brings the daylight back");
    }

    #[test]
    fn a_new_drag_after_an_undo_mid_drag_does_not_fling() {
        let mut h = editor();
        h.key_press(Key::V);
        h.run_steps(2);
        h.state_mut().playing = true;
        // Start swinging the object fast, then undo in the middle of it.
        let a = at(&h, 50.0, 65.0);
        press(&mut h, a, true);
        for k in 1..=4 {
            h.event(Event::PointerMoved(a + Vec2::new(k as f32 * 60.0, 0.0)));
            h.run_steps(1);
        }
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run_steps(2);
        press(&mut h, a + Vec2::new(240.0, 0.0), false);
        h.state_mut().playing = true;
        h.run_steps(2);
        // Now a gentle nudge of the object: it mustn't fly off.
        let p = object(&h);
        let (from, to) = (at(&h, p.x, p.y), at(&h, p.x + 1.0, p.y));
        drag(&mut h, from, to);
        let v = object(&h);
        assert!(v.vx.abs() < 20.0, "a gentle nudge stays gentle: vx {}", v.vx);
    }

    #[test]
    fn the_weather_menu_has_presets_a_master_switch_and_undo() {
        let mut h = editor();
        h.get_by_label("☁ Weather").click();
        h.run_steps(2);
        assert!(h.get_all_by_role(egui::accesskit::Role::Slider).count() >= 7, "a slider for each kind of weather");
        h.get_by_label_contains("Storm").click();
        h.run_steps(2);
        let w = h.state().sim.weather.clone();
        assert!(w.rain > 0.5 && w.lightning > 0.0 && w.on, "the storm preset: {w:?}");
        h.key_press(Key::Escape);
        h.run_steps(2);
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run_steps(3);
        assert!(h.state().sim.weather.is_clear(), "undo clears the storm again");
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
        // Find a material with the search box (it opens its group), then right-click it.
        h.state_mut().studio.material_filter = "brick".into();
        h.run_steps(2);
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
        h.event(key(Key::C, true));
        h.event(key(Key::X, true));
        h.run_steps(2);
        let c = h.state().sim.physics.controls();
        assert!(c.crouch && c.roll, "named C/X bindings reach the physical character controls");
        h.event(key(Key::C, false));
        h.event(key(Key::X, false));
        h.run_steps(2);
        assert!(!h.state().sim.physics.controls().crouch && !h.state().sim.physics.controls().roll);
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
    #[test]
    fn asset_workshop_tests_are_isolated_and_placement_is_undoable(){
        let mut h=editor();let count=h.state().sim.physics.body_count();
        h.get_by_label("Asset workshop").click();h.run_steps(3);assert!(h.state().workshop.open);
        let draft=h.state().workshop.asset.clone();
        h.get_by_label("Burn").click();h.run_steps(10);
        assert_eq!(h.state().workshop.asset,draft);assert_eq!(h.state().sim.physics.body_count(),count);
        h.get_by_label("Reset preview").click();h.run_steps(2);
        h.get_by_label("Place at scene centre").click();h.run_steps(3);
        assert!(!h.state().workshop.open);assert_eq!(h.state().sim.physics.body_count(),count+3);assert_eq!(h.state().sim.physics.joints().len(),2);
        h.get_by_label_contains("Undo").click();h.run_steps(3);assert_eq!(h.state().sim.physics.body_count(),count);
    }
    #[test]
    fn asset_workshop_grid_strokes_undo_and_new_asset_remains_editable(){
        let mut h=editor();h.get_by_label("Asset workshop").click();h.run_steps(3);
        let before=h.state().workshop.asset.clone();let grid=h.get_by_label("Material asset drawing grid").rect();
        let a=grid.min+Vec2::new(2.5*9.,2.5*9.);let b=grid.min+Vec2::new(5.5*9.,2.5*9.);
        drag(&mut h,a,b);assert!(h.state().workshop.asset!=before,"grid stroke must paint");
        h.get_by_label("Undo").click();h.run_steps(3);assert_eq!(h.state().workshop.asset,before);
        h.get_by_label("New asset").click();h.run_steps(3);assert_eq!(h.state().workshop.asset.parts.len(),1);
        h.get_by_label("Rectangle").click();h.run_steps(2);let grid=h.get_by_label("Material asset drawing grid").rect();
        drag(&mut h,grid.min+Vec2::splat(2.5*9.),grid.min+Vec2::splat(5.5*9.));
        assert!(h.state().workshop.asset.parts[0].cells.iter().any(|m|*m!=AIR));
    }

    #[test]
    fn workshop_surface_paint_controls_keep_materials_and_support_undo() {
        let mut h=editor();h.get_by_label("Asset workshop").click();h.run_steps(3);
        let before=h.state().workshop.asset.clone();
        h.get_by_label("Surface paint").click();h.run_steps(3);
        h.get_by_label("Red").click();h.run_steps(2);
        let grid=h.get_by_label("Material asset drawing grid").rect();
        drag(&mut h,grid.min+Vec2::new(10.5*9.,10.5*9.),grid.min+Vec2::new(20.5*9.,10.5*9.));
        assert_eq!(h.state().workshop.asset.composite(),before.composite());
        assert!(h.state().workshop.asset.parts[0].paint.iter().any(|v|*v!=0));
        h.get_by_label("Undo").click();h.run_steps(3);
        assert_eq!(h.state().workshop.asset,before);
        h.get_by_label("Black").click();h.run_steps(2);
        h.get_by_label("Stamp lettering").click();h.run_steps(2);
        h.get_by_label("Drop").click();h.run_steps(3);
        assert_eq!(h.state().workshop.asset,before);
    }

    #[test]
    fn workshop_can_add_and_paint_a_depth_layer_without_overwriting_the_front() {
        let mut h=editor();h.get_by_label("Asset workshop").click();h.run_steps(3);
        let before=h.state().workshop.asset.clone();
        h.get_by_label("Add depth layer").click();h.run_steps(3);
        assert_eq!(h.state().workshop.asset.layers.len(),2);
        let grid=h.get_by_label("Material asset drawing grid").rect();
        drag(&mut h,grid.min+Vec2::splat(2.5*9.),grid.min+Vec2::splat(4.5*9.));
        let a=&h.state().workshop.asset;
        assert_eq!(a.parts[..before.parts.len()],before.parts);
        assert!(a.parts.last().unwrap().cells.iter().any(|v|*v!=AIR));
        assert_eq!(a.parts.last().unwrap().layer,1);
        h.get_by_label("Reset preview").click();h.run_steps(3);
        assert_eq!(h.state().workshop.asset.layers.len(),2);
    }

    #[test]
    fn weather_cold_start_is_visible_and_undoable() {
        let mut h=editor();h.get_by_label("☁ Weather").click();h.run_steps(3);
        h.get_by_label("Cold start (-8°C)").click();h.run_steps(3);
        assert_eq!(h.state().sim.world.ambient,-8.);
        assert!(h.state().sim.world.temp.iter().any(|t|*t<0.));
        h.key_press(Key::Escape);h.run_steps(2);
        h.key_press_modifiers(Modifiers::COMMAND,Key::Z);h.run_steps(3);
        assert_eq!(h.state().sim.world.ambient,20.);
    }

    #[test]
    fn studio_asset_card_drag_places_one_group_and_undo_removes_it() {
        let mut h=editor();h.get_by_label("Assets").click();h.run_steps(3);
        let card=h.get_by_label("Place fuel_crate").rect().center();let dest=at(&h,90.,95.);
        let before=h.state().sim.physics.body_count();drag(&mut h,card,dest);
        assert!(h.state().sim.physics.body_count()>before);
        assert!(h.state().studio.placement.is_none());
        assert_eq!(assemblies::names(&h.state().sim).len(),1);
        h.key_press_modifiers(Modifiers::COMMAND,Key::Z);h.run_steps(3);
        assert_eq!(h.state().sim.physics.body_count(),before);
    }
    #[test]
    fn studio_builder_generates_preview_and_layered_placement() {
        let mut h=editor();h.get_by_label("Build").click();h.run_steps(3);
        h.get_by_label("Exterior front + interior behind").click();h.run_steps(2);
        h.get_by_label("Generate preview").click();h.run_steps(3);
        assert_eq!(h.state().studio.generated.as_ref().unwrap().layers.len(),2);
        h.get_by_label("Place generated building").click();h.run_steps(2);
        let dest=at(&h,100.,100.);press(&mut h,dest,true);press(&mut h,dest,false);
        assert_eq!(h.state().sim.layers.len(),1);
        assert_eq!(assemblies::names(&h.state().sim).len(),1);
    }
    #[test]
    fn studio_event_zone_and_neon_placement_are_saved_and_undoable() {
        let mut h=editor();h.get_by_label("Events").click();h.run_steps(3);
        h.state_mut().studio.event_text="Hello; snow 1".into();
        h.get_by_label("Place event zone").click();h.run_steps(2);
        let dest=at(&h,90.,80.);press(&mut h,dest,true);press(&mut h,dest,false);
        assert_eq!(h.state().sim.triggers.len(),1);
        let snapshot=h.state_mut().sim.snapshot();let text=scene::save(&snapshot,&h.state().mats);
        let (_,problems)=scene::load(&text,&h.state().mats,None).into_sim(&h.state().mats);assert!(problems.is_empty(),"{problems:?}");
        h.key_press_modifiers(Modifiers::COMMAND,Key::Z);h.run_steps(3);assert!(h.state().sim.triggers.is_empty());
        h.state_mut().studio.lights.kind=2;h.get_by_label("Lights").click();h.run_steps(3);
        h.get_by_label("Place light in scene").click();h.run_steps(2);
        let dest=at(&h,100.,50.);press(&mut h,dest,true);press(&mut h,dest,false);
        assert_eq!(h.state().sim.lighting.lights.len(),1);let lamp=&h.state().sim.lighting.lights[0];assert!(lamp.on.is_some());
        assert!(h.state().sim.physics.joints().iter().any(|j|j.a==GROUND||j.b==GROUND));
        h.key_press_modifiers(Modifiers::COMMAND,Key::Z);h.run_steps(3);assert!(h.state().sim.lighting.lights.is_empty());
    }
    #[test]
    fn studio_test_workspace_runs_a_copy_and_space_controls_only_the_test() {
        let mut h=editor();let before=h.state().sim.world.mat.clone();
        h.get_by_label("Tests").click();h.run_steps(3);h.get_by_label("Run test from fresh copy").click();h.run_steps(4);
        assert!(h.state().studio.test.is_some());assert!(!h.state().playing);
        h.key_press(Key::Space);h.run_steps(2);assert!(!h.state().studio.test_running);assert!(!h.state().playing);
        assert_eq!(h.state().sim.world.mat,before);
    }

    #[test]
    fn delayed_editor_frame_does_not_replay_an_unbounded_physics_backlog() {
        let mut h=editor();let frame=h.state().sim.world.frame;
        h.state_mut().playing=true;h.state_mut().sim_clock=100.;h.run_steps(1);
        let advanced=h.state().sim.world.frame-frame;
        assert!((1..=4).contains(&advanced),"advanced {advanced}");assert!(h.state().sim_clock<=1.);
    }

}
