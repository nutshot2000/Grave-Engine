//! Editor workspace state. No simulation or file changes happen merely by
//! opening a panel; canvas placements and inspector edits are undoable actions.
use eframe::egui::{self, TextureHandle};
use grave_engine::{Materials, asset::Asset, building::Building, test_bay::Recipe};
use std::path::PathBuf;
#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Assets,
    Materials,
    Build,
    Events,
    Lights,
    Tests,
}
impl Tab {
    pub const ALL: [(Self, &'static str); 6] = [
        (Self::Assets, "Assets"),
        (Self::Materials, "Materials"),
        (Self::Build, "Build"),
        (Self::Events, "Events"),
        (Self::Lights, "Lights"),
        (Self::Tests, "Tests"),
    ];
}
pub struct Entry {
    pub path: PathBuf,
    pub asset: Asset,
    pub texture: Option<TextureHandle>,
}
#[derive(Clone)]
pub enum Placement {
    Asset(PathBuf),
    Generated(Asset),
    Event,
    EventTarget,
    Light,
}
#[derive(Clone)]
pub struct LightSettings {
    pub kind: usize,
    pub color: [u8; 3],
    pub radius: f32,
    pub strength: f32,
    pub angle: f32,
    pub spread: f32,
    pub shadows: bool,
    pub flicker: f32,
    pub width: usize,
    pub attach: bool,
}
impl Default for LightSettings {
    fn default() -> Self {
        Self {
            kind: 0,
            color: [255, 204, 140],
            radius: 45.,
            strength: 1.2,
            angle: 90.,
            spread: 40.,
            shadows: true,
            flicker: 0.,
            width: 24,
            attach: false,
        }
    }
}
pub struct Studio {
    pub tab: Tab,
    pub entries: Vec<Entry>,
    pub needs_refresh: bool,
    pub query: String,
    pub material_filter: String,
    pub category: usize,
    pub placement: Option<Placement>,
    pub dragging_asset: bool,
    pub repeat: bool,
    pub snap: bool,
    pub grid: f32,
    pub whole_assets: bool,
    pub builder: Building,
    pub generated: Option<Asset>,
    pub lights: LightSettings,
    pub light_selected: Option<usize>,
    pub event_kind: usize,
    pub event_by: usize,
    pub event_repeat: bool,
    pub event_text: String,
    pub event_size: (f32, f32),
    pub event_target: (f32, f32),
    pub event_selected: Option<usize>,
    pub test: Option<grave_engine::Sim>,
    pub test_renderer: grave_engine::render::Renderer,
    pub test_texture: Option<TextureHandle>,
    pub test_frame: u32,
    pub test_running: bool,
    pub test_clock: f64,
    pub recipe: Recipe,
    pub recipe_asset: usize,
    pub saved_tests: Vec<PathBuf>,
    pub status: String,
    pub inspector_open: bool,
}
impl Default for Studio {
    fn default() -> Self {
        Self {
            tab: Tab::Assets,
            entries: Vec::new(),
            needs_refresh: true,
            query: String::new(),
            material_filter: String::new(),
            category: 0,
            placement: None,
            dragging_asset: false,
            repeat: true,
            snap: true,
            grid: 2.,
            whole_assets: true,
            builder: Building::default(),
            generated: None,
            lights: LightSettings::default(),
            light_selected: None,
            event_kind: 0,
            event_by: 0,
            event_repeat: false,
            event_text: "Welcome!".into(),
            event_size: (20., 20.),
            event_target: (100., 100.),
            event_selected: None,
            test: None,
            test_renderer: Default::default(),
            test_texture: None,
            test_frame: 0,
            test_running: false,
            test_clock: 0.,
            recipe: Recipe::default(),
            recipe_asset: 0,
            saved_tests: Vec::new(),
            status: String::new(),
            inspector_open: true,
        }
    }
}
impl Studio {
    pub fn refresh(&mut self, m: &Materials) {
        self.entries.clear();
        let mut paths: Vec<_> = std::fs::read_dir("assets/workshop")
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "asset"))
            .collect();
        paths.sort();
        for path in paths {
            match Asset::load_file(&path, m) {
                Ok(asset) => self.entries.push(Entry {
                    path,
                    asset,
                    texture: None,
                }),
                Err(e) => self.status = format!("{}: {e}", path.display()),
            }
        }
        self.saved_tests = std::fs::read_dir("assets/tests")
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "test"))
            .collect();
        self.saved_tests.sort();
        self.needs_refresh = false;
    }
    pub fn snapped(&self, p: (f32, f32)) -> (f32, f32) {
        if self.snap {
            (
                (p.0 / self.grid).round() * self.grid,
                (p.1 / self.grid).round() * self.grid,
            )
        } else {
            p
        }
    }
}
pub fn thumbnail(a: &Asset, m: &Materials) -> egui::ColorImage {
    let mut rgba = vec![0u8; a.w * a.h * 4];
    for i in 0..a.w * a.h {
        if let Some(p) = a
            .parts
            .iter()
            .filter(|p| p.cells[i] != 0)
            .min_by_key(|p| p.layer)
        {
            let c = grave_engine::asset::paint_rgb(p.paint[i]).unwrap_or(m.get(p.cells[i]).color);
            rgba[i * 4..i * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    egui::ColorImage::from_rgba_unmultiplied([a.w, a.h], &rgba)
}
pub fn theme(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.set_visuals(egui::Visuals::dark());
    ctx.global_style_mut(|s| {
        s.visuals.panel_fill = egui::Color32::from_rgb(23, 29, 38);
        s.visuals.window_fill = egui::Color32::from_rgb(27, 34, 44);
        s.visuals.extreme_bg_color = egui::Color32::from_rgb(15, 20, 28);
        s.visuals.selection.bg_fill = egui::Color32::from_rgb(37, 103, 112);
        s.visuals.selection.stroke = egui::Stroke::new(1., egui::Color32::from_rgb(126, 228, 218));
        s.spacing.button_padding = egui::vec2(9., 6.);
        s.spacing.item_spacing = egui::vec2(7., 6.);
        for (kind, font) in s.text_styles.iter_mut() {
            if matches!(kind, egui::TextStyle::Body | egui::TextStyle::Button) {
                font.size = 14.;
            }
        }
    });
}
