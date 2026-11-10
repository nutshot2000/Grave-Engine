//! A whole running simulation: the cell grid plus the rigid objects in it.

use crate::material::Materials;
use crate::physics::{BodySpec, GROUND, JointSpec, Physics};
use crate::template::Instance;
use crate::world::World;
use std::collections::HashMap;

pub struct Sim {
    pub world: World,
    pub physics: Physics,
    /// Objects that are copies of a template, by object id.
    pub instances: HashMap<u64, Instance>,
}

/// A frozen copy of a simulation, for Reset, undo, rewind and saving.
#[derive(Clone)]
pub struct Snapshot {
    pub world: World,
    pub bodies: Vec<BodySpec>,
    /// For each object in `bodies`: which template it's a copy of, if any.
    pub instances: Vec<Option<Instance>>,
    /// Joints, with `a` and `b` meaning "object number N+1 in `bodies`"
    /// (0 = the ground).
    pub joints: Vec<JointSpec>,
}

impl Snapshot {
    /// A snapshot of just a world, with no objects.
    pub fn of_world(world: World) -> Self {
        Snapshot { world, bodies: Vec::new(), instances: Vec::new(), joints: Vec::new() }
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
        let mut sim = Sim { physics: Physics::new(world.w, world.h), world, instances: HashMap::new() };
        for (b, inst) in bodies.into_iter().zip(instances.into_iter().chain(std::iter::repeat(None))) {
            if let (Some(id), Some(inst)) = (sim.physics.add(&mut sim.world, mats, b), inst) {
                sim.instances.insert(id, inst);
            }
        }
        sim
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
        self.world.step(mats);
        self.physics.update(&mut self.world, mats);
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
        let mut world = self.world.clone();
        for i in 0..world.mat.len() {
            if world.is_owned(i) {
                world.make_air(i);
            }
        }
        Snapshot { world, bodies, instances, joints }
    }

    /// Put things back exactly as they were (objects aren't nudged out of
    /// the ground the way newly placed ones are).
    pub fn restore(snap: &Snapshot, mats: &Materials) -> Self {
        let world = snap.world.clone();
        let mut sim = Sim { physics: Physics::new(world.w, world.h), world, instances: HashMap::new() };
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
        sim
    }
}
