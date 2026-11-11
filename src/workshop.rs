//! The asset workshop's editable model and actual simulation are deliberately separate.
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions, Vec2};
use grave_engine::{
    asset::{Asset, Brush, Weld},
    material::{AIR, MatId, Materials},
    render::{Renderer, View},
    sim::Sim,
};
use std::path::Path;
#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Paint(Brush),
    Text,
    Move,
    Pivot,
    Joint,
}
pub struct Workshop {
    pub open: bool,
    pub asset: Asset,
    selected: usize,
    active_layer: usize,
    ghost_layers: bool,
    preview_layer: Option<usize>,
    material: MatId,
    surface_paint: bool,
    paint_color: [u8; 3],
    lettering: String,
    text_scale: usize,
    tool: Tool,
    zoom: f32,
    grid: bool,
    undo: Vec<Asset>,
    redo: Vec<Asset>,
    stroke: Option<((i32, i32), (i32, i32))>,
    name: String,
    part_name: String,
    new_part: String,
    width: usize,
    height: usize,
    filter: String,
    message: String,
    dirty: bool,
    parent: usize,
    child: usize,
    force: f32,
    test: Option<Sim>,
    running: bool,
    clock: f64,
    heat: bool,
    impact_speed: f32,
    renderer: Renderer,
    texture: Option<TextureHandle>,
    test_frame: u32,
}
pub enum Action {
    Place(Asset),
}
impl Workshop {
    pub fn new(mats: &Materials) -> Self {
        let requested = std::env::args()
            .skip_while(|a| a != "--workshop-file")
            .nth(1)
            .unwrap_or_else(|| "assets/workshop/fuel_crate.asset".into());
        let asset = Asset::load_file(Path::new(&requested), mats)
            .unwrap_or_else(|_| Asset::blank("new_asset", 48, 40).unwrap());
        let name = asset.name.clone();
        let (width, height) = (asset.w, asset.h);
        let part_name = asset.parts[0].name.clone();
        let child = if asset.parts.len() > 1 { 1 } else { 0 };
        Self {
            open: false,
            asset,
            selected: 0,
            active_layer: 0,
            ghost_layers: false,
            preview_layer: None,
            material: mats.id("wood").unwrap_or(AIR),
            surface_paint: false,
            paint_color: [195, 35, 30],
            lettering: "TNT".into(),
            text_scale: 1,
            tool: Tool::Paint(Brush::Pencil),
            zoom: 9.,
            grid: true,
            undo: vec![],
            redo: vec![],
            stroke: None,
            name,
            part_name,
            new_part: "part_2".into(),
            width,
            height,
            filter: String::new(),
            message: "Draw materials into named parts. Tests use a separate copy.".into(),
            dirty: false,
            parent: 0,
            child,
            force: 400.,
            test: None,
            running: false,
            clock: 0.,
            heat: false,
            impact_speed: 90.,
            renderer: Renderer::default(),
            texture: None,
            test_frame: 0,
        }
    }
    fn remember(&mut self) {
        self.undo.push(self.asset.clone());
        if self.undo.len() > 60 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    fn changed(&mut self) {
        self.dirty = true;
        self.test = None;
        self.running = false;
        self.clock = 0.;
        self.selected = self.selected.min(self.asset.parts.len() - 1);
    }
    fn sync(&mut self) {
        self.active_layer = self.asset.parts[self.selected.min(self.asset.parts.len() - 1)].layer;
        self.preview_layer = None;
        self.name = self.asset.name.clone();
        self.width = self.asset.w;
        self.height = self.asset.h;
        self.selected = self.selected.min(self.asset.parts.len() - 1);
        self.part_name = self.asset.parts[self.selected].name.clone();
        self.parent = self.parent.min(self.asset.parts.len() - 1);
        self.child = self.child.min(self.asset.parts.len() - 1);
    }
    fn undo(&mut self) {
        if let Some(a) = self.undo.pop() {
            self.redo.push(self.asset.clone());
            self.asset = a;
            self.changed();
            self.sync();
        }
    }
    fn redo(&mut self) {
        if let Some(a) = self.redo.pop() {
            self.undo.push(self.asset.clone());
            self.asset = a;
            self.changed();
            self.sync();
        }
    }
    fn load(&mut self, path: &Path, mats: &Materials) {
        match Asset::load_file(path, mats) {
            Ok(a) => {
                self.remember();
                self.asset = a;
                self.selected = 0;
                self.changed();
                self.sync();
                self.dirty = false;
                self.message = "Loaded asset. Undo can recover your previous draft.".into();
            }
            Err(e) => self.message = e,
        }
    }
    fn rebuild_test(&mut self, mats: &Materials, impact: bool) {
        let mut sim = Sim::empty(240, 180, mats);
        let stone = mats.id("stone").unwrap();
        for y in 160..180 {
            for x in 0..240 {
                sim.world.set(mats, x, y, stone);
            }
        }
        if impact {
            for y in 35..160 {
                for x in 185..191 {
                    sim.world.set(mats, x, y, stone);
                }
            }
        }
        match self.asset.instantiate(
            &mut sim,
            mats,
            (if impact { 85. } else { 120. }) + self.asset.pivot.0 - self.asset.w as f32 / 2.,
            (if self.asset.h > 80 {
                let last = self
                    .asset
                    .parts
                    .iter()
                    .flat_map(|p| p.cells.iter().enumerate())
                    .filter(|(_, m)| **m != AIR)
                    .map(|(i, _)| i / self.asset.w)
                    .max()
                    .unwrap_or(self.asset.h - 1);
                159. - (last + 1) as f32 + self.asset.h as f32 / 2.
            } else {
                65.
            }) + self.asset.pivot.1
                - self.asset.h as f32 / 2.,
        ) {
            Ok(parts) => {
                if impact {
                    for _ in 0..10 {
                        sim.step(mats);
                    }
                    for plane in &mut sim.layers {
                        for id in plane.physics.ids() {
                            plane
                                .physics
                                .set_velocity(id, self.impact_speed * 10., 0., 0.);
                        }
                    }
                    for (_, id) in parts {
                        sim.physics
                            .set_velocity(id, self.impact_speed * 10., 0., 0.);
                    }
                }
                self.message = if impact {
                    "Impact test: real contact forces, fracture and weld loads."
                } else {
                    "Preview ready. The draft is unchanged by tests."
                }
                .into();
            }
            Err(e) => self.message = e,
        }
        self.test = Some(sim);
        self.test_frame = 0;
        self.clock = 0.;
    }
    pub fn ui(&mut self, ui: &mut egui::Ui, mats: &Materials, dt: f64) -> Option<Action> {
        let mut action = None;
        if !ui.ctx().egui_wants_keyboard_input() {
            let undo = ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z));
            let redo = ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Y));
            if undo {
                self.undo();
            }
            if redo {
                self.redo();
            }
        }
        ui.heading("ASSET WORKSHOP");
        ui.horizontal_wrapped(|ui| {
            if ui.button("Back to scene").clicked() {
                self.open = false;
                self.running = false;
            }
            if ui.button("New asset").clicked() {
                self.remember();
                self.asset = Asset::blank("new_asset", 48, 40).unwrap();
                self.selected = 0;
                self.changed();
                self.sync();
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.name)
                    .desired_width(135.)
                    .hint_text("asset name"),
            );
            if ui.button("Save asset").clicked() {
                if grave_engine::asset::valid_name(&self.name) {
                    self.remember();
                    self.asset.name = self.name.clone();
                    let path = Path::new("assets/workshop").join(format!("{}.asset", self.name));
                    match self.asset.save(&path, mats) {
                        Ok(()) => {
                            self.dirty = false;
                            self.message = format!("Saved {}", path.display());
                        }
                        Err(e) => self.message = e,
                    }
                } else {
                    self.message = "Use letters, numbers, - and _ for the asset name.".into();
                }
            }
            if ui.button("Reload from disk").clicked() {
                if grave_engine::asset::valid_name(&self.name) {
                    let path = Path::new("assets/workshop").join(format!("{}.asset", self.name));
                    self.load(&path, mats);
                }
            }
            if ui
                .add_enabled(!self.undo.is_empty(), egui::Button::new("Undo"))
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled(!self.redo.is_empty(), egui::Button::new("Redo"))
                .clicked()
            {
                self.redo();
            }
            if ui.button("Place at scene centre").clicked() {
                action = Some(Action::Place(self.asset.clone()));
            }
            if self.dirty {
                ui.label(
                    egui::RichText::new("Unsaved draft").color(Color32::from_rgb(230, 180, 75)),
                );
            }
        });
        ui.label(egui::RichText::new(&self.message).color(Color32::from_rgb(100, 200, 220)));
        ui.separator();
        let h = ui.available_height().max(350.);
        let width = ui.available_width();
        let side = 215.;
        let preview = (width * 0.30).clamp(280., 430.);
        let canvas = (width - side - preview - 35.).max(230.);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(side, h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("asset_palette")
                        .max_height(h - 5.)
                        .show(ui, |ui| self.sidebar(ui, mats));
                },
            );
            ui.separator();
            ui.allocate_ui_with_layout(
                Vec2::new(canvas, h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.canvas(ui, mats, canvas, h),
            );
            ui.separator();
            ui.allocate_ui_with_layout(
                Vec2::new(preview, h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.preview(ui, mats, dt, preview),
            );
        });
        ui.ctx().request_repaint();
        action
    }
    fn sidebar(&mut self, ui: &mut egui::Ui, mats: &Materials) {
        ui.strong("Asset library");
        ui.menu_button("Open saved asset...", |ui| {
            let mut files: Vec<_> = std::fs::read_dir("assets/workshop")
                .map(|rd| {
                    rd.filter_map(|e| e.ok().map(|e| e.path()))
                        .filter(|p| p.extension().is_some_and(|e| e == "asset"))
                        .collect()
                })
                .unwrap_or_default();
            files.sort();
            for path in files {
                if ui
                    .button(path.file_stem().unwrap().to_string_lossy())
                    .clicked()
                {
                    self.load(&path, mats);
                    ui.close();
                }
            }
        });
        ui.separator();
        ui.strong("Grid and thickness");
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut self.width)
                    .range(1..=128)
                    .prefix("W "),
            );
            ui.add(
                egui::DragValue::new(&mut self.height)
                    .range(1..=128)
                    .prefix("H "),
            );
        });
        if ui.button("Resize grid").clicked() {
            self.remember();
            match self.asset.resize(self.width, self.height) {
                Ok(()) => self.changed(),
                Err(e) => self.message = e,
            }
        }
        let mut depth = self.asset.depth;
        if ui
            .add(
                egui::DragValue::new(&mut depth)
                    .range(0.001..=10.)
                    .speed(0.005)
                    .suffix(" m deep"),
            )
            .changed()
        {
            self.remember();
            self.asset.depth = depth;
            self.changed();
        }
        ui.separator();
        ui.strong("Depth layers — front to back");
        let previous = self.active_layer;
        egui::ComboBox::from_id_salt("asset_depth_layer")
            .selected_text(&self.asset.layers[self.active_layer])
            .show_ui(ui, |ui| {
                for (z, name) in self.asset.layers.iter().enumerate() {
                    ui.selectable_value(&mut self.active_layer, z, name);
                }
            });
        if self.active_layer != previous {
            if let Some(i) = self
                .asset
                .parts
                .iter()
                .position(|p| p.layer == self.active_layer)
            {
                self.selected = i;
                self.part_name = self.asset.parts[i].name.clone();
            } else if self.asset.parts.len() < 24 {
                self.remember();
                let name = format!(
                    "layer_{}_part_{}",
                    self.active_layer,
                    self.asset.parts.len()
                );
                if let Ok(i) = self.asset.add_part(&name) {
                    self.asset.parts[i].layer = self.active_layer;
                    self.selected = i;
                    self.part_name = name;
                    self.changed();
                }
            } else {
                self.active_layer = previous;
            }
        }
        if ui
            .add_enabled(
                self.asset.layers.len() < 4 && self.asset.parts.len() < 24,
                egui::Button::new("Add depth layer"),
            )
            .clicked()
        {
            self.remember();
            let z = self.asset.layers.len();
            let name = format!("interior_{z}");
            match self.asset.add_layer(&name).and_then(|z| {
                self.asset
                    .add_part(&format!("layer_{z}_part"))
                    .map(|i| (z, i))
            }) {
                Ok((z, i)) => {
                    self.asset.parts[i].layer = z;
                    self.selected = i;
                    self.active_layer = z;
                    self.part_name = self.asset.parts[i].name.clone();
                    self.changed();
                }
                Err(e) => self.message = e,
            }
        }
        ui.checkbox(&mut self.ghost_layers, "Ghost other layers on grid");
        ui.label("Each layer has its own material cells. Heat crosses neighbouring layers. Weld within a layer.");
        ui.separator();
        ui.strong("Named parts");
        for i in 0..self.asset.parts.len() {
            if self.asset.parts[i].layer != self.active_layer {
                continue;
            }
            let count = self.asset.parts[i]
                .cells
                .iter()
                .filter(|m| **m != AIR)
                .count();
            if ui
                .selectable_label(
                    i == self.selected,
                    format!("{}  ({count})", self.asset.parts[i].name),
                )
                .clicked()
            {
                self.selected = i;
                self.part_name = self.asset.parts[i].name.clone();
            }
        }
        ui.add(
            egui::TextEdit::singleline(&mut self.part_name)
                .hint_text("selected part name")
                .desired_width(175.),
        );
        if ui.button("Rename part").clicked() {
            self.remember();
            match self.asset.rename_part(self.selected, &self.part_name) {
                Ok(()) => self.changed(),
                Err(e) => self.message = e,
            }
        }
        ui.add(
            egui::TextEdit::singleline(&mut self.new_part)
                .hint_text("new part name")
                .desired_width(175.),
        );
        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.asset.parts.len() < 24, egui::Button::new("Add part"))
                .clicked()
            {
                self.remember();
                match self.asset.add_part(&self.new_part) {
                    Ok(i) => {
                        self.asset.parts[i].layer = self.active_layer;
                        self.selected = i;
                        self.changed();
                        self.sync();
                        self.new_part = format!("part_{}", self.asset.parts.len() + 1);
                    }
                    Err(e) => self.message = e,
                }
            }
            if ui
                .add_enabled(self.asset.parts.len() > 1, egui::Button::new("Remove part"))
                .clicked()
            {
                self.remember();
                self.asset.remove_part(self.selected);
                self.selected = 0;
                self.changed();
                self.sync();
            }
        });
        ui.separator();
        ui.strong("Draw with");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.surface_paint, false, "Material");
            ui.selectable_value(&mut self.surface_paint, true, "Surface paint");
        });
        if self.surface_paint {
            ui.horizontal(|ui| {
                ui.label("Colour");
                ui.color_edit_button_srgb(&mut self.paint_color)
                    .on_hover_text("Choose surface paint colour");
            });
            ui.horizontal(|ui| {
                if ui.button("Red").clicked() {
                    self.paint_color = [195, 35, 30];
                }
                if ui.button("Black").clicked() {
                    self.paint_color = [8, 8, 8];
                }
                if ui.button("White").clicked() {
                    self.paint_color = [235, 235, 225];
                }
            });
            ui.label("Paint changes appearance only. Erase removes paint; material stays.");
            ui.text_edit_singleline(&mut self.lettering);
            ui.add(egui::Slider::new(&mut self.text_scale, 1..=4).text("Letter size"));
            if ui
                .selectable_label(self.tool == Tool::Text, "Stamp lettering")
                .clicked()
            {
                self.tool = Tool::Text;
            }
            ui.label("Choose black, then click the grid for TNT lettering (A–Z, 0–9).");
        }
        ui.strong("Physical material");
        ui.add(
            egui::TextEdit::singleline(&mut self.filter)
                .hint_text("filter materials")
                .desired_width(175.),
        );
        egui::ScrollArea::vertical()
            .id_salt("asset_materials")
            .max_height(190.)
            .show(ui, |ui| {
                for (i, m) in mats.list.iter().enumerate() {
                    if i == 0 || !m.name.contains(&self.filter.to_lowercase()) {
                        continue;
                    }
                    let c = Color32::from_rgb(m.color[0], m.color[1], m.color[2]);
                    if ui
                        .selectable_label(
                            self.material == i as MatId,
                            egui::RichText::new(&m.name).color(c),
                        )
                        .clicked()
                    {
                        self.material = i as MatId;
                        self.surface_paint = false;
                        if self.tool == Tool::Text {
                            self.tool = Tool::Paint(Brush::Pencil);
                        }
                    }
                }
            });
        ui.separator();
        ui.strong("Weld attachments");
        egui::ComboBox::from_id_salt("asset_parent")
            .selected_text(format!("Parent: {}", self.asset.parts[self.parent].name))
            .show_ui(ui, |ui| {
                for (i, p) in self.asset.parts.iter().enumerate() {
                    ui.selectable_value(&mut self.parent, i, &p.name);
                }
            });
        egui::ComboBox::from_id_salt("asset_child")
            .selected_text(format!("Child: {}", self.asset.parts[self.child].name))
            .show_ui(ui, |ui| {
                for (i, p) in self.asset.parts.iter().enumerate() {
                    ui.selectable_value(&mut self.child, i, &p.name);
                }
            });
        ui.add(
            egui::DragValue::new(&mut self.force)
                .range(0.0..=100000.0)
                .speed(10.)
                .suffix(" kN to break"),
        );
        ui.label(
            egui::RichText::new("0 = unbreakable attachment")
                .small()
                .weak(),
        );
        if ui
            .add_enabled(
                self.parent != self.child,
                egui::Button::new("Click weld anchor on grid"),
            )
            .clicked()
        {
            self.tool = Tool::Joint;
        }
        let mut delete = None;
        let mut edit_force = None;
        for (i, j) in self.asset.welds.iter().enumerate() {
            ui.label(format!("{} → {}", j.a, j.b));
            ui.horizontal(|ui| {
                let mut f = j.break_force.unwrap_or(0.);
                if ui
                    .add(
                        egui::DragValue::new(&mut f)
                            .range(0.0..=100000.0)
                            .speed(5.)
                            .suffix(" kN"),
                    )
                    .changed()
                {
                    edit_force = Some((i, f));
                }
                if ui.small_button("×").clicked() {
                    delete = Some(i);
                }
            });
        }
        if let Some((i, f)) = edit_force {
            self.remember();
            self.asset.welds[i].break_force = if f == 0. { None } else { Some(f) };
            self.changed();
        }
        if let Some(i) = delete {
            self.remember();
            self.asset.welds.remove(i);
            self.changed();
        }
        if ui.button("Weld all nonempty parts").clicked() {
            self.remember();
            let names: std::collections::BTreeSet<_> = self
                .asset
                .parts
                .iter()
                .filter(|p| p.layer == self.active_layer)
                .map(|p| p.name.clone())
                .collect();
            self.asset
                .welds
                .retain(|j| !names.contains(&j.a) && !names.contains(&j.b));
            let indices: Vec<_> = self
                .asset
                .parts
                .iter()
                .enumerate()
                .filter(|(_, p)| p.layer == self.active_layer && p.cells.iter().any(|m| *m != AIR))
                .map(|(i, _)| i)
                .collect();
            if let Some(root) = indices.first() {
                for &i in indices.iter().skip(1) {
                    let p = &self.asset.parts[i];
                    let cells: Vec<_> = p
                        .cells
                        .iter()
                        .enumerate()
                        .filter(|(_, m)| **m != AIR)
                        .map(|(k, _)| {
                            (
                                (k % self.asset.w) as f32 + 0.5,
                                (k / self.asset.w) as f32 + 0.5,
                            )
                        })
                        .collect();
                    let n = cells.len() as f32;
                    self.asset.welds.push(Weld {
                        a: self.asset.parts[*root].name.clone(),
                        b: p.name.clone(),
                        x: cells.iter().map(|p| p.0).sum::<f32>() / n,
                        y: cells.iter().map(|p| p.1).sum::<f32>() / n,
                        break_force: if self.force == 0. {
                            None
                        } else {
                            Some(self.force)
                        },
                    });
                }
            }
            self.changed();
        }
    }
    fn canvas(&mut self, ui: &mut egui::Ui, mats: &Materials, width: f32, h: f32) {
        ui.strong(format!(
            "Grid — editing {}",
            self.asset.parts[self.selected].name
        ));
        ui.horizontal_wrapped(|ui| {
            for (tool, label) in [
                (Tool::Paint(Brush::Pencil), "Pencil"),
                (Tool::Paint(Brush::Erase), "Erase"),
                (Tool::Paint(Brush::Fill), "Fill"),
                (Tool::Paint(Brush::Line), "Line"),
                (Tool::Paint(Brush::Rectangle), "Rectangle"),
                (Tool::Paint(Brush::Circle), "Ellipse"),
                (Tool::Move, "Move part"),
                (Tool::Pivot, "Set pivot"),
            ] {
                ui.selectable_value(&mut self.tool, tool, label);
            }
        });
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut self.zoom, 3.0..=22.0).text("Zoom"));
            ui.checkbox(&mut self.grid, "Grid");
        });
        ui.label(format!(
            "{} × {} cells / {:.1} × {:.1} m   |   {}",
            self.asset.w,
            self.asset.h,
            self.asset.w as f32 * 0.1,
            self.asset.h as f32 * 0.1,
            if self.surface_paint {
                "Surface paint: physical materials stay unchanged"
            } else {
                &mats.get(self.material).name
            }
        ));
        let mut finish = false;
        egui::ScrollArea::both()
            .id_salt("asset_canvas")
            .max_width(width)
            .max_height((h - 155.).max(160.))
            .show(ui, |ui| {
                let size = Vec2::new(
                    self.asset.w as f32 * self.zoom,
                    self.asset.h as f32 * self.zoom,
                );
                let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Image,
                        true,
                        "Material asset drawing grid",
                    )
                });
                let painter = ui.painter();
                painter.rect_filled(rect, 0., Color32::from_rgb(19, 24, 34));
                for y in 0..self.asset.h {
                    for x in 0..self.asset.w {
                        let i = y * self.asset.w + x;
                        let selected = self.asset.parts[self.selected].cells[i];
                        let owner = if selected != AIR {
                            Some(&self.asset.parts[self.selected])
                        } else {
                            self.asset
                                .parts
                                .iter()
                                .find(|p| p.layer == self.active_layer && p.cells[i] != AIR)
                                .or_else(|| {
                                    if self.ghost_layers {
                                        self.asset.parts.iter().find(|p| p.cells[i] != AIR)
                                    } else {
                                        None
                                    }
                                })
                        };
                        if let Some(owner) = owner {
                            let c = grave_engine::asset::paint_rgb(owner.paint[i])
                                .unwrap_or(mats.get(owner.cells[i]).color);
                            let color = Color32::from_rgb(c[0], c[1], c[2]);
                            painter.rect_filled(
                                Rect::from_min_size(
                                    rect.min
                                        + Vec2::new(x as f32 * self.zoom, y as f32 * self.zoom),
                                    Vec2::splat(self.zoom),
                                ),
                                0.,
                                if selected != AIR {
                                    color
                                } else {
                                    color.gamma_multiply(0.55)
                                },
                            );
                        }
                    }
                }
                if self.grid && self.zoom >= 5. {
                    for x in 0..=self.asset.w {
                        let xx = rect.left() + x as f32 * self.zoom;
                        painter.line_segment(
                            [Pos2::new(xx, rect.top()), Pos2::new(xx, rect.bottom())],
                            Stroke::new(0.5, Color32::from_rgba_unmultiplied(100, 120, 150, 65)),
                        );
                    }
                    for y in 0..=self.asset.h {
                        let yy = rect.top() + y as f32 * self.zoom;
                        painter.line_segment(
                            [Pos2::new(rect.left(), yy), Pos2::new(rect.right(), yy)],
                            Stroke::new(0.5, Color32::from_rgba_unmultiplied(100, 120, 150, 65)),
                        );
                    }
                }
                let zoom = self.zoom;
                let pt = |x: f32, y: f32| rect.min + Vec2::new(x * zoom, y * zoom);
                let pivot = pt(self.asset.pivot.0, self.asset.pivot.1);
                painter.line_segment(
                    [pivot - Vec2::new(6., 0.), pivot + Vec2::new(6., 0.)],
                    Stroke::new(2., Color32::WHITE),
                );
                painter.line_segment(
                    [pivot - Vec2::new(0., 6.), pivot + Vec2::new(0., 6.)],
                    Stroke::new(2., Color32::WHITE),
                );
                for j in &self.asset.welds {
                    painter.circle_stroke(
                        pt(j.x, j.y),
                        4.,
                        Stroke::new(1.5, Color32::from_rgb(255, 190, 65)),
                    );
                }
                let pointer = response
                    .interact_pointer_pos()
                    .or_else(|| response.hover_pos());
                let cell = pointer
                    .map(|p| {
                        (
                            ((p.x - rect.left()) / self.zoom).floor() as i32,
                            ((p.y - rect.top()) / self.zoom).floor() as i32,
                        )
                    })
                    .map(|(x, y)| {
                        (
                            x.clamp(0, self.asset.w as i32 - 1),
                            y.clamp(0, self.asset.h as i32 - 1),
                        )
                    });
                if response.drag_started() {
                    if let Some(c) = cell {
                        self.remember();
                        self.stroke = Some((c, c));
                    }
                }
                if response.dragged() {
                    if let (Some((a, last)), Some(c)) = (self.stroke, cell) {
                        match self.tool {
                            Tool::Paint(Brush::Pencil) | Tool::Paint(Brush::Erase) => {
                                let Tool::Paint(b) = self.tool else {
                                    unreachable!()
                                };
                                self.draw(b, last, c);
                            }
                            _ => {}
                        }
                        self.stroke = Some((a, c));
                    }
                }
                if let Some((a, b)) = self.stroke {
                    let r = Rect::from_two_pos(
                        pt(a.0 as f32, a.1 as f32),
                        pt(b.0 as f32 + 1., b.1 as f32 + 1.),
                    );
                    painter.rect_stroke(
                        r,
                        0.,
                        Stroke::new(1.5, Color32::from_rgb(250, 205, 80)),
                        egui::StrokeKind::Inside,
                    );
                }
                if response.clicked() {
                    if let Some(c) = cell {
                        self.remember();
                        self.apply(c, c);
                        finish = true;
                    }
                }
                if response.drag_stopped()
                    || self.stroke.is_some() && !ui.input(|i| i.pointer.primary_down())
                {
                    if let Some((a, b)) = self.stroke.take() {
                        if !matches!(
                            self.tool,
                            Tool::Paint(Brush::Pencil) | Tool::Paint(Brush::Erase)
                        ) {
                            self.apply(a, b);
                        }
                        finish = true;
                    }
                }
            });
        if finish {
            self.changed();
        }
        ui.label(egui::RichText::new("Bright cells = selected part; dim cells = other parts. Material drawing transfers cell ownership; surface paint only colours existing cells. White cross = placement pivot; amber circles = weld anchors.").small());
    }
    fn draw(&mut self, brush: Brush, a: (i32, i32), b: (i32, i32)) {
        if self.surface_paint {
            self.asset
                .paint_surface(self.selected, brush, a, b, Some(self.paint_color));
        } else {
            self.asset.draw(self.selected, brush, a, b, self.material);
        }
    }
    fn apply(&mut self, a: (i32, i32), b: (i32, i32)) {
        match self.tool {
            Tool::Paint(brush) => self.draw(brush, a, b),
            Tool::Text => self.asset.paint_text(
                self.selected,
                b,
                &self.lettering,
                self.text_scale,
                self.paint_color,
            ),
            Tool::Move => self.asset.move_part(self.selected, b.0 - a.0, b.1 - a.1),
            Tool::Pivot => self.asset.pivot = (b.0 as f32 + 0.5, b.1 as f32 + 0.5),
            Tool::Joint => {
                if self.parent != self.child
                    && self.asset.parts[self.parent].layer == self.asset.parts[self.child].layer
                {
                    self.asset.welds.push(Weld {
                        a: self.asset.parts[self.parent].name.clone(),
                        b: self.asset.parts[self.child].name.clone(),
                        x: b.0 as f32 + 0.5,
                        y: b.1 as f32 + 0.5,
                        break_force: if self.force == 0. {
                            None
                        } else {
                            Some(self.force)
                        },
                    });
                    self.tool = Tool::Paint(Brush::Pencil);
                }
            }
        }
    }
    fn preview(&mut self, ui: &mut egui::Ui, mats: &Materials, dt: f64, width: f32) {
        ui.strong("Real material preview / test bay");
        if self.asset.layers.len() > 1 {
            egui::ComboBox::from_id_salt("asset_preview_layer")
                .selected_text(
                    self.preview_layer
                        .map(|z| self.asset.layers[z].as_str())
                        .unwrap_or("All layers — outside view"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.preview_layer, None, "All layers — outside view");
                    for (z, name) in self.asset.layers.iter().enumerate() {
                        ui.selectable_value(&mut self.preview_layer, Some(z), name);
                    }
                });
        }
        ui.label(egui::RichText::new("Tests use a copy. Your draft stays safe.").small());
        ui.horizontal_wrapped(|ui| {
            if ui.button("Drop").clicked() {
                self.rebuild_test(mats, false);
                self.running = true;
            }
            if ui.button("Impact wall").clicked() {
                self.rebuild_test(mats, true);
                self.running = true;
            }
            if ui.button("Burn").clicked() {
                self.rebuild_test(mats, false);
                if let Some(s) = &mut self.test {
                    let target = if let Some(z) = self.preview_layer {
                        if z == 0 {
                            &mut s.world
                        } else {
                            &mut s.layers[z - 1].world
                        }
                    } else {
                        &mut s.world
                    };
                    target.heat_circle(
                        120,
                        65,
                        (self.asset.w.max(self.asset.h) as i32 / 2 + 5).min(95),
                        800.,
                    );
                }
                self.running = true;
            }
            if ui.button("Reset preview").clicked() {
                self.rebuild_test(mats, false);
                self.running = false;
            }
        });
        ui.horizontal(|ui| {
            ui.toggle_value(&mut self.running, "Run / pause");
            if ui.button("Step").clicked() {
                if self.test.is_none() {
                    self.rebuild_test(mats, false);
                }
                if let Some(s) = &mut self.test {
                    s.step(mats);
                    self.test_frame += 1;
                }
                self.running = false;
            }
            ui.checkbox(&mut self.heat, "Heat view");
        });
        ui.add(egui::Slider::new(&mut self.impact_speed, 5.0..=120.0).text("Impact m/s"));
        if self.test.is_none() {
            self.rebuild_test(mats, false);
        }
        if self.running {
            self.clock += (dt.min(0.1)) * 60.;
            let mut n = 0;
            while self.clock >= 1. && n < 6 {
                if let Some(s) = &mut self.test {
                    s.step(mats);
                    self.test_frame += 1;
                }
                self.clock -= 1.;
                n += 1;
            }
        }
        let size = Vec2::new(width - 10., (width - 10.) * 180. / 240.);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        if let Some(s) = &mut self.test {
            self.renderer.light(s);
            if let Some(z) = self.preview_layer {
                self.renderer
                    .light(if z == 0 { s } else { &s.layers[z - 1] });
            }
            let world = if let Some(z) = self.preview_layer {
                if z == 0 {
                    &s.world
                } else {
                    &s.layers[z - 1].world
                }
            } else {
                &s.world
            };
            let rgba = self.renderer.draw(world, mats, View { heat: self.heat });
            let img = egui::ColorImage::from_rgba_unmultiplied([240, 180], rgba);
            if let Some(t) = &mut self.texture {
                t.set(img, TextureOptions::NEAREST);
            } else {
                self.texture = Some(ui.ctx().load_texture(
                    "asset_test",
                    img,
                    TextureOptions::NEAREST,
                ));
            }
            ui.painter().image(
                self.texture.as_ref().unwrap().id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                Color32::WHITE,
            );
            if response.clicked() {
                if let Some(p) = response.interact_pointer_pos() {
                    let x = ((p.x - rect.left()) / rect.width() * 240.) as i32;
                    let y = ((p.y - rect.top()) / rect.height() * 180.) as i32;
                    let target = if let Some(z) = self.preview_layer {
                        if z == 0 {
                            &mut s.world
                        } else {
                            &mut s.layers[z - 1].world
                        }
                    } else {
                        &mut s.world
                    };
                    target.heat_circle(x, y, 5, 900.);
                    self.running = true;
                }
            }
            ui.label(format!(
                "{:.1}s   {} bodies   {} welds",
                self.test_frame as f32 / 60.,
                s.physics.body_count(),
                s.physics.joints().len()
            ));
        }
        ui.label(egui::RichText::new("Click the preview to apply a small heat torch.").small());
        ui.separator();
        ui.strong("Checks");
        let checks = self.asset.validate(mats);
        if checks.is_empty() {
            ui.label(
                egui::RichText::new("No empty, overlapping or disconnected parts found.")
                    .color(Color32::from_rgb(100, 205, 130)),
            );
        }
        for check in checks.iter().take(12) {
            ui.label(
                egui::RichText::new(check)
                    .color(Color32::from_rgb(235, 185, 90))
                    .small(),
            );
        }
        let composite = self.asset.composite();
        let count = composite.iter().filter(|m| **m != AIR).count();
        let mass: f32 = composite
            .iter()
            .filter(|m| **m != AIR)
            .map(|m| mats.get(*m).density * 0.01 * self.asset.depth)
            .sum();
        ui.label(format!(
            "{count} material cells / estimated mass {mass:.1} kg"
        ));
        ui.separator();
        ui.label(egui::RichText::new("AI can edit assets/workshop/*.asset with named parts, material legends, grids, rectangles, lines and circles. Reload from disk to inspect its changes.").small());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn impact_test_detaches_breakable_sample_without_changing_draft() {
        let mats = Materials::load_dir("materials");
        let mut w = Workshop::new(&mats);
        let draft = w.asset.clone();
        w.rebuild_test(&mats, true);
        for _ in 0..60 {
            w.test.as_mut().unwrap().step(&mats);
        }
        let s = w.test.as_ref().unwrap();
        assert!(s.physics.joints().len() < 2 || s.physics.body_count() != 3);
        assert_eq!(w.asset, draft);
    }
    #[test]
    fn changing_the_pivot_does_not_push_the_preview_outside_its_bay() {
        let mats = Materials::load_dir("materials");
        let mut w = Workshop::new(&mats);
        w.asset.pivot = (0., 0.);
        w.rebuild_test(&mats, false);
        let s = w.test.as_ref().unwrap();
        for id in s.physics.ids() {
            let b = s.physics.spec_of(&s.world, id).unwrap();
            assert!(b.x > 40. && b.x < 200. && b.y > 10. && b.y < 130.);
        }
    }
}
