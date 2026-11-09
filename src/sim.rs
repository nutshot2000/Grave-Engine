//! A whole running simulation: the cell grid plus the rigid objects in it.

use crate::material::Materials;
use crate::physics::{BodySpec, Physics};
use crate::world::World;

pub struct Sim {
    pub world: World,
    pub physics: Physics,
}

/// A frozen copy of a simulation, for Reset and for saving.
#[derive(Clone)]
pub struct Snapshot {
    pub world: World,
    pub bodies: Vec<BodySpec>,
}

impl Sim {
    /// Build from a world (without objects) and a list of objects to add.
    pub fn new(world: World, bodies: Vec<BodySpec>, mats: &Materials) -> Self {
        let mut sim = Sim { physics: Physics::new(world.w, world.h), world };
        for b in bodies {
            sim.physics.add(&mut sim.world, mats, b);
        }
        sim
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
    }

    pub fn snapshot(&mut self) -> Snapshot {
        let bodies = self.physics.export(&mut self.world);
        let mut world = self.world.clone();
        for i in 0..world.mat.len() {
            if world.is_owned(i) {
                world.make_air(i);
            }
        }
        Snapshot { world, bodies }
    }

    pub fn restore(snap: &Snapshot, mats: &Materials) -> Self {
        Sim::new(snap.world.clone(), snap.bodies.clone(), mats)
    }
}
