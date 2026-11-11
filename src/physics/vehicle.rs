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
//! - **Balance** (`upright NAME strength S [air A] [heavy H]`, or
//!   `set_balance`): two styles. `air 1` (the default, arcade): it always
//!   levels out, on the ground and in the air. `air` below 1 (trials): on
//!   the ground it follows the slope and leaning shifts the rider's weight
//!   (wheelies, keeping the front down on climbs); in the air it only
//!   levels out that much, and leaning spins it (flips, lining up landings).
//!   `heavy` scales gravity on the whole vehicle (1.6: lands hard, less
//!   floaty jumps).
//!   The assist keeps an object (and everything welded or wheeled to it)
//!   upright, like a rider balancing; **lean** (`set_lean`, -1..1) tips it
//!   forward or back.
//!
//! - **Tyres throw loose stuff**: where a drive wheel's tread moves against
//!   sand, gravel, mud or water (wheelspin, skids, fast rolling), those cells
//!   are flung off at the tread's speed (flying cells: they land again), so
//!   tyres spray sand, spatter mud and dig ruts.
//!
//! Games set throttle, brake and lean from their keys; everything else is
//! scene and material data.

use super::*;

/// A balance assist's settings (see the top of this file).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Balance {
    /// How strong (60 is a good rider; 0 = off).
    pub strength: f32,
    /// How much it levels out in the air: 1 = always (arcade), 0 = not at
    /// all (trials: lean to rotate).
    pub air: f32,
    /// Gravity on the whole vehicle, times normal (1.6 = heavier, lands hard).
    pub heavy: f32,
}

impl Default for Balance {
    fn default() -> Self {
        Balance { strength: 60.0, air: 1.0, heavy: 1.0 }
    }
}

/// Trials style: how hard it's pulled to the slope on the ground, how much
/// leaning shifts the weight there, and how fast leaning spins it in the air.
const TRIALS_GROUND_LEVEL: f32 = 70.0;
const TRIALS_GROUND_LEAN: f32 = 30.0;
const TRIALS_AIR_LEAN: f32 = 32.0;
const TRIALS_MAX_SPIN: f32 = 4.5;
/// How quickly lean follows the keys (per frame): keys are all-or-nothing,
/// a rider shifts their weight over a moment (about a quarter second).
const TRIALS_LEAN_EASE: f32 = 0.09;
/// Spin damping in the air when you let go of lean (so it stops turning),
/// and while leaning.
const TRIALS_AIR_HOLD: f32 = 7.0;
const TRIALS_AIR_FREE: f32 = 1.0;

/// How hard brakes can hold a wheel, compared with its drive motor.
const BRAKE_STRENGTH: f32 = 6.0;
/// How firmly brakes hold a wheel at a standstill (the motor's damping):
/// much firmer than driving, or the wheel just keeps turning.
const BRAKE_RESPONSE: f32 = 100_000.0;
/// Tread speed against loose ground (m/s) before it starts throwing it.
const KICK_SPEED: f32 = 1.2;
/// The most cells one tyre throws per frame.
const KICK_MAX: usize = 3;
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
    /// strength (0 = off; 60 is a good rider), arcade style.
    pub fn set_upright(&mut self, id: u64, strength: f32) {
        self.set_balance(id, Balance { strength, ..Balance::default() });
    }

    /// Keep an object upright with these balance settings (strength 0 = off).
    pub fn set_balance(&mut self, id: u64, b: Balance) {
        if b.strength > 0.0 {
            self.upright.insert(id, b);
        } else {
            self.upright.remove(&id);
        }
    }

    /// Objects with a balance assist, and its settings.
    pub fn upright(&self) -> Vec<(u64, Balance)> {
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

    /// Is a balanced object's vehicle on the ground (a wheel touching
    /// something), and the slope of the ground under its wheels (radians,
    /// same sense as object angles), if both wheels have ground under them.
    fn ground_under(&self, world: &World, mats: &Materials, id: u64) -> (bool, Option<f32>) {
        let mut grounded = false;
        let mut feet: Vec<(f32, f32)> = Vec::new();
        for j in &self.joints {
            if j.spec.a != id || !matches!(j.spec.kind, JointKind::Wheel { .. }) {
                continue;
            }
            let Some(n) = self.find(j.spec.b) else { continue };
            let b = &self.bodies[n];
            grounded |= Self::touching(b, world, mats).0;
            let (cx, cy, _) = self.pose(b);
            let r = b.w.max(b.h) as f32 / 2.0;
            // The ground straight below the wheel (within a little).
            let x = cx.round() as i32;
            let (y0, y1) = ((cy + r * 0.5) as i32, (cy + r + 10.0) as i32);
            for y in y0..=y1 {
                if !world.in_bounds(x, y) {
                    break;
                }
                let i = world.idx(x as usize, y as usize);
                if !world.is_owned(i) && matches!(mats.get(world.mat[i]).state, State::Solid | State::Powder) {
                    feet.push((cx, y as f32));
                    break;
                }
            }
        }
        let slope = (feet.len() == 2).then(|| {
            let (a, b) = if feet[0].0 <= feet[1].0 { (feet[0], feet[1]) } else { (feet[1], feet[0]) };
            (b.1 - a.1).atan2((b.0 - a.0).max(1.0))
        });
        (grounded, slope)
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

    /// Drive wheels fling the loose cells their tread moves against.
    fn kick_up(&mut self, world: &mut World, mats: &Materials) {
        let pairs: Vec<(u64, u64)> = self
            .joints
            .iter()
            .filter(|j| matches!(j.spec.kind, JointKind::Wheel { .. }) && j.spec.motor.as_ref().is_some_and(|m| m.drive))
            .map(|j| (j.spec.a, j.spec.b))
            .collect();
        // Only the rearmost driven tyre of each vehicle: a front tyre's ruts
        // would trip the tyre behind it (like hitting a kerb at speed).
        let mut wheels = Vec::new();
        for &(body, id) in &pairs {
            let Some(me) = self.centre(id) else { continue };
            let Some(n) = self.find(id) else { continue };
            let dir = self.set[self.bodies[n].handle].linvel().x.signum();
            let behind_me = pairs.iter().any(|&(other_body, other)| {
                other != id && (other_body == body || self.assembly_of(body).contains(&other)) && self.centre(other).is_some_and(|o| (o.0 - me.0) * dir < -1.0)
            });
            if !behind_me {
                wheels.push(id);
            }
        }
        for id in wheels {
            let Some(n) = self.find(id) else { continue };
            let b = &self.bodies[n];
            let rb = &self.set[b.handle];
            let (c, v, spin) = (rb.translation(), rb.linvel(), rb.angvel());
            // Only while actually moving: a wheel spinning in place would
            // otherwise dig itself a pit it can never climb out of.
            if v.x.abs() < 1.5 {
                continue;
            }
            // The ground just behind the tyre: in each column from under the
            // axle back to the tyre's rear edge, the top grain within a few
            // cells below the tyre. (Colliders are smoothed, so the tyre's
            // own cells don't always quite touch the sand.)
            let radius = (b.w.max(b.h) as f32) / 2.0;
            let (cx, cy) = (c.x / CELL, c.y / CELL);
            let dir = v.x.signum();
            let ground_speed = v.x.abs();
            let mut thrown = 0;
            // (Not from right under the tyre: that is what it rolls on.)
            for k in (radius * 0.7).ceil() as i32..=(radius.ceil() as i32 + 2) {
                if thrown >= KICK_MAX {
                    break;
                }
                let x = (cx - dir * k as f32).floor() as i32;
                let Some(y) = (cy as i32..cy as i32 + radius as i32 + 4).find(|&y| {
                    world.in_bounds(x, y) && {
                        let i = world.idx(x as usize, y as usize);
                        !world.is_owned(i) && mats.get(world.mat[i]).state != State::Gas
                    }
                }) else {
                    continue;
                };
                let j = world.idx(x as usize, y as usize);
                let m = mats.get(world.mat[j]);
                if !(matches!(m.state, State::Powder | State::Liquid) || m.tyres_dig) {
                    continue;
                }
                let (material, clump) = (world.mat[j], m.clump.max(1) as usize);
                // How much: harder on the gas and faster means more, plus
                // any wheelspin (the tread scrubbing past the ground).
                let (px, py) = ((x as f32 + 0.5) * CELL - c.x, (y as f32 + 0.5) * CELL - c.y);
                let (tx, ty) = (v.x - spin * py, v.y + spin * px);
                let scrub = (tx * tx + ty * ty).sqrt();
                let chance = (self.throttle.abs() * (ground_speed / 10.0).min(1.0) * 0.8 + ((scrub - KICK_SPEED).max(0.0) / 12.0)).min(0.6);
                let roll = (hash(j as u32 ^ self.tick.wrapping_mul(0x9E37_79B9)) % 1000) as f32 / 1000.0;
                if roll > chance {
                    continue;
                }
                // Back and up, faster at speed, a little random.
                let jitter = 0.6 + (hash(j as u32 ^ 0x51ED ^ self.tick) % 800) as f32 / 1000.0;
                let per_frame = 1.0 / CELL / 60.0;
                let back = -dir * (ground_speed * 0.5 + 1.5) * jitter;
                let up = -(ground_speed * 0.35 + 1.5) * (1.6 - jitter * 0.6);
                // Some grains stick to the tread and come off over the top of
                // the tyre (up between it and the mudguard), moving on with
                // the bike, then dropping back behind it.
                let carried = hash(j as u32 ^ 0xC0FFEE ^ self.tick) % 4 == 0;
                if carried {
                    let a = 0.15 + (hash(j as u32 ^ 0xA11 ^ self.tick) % 1000) as f32 / 1000.0 * 0.45; // radians before straight up, towards the back
                    let r = radius + 1.2;
                    let (fx, fy) = (cx - dir * r * a.sin(), cy - r * a.cos());
                    let (cvx, cvy) = (v.x * 0.9 - dir * 0.6, v.y - (1.5 + ground_speed * 0.15) * jitter);
                    if world.throw_from(mats, x as usize, y as usize, fx, fy, cvx * per_frame, cvy * per_frame) {
                        thrown += 1;
                        Self::clump_with(world, mats, x, y, material, clump, Some((fx, fy)), (cvx * per_frame, cvy * per_frame));
                        continue;
                    }
                }
                if world.throw(mats, x as usize, y as usize, back * per_frame, up * per_frame) {
                    thrown += 1;
                    Self::clump_with(world, mats, x, y, material, clump, None, (back * per_frame, up * per_frame));
                }
            }
        }
    }

    /// Sticky stuff (wet sand, dirt) comes off in clumps: throw up to
    /// `clump - 1` more touching cells of the same material with the same
    /// speed, so they fly together. `from`: where the first one started (if
    /// it was carried over the tyre), so the rest start next to it.
    #[allow(clippy::too_many_arguments)]
    fn clump_with(world: &mut World, mats: &Materials, x: i32, y: i32, material: MatId, clump: usize, from: Option<(f32, f32)>, v: (f32, f32)) {
        let mut more = clump.saturating_sub(1);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (1, 1), (-1, 1), (0, 2)] {
            if more == 0 {
                break;
            }
            let (nx, ny) = (x + dx, y + dy);
            if !world.in_bounds(nx, ny) {
                continue;
            }
            let i = world.idx(nx as usize, ny as usize);
            if world.mat[i] != material || world.is_owned(i) {
                continue;
            }
            let ok = match from {
                Some((fx, fy)) => world.throw_from(mats, nx as usize, ny as usize, fx + dx as f32, fy + dy as f32, v.0, v.1),
                None => world.throw(mats, nx as usize, ny as usize, v.0, v.1),
            };
            if ok {
                more -= 1;
            }
        }
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
    pub(super) fn vehicles(&mut self, world: &mut World, mats: &Materials) {
        self.kick_up(world, mats);
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
        // Balance assists (with lean), and heavier vehicles.
        let lean = self.lean;
        // Trials riders shift their weight over a moment, not instantly.
        self.lean_now += (self.lean - self.lean_now) * TRIALS_LEAN_EASE;
        let eased = self.lean_now;
        let mut heavy: std::collections::HashMap<u64, f32> = Default::default();
        for (id, bal) in self.upright() {
            let Some(n) = self.find(id) else {
                self.upright.remove(&id);
                continue;
            };
            if (bal.heavy - 1.0).abs() > 1e-3 {
                for part in self.assembly_of(id) {
                    heavy.insert(part, bal.heavy);
                }
            }
            let rb = &self.set[self.bodies[n].handle];
            let angle = rb.rotation().angle();
            let spin = rb.angvel();
            let k = bal.strength / 60.0;
            if bal.air >= 0.999 {
                // Arcade: always levels out.
                let push = (-angle * 60.0 * k - spin * 15.0 * k + lean * 28.0 * k).clamp(-100.0 * k, 100.0 * k);
                self.steer_rotation(id, push, BALANCE_MAX_SPIN);
                continue;
            }
            // Trials: on the ground, follow the slope under the wheels and
            // lean to shift weight; in the air, lean spins it.
            let (grounded, slope) = self.ground_under(world, mats, id);
            let push = if grounded {
                let target = slope.unwrap_or(angle);
                let off = (angle - target + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                -off * TRIALS_GROUND_LEVEL * k - spin * 18.0 * k + eased * TRIALS_GROUND_LEAN * k
            } else {
                // Let go of lean and it stops turning; hold it and it turns steadily.
                let hold = TRIALS_AIR_FREE + (TRIALS_AIR_HOLD - TRIALS_AIR_FREE) * (1.0 - eased.abs());
                -angle * 60.0 * bal.air * k - spin * hold * k + eased * TRIALS_AIR_LEAN * k
            };
            self.steer_rotation(id, push.clamp(-140.0 * k, 140.0 * k), TRIALS_MAX_SPIN);
        }
        // Gravity scale: heavy vehicles' parts (and back to normal for parts
        // that broke off).
        for b in &self.bodies {
            let want = heavy.get(&b.id).copied().unwrap_or(1.0);
            let rb = &mut self.set[b.handle];
            if (rb.gravity_scale() - want).abs() > 1e-4 {
                rb.set_gravity_scale(want, true);
            }
        }
    }
}
