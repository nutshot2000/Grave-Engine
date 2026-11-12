//! A whole running simulation: the cell grid plus the rigid objects in it.

use crate::material::Materials;
use crate::physics::{BodySpec, GROUND, JointSpec, Physics};
use crate::template::Instance;
use crate::world::World;
use std::collections::HashMap;

pub struct Sim {
    pub world: World,
    /// Independent aligned depth planes, nearest first. Plane zero is this world.
    pub layers: Vec<Sim>,
    pub layer_names: Vec<String>,
    pub physics: Physics,
    /// Objects that are copies of a template, by object id.
    pub instances: HashMap<u64, Instance>,
    /// The object the camera follows, if any.
    pub follow: Option<u64>,
    /// How close the following camera is (times closer than showing the
    /// whole world). 0 = the editor decides.
    pub zoom: f32,
    /// Level triggers (see trigger.rs), what they did since the game last
    /// looked (`events`, drain them), and the last checkpoint reached.
    pub triggers: Vec<crate::trigger::Trigger>,
    pub events: Vec<crate::trigger::Event>,
    pub checkpoint: Option<(f32, f32)>,
    /// Camera effects asked for by triggers or the game: slow motion, zoom,
    /// focus, shake (see camera.rs). Not saved: Reset and rewind clear them.
    pub camera: crate::camera::Effects,
    /// The scene's lighting: overall light, sun, lamps (see light.rs). Lamps
    /// `on` an object hold its id.
    pub lighting: crate::light::Lighting,
    /// Sky and scenery layers behind the world (see backdrop.rs).
    pub backdrop: crate::backdrop::Backdrop,
    /// Weather: snow, rain, hail, dust, fog, gusts, lightning (see weather.rs).
    pub weather: crate::weather::Weather,
    /// What the weather is doing right now (flash, bolt, strike timer).
    pub weather_run: crate::weather::WeatherRun,
    /// Where weather falls: the part of the world on screen (x0, y0, x1, y1),
    /// set each frame by the editor or game. None = around the followed
    /// object, or everywhere.
    pub weather_view: Option<(f32, f32, f32, f32)>,
}

/// A frozen copy of a simulation, for Reset, undo, rewind and saving.
#[derive(Clone)]
pub struct Snapshot {
    pub world: World,
    pub layers: Vec<Snapshot>,
    pub layer_names: Vec<String>,
    pub bodies: Vec<BodySpec>,
    /// For each object in `bodies`: which template it's a copy of, if any.
    pub instances: Vec<Option<Instance>>,
    /// Joints, with `a` and `b` meaning "object number N+1 in `bodies`"
    /// (0 = the ground).
    pub joints: Vec<JointSpec>,
    /// The object the camera follows (number N+1 in `bodies`), and how close.
    pub follow: Option<u64>,
    pub zoom: f32,
    /// Objects with a balance assist: (number N+1 in `bodies`, strength).
    pub upright: Vec<(u64, crate::physics::Balance)>,
    /// Triggers (`by` an object = number N+1) and the last checkpoint.
    pub triggers: Vec<crate::trigger::Trigger>,
    pub checkpoint: Option<(f32, f32)>,
    /// Lighting (lamps `on` an object = its number N+1).
    pub lighting: crate::light::Lighting,
    pub backdrop: crate::backdrop::Backdrop,
    pub weather: crate::weather::Weather,
    /// Characters (the player).
    pub actors: Vec<crate::physics::ActorSpec>,
}

impl Snapshot {
    /// A snapshot of just a world, with no objects.
    pub fn of_world(world: World) -> Self {
        Snapshot { world, layers: Vec::new(), layer_names: Vec::new(), bodies: Vec::new(), instances: Vec::new(), joints: Vec::new(), follow: None, zoom: 0.0, upright: Vec::new(), triggers: Vec::new(), checkpoint: None, lighting: Default::default(), backdrop: Default::default(), weather: Default::default(), actors: Vec::new() }
    }
}

impl Sim {
    /// Build from a world (without objects) and a list of objects to add.
    pub fn new(world: World, bodies: Vec<BodySpec>, mats: &Materials) -> Self {
        let n = bodies.len();
        Sim::new_with(world, bodies, vec![None; n], mats)
    }

    /// Like `new`, with template links for the objects (same order).
    pub fn new_with(world: World, bodies: Vec<BodySpec>, instances: Vec<Option<Instance>>, mats: &Materials) -> Self {
        let mut sim = Sim { physics: Physics::new(world.w, world.h), world, layers: Vec::new(), layer_names: Vec::new(), instances: HashMap::new(), follow: None, zoom: 0.0, triggers: Vec::new(), events: Vec::new(), checkpoint: None, camera: Default::default(), lighting: Default::default(), backdrop: Default::default(), weather: Default::default(), weather_run: Default::default(), weather_view: None };
        for (b, inst) in bodies.into_iter().zip(instances.into_iter().chain(std::iter::repeat(None))) {
            if let (Some(id), Some(inst)) = (sim.physics.add(&mut sim.world, mats, b), inst) {
                sim.instances.insert(id, inst);
            }
        }
        sim
    }

    /// Add an aligned plane, inheriting fixed terrain but never front objects.
    pub fn ensure_layer(&mut self, index: usize, name: &str, mats: &Materials) -> &mut Sim {
        assert!((1..=3).contains(&index), "depth plane must be 1..=3");
        while self.layers.len() < index {
            let mut world=self.world.clone();
            for i in 0..world.mat.len() { if world.is_owned(i) {world.make_air(i);} }
            world.flying.clear(); world.blasts.clear();
            self.layers.push(Sim::new(world,Vec::new(),mats));
            self.layer_names.push(format!("interior_{}",self.layers.len()));
        }
        self.layer_names[index-1]=name.to_string();
        &mut self.layers[index-1]
    }
    fn step_layers(&mut self, mats: &Materials) {
        if !self.layers.is_empty() {
            if self.world.depth_air.iter().any(|v|!*v) {self.world.depth_air.fill(true);self.world.wake_all();}
            let feeding: Vec<Vec<bool>>=std::iter::once(&self.world).chain(self.layers.iter().map(|l|&l.world)).map(|w|w.mat.iter().map(|id|mats.get(*id).feeds_fire).collect()).collect();
            let total=self.layers.len();
            for (z,layer) in self.layers.iter_mut().enumerate() {
                for i in 0..layer.world.mat.len() {
                    let exposed=feeding[z].get(i).copied().unwrap_or(false) || if z+1==total {true} else {feeding[z+2].get(i).copied().unwrap_or(false)};
                    if layer.world.depth_air[i]!=exposed {layer.world.depth_air[i]=exposed;layer.world.wake_at(i);}
                }
            }
        }
        let mut previous=&mut self.world;
        for layer in &mut self.layers {
            crate::layers::exchange_heat(previous,&mut layer.world,mats);
            previous=&mut layer.world;
        }
        for layer in &mut self.layers { layer.step_depth(mats,true); }
    }
    /// Reconnect joints numbered by position (1 = first object, 0 = ground).
    pub fn add_joints(&mut self, joints: &[JointSpec], ids: &[Option<u64>], mats: &Materials) {
        let id_of = |n: u64| -> Option<u64> {
            if n == GROUND {
                return Some(GROUND);
            }
            ids.get(n as usize - 1).copied().flatten()
        };
        for j in joints {
            if let (Some(a), Some(b)) = (id_of(j.a), id_of(j.b)) {
                self.physics.add_joint(mats, JointSpec { a, b, ..j.clone() });
            }
        }
    }

    pub fn empty(w: usize, h: usize, mats: &Materials) -> Self {
        let mut world = World::new(w, h);
        world.fill(mats, crate::material::AIR);
        Sim::new(world, Vec::new(), mats)
    }

    /// One frame: cells first (heat, fire, flowing), then objects.
    pub fn step(&mut self, mats: &Materials) {
        self.step_depth(mats,false);
    }
    fn step_depth(&mut self, mats: &Materials, inherited_air: bool) {
        if !inherited_air && self.layers.is_empty() {self.world.depth_air.fill(false);}
        self.step_layers(mats);
        self.world.step(mats);
        // Weather falls (and lightning strikes, adding blasts) before the
        // objects feel this frame's physics.
        let follow = self.follow.and_then(|id| self.physics.centre(id));
        crate::weather::step(&mut self.world, mats, &mut self.weather, &mut self.weather_run, self.weather_view, follow, &mut self.camera);
        self.physics.update(&mut self.world, mats);
        if !self.triggers.is_empty() {
            self.run_triggers();
        }
        self.camera.step();
        self.lighting.step();
        self.backdrop.step();
        if !self.instances.is_empty() {
            // A damaged copy has gone its own way: it's no longer linked to
            // its template (editing the template mustn't heal it).
            for id in self.physics.damaged() {
                self.instances.remove(id);
            }
            // Copies that broke apart are just debris now.
            let alive: std::collections::HashSet<u64> = self.physics.ids().into_iter().collect();
            self.instances.retain(|id, _| alive.contains(id));
        }
        // The followed object is gone (broke apart, burned up): stop following.
        if let Some(id) = self.follow
            && self.physics.spec_of(&self.world, id).is_none() {
                self.follow = None;
            }
    }

    /// Material settings changed: objects get new shapes and weights, and
    /// the whole world wakes up so sleeping areas follow the new settings
    /// (a lower melting point, a solid made into powder...).
    pub fn materials_changed(&mut self, mats: &Materials) {
        for layer in &mut self.layers { layer.materials_changed(mats); }
        self.physics.materials_changed(mats);
        self.world.wake_all();
    }

    /// Where the lamps are right now (lamps on objects move with them).
    pub fn lamps(&self) -> Vec<crate::light::Placed> {
        self.lighting.place(self.world.frame, |id| self.physics.pose_of(id))
    }

    /// Set off triggers that something has entered.
    fn run_triggers(&mut self) {
        use crate::trigger::By;
        let bodies: Vec<(u64, (f32, f32))> = self.physics.ids().into_iter().filter_map(|id| Some((id, self.physics.centre(id)?))).collect();
        let actors: Vec<(f32, f32)> = self.physics.actor_status().iter().map(|a| (a.x, a.y)).collect();
        for t in self.triggers.iter_mut() {
            if t.fired && !t.repeat {
                continue;
            }
            use crate::camera::Focus;
            let body_in = |only: Option<u64>| bodies.iter().find(|(b, p)| only.is_none_or(|o| o == *b) && t.contains(p.0, p.1)).map(|(b, _)| Focus::Object(*b));
            let actor_in = || actors.iter().position(|p| t.contains(p.0, p.1)).map(Focus::Actor);
            // What set it off (for `focus it`).
            let who = match t.by {
                By::Object(id) => body_in(Some(id)),
                By::Player => actor_in(),
                By::Any => actor_in().or_else(|| body_in(None)),
            };
            let inside = who.is_some();
            if inside && !t.occupied {
                t.fire(&mut self.world, &mut self.events, &mut self.camera, &mut self.lighting, &mut self.backdrop, &mut self.weather, who);
                t.fired = true;
            }
            t.occupied = inside;
        }
        for e in &self.events {
            if let crate::trigger::Event::Checkpoint(x, y) = e {
                self.checkpoint = Some((*x, *y));
            }
        }
    }

    /// Mark once-only triggers left of `x` as already done (coming back at
    /// a checkpoint further along: the fuses behind you stay lit... or not).
    pub fn skip_triggers_before(&mut self, x: f32) {
        for t in self.triggers.iter_mut() {
            if t.x + t.w <= x {
                t.fired = true;
            }
        }
    }

    pub fn snapshot(&mut self) -> Snapshot {
        let bodies = self.physics.export(&mut self.world);
        let ids = self.physics.ids();
        let instances = ids.iter().map(|id| self.instances.get(id).cloned()).collect();
        let index = |id: u64| -> Option<u64> {
            if id == GROUND {
                return Some(GROUND);
            }
            ids.iter().position(|&i| i == id).map(|p| p as u64 + 1)
        };
        let joints = self
            .physics
            .joints()
            .into_iter()
            .filter_map(|mut j| {
                j.a = index(j.a)?;
                j.b = index(j.b)?;
                Some(j)
            })
            .collect();
        let follow = self.follow.and_then(index);
        let upright = self.physics.upright().into_iter().filter_map(|(id, s)| Some((index(id)?, s))).collect();
        let actors = self.physics.export_actors();
        let mut world = self.world.clone();
        for i in 0..world.mat.len() {
            if world.is_owned(i) {
                world.make_air(i);
            }
        }
        let triggers = self
            .triggers
            .iter()
            .filter_map(|t| {
                let mut t = t.clone();
                if let crate::trigger::By::Object(id) = t.by {
                    t.by = crate::trigger::By::Object(index(id)?);
                }
                Some(t)
            })
            .collect();
        let mut lighting = self.lighting.clone();
        lighting.lights.retain_mut(|l| match l.on {
            Some(id) => index(id).map(|n| l.on = Some(n)).is_some(),
            None => true,
        });
        Snapshot { world, layers: self.layers.iter_mut().map(|l| l.snapshot()).collect(), layer_names: self.layer_names.clone(), bodies, instances, joints, follow, zoom: self.zoom, upright, triggers, checkpoint: self.checkpoint, lighting, backdrop: self.backdrop.clone(), weather: self.weather.clone(), actors }
    }

    /// Put things back exactly as they were (objects aren't nudged out of
    /// the ground the way newly placed ones are).
    pub fn restore(snap: &Snapshot, mats: &Materials) -> Self {
        let mut world = snap.world.clone();
        world.wake_all(); // restored: look at everything again
        let mut sim = Sim { physics: Physics::new(world.w, world.h), world, layers: Vec::new(), layer_names: Vec::new(), instances: HashMap::new(), follow: None, zoom: snap.zoom, triggers: Vec::new(), events: Vec::new(), checkpoint: snap.checkpoint, camera: Default::default(), lighting: Default::default(), backdrop: snap.backdrop.clone(), weather: snap.weather.clone(), weather_run: Default::default(), weather_view: None };
        sim.layers = snap.layers.iter().map(|s| Sim::restore(s,mats)).collect();
        sim.layer_names = snap.layer_names.clone();
        let links = snap.instances.iter().cloned().chain(std::iter::repeat(None));
        let mut new_ids = Vec::new();
        for (b, inst) in snap.bodies.iter().cloned().zip(links) {
            let id = sim.physics.add_exact(&mut sim.world, mats, b);
            if let (Some(id), Some(inst)) = (id, inst) {
                sim.instances.insert(id, inst);
            }
            new_ids.push(id);
        }
        sim.add_joints(&snap.joints, &new_ids, mats);
        for a in &snap.actors {
            sim.physics.add_actor(&mut sim.world, mats, a.clone());
        }
        sim.follow = snap.follow.filter(|&n| n > 0).and_then(|n| new_ids.get(n as usize - 1).copied().flatten());
        sim.lighting = snap.lighting.clone();
        sim.lighting.lights.retain_mut(|l| match l.on {
            Some(n) => match (n > 0).then(|| new_ids.get(n as usize - 1).copied().flatten()).flatten() {
                Some(id) => {
                    l.on = Some(id);
                    true
                }
                None => false,
            },
            None => true,
        });
        for t in &snap.triggers {
            let mut t = t.clone();
            if let crate::trigger::By::Object(n) = t.by {
                match (n > 0).then(|| new_ids.get(n as usize - 1).copied().flatten()).flatten() {
                    Some(id) => t.by = crate::trigger::By::Object(id),
                    None => continue,
                }
            }
            sim.triggers.push(t);
        }
        for &(n, bal) in &snap.upright {
            if let Some(id) = (n > 0).then(|| new_ids.get(n as usize - 1).copied().flatten()).flatten() {
                sim.physics.set_balance(id, bal);
            }
        }
        sim
    }
}
