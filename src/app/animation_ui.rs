//! Animation graph authoring and an isolated character preview.

use super::*;
use grave_engine::animation::{Animator, Graph, State, Transition};
use std::sync::Arc;

pub(super) struct AnimationEditor {
    pub open: bool,
    actor: usize,
    graph: Graph,
    path: String,
    message: String,
    preview: Animator,
    running: bool,
    texture: Option<TextureHandle>,
    source: String,
    events: String,
    selected_action: String,
    selected_state: String,
}

impl Default for AnimationEditor {
    fn default() -> Self {
        let graph = Graph::parse(grave_engine::animation::DEFAULT).unwrap();
        Self {
            open: false,
            actor: 0,
            path: String::new(),
            message: String::new(),
            preview: Animator::new(Arc::new(graph.clone())).unwrap(),
            graph,
            running: true,
            texture: None,
            source: String::new(),
            events: String::new(),
            selected_action: String::new(),
            selected_state: String::new(),
        }
    }
}

impl AnimationEditor {
    pub fn select(&mut self, sim: &Sim, actor: usize) {
        let actors = sim.physics.export_actors();
        let Some(a) = actors.get(actor) else {
            self.message = "Load a scene with a character first.".into();
            return;
        };
        self.actor = actor;
        self.graph = a.animation.as_ref().unwrap().graph.as_ref().clone();
        self.preview = Animator::new(Arc::new(self.graph.clone())).unwrap();
        self.path = a.look.as_ref().map_or_else(
            || "assets/animations/custom.graph".into(),
            |l| l.dir.join("animation.graph").to_string_lossy().into_owned(),
        );
        self.message.clear();
        self.source = self.graph.to_text();
    }

    fn validated(&self, look: Option<&grave_engine::look::Look>) -> Result<Arc<Graph>, String> {
        self.graph.validate()?;
        if let Some(l) = look {
            for s in &self.graph.states {
                if !l.anims.contains_key(&s.clip) {
                    return Err(format!("State {} needs missing clip {}", s.name, s.clip));
                }
            }
        }
        Ok(Arc::new(self.graph.clone()))
    }

    /// Returns a graph to apply to the scene only after an explicit button press.
    pub fn ui(&mut self, ctx: &egui::Context, sim: &Sim, dt: f32) -> Option<(usize, Arc<Graph>)> {
        if !self.open {
            return None;
        }
        let actors = sim.physics.actor_status();
        let look = actors.get(self.actor).and_then(|a| a.look.as_deref());
        let mut open = self.open;
        let mut apply = None;
        egui::Window::new("Animation state machine").open(&mut open).default_size([950.0, 680.0]).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Character");
                egui::ComboBox::from_id_salt("graph_actor").selected_text(format!("{}", self.actor + 1)).show_ui(ui, |ui| {
                    for i in 0..actors.len() { if ui.selectable_label(i == self.actor, format!("{}: {}", i + 1, actors[i].look.as_ref().map_or("cell figure", |l| l.name.as_str()))).clicked() { self.select(sim, i); } }
                });
                if let Some(a) = actors.get(self.actor) { ui.label(format!("Live: {} → {}", a.animation_state, a.anim)); }
            });
            if actors.is_empty() { ui.label("Load a scene with a character to apply a graph. The preview controls still work."); }
            ui.horizontal_wrapped(|ui| {
                if ui.button("Restart preview").clicked() {
                    match self.validated(look) { Ok(g) => { self.preview = Animator::new(g).unwrap(); self.message = "Preview updated".into(); }, Err(e) => self.message = e }
                }
                ui.checkbox(&mut self.running, "Play preview");
                if ui.button("Apply to character").clicked() {
                    match self.validated(look) { Ok(g) if !actors.is_empty() => apply = Some((self.actor, g)), Ok(_) => self.message = "No character in scene".into(), Err(e) => self.message = e }
                }
                if ui.button("Save graph + apply").clicked() {
                    match self.validated(look) {
                        Ok(g) => match std::fs::write(&self.path, self.graph.to_text()) {
                            Ok(()) => { self.message = "Saved. Character folders load animation.graph automatically.".into(); if !actors.is_empty() { apply = Some((self.actor, g)); } },
                            Err(e) => self.message = format!("Could not save: {e}"),
                        }, Err(e) => self.message = e,
                    }
                }
                if ui.button("Load graph").clicked() {
                    match std::fs::read_to_string(&self.path).map_err(|e| e.to_string()).and_then(|t| Graph::parse(&t)) {
                        Ok(g) => { self.graph = g; self.source = self.graph.to_text(); self.message = "Loaded into draft. Restart preview or apply when ready.".into(); }, Err(e) => self.message = e,
                    }
                }
            });
            ui.horizontal(|ui| { ui.label("Graph file"); ui.text_edit_singleline(&mut self.path); });
            if !self.message.is_empty() { ui.label(&self.message); }
            ui.separator();
            egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
                ui.columns(2, |cols| {
                    cols[0].heading("Preview inputs");
                    cols[0].label("These controls test the graph without moving the game character.");
                    for (p, value) in &mut self.preview.parameters {
                        cols[0].horizontal(|ui| { ui.label(p); ui.add(egui::DragValue::new(value).speed(0.1)); });
                    }
                    let triggers: Vec<_> = self.preview.graph.triggers.iter().cloned().collect();
                    if triggers.len() <= 8 {
                        for p in triggers { if cols[0].button(format!("Trigger {p}")).clicked() { self.preview.trigger(&p); } }
                    } else {
                        if !triggers.contains(&self.selected_action) { self.selected_action = triggers[0].clone(); }
                        cols[0].horizontal(|ui| {
                            ui.label("Action");
                            egui::ComboBox::from_id_salt("preview_action").selected_text(&self.selected_action).show_ui(ui, |ui| {
                                for p in &triggers { ui.selectable_value(&mut self.selected_action, p.clone(), p); }
                            });
                            if ui.button("Trigger action").clicked() { self.preview.trigger(&self.selected_action); }
                        });
                    }
                    cols[0].horizontal(|ui| {
                        ui.label("Inspect clip");
                        if !self.graph.states.iter().any(|s| s.name == self.selected_state) { self.selected_state = self.graph.initial.clone(); }
                        egui::ComboBox::from_id_salt("inspect_animation_state").selected_text(&self.selected_state).show_ui(ui, |ui| {
                            for s in &self.graph.states { ui.selectable_value(&mut self.selected_state, s.name.clone(), &s.name); }
                        });
                        if ui.button("Play clip only").clicked() {
                            let mut graph = self.graph.clone(); graph.initial = self.selected_state.clone(); graph.transitions.clear();
                            match Animator::new(Arc::new(graph)) { Ok(a) => { self.preview = a; self.message = "Inspecting one clip. Restart preview restores transitions.".into(); }, Err(e) => self.message = e }
                        }
                    });
                    if self.running {
                        let e = self.preview.advance(dt.min(0.1), |c| look.map_or(1.0, |l| l.duration(c)));
                        if !e.is_empty() { self.events = e.iter().map(|e| format!("{}: {}", e.state, e.name)).collect::<Vec<_>>().join(", "); }
                    }
                    cols[1].heading(format!("{} → {}", self.preview.state().name, self.preview.state().clip));
                    cols[1].label(format!("State time {:.2}s · clip time {:.2}s", self.preview.elapsed, self.preview.time));
                    if let Some(l) = look {
                        let (f, _) = l.frame(&self.preview.state().clip, self.preview.sample_time(|c| l.duration(c)));
                        let img = egui::ColorImage::from_rgba_unmultiplied([f.w, f.h], &f.rgba);
                        if let Some(t) = &mut self.texture { t.set(img, TextureOptions::LINEAR); } else { self.texture = Some(ctx.load_texture("animation-preview", img, TextureOptions::LINEAR)); }
                        cols[1].image((self.texture.as_ref().unwrap().id(), Vec2::splat(180.0)));
                    }
                    cols[1].label(format!("Events: {}", self.events));
                });
                ui.separator();
                ui.heading("States");
                ui.horizontal_wrapped(|ui| {
                    for s in &self.graph.states { ui.selectable_label(self.preview.state().name == s.name, &s.name).on_hover_text(format!("clip: {}", s.clip)); }
                });
                ui.horizontal(|ui| { ui.label("Initial state"); ui.text_edit_singleline(&mut self.graph.initial); });
                let mut remove = None;
                for (i, s) in self.graph.states.iter_mut().enumerate() {
                    ui.push_id(("state", i), |ui| { ui.collapsing(format!("{} — {}", s.name, s.clip), |ui| {
                        ui.horizontal(|ui| { ui.label("Name"); ui.text_edit_singleline(&mut s.name); ui.label("Clip"); ui.text_edit_singleline(&mut s.clip); });
                        ui.horizontal(|ui| { ui.checkbox(&mut s.looping, "Loop"); ui.label("Speed"); ui.add(egui::DragValue::new(&mut s.speed).speed(0.05)); if ui.button("Delete state").clicked() { remove = Some(i); } });
                        let mut override_duration = s.duration.is_some();
                        if ui.checkbox(&mut override_duration, "Override duration (seconds)").changed() { s.duration = override_duration.then_some(0.2); }
                        if let Some(d) = &mut s.duration { ui.add(egui::DragValue::new(d).speed(0.01)); }
                        let mut rate = s.rate.is_some();
                        if ui.checkbox(&mut rate, "Drive playback speed from a parameter").changed() { s.rate = rate.then_some(("speed".into(), 0.25, 0.3, 1.5)); }
                        if let Some((p, scale, min, max)) = &mut s.rate { ui.horizontal(|ui| { ui.text_edit_singleline(p); ui.label("scale"); ui.add(egui::DragValue::new(scale).speed(0.01)); ui.label("min/max"); ui.add(egui::DragValue::new(min).speed(0.01)); ui.add(egui::DragValue::new(max).speed(0.01)); }); }
                        let mut delete_event = None;
                        for (j, (time, name)) in s.events.iter_mut().enumerate() { ui.push_id(j, |ui| { ui.horizontal(|ui| { ui.label("Event at"); ui.add(egui::DragValue::new(time).speed(0.01)); ui.text_edit_singleline(name); if ui.button("Remove").clicked() { delete_event = Some(j); } }); }); }
                        if let Some(j) = delete_event { s.events.remove(j); }
                        if ui.button("Add timed event").clicked() { s.events.push((0.0, "marker".into())); }
                    }); });
                }
                if let Some(i) = remove { self.graph.states.remove(i); }
                if ui.button("Add state").clicked() { self.graph.states.push(State { name: format!("state_{}", self.graph.states.len()), clip: "idle".into(), speed: 1.0, looping: false, duration: None, rate: None, events: vec![] }); }
                ui.heading("Transitions");
                ui.label("Higher priority wins. Equal priorities use list order. * means any state. Conditions use & for AND.");
                let mut remove = None;
                let mut reorder = None;
                for (i, t) in self.graph.transitions.iter_mut().enumerate() {
                    ui.push_id(("transition", i), |ui| { ui.collapsing(format!("{} → {} · {}", t.from, t.to, t.conditions), |ui| {
                        ui.horizontal(|ui| { ui.label("From"); ui.text_edit_singleline(&mut t.from); ui.label("To"); ui.text_edit_singleline(&mut t.to); });
                        ui.horizontal(|ui| { ui.label("Priority"); ui.add(egui::DragValue::new(&mut t.priority)); ui.label("Minimum time"); ui.add(egui::DragValue::new(&mut t.after).speed(0.01)); if i > 0 && ui.button("Move up").clicked() { reorder = Some(i); } if ui.button("Delete").clicked() { remove = Some(i); } });
                        ui.add(egui::TextEdit::singleline(&mut t.conditions).hint_text("finished > 0 & speed <= 0.4").desired_width(f32::INFINITY));
                    }); });
                }
                if let Some(i) = remove { self.graph.transitions.remove(i); } else if let Some(i) = reorder { self.graph.transitions.swap(i, i - 1); }
                if ui.button("Add transition").clicked() { self.graph.transitions.push(Transition { from: self.graph.initial.clone(), to: self.graph.initial.clone(), priority: 0, after: 0.0, conditions: String::new() }); }
                ui.collapsing("Parameters, triggers and text format", |ui| {
                    ui.label("Use the text format to add custom parameters or triggers. Import validates before changing the draft.");
                    if ui.button("Export draft to text").clicked() { self.source = self.graph.to_text(); }
                    ui.add(egui::TextEdit::multiline(&mut self.source).font(egui::TextStyle::Monospace).desired_rows(12).desired_width(f32::INFINITY));
                    if ui.button("Import text into draft").clicked() { match Graph::parse(&self.source) { Ok(g) => { self.graph = g; self.message = "Draft imported".into(); }, Err(e) => self.message = e } }
                });
                if let Err(e) = self.validated(look) { ui.colored_label(Color32::from_rgb(255, 160, 90), e); }
            });
        });
        self.open = open;
        apply
    }
}

impl Sandbox {
    pub(super) fn sync_animation_reset(&mut self) {
        for (saved, current) in self
            .snapshot
            .actors
            .iter_mut()
            .zip(self.sim.physics.export_actors())
        {
            saved.animation = current.animation.map(|a| Animator::new(a.graph).unwrap());
        }
    }

    pub(super) fn animation_window(&mut self, ctx: &egui::Context, dt: f32) {
        if let Some((i, g)) = self.animation_editor.ui(ctx, &self.sim, dt) {
            // Apply is undoable and the reset snapshot keeps the edited graph.
            self.change("animation graph", |app| {
                if let Err(e) = app.sim.physics.set_actor_graph(i, g.clone()) {
                    app.say(e);
                }
            });
            self.sync_animation_reset();
        }
    }
}
