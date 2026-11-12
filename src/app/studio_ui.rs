//! The editor's authoring workspaces and direct canvas placement.
use super::*;
use crate::studio::{Placement, Tab};
use grave_engine::{
    assemblies,
    asset::{Asset, Brush},
    light::Light,
    test_bay::{Recipe, TestKind},
    trigger::{Action, By, Trigger},
};

impl Sandbox {
    pub(super) fn workspace(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (tab, name) in Tab::ALL {
                if ui
                    .selectable_value(&mut self.studio.tab, tab, name)
                    .clicked()
                {
                    self.studio.placement = None;
                    if tab == Tab::Tests {
                        self.playing = false;
                    }
                    if tab == Tab::Materials {
                        self.tool = Tool::Paint;
                    }
                }
            }
        });
        ui.separator();
    }
    pub(super) fn asset_browser(&mut self, ui: &mut egui::Ui) {
        if self.studio.needs_refresh {
            self.studio.refresh(&self.mats);
        }
        ui.heading("Asset library");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.studio.query)
                    .hint_text("Search props and buildings")
                    .desired_width((ui.available_width() - 50.).max(70.)),
            );
            if ui
                .small_button("↻")
                .on_hover_text("Refresh asset library")
                .clicked()
            {
                self.studio.refresh(&self.mats);
            }
        });
        ui.horizontal(|ui| {
            for (i, name) in ["All", "Buildings", "Props"].iter().enumerate() {
                ui.selectable_value(&mut self.studio.category, i, *name);
            }
        });
        ui.label(
            RichText::new("Drag a card into the scene, or click it then stamp copies.")
                .small()
                .weak(),
        );
        ui.checkbox(&mut self.studio.repeat, "Keep placing copies");
        let indices: Vec<_> = self
            .studio
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.asset
                    .name
                    .to_lowercase()
                    .contains(&self.studio.query.to_lowercase())
                    && (self.studio.category == 0
                        || (e.asset.w > 64) == (self.studio.category == 1))
            })
            .map(|(i, _)| i)
            .collect();
        let card_w = (ui.available_width() - 8.) / 2.;
        egui::ScrollArea::vertical().id_salt("asset_cards").max_height(ui.available_height().max(100.)).show(ui,|ui| {
            for row in indices.chunks(2) {ui.horizontal(|ui| {for &i in row {
                let entry=&mut self.studio.entries[i];
                if entry.texture.is_none(){entry.texture=Some(ui.ctx().load_texture(format!("asset_{}",entry.asset.name),crate::studio::thumbnail(&entry.asset,&self.mats),TextureOptions::NEAREST));}
                let (r,response)=ui.allocate_exact_size(Vec2::new(card_w,117.),Sense::click_and_drag());
                response.widget_info(||egui::WidgetInfo::labeled(egui::Role::Button,true,format!("Place {}",entry.asset.name)));
                ui.painter().rect_filled(r,6.,Color32::from_rgb(34,43,55));
                if response.hovered(){ui.painter().rect_stroke(r,6.,Stroke::new(1.5,Color32::from_rgb(98,207,195)),egui::StrokeKind::Inside);}
                let scale=((card_w-12.)/entry.asset.w as f32).min(78./entry.asset.h as f32);
                let image=Rect::from_center_size(r.center()-Vec2::new(0.,12.),Vec2::new(entry.asset.w as f32*scale,entry.asset.h as f32*scale));
                ui.painter().image(entry.texture.as_ref().unwrap().id(),image,Rect::from_min_max(Pos2::ZERO,Pos2::new(1.,1.)),Color32::WHITE);
                ui.painter().text(Pos2::new(r.center().x,r.bottom()-24.),egui::Align2::CENTER_CENTER,entry.asset.name.replace('_'," "),egui::FontId::proportional(12.),Color32::from_rgb(220,230,236));
                ui.painter().text(Pos2::new(r.center().x,r.bottom()-9.),egui::Align2::CENTER_CENTER,format!("{} parts · {} layers",entry.asset.parts.len(),entry.asset.layers.len()),egui::FontId::proportional(10.),Color32::from_rgb(132,153,169));
                if response.clicked()||response.drag_started(){self.studio.placement=Some(Placement::Asset(entry.path.clone()));self.studio.dragging_asset=response.drag_started();self.tool=Tool::Select;}
                if response.double_clicked(){let path=entry.path.clone();self.workshop.load(&path,&self.mats);self.workshop.open=true;self.studio.placement=None;self.playing=false;}
                response.on_hover_text("Drag or click to place. Double-click to edit its named parts and materials.");
            }});ui.add_space(3.);}
        });
    }
    pub(super) fn builder_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Building designer");
        ui.label("Generate a reusable building from real material parts.");
        let b = &mut self.studio.builder;
        ui.text_edit_singleline(&mut b.name);
        ui.add(egui::Slider::new(&mut b.width, 48..=128).text("Width"));
        ui.add(egui::Slider::new(&mut b.height, 64..=128).text("Height"));
        ui.add(egui::Slider::new(&mut b.storeys, 1..=3).text("Storeys"));
        ui.checkbox(&mut b.facade, "Exterior front + interior behind");
        ui.checkbox(&mut b.timber, "Timber walls (burnable)");
        ui.label("Untick timber for brick walls. Floors, doors and rafters stay wooden; windows are glass.");
        if ui.button("Generate preview").clicked() {
            match self.studio.builder.build(&self.mats) {
                Ok(a) => {
                    self.studio.generated = Some(a);
                    self.studio.status = "Building generated. Save it or place a copy.".into();
                }
                Err(e) => self.studio.status = e,
            }
        }
        if let Some(a) = self.studio.generated.clone() {
            ui.separator();
            ui.label(format!("{} · {} named parts", a.name, a.parts.len()));
            let tex = ui.ctx().load_texture(
                "generated_building",
                crate::studio::thumbnail(&a, &self.mats),
                TextureOptions::NEAREST,
            );
            let scale = (ui.available_width() / a.w as f32).min(200. / a.h as f32);
            ui.add(egui::Image::new((
                tex.id(),
                Vec2::new(a.w as f32 * scale, a.h as f32 * scale),
            )));
            if ui.button("Place generated building").clicked() {
                self.studio.placement = Some(Placement::Generated(a.clone()));
                self.tool = Tool::Select;
            }
            if ui.button("Save building to asset library").clicked() {
                let path = Path::new("assets/workshop").join(format!("{}.asset", a.name));
                match a.save(&path, &self.mats) {
                    Ok(()) => {
                        self.studio.needs_refresh = true;
                        self.studio.status = format!("Saved {}", a.name);
                    }
                    Err(e) => self.studio.status = e,
                }
            }
            ui.label("Doors are breakable panels. Use the Join tool for hinges or motors.");
        }
    }
    pub(super) fn event_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Level events");
        ui.label("Place a zone that reacts when something enters it.");
        egui::ComboBox::from_id_salt("event_when")
            .selected_text(
                [
                    "Any object enters",
                    "Player enters",
                    "Selected object enters",
                ][self.studio.event_by],
            )
            .show_ui(ui, |ui| {
                for (i, name) in [
                    "Any object enters",
                    "Player enters",
                    "Selected object enters",
                ]
                .iter()
                .enumerate()
                {
                    ui.selectable_value(&mut self.studio.event_by, i, *name);
                }
            });
        egui::ComboBox::from_id_salt("event_action")
            .selected_text(
                [
                    "Show message",
                    "Ignite target area",
                    "Detonate a charge",
                    "Start snow",
                    "Change lighting",
                    "Finish level",
                ][self.studio.event_kind],
            )
            .show_ui(ui, |ui| {
                for (i, name) in [
                    "Show message",
                    "Ignite target area",
                    "Detonate a charge",
                    "Start snow",
                    "Change lighting",
                    "Finish level",
                ]
                .iter()
                .enumerate()
                {
                    ui.selectable_value(&mut self.studio.event_kind, i, *name);
                }
            });
        if self.studio.event_kind == 0 {
            ui.text_edit_singleline(&mut self.studio.event_text);
        }
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut self.studio.event_size.0)
                    .range(2.0..=128.)
                    .prefix("Width "),
            );
            ui.add(
                egui::DragValue::new(&mut self.studio.event_size.1)
                    .range(2.0..=128.)
                    .prefix("Height "),
            );
        });
        ui.checkbox(&mut self.studio.event_repeat, "Repeat on re-entry");
        if matches!(self.studio.event_kind, 1 | 2) {
            ui.label(format!(
                "Target {:.0}, {:.0}",
                self.studio.event_target.0, self.studio.event_target.1
            ));
            if ui.button("Pick event target in scene").clicked() {
                self.studio.placement = Some(Placement::EventTarget);
            }
        }
        if ui.button("Place event zone").clicked() {
            self.studio.placement = Some(Placement::Event);
            self.show_triggers = true;
            self.tool = Tool::Select;
        }
        ui.separator();
        let mut remove = None;
        let mut fire = None;
        for (i, t) in self.sim.triggers.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(
                        self.studio.event_selected == Some(i),
                        format!(
                            "Zone {} · {}",
                            i + 1,
                            if t.fired { "fired" } else { "ready" }
                        ),
                    )
                    .clicked()
                {
                    self.studio.event_selected = Some(i);
                }
                if ui
                    .small_button("Try")
                    .on_hover_text("Fire this event now (undoable)")
                    .clicked()
                {
                    fire = Some(i);
                }
                if ui.small_button("×").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            self.change("remove event", |app| {
                app.sim.triggers.remove(i);
            });
        }
        if let Some(i) = fire {
            let t = self.sim.triggers[i].clone();
            self.change("test event", |app| {
                t.fire(
                    &mut app.sim.world,
                    &mut app.sim.events,
                    &mut app.sim.camera,
                    &mut app.sim.lighting,
                    &mut app.sim.backdrop,
                    &mut app.sim.weather,
                    None,
                );
            });
            self.play();
        }
    }
    pub(super) fn lights_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Lighting workspace");
        let s = &mut self.studio.lights;
        egui::ComboBox::from_id_salt("studio_light_type")
            .selected_text(
                [
                    "Point light",
                    "Spotlight",
                    "Neon strip",
                    "Area fill (4 lamps)",
                ][s.kind],
            )
            .show_ui(ui, |ui| {
                for (i, name) in [
                    "Point light",
                    "Spotlight",
                    "Neon strip",
                    "Area fill (4 lamps)",
                ]
                .iter()
                .enumerate()
                {
                    ui.selectable_value(&mut s.kind, i, *name);
                }
            });
        ui.horizontal(|ui| {
            ui.label("Colour");
            ui.color_edit_button_srgb(&mut s.color);
        });
        ui.add(egui::Slider::new(&mut s.radius, 5.0..=160.).text("Range"));
        ui.add(egui::Slider::new(&mut s.strength, 0.1..=4.).text("Brightness"));
        if s.kind == 1 {
            ui.add(egui::Slider::new(&mut s.angle, -180.0..=180.).text("Direction"));
            ui.add(egui::Slider::new(&mut s.spread, 5.0..=120.).text("Cone width"));
        }
        if s.kind >= 2 {
            ui.add(egui::Slider::new(&mut s.width, 4..=100).text("Width"));
        }
        ui.checkbox(&mut s.shadows, "Cast shadows");
        ui.add(egui::Slider::new(&mut s.flicker, 0.0..=1.).text("Flicker"));
        ui.checkbox(&mut s.attach, "Attach to selected part");
        if ui.button("Place light in scene").clicked() {
            self.studio.placement = Some(Placement::Light);
            self.tool = Tool::Select;
        }
        ui.separator();
        ui.label("Existing lamps");
        let mut remove = None;
        let mut select = None;
        for (i, l) in self.sim.lighting.lights.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(
                        self.studio.light_selected == Some(i),
                        format!("Lamp {} · {:.0} cell range", i + 1, l.radius),
                    )
                    .clicked()
                {
                    select = Some(i);
                }
                if ui.small_button("×").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = select {
            let l = &self.sim.lighting.lights[i];
            self.studio.light_selected = Some(i);
            let s = &mut self.studio.lights;
            s.color = l.color;
            s.radius = l.radius;
            s.strength = l.strength;
            s.shadows = l.shadows;
            s.flicker = l.flicker;
            s.kind = usize::from(l.cone.is_some());
            if let Some((a, w)) = l.cone {
                s.angle = a;
                s.spread = w;
            }
        }
        if let Some(i) = remove {
            self.change("remove lamp", |app| {
                app.sim.lighting.lights.remove(i);
            });
            self.studio.light_selected = None;
        }
        if let Some(i) = self
            .studio
            .light_selected
            .filter(|i| *i < self.sim.lighting.lights.len())
        {
            if ui.button("Apply settings to selected lamp").clicked() {
                let s = self.studio.lights.clone();
                self.change("edit lamp", |app| {
                    let l = &mut app.sim.lighting.lights[i];
                    l.color = s.color;
                    l.radius = s.radius;
                    l.strength = s.strength;
                    l.shadows = s.shadows;
                    l.flicker = s.flicker;
                    l.cone = (s.kind == 1).then_some((s.angle, s.spread));
                });
            }
        }
    }
    pub(super) fn tests_panel(&mut self, ui: &mut egui::Ui) {
        if self.studio.needs_refresh {
            self.studio.refresh(&self.mats);
        }
        ui.heading("Destruction lab");
        ui.label(
            "Repeatable tests run a separate asset copy. Your level and source asset stay intact.",
        );
        let n = self.studio.entries.len();
        if n == 0 {
            ui.label("Save an asset first.");
            return;
        }
        self.studio.recipe_asset = self.studio.recipe_asset.min(n - 1);
        egui::ComboBox::from_id_salt("test_asset")
            .selected_text(&self.studio.entries[self.studio.recipe_asset].asset.name)
            .show_ui(ui, |ui| {
                for (i, e) in self.studio.entries.iter().enumerate() {
                    ui.selectable_value(&mut self.studio.recipe_asset, i, &e.asset.name);
                }
            });
        let r = &mut self.studio.recipe;
        ui.text_edit_singleline(&mut r.name);
        egui::ComboBox::from_id_salt("test_kind")
            .selected_text(r.kind.name())
            .show_ui(ui, |ui| {
                for kind in TestKind::ALL {
                    ui.selectable_value(&mut r.kind, kind, kind.name());
                }
            });
        ui.add(egui::Slider::new(&mut r.speed, 5.0..=120.).text("Impact m/s"));
        ui.add(egui::Slider::new(&mut r.heat, 300.0..=2000.).text("Heat °C"));
        ui.add(egui::Slider::new(&mut r.frames, 60..=1800).text("Test frames"));
        if ui.button("Run test from fresh copy").clicked() {
            self.start_studio_test();
        }
        if ui.button("Save test recipe").clicked() {
            let r = &mut self.studio.recipe;
            r.asset = self.studio.entries[self.studio.recipe_asset]
                .asset
                .name
                .clone();
            let text = r.text();
            match Recipe::parse(&text) {
                Ok(r) => {
                    let path = Path::new("assets/tests").join(format!("{}.test", r.name));
                    let result = std::fs::create_dir_all("assets/tests")
                        .and_then(|_| std::fs::write(&path, text));
                    self.studio.status = result
                        .map(|_| "Saved test recipe".into())
                        .unwrap_or_else(|e| e.to_string());
                    self.studio.needs_refresh = true;
                }
                Err(e) => self.studio.status = e,
            }
        }
        ui.separator();
        let mut load = None;
        for path in &self.studio.saved_tests {
            if ui
                .button(path.file_stem().unwrap().to_string_lossy())
                .clicked()
            {
                load = Some(path.clone());
            }
        }
        if let Some(path) = load {
            match std::fs::read_to_string(path)
                .map_err(|e| e.to_string())
                .and_then(|s| Recipe::parse(&s))
            {
                Ok(r) => {
                    if let Some(i) = self
                        .studio
                        .entries
                        .iter()
                        .position(|e| e.asset.name == r.asset)
                    {
                        self.studio.recipe_asset = i;
                    } else {
                        self.studio.status = format!(
                            "Missing asset {}. Restore it in the asset library first.",
                            r.asset
                        );
                        return;
                    }
                    self.studio.recipe = r;
                }
                Err(e) => self.studio.status = e,
            }
        }
    }
    pub(super) fn start_studio_test(&mut self) {
        if let Some(e) = self.studio.entries.get(self.studio.recipe_asset) {
            match self.studio.recipe.start(&e.asset, &self.mats) {
                Ok(s) => {
                    self.studio.test = Some(s);
                    self.studio.test_frame = 0;
                    self.studio.test_running = true;
                    self.studio.test_clock = 0.;
                    self.playing = false;
                }
                Err(e) => self.studio.status = e,
            }
        }
    }
    pub(super) fn studio_test_canvas(&mut self, ui: &mut egui::Ui, dt: f64) {
        ui.horizontal(|ui| {
            ui.heading("Destruction lab");
            if ui.button("Restart test").clicked() {
                self.start_studio_test();
            }
            ui.toggle_value(&mut self.studio.test_running, "Run / pause test");
            ui.checkbox(&mut self.view.heat, "Test heat view");
        });
        if self.studio.test_running {
            self.studio.test_clock += dt.min(0.1) * 60.;
            while self.studio.test_clock >= 1. && self.studio.test_frame < self.studio.recipe.frames
            {
                if let Some(s) = &mut self.studio.test {
                    s.step(&self.mats);
                }
                self.studio.test_frame += 1;
                self.studio.test_clock -= 1.;
            }
            if self.studio.test_frame >= self.studio.recipe.frames {
                self.studio.test_running = false;
            }
        }
        if let Some(s) = &self.studio.test {
            self.studio.test_renderer.light(s);
            let rgba = self
                .studio
                .test_renderer
                .draw(&s.world, &self.mats, self.view);
            let image = egui::ColorImage::from_rgba_unmultiplied([240, 180], rgba);
            if let Some(t) = &mut self.studio.test_texture {
                t.set(image, TextureOptions::NEAREST);
            } else {
                self.studio.test_texture = Some(ui.ctx().load_texture(
                    "studio_test",
                    image,
                    TextureOptions::NEAREST,
                ));
            }
            let scale = (ui.available_width() / 240.)
                .min((ui.available_height() - 35.) / 180.)
                .max(0.1);
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(240. * scale, 180. * scale), Sense::hover());
            ui.painter().image(
                self.studio.test_texture.as_ref().unwrap().id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                Color32::WHITE,
            );
            let bodies = s.physics.body_count()
                + s.layers
                    .iter()
                    .map(|l| l.physics.body_count())
                    .sum::<usize>();
            ui.label(format!(
                "Frame {} / {} · {} bodies · {} depth planes · source unchanged",
                self.studio.test_frame,
                self.studio.recipe.frames,
                bodies,
                s.layers.len() + 1
            ));
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("Choose an asset and a test on the left, then run it.");
            });
        }
    }
    pub(super) fn studio_inspector(&mut self, ui: &mut egui::Ui) {
        ui.heading("Selection");
        ui.checkbox(
            &mut self.studio.whole_assets,
            "Select whole asset across layers",
        );
        let ids = self.selected_ids();
        let groups = assemblies::selected_names(&self.sim, &ids);
        if !groups.is_empty() {
            for name in &groups {
                ui.label(RichText::new(name.replace('_', " ")).strong());
            }
            let count: usize = assemblies::members(&self.sim, &ids, self.studio.whole_assets)
                .iter()
                .map(Vec::len)
                .sum();
            ui.label(format!("{count} parts in all depth planes"));
            ui.horizontal(|ui| {
                if ui.button("Rotate group ↶").clicked() {
                    self.rotate_asset(-15f32.to_radians());
                }
                if ui.button("Rotate group ↷").clicked() {
                    self.rotate_asset(15f32.to_radians());
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Duplicate asset").clicked() {
                    self.duplicate();
                }
                if ui.button("Delete asset").clicked() {
                    self.delete_selection();
                }
            });
            ui.separator();
        }
        if self.studio.whole_assets && !groups.is_empty() {
            ui.collapsing("Selected part properties", |ui| self.inspector(ui));
        } else {
            self.inspector(ui);
        }
        ui.collapsing("Scene notes", |ui| self.notes_panel(ui));
        if !self.studio.status.is_empty() {
            ui.separator();
            ui.label(&self.studio.status);
        }
    }
    fn rotate_asset(&mut self, angle: f32) {
        let ids = self.selected_ids();
        let p = ids.first().and_then(|id| self.sim.physics.centre(*id));
        if let Some(pivot) = p {
            let whole = self.studio.whole_assets;
            self.change("rotate asset", |app| {
                assemblies::transform(&mut app.sim, &app.mats, &ids, whole, (0., 0.), angle, pivot)
            });
        }
    }

    pub(super) fn studio_canvas_input(
        &mut self,
        ui: &mut egui::Ui,
        resp: &egui::Response,
        pointer: Option<Pos2>,
        to_cells: &impl Fn(Pos2) -> (f32, f32),
        to_screen: &impl Fn((f32, f32)) -> Pos2,
    ) -> bool {
        let Some(mode) = self.studio.placement.clone() else {
            return false;
        };
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.studio.placement = None;
            return true;
        }
        let Some(pointer) = pointer else {
            if self.studio.dragging_asset && ui.input(|i| i.pointer.any_released()) {
                self.studio.dragging_asset = false;
                self.studio.placement = None;
            }
            return true;
        };
        let at = self.studio.snapped(to_cells(pointer));
        let painter = ui.painter();
        let colour = Color32::from_rgb(91, 216, 199);
        let asset = match &mode {
            Placement::Asset(path) => self
                .studio
                .entries
                .iter()
                .find(|e| e.path == *path)
                .map(|e| e.asset.clone()),
            Placement::Generated(a) => Some(a.clone()),
            _ => None,
        };
        let mut origin = at;
        if let Some(a) = &asset {
            let last = a
                .parts
                .iter()
                .flat_map(|p| p.cells.iter().enumerate())
                .filter(|(_, m)| **m != AIR)
                .map(|(i, _)| i / a.w)
                .max()
                .unwrap_or(a.h - 1);
            origin = (
                at.0 + a.pivot.0 - a.w as f32 / 2.,
                at.1 + a.pivot.1 - (last + 1) as f32,
            );
            let corner = (origin.0 - a.pivot.0, origin.1 - a.pivot.1);
            let r = Rect::from_two_pos(
                to_screen(corner),
                to_screen((corner.0 + a.w as f32, corner.1 + a.h as f32)),
            );
            let tex = ui.ctx().load_texture(
                "placement_ghost",
                crate::studio::thumbnail(a, &self.mats),
                TextureOptions::NEAREST,
            );
            painter.image(
                tex.id(),
                r,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                Color32::from_white_alpha(160),
            );
            painter.rect_stroke(r, 0., Stroke::new(2., colour), egui::StrokeKind::Inside);
            painter.text(
                r.left_top(),
                egui::Align2::LEFT_BOTTOM,
                a.name.replace('_', " "),
                egui::FontId::proportional(14.),
                colour,
            );
        } else if matches!(mode, Placement::Event) {
            let r = Rect::from_two_pos(
                to_screen(at),
                to_screen((
                    at.0 + self.studio.event_size.0,
                    at.1 + self.studio.event_size.1,
                )),
            );
            painter.rect_stroke(r, 0., Stroke::new(2., colour), egui::StrokeKind::Inside);
        } else {
            let p = to_screen(at);
            painter.circle_stroke(
                p,
                if matches!(mode, Placement::Light) {
                    (to_screen((at.0 + self.studio.lights.radius, at.1)).x - p.x).abs()
                } else {
                    8.
                },
                Stroke::new(1.5, colour),
            );
        }
        let dropped = self.studio.dragging_asset
            && ui.input(|i| i.pointer.any_released())
            && resp.rect.contains(pointer);
        if resp.clicked() || dropped {
            if let Some(a) = asset {
                let mut ids = Vec::new();
                self.change("place asset", |app| {
                    match a.instantiate(&mut app.sim, &app.mats, origin.0, origin.1) {
                        Ok(parts) => ids = parts.into_iter().map(|(_, id)| id).collect(),
                        Err(e) => app.studio.status = e,
                    }
                });
                self.selection = Selection::Objects(ids);
                if !self.studio.repeat || dropped {
                    self.studio.placement = None;
                }
            } else {
                match mode {
                    Placement::Event => self.place_event(at),
                    Placement::EventTarget => {
                        self.studio.event_target = at;
                    }
                    Placement::Light => self.place_studio_light(at),
                    _ => {}
                }
                self.studio.placement = None;
            }
            self.studio.dragging_asset = false;
        }
        true
    }
    fn place_event(&mut self, at: (f32, f32)) {
        let (w, h) = self.studio.event_size;
        let target = self.studio.event_target;
        let kind = self.studio.event_kind;
        let message = self.studio.event_text.replace([';', '\n', '\r'], " ");
        let repeat = self.studio.event_repeat;
        let by = match self.studio.event_by {
            1 => By::Player,
            2 => {
                let Some(id) = self.selected_ids().first().copied() else {
                    self.studio.status = "Select an object first.".into();
                    return;
                };
                By::Object(id)
            }
            _ => By::Any,
        };
        self.change("add level event", |app| {
            if kind == 2 {
                if let Some(powder) = app.mats.id("gunpowder") {
                    let spec = BodySpec::from_mats(
                        5,
                        5,
                        &vec![Some(powder); 25],
                        &app.mats,
                        target.0,
                        target.1,
                    );
                    if let Some(id) = app
                        .sim
                        .physics
                        .add_exact(&mut app.sim.world, &app.mats, spec)
                    {
                        app.sim.physics.join(
                            &app.mats,
                            GROUND,
                            id,
                            JointKind::Weld,
                            target,
                            None,
                            None,
                        );
                    }
                }
            }
            let actions = match kind {
                0 => vec![Action::Message(message)],
                1 | 2 => vec![Action::HeatCircle(
                    target.0 as i32,
                    target.1 as i32,
                    8,
                    1500.,
                )],
                3 => vec![Action::Weather(
                    Some(grave_engine::weather::Kind::Snow),
                    0.8,
                    2.,
                )],
                4 => vec![Action::Lighting(0.2, 1., None)],
                _ => vec![Action::Finish],
            };
            app.sim.triggers.push(Trigger {
                x: at.0,
                y: at.1,
                w,
                h,
                by,
                repeat,
                actions,
                fired: false,
                occupied: false,
            });
        });
        self.studio.status =
            "Event zone placed. Try it from the Events panel or move its chosen object through it."
                .into();
    }
    fn place_studio_light(&mut self, at: (f32, f32)) {
        let settings = self.studio.lights.clone();
        let attach = if settings.attach {
            self.selected_ids().first().copied()
        } else {
            None
        };
        self.change("place light", |app| {
            let mut owner = attach;
            if settings.kind == 2 {
                let mut a = Asset::blank("neon_strip", settings.width, 3).unwrap();
                let mat = app
                    .mats
                    .id("lamp_glass")
                    .or_else(|| app.mats.id("glass"))
                    .unwrap();
                a.draw(
                    0,
                    Brush::Rectangle,
                    (0, 0),
                    (settings.width as i32 - 1, 1),
                    mat,
                );
                a.paint_surface(
                    0,
                    Brush::Rectangle,
                    (0, 0),
                    (settings.width as i32 - 1, 1),
                    Some(settings.color),
                );
                if let Ok(ids) = a.instantiate(&mut app.sim, &app.mats, at.0, at.1) {
                    owner = ids.first().map(|(_, id)| *id);
                    if let Some(id) = owner {
                        app.sim.physics.join(
                            &app.mats,
                            attach.unwrap_or(GROUND),
                            id,
                            JointKind::Weld,
                            at,
                            None,
                            None,
                        );
                    }
                }
            }
            let offsets: Vec<_> = if settings.kind == 3 {
                let d = settings.width as f32 / 3.;
                vec![(-d, -d / 2.), (d, -d / 2.), (-d, d / 2.), (d, d / 2.)]
            } else {
                vec![(0., 0.)]
            };
            for offset in offsets {
                let pos = (at.0 + offset.0, at.1 + offset.1);
                let mut l = Light::at(pos.0, pos.1, settings.radius);
                l.color = settings.color;
                l.strength = settings.strength / if settings.kind == 3 { 2. } else { 1. };
                l.shadows = settings.shadows;
                l.flicker = settings.flicker;
                l.cone = (settings.kind == 1).then_some((settings.angle, settings.spread));
                if let Some(id) = owner {
                    if let Some((x, y, a)) = app.sim.physics.pose_of(id) {
                        let (dx, dy) = (pos.0 - x, pos.1 - y);
                        l.x = dx * a.cos() + dy * a.sin();
                        l.y = -dx * a.sin() + dy * a.cos();
                        l.on = Some(id);
                    }
                }
                app.sim.lighting.lights.push(l);
            }
        });
    }
}
