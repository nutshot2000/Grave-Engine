//! Characters ("actors"). For now there's the player; later, enemies and
//! other creatures use the same thing with a brain instead of a keyboard.
//!
//! An actor is an upright figure made of cells, so it really is *in* the
//! world: sand piles up on its head, water is pushed aside, and it warms up
//! next to fire through the normal heat rules. It walks, jumps and swims.
//! Its health goes down when its body gets too hot or too cold, when it runs
//! out of breath under water, when it lands (or gets hit) too hard, and when
//! something blows up near it. When it dies it comes back at its start.

use super::*;
use crate::look::{Look, MASK_W};
use crate::world::Blast;
use std::sync::Arc;

/// The figure's size in cells (0.6 m wide, 1.6 m tall).
pub const W: usize = 6;
pub const H: usize = 16;
/// Walking speed and how quickly it gets there (m/s, m/s²).
const WALK: f32 = 4.0;
const ACCEL_GROUND: f32 = 40.0;
const ACCEL_AIR: f32 = 14.0;
/// Take-off speed of a jump (m/s): about 1.5 m high.
const JUMP: f32 = 5.5;
/// Swimming upwards (m/s).
const SWIM_UP: f32 = 2.5;
/// Clawing up out of sand (m/s).
const CLAW_UP: f32 = 1.5;
/// A jump still works this many frames after walking off an edge.
const COYOTE: u32 = 6;
pub const MAX_HEALTH: f32 = 100.0;
/// Seconds of breath.
pub const MAX_BREATH: f32 = 10.0;
/// The body keeps itself at this temperature, and gets hurt outside HOT..COLD.
const BODY_TEMP: f32 = 37.0;
const HOT: f32 = 45.0;
const COLD: f32 = 5.0;
/// Slowing down faster than this in one frame hurts (m/s): roughly a fall of 4 m.
const SAFE_LANDING: f32 = 9.0;
/// Frames before a dead actor comes back.
const RESPAWN_FRAMES: u32 = 120;

/// What the player is pressing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls {
    /// -1 = left, 1 = right.
    pub walk: f32,
    /// Jump (or swim up) held.
    pub jump: bool,
}

/// An actor's state: enough to save it, rewind it and put it back.
#[derive(Clone, Debug, PartialEq)]
pub struct ActorSpec {
    /// Runtime graph state is included in snapshots and rewind.
    pub animation: Option<crate::animation::Animator>,
    /// Centre of the figure (cells) and velocity (cells per second).
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    /// What it's made of (heat, weight, how it's drawn).
    pub mat: MatId,
    pub health: f32,
    /// Seconds of breath left.
    pub breath: f32,
    /// Body temperature (°C).
    pub temp: f32,
    /// Where it comes back after dying (centre, cells).
    pub spawn: (f32, f32),
    /// Frames since it died (0 = alive).
    pub dead_for: u32,
    /// Which way it's looking (-1 left, 1 right).
    pub facing: f32,
    /// How it's drawn: animated pictures (see `look.rs`), or None for the
    /// plain cell figure.
    pub look: Option<Arc<Look>>,
}

impl ActorSpec {
    /// A fresh actor standing with its feet at (x, y).
    pub fn standing_at(mat: MatId, x: f32, feet: f32) -> Self {
        let y = feet - H as f32 / 2.0;
        ActorSpec {
            animation: None,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            mat,
            health: MAX_HEALTH,
            breath: MAX_BREATH,
            temp: BODY_TEMP,
            spawn: (x, y),
            dead_for: 0,
            facing: 1.0,
            look: None,
        }
    }

    /// Where its feet are (cells).
    pub fn feet(&self) -> (f32, f32) {
        (self.x, self.y + H as f32 / 2.0)
    }
}

/// What the HUD needs to show.
#[derive(Clone, Debug)]
pub struct ActorStatus {
    pub x: f32,
    pub y: f32,
    pub health: f32,
    pub breath: f32,
    pub temp: f32,
    pub dead: bool,
    pub grounded: bool,
    pub swimming: bool,
    /// What hurt it last, while that's recent.
    pub hurt_by: Option<&'static str>,
    /// Which way it faces (-1 left, 1 right), how it's drawn, and which
    /// animation is playing (and for how long, in seconds).
    pub facing: f32,
    pub look: Option<Arc<Look>>,
    pub anim: String,
    pub animation_state: String,
    pub anim_time: f32,
}

pub(super) struct Actor {
    spec: ActorSpec,
    handle: RigidBodyHandle,
    collider: ColliderHandle,
    /// (world cell, figure cell) written by the last stamp.
    stamped: Vec<(u32, u16)>,
    /// Temperature of each cell of the figure (row by row, W x H): a foot in
    /// lava gets hot long before the rest of the body does.
    temps: Vec<f32>,
    /// Is air touching the head (checked while the figure is in the world)?
    /// And is it water that's there instead?
    head_air: bool,
    head_wet: bool,
    /// Frames since it last stood on something.
    since_ground: u32,
    /// How much of it is in liquid (0..1), and the liquid's density.
    wet: f32,
    liquid_density: f32,
    jump_wait: u32,
    /// Velocity just before the physics step (to measure knocks).
    v_before: Vector,
    /// Frames spent walking (for the leg animation).
    stride: u32,
    hurt: Option<(&'static str, u32)>,
    /// Markers waiting for a game to consume them.
    animation_events: Vec<crate::animation::Event>,
    /// Flung by a bounce pad and not landed yet (that landing doesn't hurt).
    launched: bool,
    /// Hurt this frame, and by what.
    pending: f32,
    pending_cause: Option<&'static str>,
}

/// The figure, row by row: head, neck, body, legs (two leg poses for walking).
fn figure(stride: u32) -> [[bool; W]; H] {
    const STAND: [&str; H] = [
        ".####.", "######", "######", "######", ".####.", "..##..", "######", "######", "######", "######", "######", "######",
        ".#..#.", ".#..#.", ".#..#.", ".#..#.",
    ];
    let mut out = [[false; W]; H];
    for (y, row) in STAND.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            out[y][x] = c == '#';
        }
    }
    if (stride / 8) % 2 == 1 {
        // Mid-stride: legs apart.
        for y in 14..H {
            out[y] = [true, false, false, false, false, true];
        }
    }
    out
}

impl Physics {
    /// Add a character. Returns its number.
    pub fn add_actor(&mut self, world: &mut World, mats: &Materials, mut spec: ActorSpec) -> usize {
        if spec.animation.is_none() {
            let graph = spec.look.as_ref().map_or_else(|| Arc::new(crate::animation::Graph::parse(crate::animation::DEFAULT).expect("default graph")), |l| l.graph.clone());
            spec.animation = Some(crate::animation::Animator::new(graph).expect("validated look graph"));
        }
        let radius = W as f32 / 2.0 * CELL;
        let half = H as f32 / 2.0 * CELL - radius;
        let rb = RigidBodyBuilder::dynamic()
            .translation(Vector::new(spec.x * CELL, spec.y * CELL))
            .linvel(Vector::new(spec.vx * CELL, spec.vy * CELL))
            .lock_rotations()
            .ccd_enabled(true);
        let handle = self.set.insert(rb);
        // No friction: walking is done by setting the speed, so it doesn't
        // stick to walls when jumping against them.
        let col = ColliderBuilder::capsule_y(half, radius)
            .user_data(ACTOR_TAG)
            .active_hooks(ActiveHooks::MODIFY_SOLVER_CONTACTS)
            .friction(0.0)
            .friction_combine_rule(CoefficientCombineRule::Min)
            .restitution(0.0)
            .density(mats.get(spec.mat).density.max(50.0));
        let collider = self.colliders.insert_with_parent(col, handle, &mut self.set);
        let mut a = Actor {
            temps: vec![spec.temp; MASK_W * H],
            head_air: true,
            head_wet: false,
            spec,
            handle,
            collider,
            stamped: Vec::new(),
            since_ground: COYOTE,
            wet: 0.0,
            liquid_density: 1000.0,
            jump_wait: 0,
            v_before: Vector::new(0.0, 0.0),
            stride: 0,
            hurt: None,
            animation_events: Vec::new(),
            launched: false,
            pending: 0.0,
            pending_cause: None,
        };
        Self::stamp_actor(&mut a, &self.set, world, mats);
        self.actors.push(a);
        self.actors.len() - 1
    }

    pub fn actor_count(&self) -> usize {
        self.actors.len()
    }

    /// What the player is pressing (all actors follow it, for now).
    pub fn set_controls(&mut self, c: Controls) {
        self.controls = c;
    }

    pub fn controls(&self) -> Controls {
        self.controls
    }

    /// Install a validated graph on one actor, leaving other entities independent.
    pub fn set_actor_graph(&mut self, index: usize, graph: Arc<crate::animation::Graph>) -> Result<(), String> {
        let a = self.actors.get_mut(index).ok_or("no such actor")?;
        if let Some(look) = &a.spec.look {
            for s in &graph.states { if !look.anims.contains_key(&s.clip) { return Err(format!("missing clip `{}`", s.clip)); } }
        }
        a.spec.animation = Some(crate::animation::Animator::new(graph)?);
        a.animation_events.clear();
        Ok(())
    }

    /// Set a custom game parameter (built-in movement parameters update each tick).
    pub fn set_actor_animation_parameter(&mut self, index: usize, name: &str, value: f32) -> bool {
        self.actors.get_mut(index).and_then(|a| a.spec.animation.as_mut()).is_some_and(|a| a.set(name, value))
    }

    /// Request a one-tick action, such as attack or reload, on one actor.
    pub fn trigger_actor_animation(&mut self, index: usize, name: &str) -> bool {
        self.actors.get_mut(index).and_then(|a| a.spec.animation.as_mut()).is_some_and(|a| a.trigger(name))
    }

    /// Drain markers since the last call (for sounds or gameplay). Bounded to 256.
    pub fn take_actor_animation_events(&mut self, index: usize) -> Vec<crate::animation::Event> {
        self.actors.get_mut(index).map_or_else(Vec::new, |a| std::mem::take(&mut a.animation_events))
    }

    fn advance_animation(a: &mut Actor, bodies: &RigidBodySet) {
        let v = bodies[a.handle].linvel();
        let anim = a.spec.animation.as_mut().unwrap();
        for (p, value) in [
            ("dead", (a.spec.dead_for > 0) as u8 as f32),
            ("swimming", (a.wet > 0.4) as u8 as f32),
            // Immediate takeoff for real jumps; ignore tiny contact corrections
            // from the animated collision outline while walking on a floor.
            ("airborne", (a.since_ground > 8 || (a.jump_wait > 0 && v.y < -0.2)) as u8 as f32),
            ("grounded", (a.since_ground == 0) as u8 as f32),
            ("speed", v.x.abs()), ("vy", v.y), ("hurt", a.hurt.is_some() as u8 as f32),
        ] { anim.set(p, value); }
        let look = &a.spec.look;
        let events = anim.advance(DT, |clip| look.as_ref().map_or(1.0, |l| l.duration(clip)));
        for event in events { if a.animation_events.len() < 256 { a.animation_events.push(event); } }
    }

    /// Every actor's current state (for saving and rewinding).
    pub fn export_actors(&self) -> Vec<ActorSpec> {
        self.actors
            .iter()
            .map(|a| {
                let rb = &self.set[a.handle];
                let (t, v) = (rb.translation(), rb.linvel());
                ActorSpec { x: t.x / CELL, y: t.y / CELL, vx: v.x / CELL, vy: v.y / CELL, ..a.spec.clone() }
            })
            .collect()
    }

    /// Health, breath and so on, for the HUD.
    pub fn actor_status(&self) -> Vec<ActorStatus> {
        self.actors
            .iter()
            .map(|a| {
                let t = self.set[a.handle].translation();
                ActorStatus {
                    x: t.x / CELL,
                    y: t.y / CELL,
                    health: a.spec.health,
                    breath: a.spec.breath,
                    temp: a.spec.temp,
                    dead: a.spec.dead_for > 0,
                    grounded: a.since_ground == 0,
                    swimming: a.wet > 0.4,
                    hurt_by: a.hurt.map(|h| h.0),
                    facing: a.spec.facing,
                    look: a.spec.look.clone(),
                    anim: a.spec.animation.as_ref().unwrap().state().clip.clone(),
                    animation_state: a.spec.animation.as_ref().unwrap().state().name.clone(),
                    anim_time: a.spec.animation.as_ref().unwrap().sample_time(|clip| a.spec.look.as_ref().map_or(1.0, |l| l.duration(clip))),
                }
            })
            .collect()
    }

    /// Take the actors' cells out of the grid, reading how warm they got.
    pub(super) fn actors_read_back(&mut self, world: &mut World, mats: &Materials) {
        for a in self.actors.iter_mut() {
            // Breathing: is there air (any gas) next to the head? Under
            // water, or buried in sand, there isn't.
            if !a.stamped.is_empty() {
                a.head_air = false;
                a.head_wet = false;
                let top_row = a.stamped.iter().map(|&(_, li)| li as usize / MASK_W).min().unwrap_or(0);
                for &(wi, li) in &a.stamped {
                    if li as usize / MASK_W >= top_row + 5 {
                        continue;
                    }
                    let wi = wi as usize;
                    let (x, y) = (wi % world.w, wi / world.w);
                    let near = [
                        (x > 0).then(|| wi - 1),
                        (x + 1 < world.w).then(|| wi + 1),
                        (y > 0).then(|| wi - world.w),
                        (y + 1 < world.h).then(|| wi + world.w),
                    ];
                    for j in near.into_iter().flatten() {
                        if world.is_owned(j) {
                            continue;
                        }
                        match mats.get(world.mat[j]).state {
                            State::Gas => a.head_air = true,
                            State::Liquid => a.head_wet = true,
                            _ => {}
                        }
                    }
                }
            }
            for &(wi, li) in &a.stamped {
                let wi = wi as usize;
                if world.is_owned(wi) && world.mat[wi] == a.spec.mat {
                    a.temps[li as usize] = world.temp[wi];
                    world.make_air(wi);
                }
            }
            // Blood evens the body out a little, and a living body warms or
            // cools itself back towards normal.
            // (Only the cells that are body right now count.)
            if a.stamped.is_empty() {
                continue;
            }
            let n = a.stamped.len() as f32;
            let avg = a.stamped.iter().map(|&(_, li)| a.temps[li as usize]).sum::<f32>() / n;
            let regulate = if a.spec.dead_for == 0 { 0.02 } else { 0.0 };
            for t in a.temps.iter_mut() {
                *t += (avg - *t) * 0.03 + (BODY_TEMP - *t) * regulate;
            }
            a.spec.temp = a.stamped.iter().map(|&(_, li)| a.temps[li as usize]).sum::<f32>() / n;
        }
    }

    /// Look around, get hurt, and move (before the physics step).
    pub(super) fn actors_think(&mut self, world: &World, mats: &Materials, blasts: &[Blast]) {
        if self.actors.is_empty() {
            return;
        }
        let blasts = merge_blasts(blasts);
        let controls = self.controls;
        for a in self.actors.iter_mut() {
            let rb = &self.set[a.handle];
            let pos = rb.translation();
            let (cx, cy) = (pos.x / CELL, pos.y / CELL);

            // Standing on something? (A contact pushing up from below.)
            let mut grounded = false;
            let mut wall = false;
            for pair in self.narrow.contact_pairs_with(a.collider) {
                for m in pair.manifolds() {
                    if !m.points.iter().any(|p| p.dist < 0.05) {
                        continue;
                    }
                    let n = if pair.collider1 == a.collider { m.data.normal } else { -m.data.normal };
                    if n.y > 0.6 {
                        grounded = true;
                    }
                    wall |= n.x.abs() > 0.6;
                }
            }
            a.since_ground = if grounded { 0 } else { a.since_ground.saturating_add(1) };

            // How deep in liquid, and is the head under?
            // (Rounded the same way as `stamp_actor`, so "just outside the
            // figure" really is outside it.)
            let left = (cx - W as f32 / 2.0).round() as i32;
            let top = (cy - H as f32 / 2.0).round() as i32;
            let is_liquid = |x: i32, y: i32| -> Option<f32> {
                if !world.in_bounds(x, y) {
                    return None;
                }
                let i = world.idx(x as usize, y as usize);
                let m = mats.get(world.mat[i]);
                (m.state == State::Liquid && !world.is_owned(i)).then_some(m.density)
            };
            let (mut wet_rows, mut dens) = (0, 0.0);
            for row in 0..H as i32 {
                let side = is_liquid(left - 1, top + row).or_else(|| is_liquid(left + W as i32, top + row));
                if let Some(d) = side {
                    wet_rows += 1;
                    dens += d;
                }
            }
            a.wet = wet_rows as f32 / H as f32;
            a.liquid_density = if wet_rows > 0 { dens / wet_rows as f32 } else { 1000.0 };
            let (head_under, head_wet) = (!a.head_air, a.head_wet);

            if let Some(h) = a.hurt.as_mut() {
                h.1 = h.1.saturating_sub(1);
                if h.1 == 0 {
                    a.hurt = None;
                }
            }

            // Dead: lie still until it's time to come back.
            let rb = &mut self.set[a.handle];
            if a.spec.dead_for > 0 {
                a.spec.dead_for += 1;
                if a.spec.dead_for >= RESPAWN_FRAMES {
                    let (sx, sy) = a.spec.spawn;
                    rb.set_translation(Vector::new(sx * CELL, sy * CELL), true);
                    rb.set_linvel(Vector::new(0.0, 0.0), true);
                    a.spec.health = MAX_HEALTH;
                    a.spec.breath = MAX_BREATH;
                    a.spec.temp = BODY_TEMP;
                    a.temps.fill(BODY_TEMP);
                    a.spec.animation.as_mut().unwrap().reset();
                    a.spec.dead_for = 0;
                    a.hurt = None;
                } else {
                    let v = rb.linvel();
                    rb.set_linvel(Vector::new(v.x * 0.9, v.y), true);
                }
                a.v_before = rb.linvel();
                continue;
            }

            // What hurts.
            let mut hurt = |amount: f32, why: &'static str| {
                if amount > 0.0 {
                    a.pending += amount;
                    a.pending_cause = Some(why);
                }
            };
            // Burns and frostbite: how far each part of the body is past
            // safe, averaged over the body.
            let n = a.stamped.len().max(1) as f32;
            let body = || a.stamped.iter().map(|&(_, li)| a.temps[li as usize]);
            let too_hot = body().map(|t| (t - HOT).max(0.0)).sum::<f32>() / n;
            let too_cold = body().map(|t| (COLD - t).max(0.0)).sum::<f32>() / n;
            hurt(too_hot * 3.0 * DT, "burned");
            hurt(too_cold * 2.0 * DT, "froze");
            if head_under {
                a.spec.breath -= DT;
                if a.spec.breath <= 0.0 {
                    a.spec.breath = 0.0;
                    hurt(15.0 * DT, if head_wet { "drowned" } else { "suffocated" });
                }
            } else {
                a.spec.breath = (a.spec.breath + 3.0 * DT).min(MAX_BREATH);
            }
            let mut knock = Vector::new(0.0, 0.0);
            for &(bx, by, r, power) in &blasts {
                let (dx, dy) = (cx - bx, cy - by);
                let d = (dx * dx + dy * dy).sqrt();
                let reach = r * 2.0;
                if d < reach {
                    let k = 1.0 - d / reach;
                    let strength = (power / 20_000.0).sqrt().min(2.0);
                    hurt(70.0 * k * k * strength, "blown up");
                    let (ux, uy) = if d > 0.01 { (dx / d, dy / d) } else { (0.0, -1.0) };
                    knock += Vector::new(ux, uy - 0.3) * (18.0 * k * strength);
                }
            }

            // Moving.
            let v = rb.linvel();
            let swimming = a.wet > 0.4;
            let walk = controls.walk.clamp(-1.0, 1.0);
            if walk != 0.0 {
                a.spec.facing = walk.signum();
                a.stride = a.stride.wrapping_add(1);
            } else {
                a.stride = 0;
            }
            // Buried in sand (no air at the head, and it isn't water): slow
            // going sideways, and jump claws upwards through it.
            let above = (left..left + W as i32).filter(|&x| {
                world.in_bounds(x, top - 1) && {
                    let i = world.idx(x as usize, (top - 1) as usize);
                    !world.is_owned(i) && mats.get(world.mat[i]).state == State::Powder
                }
            }).count();
            let buried = head_under && !head_wet && above >= 2;
            let top_speed = if buried { 1.0 } else if swimming { WALK * 0.5 } else { WALK };
            let accel = if a.since_ground == 0 { ACCEL_GROUND } else { ACCEL_AIR } * DT;
            let target = walk * top_speed;
            let mut vx = v.x + (target - v.x).clamp(-accel, accel);
            let mut vy = v.y;
            a.jump_wait = a.jump_wait.saturating_sub(1);
            if controls.jump && buried {
                vy += (-CLAW_UP - vy).clamp(-15.0 * DT, 15.0 * DT);
            } else if controls.jump {
                if a.since_ground < COYOTE && a.jump_wait == 0 && !swimming {
                    vy = -JUMP;
                    a.jump_wait = 15;
                    a.since_ground = COYOTE;
                } else if swimming && !head_under && (wall || grounded) && a.jump_wait == 0 {
                    // Head above water, against the bank: climb out.
                    vy = -JUMP;
                    a.jump_wait = 15;
                } else if swimming {
                    vy += (-SWIM_UP - vy).clamp(-20.0 * DT, 20.0 * DT);
                }
            }
            // Bounce pads: standing or landing on something with `launch`.
            let feet_row = top + H as i32;
            let mut pad = 0.0f32;
            for y in [feet_row, feet_row - 1] {
                for x in left..left + W as i32 {
                    if world.in_bounds(x, y) {
                        let i = world.idx(x as usize, y as usize);
                        if !world.is_owned(i) {
                            pad = pad.max(mats.get(world.mat[i]).launch);
                        }
                    }
                }
            }
            if pad > 0.0 && vy > -pad * 0.5 {
                vy = -pad;
                a.since_ground = COYOTE;
                a.jump_wait = 15;
                a.launched = true;
            } else if (a.since_ground == 0 || swimming) && pad == 0.0 && a.jump_wait == 0 {
                // (jump_wait: not in the first moments, while still leaving the pad)
                a.launched = false; // landed (or splashed down) after the flight
            }
            if a.wet > 0.0 {
                // Floating: the liquid holds up the part of it that's under.
                let body = mats.get(a.spec.mat).density.max(50.0);
                vy -= GRAVITY * DT * a.wet * (a.liquid_density / body);
                vx *= 1.0 - 0.04 * a.wet;
                vy *= 1.0 - 0.06 * a.wet;
            }
            let v = Vector::new(vx, vy) + knock;
            rb.set_linvel(v, true);
            a.v_before = v;
        }
    }

    /// After the physics step: hard knocks hurt; death; put the figures
    /// back into the grid.
    pub(super) fn actors_after(&mut self, world: &mut World, mats: &Materials) {
        for a in self.actors.iter_mut() {
            let v = self.set[a.handle].linvel();
            if a.spec.dead_for == 0 {
                // Slowing down (or being shoved) hard in one frame: a hard
                // landing or getting hit by something heavy. Gravity alone
                // only changes speed by 0.16 m/s a frame.
                let dv = (v - a.v_before).length();
                if a.launched && dv > SAFE_LANDING {
                    // The landing after a bounce pad: games don't punish that.
                    a.launched = false;
                } else if dv > SAFE_LANDING {
                    a.pending += (dv - SAFE_LANDING) * 10.0;
                    a.pending_cause = Some("hit too hard");
                }
                if a.pending > 0.0 {
                    a.spec.health -= a.pending;
                    a.hurt = Some((a.pending_cause.unwrap_or("hurt"), 90));
                    if a.spec.health <= 0.0 {
                        a.spec.health = 0.0;
                        a.spec.dead_for = 1;
                    }
                }
            }
            a.pending = 0.0;
            a.pending_cause = None;
            Self::advance_animation(a, &self.set);
            Self::stamp_actor(a, &self.set, world, mats);
        }
    }

    /// Draw an actor into the grid as cells of its material: the outline of
    /// its current animation frame, or the plain figure.
    fn stamp_actor(a: &mut Actor, set: &RigidBodySet, world: &mut World, mats: &Materials) {
        a.stamped.clear();
        let t = set[a.handle].translation();
        let (cx, cy) = (t.x / CELL, t.y / CELL);
        let left = (cx - MASK_W as f32 / 2.0).round() as i32;
        let top = (cy - H as f32 / 2.0).round() as i32;
        let (mask, mirror, hidden) = match &a.spec.look {
            Some(look) => {
                let animator = a.spec.animation.as_ref().unwrap();
                let (frame, _) = look.frame(&animator.state().clip, animator.sample_time(|c| look.duration(c)));
                (frame.mask.clone(), (a.spec.facing < 0.0) == look.faces_right, crate::world::HIDDEN)
            }
            None => {
                let shape = figure(if a.since_ground == 0 { a.stride } else { 8 });
                let mut m = vec![false; MASK_W * H];
                let off = (MASK_W - W) / 2;
                for (row, cells) in shape.iter().enumerate() {
                    for (col, &on) in cells.iter().enumerate() {
                        m[row * MASK_W + off + col] = on;
                    }
                }
                (m, a.spec.facing < 0.0, 0)
            }
        };
        for row in 0..H {
            for col in 0..MASK_W {
                if !mask[row * MASK_W + col] {
                    continue;
                }
                // Mirror the figure when it faces the other way.
                let col_w = if mirror { MASK_W - 1 - col } else { col };
                let (wx, wy) = (left + col_w as i32, top + row as i32);
                if !world.in_bounds(wx, wy) {
                    continue;
                }
                let wi = world.idx(wx as usize, wy as usize);
                if world.is_owned(wi) {
                    continue;
                }
                let under = mats.get(world.mat[wi]);
                match under.state {
                    _ if under.crushable => {}
                    State::Solid => continue,
                    // People shove sand and water aside (objects don't).
                    State::Liquid => {
                        push_liquid_up(world, mats, wx, wy);
                    }
                    // Sand goes underneath first: climbing up through a
                    // pile leaves it behind, under the feet.
                    State::Powder => {
                        push_aside(world, mats, wx, wy, true);
                    }
                    State::Gas => {}
                }
                world.wake_at(wi);
                world.mat[wi] = a.spec.mat;
                let li = row * MASK_W + col;
                world.temp[wi] = a.temps[li];
                world.burn[wi] = 0;
                world.life[wi] = 0;
                // With a look, the pictures are drawn instead of these cells.
                world.flags[wi] = OWNED | hidden;
                // Head, body and legs in slightly different shades.
                world.shade[wi] = match row {
                    0..=4 => 200,
                    5..=11 => 60,
                    _ => 110,
                };
                world.tex[wi] = crate::world::pack_tex(col, row);
                a.stamped.push((wi as u32, li as u16));
            }
        }
    }
}

/// Blasts close together are one bigger blast: (x, y, radius, power).
fn merge_blasts(blasts: &[Blast]) -> Vec<(f32, f32, f32, f32)> {
    let mut groups: std::collections::BTreeMap<(i32, i32), (f32, f32, f32, f32, f32)> = Default::default();
    for b in blasts {
        let e = groups.entry(((b.x / 8.0) as i32, (b.y / 8.0) as i32)).or_insert((0.0, 0.0, 0.0, 0.0, 0.0));
        e.0 += b.x;
        e.1 += b.y;
        e.2 += 1.0;
        e.3 = e.3.max(b.size);
        e.4 = e.4.max(b.power);
    }
    groups
        .into_values()
        .map(|(sx, sy, n, size, power)| (sx / n, sy / n, (size * (1.0 + 0.35 * n.ln())).min(48.0), power * (1.0 + n.ln())))
        .collect()
}
