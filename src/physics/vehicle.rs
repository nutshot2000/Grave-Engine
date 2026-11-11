//! Vehicles: reusable controls for anything built from objects, joints and
//! motors (bikes, cars, carts).
//!
//! - **Throttle** (`set_throttle`): drive motors (`wheel ... motor ... drive`).
//! - **Brakes** (`set_brake`): drive wheels are held still by their motors,
//!   so stopping comes from real tyre grip: hard on stone, hardly at all on
//!   ice (the tyre just slides). No effect in the air.
//! - **Rolling resistance** (material key `rolling_resistance`): loose or
//!   soft ground the drive wheels touch slows the whole vehicle down (sand,
//!   mud, water). Read from the actual cells around each tyre.
//! - **Balance** (`upright NAME strength S` in a scene, or `set_upright`):
//!   an assist that keeps an object (and everything welded or wheeled to it)
//!   upright, like a rider balancing; **lean** (`set_lean`, -1..1) tips it
//!   forward or back for jumps and wheelies.
//!
//! Games set throttle, brake and lean from their keys; everything else is
//! scene and material data.

use super::*;

/// How hard brakes can hold a wheel, compared with its drive motor.
const BRAKE_STRENGTH: f32 = 6.0;
/// How firmly brakes hold a wheel at a standstill (the motor's damping):
/// much firmer than driving, or the wheel just keeps turning.
const BRAKE_RESPONSE: f32 = 100_000.0;
/// The most a balance assist turns an object (radians per second).
const BALANCE_MAX_SPIN: f32 = 3.0;

/// What a vehicle's drive wheels are on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WheelContact {
    /// Any drive wheel touching something that isn't gas.
    pub grounded: bool,
    /// The most resistant material under a drive wheel (if any resists).
    pub surface: Option<MatId>,
    /// That material's `rolling_resistance` (per second).
    pub resistance: f32,
}

impl Physics {
    /// Brakes, 0..1: hold the drive wheels still (grip does the stopping).
    pub fn set_brake(&mut self, brake: f32) {
        self.brake = brake.clamp(0.0, 1.0);
    }

    pub fn brake(&self) -> f32 {
        self.brake
    }

    /// Lean, -1..1 (back..forward): tips balanced objects for jumps and wheelies.
    pub fn set_lean(&mut self, lean: f32) {
        self.lean = lean.clamp(-1.0, 1.0);
    }

    /// Keep an object (and what's welded or wheeled to it) upright with this
    /// strength (0 = off; 60 is a good rider).
    pub fn set_upright(&mut self, id: u64, strength: f32) {
        if strength > 0.0 {
            self.upright.insert(id, strength);
        } else {
            self.upright.remove(&id);
        }
    }

    /// Objects with a balance assist, and how strong.
    pub fn upright(&self) -> Vec<(u64, f32)> {
        self.upright.iter().map(|(&id, &s)| (id, s)).collect()
    }

    /// What the drive wheels attached to this object (or the object, if it
    /// is a wheel) are touching right now.
    pub fn wheel_contact(&self, world: &World, mats: &Materials, id: u64) -> WheelContact {
        let assembly = self.assembly_of(id);
        let mut out = WheelContact::default();
        for j in &self.joints {
            let drive = j.spec.motor.as_ref().is_some_and(|m| m.drive);
            if !matches!(j.spec.kind, JointKind::Wheel { .. }) || !drive || !assembly.contains(&j.spec.b) {
                continue;
            }
            let Some(n) = self.find(j.spec.b) else { continue };
            let (grounded, surface, resistance) = Self::touching(&self.bodies[n], world, mats);
            out.grounded |= grounded;
            if resistance > out.resistance {
                out.resistance = resistance;
                out.surface = surface;
            }
            if out.surface.is_none() && grounded {
                out.surface = surface;
            }
        }
        out
    }

    /// What a body's cells touch (outside itself): anything not gas, and the
    /// most resistant material. Uses where it was stamped last frame.
    fn touching(b: &Body, world: &World, mats: &Materials) -> (bool, Option<MatId>, f32) {
        let own: std::collections::HashSet<u32> = b.stamped.iter().map(|&(wi, _)| wi).collect();
        let (mut grounded, mut surface, mut resistance) = (false, None, 0.0f32);
        for &(wi, _) in &b.stamped {
            let wi = wi as usize;
            let (x, y) = (wi % world.w, wi / world.w);
            let near = [(x > 0).then(|| wi - 1), (x + 1 < world.w).then(|| wi + 1), (y > 0).then(|| wi - world.w), (y + 1 < world.h).then(|| wi + world.w)];
            for j in near.into_iter().flatten() {
                if own.contains(&(j as u32)) || world.is_owned(j) {
                    continue;
                }
                let m = mats.get(world.mat[j]);
                if m.state == State::Gas {
                    continue;
                }
                grounded = true;
                if surface.is_none() || m.rolling_resistance > resistance {
                    surface = Some(world.mat[j]);
                    resistance = resistance.max(m.rolling_resistance);
                }
            }
        }
        (grounded, surface, resistance)
    }

    /// An object and everything welded to it, plus wheels on it.
    pub(super) fn assembly_of(&self, id: u64) -> std::collections::BTreeSet<u64> {
        let mut rigid = std::collections::BTreeSet::from([id]);
        loop {
            let before = rigid.len();
            for j in &self.joints {
                if matches!(j.spec.kind, JointKind::Weld) && j.spec.a != GROUND && j.spec.b != GROUND && (rigid.contains(&j.spec.a) || rigid.contains(&j.spec.b)) {
                    rigid.insert(j.spec.a);
                    rigid.insert(j.spec.b);
                }
            }
            if rigid.len() == before {
                break;
            }
        }
        let mut all = rigid.clone();
        for j in &self.joints {
            if matches!(j.spec.kind, JointKind::Wheel { .. }) && (rigid.contains(&j.spec.a) || rigid.contains(&j.spec.b)) {
                all.insert(j.spec.a);
                all.insert(j.spec.b);
            }
        }
        all.remove(&GROUND);
        all
    }

    /// Every frame, before the physics step: drive motors and brakes,
    /// rolling resistance, balance.
    pub(super) fn vehicles(&mut self, world: &World, mats: &Materials) {
        // Motors: throttle, or brakes holding the wheel still.
        let (throttle, brake) = (self.throttle, self.brake);
        for j in &self.joints {
            let Some(m) = j.spec.motor.as_ref().filter(|m| m.drive) else { continue };
            let Some(joint) = self.impulse_joints.get_mut(j.handle, false) else { continue };
            if brake > 0.0 {
                joint.data.set_motor_velocity(JointAxis::AngX, 0.0, BRAKE_RESPONSE);
                joint.data.set_motor_max_force(JointAxis::AngX, m.strength * 1000.0 * BRAKE_STRENGTH * brake);
            } else {
                joint.data.set_motor_velocity(JointAxis::AngX, m.speed * throttle * std::f32::consts::TAU, MOTOR_RESPONSE);
                joint.data.set_motor_max_force(JointAxis::AngX, motor_force(m, throttle));
            }
        }
        // Rolling resistance: whole vehicles slow down on soft ground.
        let mut done = std::collections::BTreeSet::new();
        let drive_wheels: Vec<(u64, u64)> = self
            .joints
            .iter()
            .filter(|j| matches!(j.spec.kind, JointKind::Wheel { .. }) && j.spec.motor.as_ref().is_some_and(|m| m.drive))
            .map(|j| (j.spec.a, j.spec.b))
            .collect();
        for (body, _) in drive_wheels {
            if body == GROUND || done.contains(&body) {
                continue;
            }
            let assembly = self.assembly_of(body);
            done.extend(assembly.iter().copied());
            let contact = self.wheel_contact(world, mats, body);
            if contact.resistance > 0.0 {
                let ids: Vec<u64> = assembly.into_iter().collect();
                self.damp_horizontal(&ids, (-contact.resistance * DT).exp());
            }
        }
        // Balance assists (with lean).
        let lean = self.lean;
        for (id, strength) in self.upright() {
            let Some(n) = self.find(id) else {
                self.upright.remove(&id);
                continue;
            };
            let rb = &self.set[self.bodies[n].handle];
            let angle = rb.rotation().angle();
            let spin = rb.angvel();
            let k = strength / 60.0;
            let push = (-angle * 60.0 * k - spin * 15.0 * k + lean * 28.0 * k).clamp(-100.0 * k, 100.0 * k);
            self.steer_rotation(id, push, BALANCE_MAX_SPIN);
        }
    }
}
