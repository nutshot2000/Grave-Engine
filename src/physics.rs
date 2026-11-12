//! Rigid objects: groups of cells that move together as one solid body.
//!
//! Objects live in the cell grid like everything else (marked `OWNED`), so they
//! still heat up, burn, melt and freeze by the normal rules. Each frame:
//! 1. **Read back**: copy each object's cells out of the grid. Cells that have
//!    changed (burned to ash, melted, erased) leave the object. If that splits
//!    the object into separate pieces, each piece becomes its own object.
//! 2. **Support**: if solid scenery lost cells (burned, melted, erased), any
//!    piece that is no longer connected to the edge of the world (the ground)
//!    breaks loose and becomes an object, so it falls. Overhangs that stick out
//!    further than their material's `strength` snap off too.
//! 3. **Terrain**: solid and powder cells that aren't objects become static
//!    collision boxes (rebuilt per 32x32 chunk only when they change).
//! 4. **Physics**: Rapier moves the objects (gravity, collisions, floating).
//! 5. **Stamp**: write each object's cells back into the grid at its new
//!    position and angle, pushing any liquid out of the way.
//! 6. **Damage**: hard hits (and heavy loads) crack materials whose
//!    `toughness` they beat, and explosions crack, fling and burn things.
//!    Cracked cells turn into `breaks_into` (stone -> gravel) or splinter
//!    into small loose pieces.
//!
//! Units: one cell is 10 cm, so a 10-cell crate is a metre wide.

use crate::material::{MatId, Materials, State};
use crate::world::{OWNED, World};
use rapier2d::prelude::*;
use std::collections::HashMap;

/// Metres per cell.
pub const CELL: f32 = 0.1;
const CHUNK: usize = 32;

static NEXT_RUN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Collider tags (in `user_data`): ground made of powder, and characters.
const POWDER_TAG: u128 = 1;
const ACTOR_TAG: u128 = 2;

/// Loose powder (sand, gravel) only holds characters up from below, like a
/// one-way platform: you stand on a dune, but sand piled on or around you
/// can't trap you or (when it overlaps you) fling you out.
struct OneWayPowder { assemblies: HashMap<RigidBodyHandle, u64> }

impl PhysicsHooks for OneWayPowder {
    fn filter_contact_pair(&self, ctx: &PairFilterContext) -> Option<SolverFlags> {
        if let (Some(a), Some(b)) = (ctx.rigid_body1, ctx.rigid_body2)
            && self.assemblies.get(&a).is_some_and(|root| self.assemblies.get(&b) == Some(root)) { return None; }
        Some(SolverFlags::COMPUTE_RIGID_IMPULSES)
    }

    fn modify_solver_contacts(&self, ctx: &mut ContactModificationContext) {
        let (c1, c2) = (&ctx.colliders[ctx.collider1], &ctx.colliders[ctx.collider2]);
        let (actor_first, other) = if c1.user_data == ACTOR_TAG {
            (true, c2)
        } else if c2.user_data == ACTOR_TAG {
            (false, c1)
        } else {
            return;
        };
        if other.user_data != POWDER_TAG {
            return;
        }
        if let Some(m) = ctx.rigid_mut() {
            // The normal points from collider 1 to collider 2: make it point
            // from the character to the powder. Keep the contact only if the
            // powder is underneath (y grows downwards).
            let n = if actor_first { *m.normal } else { -*m.normal };
            if n.y < 0.5 {
                m.solver_contacts.clear();
            }
        }
    }
}

mod actor;
mod bend;
mod vehicle;
pub use actor::{ActorSpec, ActorStatus, Controls};
pub use vehicle::{Balance, WheelContact};

/// How tall a character's body is, in cells.
pub fn actor_height() -> usize {
    actor::H
}
const GRAVITY: f32 = 9.81;
const DT: f32 = 1.0 / 60.0;
/// How eagerly motors chase their target speed (higher = snappier).
const MOTOR_RESPONSE: f32 = 10.0;
/// How far (in cells) a straightened outline may stray from the pixel edge.
const SMOOTHING: f32 = 0.9;
/// Contacts softer than this (kN) are never checked for damage.
const MIN_DAMAGE_KN: f32 = 50.0;
/// At most this many hard contacts crack things per frame.
const MAX_HITS_PER_TICK: usize = 24;
/// New pieces ignore hits for this many frames while they settle.
const SETTLE_TICKS: u32 = 8;
/// Pieces that just broke off can't be broken again for this many frames.
const FRESH_TICKS: u32 = 20;
/// Hits break things into chunks this many times bigger than blasts do
/// (a plank snaps into a few pieces rather than crumbling).
const IMPACT_CHUNKS: f32 = 2.5;
/// Broken bits smaller than this (cells) are glued onto a neighbouring piece.
const MIN_PIECE: usize = 6;
/// Above this many objects, new splinters turn to dust instead.
const MAX_OBJECTS: usize = 1500;
/// How hard explosions shove objects (newton-seconds per kN per sqrt(cell)).
const BLAST_PUSH: f32 = 0.012;
/// Objects faster than this (cells per frame; 0.6 = 3.6 m/s) splash liquid.
const SPLASH_SPEED: f32 = 0.6;
/// The most speed a blast can give an object (metres per second).
const BLAST_MAX_SPEED: f32 = 35.0;

/// The ground (fixed scenery), when used as one end of a joint.
pub const GROUND: u64 = 0;

/// How two things are joined.
#[derive(Clone, Debug, PartialEq)]
pub enum JointKind {
    /// A pin both can turn around (door, pendulum, seesaw).
    Hinge,
    /// An axle with a suspension spring: the second object turns freely and
    /// can move up and down (in the first object's frame) against a spring.
    /// `stiffness` in kN per metre, `damping` in kN per (m/s), `travel` in cells.
    Wheel { stiffness: f32, damping: f32, travel: f32 },
    /// Glued rigidly together.
    Weld,
    /// Can't get further apart than `length` cells.
    Rope { length: f32 },
}

/// A motor on a hinge or wheel.
#[derive(Clone, Debug, PartialEq)]
pub struct Motor {
    /// Target turning speed, in turns per second (+ = clockwise on screen).
    pub speed: f32,
    /// How strong it is (kN·m of twist).
    pub strength: f32,
    /// Controlled by the player (arrow keys / A and D) instead of always on.
    pub drive: bool,
}

/// A joint between two objects (or an object and the ground, `GROUND`).
#[derive(Clone, Debug, PartialEq)]
pub struct JointSpec {
    pub a: u64,
    pub b: u64,
    pub kind: JointKind,
    /// Where the joint is on each, in cells from that object's centre (in its
    /// own unrotated frame; for the ground, in world cells).
    pub anchor_a: (f32, f32),
    pub anchor_b: (f32, f32),
    /// Optional joint failure load in kN. None keeps the joint unbreakable.
    pub break_force: Option<f32>,
    pub motor: Option<Motor>,
}

struct Joint {
    spec: JointSpec,
    handle: ImpulseJointHandle,
}

/// One cell of an object, with all its simulation state.
#[derive(Clone, Copy, Debug)]
pub struct BodyCell {
    pub mat: MatId,
    pub temp: f32,
    pub burn: u16,
    pub life: u16,
    pub flags: u8,
    pub shade: u8,
    pub paint: u32,
}

/// An object's shape and where it is. Used to create objects, save them and
/// restore them (for Reset).
#[derive(Clone, Debug)]
pub struct BodySpec {
    /// Persistent editable asset instance, shared across depth planes.
    pub assembly: Option<String>,
    /// Physical depth in metres, perpendicular to the 2D world.
    pub depth: f32,
    pub w: usize,
    pub h: usize,
    /// `w * h` cells, row by row, `None` = empty.
    pub cells: Vec<Option<BodyCell>>,
    /// World position of the shape's centre, in cells.
    pub x: f32,
    pub y: f32,
    /// Rotation in radians (clockwise on screen).
    pub angle: f32,
    /// Velocity in cells per second, and spin in radians per second.
    pub vx: f32,
    pub vy: f32,
    pub spin: f32,
}

impl BodySpec {
    /// Build a shape from material ids, at room/normal temperature.
    pub fn from_mats(w: usize, h: usize, mats_grid: &[Option<MatId>], mats: &Materials, cx: f32, cy: f32) -> Self {
        let cells = mats_grid
            .iter()
            .enumerate()
            .map(|(i, m)| {
                m.map(|mat| BodyCell {
                    mat,
                    temp: mats.get(mat).temperature,
                    burn: 0,
                    life: 0,
                    flags: 0,
                    paint: 0,
                    shade: (hash(i as u32 ^ (cx as u32).wrapping_mul(7919) ^ (cy as u32).wrapping_mul(104729)) >> 24) as u8,
                })
            })
            .collect();
        BodySpec { assembly: None, depth: 1.0, w, h, cells, x: cx, y: cy, angle: 0.0, vx: 0.0, vy: 0.0, spin: 0.0 }
    }

    pub fn cell_count(&self) -> usize {
        self.cells.iter().filter(|c| c.is_some()).count()
    }
}

fn hash(a: u32) -> u32 {
    let mut x = a.wrapping_mul(0x9E37_79B1);
    x ^= x >> 15;
    x = x.wrapping_mul(0x85EB_CA77);
    x ^= x >> 13;
    x
}

struct Body {
    /// Occupied local rectangle, refreshed whenever the shape changes.
    bounds: (usize,usize,usize,usize),
    assembly: Option<String>,
    depth: f32,
    w: usize,
    h: usize,
    cells: Vec<Option<BodyCell>>,
    handle: RigidBodyHandle,
    /// (world cell index, local cell index) written by the last stamp.
    stamped: Vec<(u32, u32)>,
    /// The physics tick this object appeared (brand-new pieces don't crack
    /// things while they settle).
    born: u32,
    /// Cells were cut out of it directly (splinters): rebuild or split it.
    dirty: bool,
    /// It's a piece that just broke off something (briefly unbreakable).
    shard: bool,
    /// Rolls on a perfectly round collision shape (wheels).
    round: bool,
    /// Stable name for the editor (survives moves and edits; pieces that
    /// break off get new ids).
    id: u64,
}

struct Chunk {
    sig: u64,
    colliders: Vec<ColliderHandle>,
}

pub struct Physics {
    bodies: Vec<Body>,
    next_id: u64,
    /// Objects that lost cells (burned, chipped, broke) during the last update.
    damaged: Vec<u64>,
    joints: Vec<Joint>,
    /// Player throttle for `drive` motors, -1..1.
    throttle: f32,
    /// Characters (the player).
    actors: Vec<actor::Actor>,
    /// Vehicle controls: brakes (0..1), lean (-1..1), and objects kept
    /// upright by a balance assist (id -> strength). See vehicle.rs.
    brake: f32,
    lean: f32,
    /// Lean as the rider actually has it (eases towards `lean`).
    lean_now: f32,
    upright: std::collections::BTreeMap<u64, vehicle::Balance>,
    /// Hard hits on metal objects this frame, and how far each object has
    /// bent so far (radians). See bend.rs.
    bend_hits: Vec<bend::BendHit>,
    bent: HashMap<u64, f32>,
    /// A number unique to this physics world (object ids restart in each).
    run_id: u64,
    /// What the player is pressing.
    controls: Controls,
    set: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    soft: SoftBodySet,
    pipeline: PhysicsPipeline,
    broad: BroadPhaseBvh,
    narrow: NarrowPhase,
    islands: IslandManager,
    ccd: CCDSolver,
    params: IntegrationParameters,
    ground: RigidBodyHandle,
    chunks: Vec<Chunk>,
    /// Which cells were fixed solid scenery last frame (to spot ones that went away).
    solid_prev: Vec<bool>,
    /// Flood-fill bookkeeping for the support check.
    mark: Vec<u32>,
    epoch: u32,
    /// Scenery changed since the last strength check.
    strength_dirty: bool,
    tick: u32,
    cw: usize,
    ch: usize,
    w: usize,
    h: usize,
}

/// Rotate a local offset into the world (both in cells).
#[inline]
fn rot(c: f32, s: f32, x: f32, y: f32) -> (f32, f32) {
    (c * x - s * y, s * x + c * y)
}

/// Split a grid into rectangles of equal keys (greedy: grow right, then down).
fn rects<K: Copy + PartialEq>(w: usize, h: usize, key: impl Fn(usize, usize) -> Option<K>) -> Vec<(usize, usize, usize, usize, K)> {
    let mut used = vec![false; w * h];
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if used[y * w + x] {
                continue;
            }
            let Some(k) = key(x, y) else { continue };
            let mut rw = 1;
            while x + rw < w && !used[y * w + x + rw] && key(x + rw, y) == Some(k) {
                rw += 1;
            }
            let mut rh = 1;
            'grow: while y + rh < h {
                for xx in x..x + rw {
                    if used[(y + rh) * w + xx] || key(xx, y + rh) != Some(k) {
                        break 'grow;
                    }
                }
                rh += 1;
            }
            for yy in y..y + rh {
                for xx in x..x + rw {
                    used[yy * w + xx] = true;
                }
            }
            out.push((x, y, rw, rh, k));
        }
    }
    out
}

impl Physics {
    pub fn new(w: usize, h: usize) -> Self {
        let mut set = RigidBodySet::new();
        let mut colliders = ColliderSet::new();
        let ground = set.insert(RigidBodyBuilder::fixed());
        // Walls around the edge of the world.
        let (ww, hh) = (w as f32 * CELL, h as f32 * CELL);
        let t = 1.0;
        for (hx, hy, x, y) in [
            (ww / 2.0 + t, t, ww / 2.0, hh + t),  // floor
            (ww / 2.0 + t, t, ww / 2.0, -t),      // ceiling
            (t, hh / 2.0 + t, -t, hh / 2.0),      // left
            (t, hh / 2.0 + t, ww + t, hh / 2.0),  // right
        ] {
            colliders.insert_with_parent(
                ColliderBuilder::cuboid(hx, hy).translation(Vector::new(x, y)).friction(0.8),
                ground,
                &mut set,
            );
        }
        let (cw, ch) = (w.div_ceil(CHUNK), h.div_ceil(CHUNK));
        let mut params = IntegrationParameters::default();
        params.dt = DT;
        Physics {
            bodies: Vec::new(),
            next_id: 1,
            damaged: Vec::new(),
            joints: Vec::new(),
            throttle: 0.0,
            actors: Vec::new(),
            brake: 0.0,
            lean: 0.0,
            lean_now: 0.0,
            upright: Default::default(),
            bend_hits: Vec::new(),
            bent: HashMap::new(),
            run_id: NEXT_RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            controls: Controls::default(),
            set,
            colliders,
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            soft: SoftBodySet::new(),
            pipeline: PhysicsPipeline::new(),
            broad: BroadPhaseBvh::new(),
            narrow: NarrowPhase::new(),
            islands: IslandManager::new(),
            ccd: CCDSolver::new(),
            params,
            ground,
            chunks: (0..cw * ch).map(|_| Chunk { sig: 0, colliders: Vec::new() }).collect(),
            solid_prev: Vec::new(),
            mark: vec![0; w * h],
            epoch: 0,
            strength_dirty: true,
            tick: 0,
            cw,
            ch,
            w,
            h,
        }
    }

    /// A number unique to this physics world: object ids are only
    /// meaningful together with it (a restored world numbers objects anew).
    pub fn run_id(&self) -> u64 {
        self.run_id
    }

    /// Where an object's centre is (cells), cheaply.
    pub fn centre(&self, id: u64) -> Option<(f32, f32)> {
        let n = self.find(id)?;
        let t = self.set[self.bodies[n].handle].translation();
        Some((t.x / CELL, t.y / CELL))
    }

    /// An object's centre (cells) and angle (radians), cheaply.
    pub fn pose_of(&self, id: u64) -> Option<(f32, f32, f32)> {
        let n = self.find(id)?;
        Some(self.pose(&self.bodies[n]))
    }

    pub fn assembly(&self,id:u64)->Option<&str> {self.find(id).and_then(|n|self.bodies[n].assembly.as_deref())}
    pub fn set_assembly(&mut self,id:u64,name:Option<String>) {if let Some(n)=self.find(id) {self.bodies[n].assembly=name;}}
    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    /// Add an object and put it into the world straight away. If it was put
    /// inside solid ground, it's moved up until it's clear.
    pub fn add(&mut self, world: &mut World, mats: &Materials, mut spec: BodySpec) -> Option<u64> {
        let overlap = |spec: &BodySpec| -> usize {
            let (c, s) = (spec.angle.cos(), spec.angle.sin());
            let mut n = 0;
            for (k, cell) in spec.cells.iter().enumerate() {
                if cell.is_none() {
                    continue;
                }
                let lx = (k % spec.w) as f32 + 0.5 - spec.w as f32 / 2.0;
                let ly = (k / spec.w) as f32 + 0.5 - spec.h as f32 / 2.0;
                let (dx, dy) = rot(c, s, lx, ly);
                let (x, y) = ((spec.x + dx).floor() as i32, (spec.y + dy).floor() as i32);
                if world.in_bounds(x, y) && Self::is_terrain(world, mats, world.idx(x as usize, y as usize)) {
                    n += 1;
                }
            }
            n
        };
        if overlap(&spec) > 0 {
            for _ in 0..(spec.h + world.h) {
                spec.y -= 1.0;
                if overlap(&spec) == 0 || spec.y < -(spec.h as f32) {
                    break;
                }
            }
        }
        self.add_raw(world, mats, spec)
    }

    /// Add an object exactly where its spec says, without moving it out of
    /// the ground (restoring a saved moment).
    pub fn add_exact(&mut self, world: &mut World, mats: &Materials, spec: BodySpec) -> Option<u64> {
        self.add_raw(world, mats, spec)
    }

    /// Add an object exactly where its spec says (pieces of broken things).
    fn add_raw(&mut self, world: &mut World, mats: &Materials, spec: BodySpec) -> Option<u64> {
        if spec.cell_count() == 0 {
            return None;
        }
        let rb = RigidBodyBuilder::dynamic()
            .translation(Vector::new(spec.x * CELL, spec.y * CELL))
            .rotation(spec.angle)
            .linvel(Vector::new(spec.vx * CELL, spec.vy * CELL))
            .angvel(spec.spin)
            .ccd_enabled(true);
        let handle = self.set.insert(rb);
        let mut body = Body { bounds:(0,0,0,0), assembly: spec.assembly, depth: spec.depth, w: spec.w, h: spec.h, cells: spec.cells, handle, stamped: Vec::new(), born: self.tick, dirty: false, shard: false, round: false, id: self.next_id };
        self.next_id += 1;
        self.build_colliders(&mut body, mats);
        self.stamp(&mut body, world, mats);
        let id = body.id;
        self.bodies.push(body);
        Some(id)
    }

    fn build_colliders(&mut self, body: &mut Body, mats: &Materials) {
        let mut bounds=(body.w,body.h,0,0);
        for (i,cell) in body.cells.iter().enumerate() {if cell.is_some() {let (x,y)=(i%body.w,i/body.w);bounds.0=bounds.0.min(x);bounds.1=bounds.1.min(y);bounds.2=bounds.2.max(x+1);bounds.3=bounds.3.max(y+1);}}
        body.bounds=bounds;
        let old: Vec<ColliderHandle> = self.set[body.handle].colliders().to_vec();
        for c in old {
            self.colliders.remove(c, &mut self.islands, &mut self.set, &mut self.soft, true);
        }
        let (w, h) = (body.w, body.h);
        let cells = &body.cells;
        if body.round {
            // One smooth circle, made of the most common material.
            let mut count: std::collections::BTreeMap<MatId, usize> = Default::default();
            for c in cells.iter().flatten() {
                *count.entry(c.mat).or_default() += 1;
            }
            if let Some((&mat, _)) = count.iter().max_by_key(|(_, n)| **n) {
                let m = mats.get(mat);
                let n = cells.iter().flatten().count() as f32;
                let r = (w.max(h) as f32) / 2.0 * CELL;
                // Same mass as the cells, spread over the circle.
                let density = cells.iter().flatten().map(|c| mats.get(c.mat).density).sum::<f32>() / n * (n * CELL * CELL)
                    / (std::f32::consts::PI * r * r);
                let col = ColliderBuilder::ball(r)
                    .friction_combine_rule(CoefficientCombineRule::Min)
                    .restitution_combine_rule(CoefficientCombineRule::Max)
                    .density(density.max(50.0) * body.depth)
                    .friction(m.friction)
                    .restitution(m.bounciness);
                self.colliders.insert_with_parent(col, body.handle, &mut self.set);
                self.set[body.handle].wake_up(true);
                return;
            }
        }
        for (x, y, rw, rh, mat) in rects(w, h, |x, y| cells[y * w + x].map(|c| c.mat)) {
            let cx = (x as f32 + rw as f32 / 2.0 - w as f32 / 2.0) * CELL;
            let cy = (y as f32 + rh as f32 / 2.0 - h as f32 / 2.0) * CELL;
            let m = mats.get(mat);
            let col = ColliderBuilder::cuboid(rw as f32 * CELL / 2.0, rh as f32 * CELL / 2.0)
                .translation(Vector::new(cx, cy))
                .friction_combine_rule(CoefficientCombineRule::Min)
                .restitution_combine_rule(CoefficientCombineRule::Max)
                .density(m.density.max(50.0) * body.depth)
                .friction(m.friction)
                .restitution(m.bounciness);
            self.colliders.insert_with_parent(col, body.handle, &mut self.set);
        }
        self.set[body.handle].wake_up(true);
    }

    fn pose(&self, body: &Body) -> (f32, f32, f32) {
        let rb = &self.set[body.handle];
        let t = rb.translation();
        (t.x / CELL, t.y / CELL, rb.rotation().angle())
    }

    /// Write an object's cells into the grid at its current position.
    fn stamp(&self, body: &mut Body, world: &mut World, mats: &Materials) {
        body.stamped.clear();
        let (cx, cy, angle) = self.pose(body);
        let (c, s) = (angle.cos(), angle.sin());
        let (hw, hh) = (body.w as f32 / 2.0, body.h as f32 / 2.0);
        let (bx0,by0,bx1,by1)=body.bounds;
        if bx0>=bx1 || by0>=by1 {return;}
        let mut limits=(f32::INFINITY,f32::INFINITY,f32::NEG_INFINITY,f32::NEG_INFINITY);
        for (x,y) in [(bx0,by0),(bx1,by0),(bx1,by1),(bx0,by1)] {let(dx,dy)=(x as f32-hw,y as f32-hh);let(wx,wy)=(cx+c*dx-s*dy,cy+s*dx+c*dy);limits.0=limits.0.min(wx);limits.1=limits.1.min(wy);limits.2=limits.2.max(wx);limits.3=limits.3.max(wy);}
        // Fast objects splash the liquid they hit (speeds in cells per frame).
        let v = self.set[body.handle].linvel();
        let (bvx, bvy) = (v.x / CELL / 60.0, v.y / CELL / 60.0);
        let speed = (bvx * bvx + bvy * bvy).sqrt();
        let splash = speed > SPLASH_SPEED;
        let (x0,x1)=((limits.0.floor() as i32-1).max(0),(limits.2.ceil() as i32+1).min(world.w as i32-1));
        let (y0,y1)=((limits.1.floor() as i32-1).max(0),(limits.3.ceil() as i32+1).min(world.h as i32-1));
        for wy in y0..=y1 {
            for wx in x0..=x1 {
                if !world.in_bounds(wx, wy) {
                    continue;
                }
                // Which local cell lands here? (Inverse-rotate the cell centre.)
                let (dx, dy) = (wx as f32 + 0.5 - cx, wy as f32 + 0.5 - cy);
                let (lx, ly) = (c * dx + s * dy + hw, -s * dx + c * dy + hh);
                if lx < 0.0 || ly < 0.0 {
                    continue;
                }
                let (li, lj) = (lx as usize, ly as usize);
                if li >= body.w || lj >= body.h {
                    continue;
                }
                let local = lj * body.w + li;
                let Some(cell) = body.cells[local] else { continue };
                let wi = world.idx(wx as usize, wy as usize);
                if world.is_owned(wi) {
                    continue; // overlapping another object
                }
                let under = mats.get(world.mat[wi]);
                match under.state {
                    _ if under.crushable => {} // crushed to nothing
                    State::Solid | State::Powder => continue, // terrain wins
                    State::Liquid => {
                        let to = push_liquid_up(world, mats, wx, wy);
                        if let (true, Some(j)) = (splash, to) {
                            let h = hash(j as u32 ^ self.tick.wrapping_mul(0x9E37_79B9));
                            let jitter = 0.6 + (h % 800) as f32 / 1000.0;
                            let out = if (wx as f32 + 0.5) < cx { -1.0 } else { 1.0 };
                            let up = speed * 0.7 * jitter + 0.3;
                            let side = bvx * 0.4 + out * speed * 0.35 * jitter;
                            world.throw(mats, j % world.w, j / world.w, side, -up);
                        }
                    }
                    State::Gas => {}
                }
                world.wake_at(wi); // objects keep the area round them awake
                world.mat[wi] = cell.mat;
                world.temp[wi] = cell.temp;
                world.burn[wi] = cell.burn;
                world.life[wi] = cell.life;
                world.flags[wi] = cell.flags | OWNED;
                world.shade[wi] = cell.shade;
                world.paint[wi] = cell.paint;
                world.tex[wi] = crate::world::pack_tex(li, lj);
                body.stamped.push((wi as u32, local as u32));
            }
        }
    }

    /// Copy cells out of the grid. Returns true if any cell left the object.
    /// With `clear`, the object's cells are also removed from the grid.
    fn read_back(body: &mut Body, world: &mut World, clear: bool) -> bool {
        let mut lost = vec![false; body.cells.len()];
        let mut any_lost = false;
        for &(wi, li) in &body.stamped {
            let (wi, li) = (wi as usize, li as usize);
            let Some(cell) = body.cells[li].as_mut() else { continue };
            if world.is_owned(wi) && world.mat[wi] == cell.mat {
                cell.paint = world.paint[wi];
                cell.temp = world.temp[wi];
                cell.burn = world.burn[wi];
                cell.life = world.life[wi];
                cell.flags = world.flags[wi] & !OWNED;
                if clear {
                    world.make_air(wi);
                }
            } else {
                lost[li] = true;
                any_lost = true;
            }
        }
        if clear {
            for (li, gone) in lost.iter().enumerate() {
                if *gone {
                    body.cells[li] = None;
                }
            }
            // `stamped` is kept until the next stamp: floating uses it.
        }
        any_lost
    }

    /// Advance all objects by one frame. Call right after `World::step`.
    pub fn update(&mut self, world: &mut World, mats: &Materials) {
        let _all=crate::profile::scope("physics.total");
        let phase=crate::profile::scope("physics.read_split");
        // 1. Read back and lift every object out of the grid.
        self.actors_read_back(world, mats);
        let mut bodies = std::mem::take(&mut self.bodies);
        let mut changed = vec![false;bodies.len()];
        self.damaged.clear();
        for (n, b) in bodies.iter_mut().enumerate() {
            if Self::read_back(b, world, true) || b.dirty {
                b.dirty = false;
                changed[n]=true;
                self.damaged.push(b.id);
            }
        }
        // Objects that lost cells may have split into pieces.
        let mut keep = Vec::with_capacity(bodies.len());
        let mut new_specs = Vec::new();
        for (n, mut b) in bodies.into_iter().enumerate() {
            if !changed[n] {
                keep.push(b);
                continue;
            }
            let pieces = components(b.w, b.h, &b.cells);
            if pieces.len() == 1 && pieces[0].len() == b.cells.iter().filter(|c| c.is_some()).count() {
                self.build_colliders(&mut b, mats);
                keep.push(b);
            } else {
                new_specs.extend(self.split(&b, &pieces));
                self.set.remove(
                    b.handle,
                    &mut self.islands,
                    &mut self.colliders,
                    &mut self.impulse_joints,
                    &mut self.multibody_joints,
                    &mut self.soft,
                    true,
                );
            }
        }
        self.bodies = keep;
        self.tidy_joints();

        drop(phase);let phase=crate::profile::scope("physics.terrain");
        // 2. Scenery that lost its support breaks loose.
        new_specs.extend(self.collapse(world, mats));

        // 3. Terrain colliders for whatever changed.
        self.update_terrain(world, mats);

        drop(phase);let phase=crate::profile::scope("physics.forces_groups");
        // 3. Floating and drag in liquids, then step the physics.
        let floats: Vec<(RigidBodyHandle, Option<(f32, Vector)>)> =
            self.bodies.iter().map(|b| (b.handle, Self::buoyancy(b, world, mats))).collect();
        for (h, f) in floats {
            let rb = &mut self.set[h];
            match f {
                Some((force, at)) => {

                    rb.add_force_at_point(Vector::new(0.0, -force), at, true);
                    rb.set_linear_damping(2.5);
                    rb.set_angular_damping(2.5);
                }
                None => {
                    rb.set_linear_damping(0.0);
                    rb.set_angular_damping(0.05);
                }
            }
        }
        // Vehicles: motors and brakes, rolling resistance, balance.
        self.vehicles(world, mats);
        // Bounce pads fling objects resting or landing on them.
        for b in &self.bodies {
            let mut pad = 0.0f32;
            for &(wi, _) in &b.stamped {
                let below = wi as usize + world.w;
                if below < world.mat.len() && !world.is_owned(below) {
                    pad = pad.max(mats.get(world.mat[below]).launch);
                }
            }
            if pad > 0.0 {
                let rb = &mut self.set[b.handle];
                let v = rb.linvel();
                if v.y > -pad * 0.5 {
                    rb.set_linvel(Vector::new(v.x, -pad), true);
                }
            }
        }
        let blasts = world.blasts.clone();
        self.actors_think(world, mats, &blasts);
        // Welded vehicle parts must not fight each other through internal collisions.
        // Recompute each frame so released parts collide normally on the next step.
        let mut roots: HashMap<u64,u64> = self.bodies.iter().map(|b|(b.id,b.id)).collect();
        for _ in 0..self.joints.len() {
            let mut changed=false;
            for j in &self.joints {
                if j.spec.a==GROUND || j.spec.b==GROUND || !matches!(j.spec.kind,JointKind::Weld|JointKind::Wheel{..}) {continue;}
                if let (Some(&a),Some(&b))=(roots.get(&j.spec.a),roots.get(&j.spec.b))
                    && a!=b {let root=a.min(b);for value in roots.values_mut(){if *value==a || *value==b {*value=root;}}changed=true;}
            }
            if !changed {break;}
        }
        let hooks=OneWayPowder {assemblies:self.bodies.iter().map(|b|(b.handle,roots[&b.id])).collect()};
        let mut group_sizes=HashMap::<u64,usize>::new();
        for root in roots.values() {*group_sizes.entry(*root).or_default()+=1;}
        for b in &self.bodies {let grouped=group_sizes[&roots[&b.id]]>1;for h in self.set[b.handle].colliders() {
            let Some(c)=self.colliders.get(*h) else {continue};
            let old=c.active_hooks();

            let new=if grouped {old | ActiveHooks::FILTER_CONTACT_PAIRS} else {old & !ActiveHooks::FILTER_CONTACT_PAIRS};
            // Mutating unchanged colliders invalidates cached contacts and friction.
            if new!=old&& let Some(c)=self.colliders.get_mut(*h) {c.set_active_hooks(new);}
        }}
        drop(phase);let phase=crate::profile::scope("physics.solver");
        self.pipeline.step(
            Vector::new(0.0, GRAVITY),
            &self.params,
            &mut self.islands,
            &mut self.broad,
            &mut self.narrow,
            &mut self.set,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.soft,
            &mut self.ccd,
            &hooks,
            &(),
        );

        drop(phase);let phase=crate::profile::scope("physics.stamp");
        self.break_overloaded_joints();

        // 4. Put everything back into the grid (new pieces too).
        let mut bodies = std::mem::take(&mut self.bodies);
        for b in bodies.iter_mut() {
            let rb = &mut self.set[b.handle];
            rb.reset_forces(false);
            rb.reset_torques(false);
            self.stamp(b, world, mats);
        }
        self.bodies = bodies;
        self.actors_after(world, mats);
        for spec in new_specs {
            self.add_raw(world, mats, spec);
        }

        drop(phase);let _phase=crate::profile::scope("physics.damage");
        // 6. Damage: hard hits, heavy loads and explosions.
        let mut pieces = self.impacts(world, mats);
        pieces.extend(self.explosions(world, mats));
        for spec in pieces {
            self.add_raw(world, mats, spec);
            if let Some(b) = self.bodies.last_mut() {
                b.shard = true;
            }
        }
        // Metal that was hit hard enough bends (or tears).
        self.bends(world, mats);
    }

    /// Which object owns each world cell right now.
    fn owners(&self) -> HashMap<u32, usize> {
        let mut m = HashMap::new();
        for (n, b) in self.bodies.iter().enumerate() {
            for &(wi, _) in &b.stamped {
                m.insert(wi, n);
            }
        }
        m
    }

    /// Look at every contact from this step. Where the push was harder than a
    /// material's `toughness`, crack it there (both sides can crack).
    fn impacts(&mut self, world: &mut World, mats: &Materials) -> Vec<BodySpec> {
        // Gather the hard contacts first: total force of each touching pair (kN)
        // and the patch of points where they touch (in cells).
        let mut hits: Vec<Hit> = Vec::new();
        let index: HashMap<RigidBodyHandle, usize> =
            self.bodies.iter().enumerate().map(|(n, b)| (b.handle, n)).collect();
        for pair in self.narrow.contact_pairs() {
            if !pair.has_any_active_contact() {
                continue;
            }
            let (Some(c1), Some(c2)) = (self.colliders.get(pair.collider1), self.colliders.get(pair.collider2)) else {
                continue;
            };
            let (b1, b2) = (c1.parent(), c2.parent());
            // Brand-new pieces push hard while they settle: skip them for a moment.
            let settling = |h: Option<RigidBodyHandle>| {
                h.and_then(|h| index.get(&h))
                    .is_some_and(|&n| self.tick.wrapping_sub(self.bodies[n].born) < SETTLE_TICKS)
            };
            if settling(b1) || settling(b2) {
                continue;
            }
            let pose = c1.position();
            let (re, im) = (pose.rotation.re, pose.rotation.im);
            let mut impulse = 0.0;
            let mut patch = Vec::new();
            let mut push = (0.0f32, 0.0f32);
            for m in pair.solver_manifolds() {
                for pt in &m.points {
                    impulse += pt.data.impulse;
                    push.0 += m.data.normal.x * pt.data.impulse;
                    push.1 += m.data.normal.y * pt.data.impulse;
                    let l = pt.local_p1;
                    patch.push((
                        (pose.translation.x + re * l.x - im * l.y) / CELL,
                        (pose.translation.y + im * l.x + re * l.y) / CELL,
                    ));
                }
            }
            let force = impulse / DT / 1000.0;
            if force >= MIN_DAMAGE_KN / 4.0 && !patch.is_empty() {
                let len = push.0.hypot(push.1).max(1e-6);
                hits.push((force, patch, b1, b2, (push.0 / len * force, push.1 / len * force)));
            }
        }
        if hits.is_empty() {
            return Vec::new();
        }
        // One object landing across several pieces of ground is one hit.
        let mut merged: Vec<Hit> = Vec::new();
        for (f, patch, b1, b2, push) in hits {
            match merged.iter_mut().find(|m| (m.2 == b1 && m.3 == b2) || (m.2 == b2 && m.3 == b1)) {
                Some(m) => {
                    m.0 += f;
                    m.1.extend(patch);
                    let flip = if m.2 == b1 { 1.0 } else { -1.0 };
                    m.4 = (m.4 .0 + push.0 * flip, m.4 .1 + push.1 * flip);
                }
                None => merged.push((f, patch, b1, b2, push)),
            }
        }
        let mut hits = merged;
        hits.sort_by(|a, b| b.0.total_cmp(&a.0));
        hits.truncate(MAX_HITS_PER_TICK);

        if hits.first().is_none_or(|h|h.0<MIN_DAMAGE_KN) {return Vec::new();}
        let owners = self.owners();
        let mut out = Vec::new();
        for (force, patch, b1, b2, push) in hits {
            if force < MIN_DAMAGE_KN {
                continue;
            }
            // Points along the whole contact patch, about a cell apart.
            let mut sorted = patch.clone();
            sorted.sort_by(|a, b| (a.0 + a.1 * 0.001).total_cmp(&(b.0 + b.1 * 0.001)));
            let mut samples = Vec::new();
            for w in sorted.windows(2) {
                let d = ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt();
                let n = (d.ceil() as usize).clamp(1, 40);
                for k in 0..n {
                    let t = k as f32 / n as f32;
                    samples.push((w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t));
                }
            }
            samples.push(*sorted.last().unwrap());
            samples.truncate(60);

            // Metal objects may bend (bend.rs): the push on each side, at
            // the middle of the patch.
            let plen = push.0.hypot(push.1).max(1e-6);
            let mid = patch.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0 / patch.len() as f32, a.1 + p.1 / patch.len() as f32));
            for (side, sign) in [(b1, -1.0f32), (b2, 1.0)] {
                let Some(&n) = side.and_then(|h| index.get(&h)) else { continue };
                let Some(cell) = nearest_cell(world, mats, &owners, Which::Body(n), mid.0, mid.1, 2.0) else { continue };
                if Self::bendable(mats, world.mat[cell]) {
                    let at = ((cell % world.w) as f32 + 0.5, (cell / world.w) as f32 + 0.5);
                    self.bend_hits.push(bend::BendHit { id: self.bodies[n].id, force, at, dir: (push.0 / plen * sign, push.1 / plen * sign) });
                }
            }
            for side in [b1, b2] {
                let which = match side {
                    Some(h) if h == self.ground => Which::Scenery,
                    Some(h) => match index.get(&h) {
                        Some(&n) => Which::Body(n),
                        None => continue,
                    },
                    None => continue,
                };
                let vel = match which {
                    Which::Body(n) => {
                        let v = self.set[self.bodies[n].handle].linvel();
                        (v.x / CELL, v.y / CELL)
                    }
                    _ => (0.0, 0.0),
                };
                for &(px, py) in &samples {
                    // The nearest cell on this side tells us what got hit.
                    let Some(cell) = nearest_cell(world, mats, &owners, which, px, py, 2.0) else { continue };
                    let Some(tough) = mats.get(world.mat[cell]).toughness else { continue };
                    // Hot metal (past `softens_at`) is weaker.
                    let tough = tough * mats.get(world.mat[cell]).firmness(world.temp[cell]).max(0.02);
                    if force <= tough {
                        continue;
                    }
                    let r = (1.5 + 2.0 * (force / tough).ln()).clamp(1.5, 7.0);
                    let (cx, cy) = ((cell % world.w) as f32 + 0.5, (cell / world.w) as f32 + 0.5);
                    out.extend(self.fracture(world, mats, &owners, which, cx, cy, r, |_| force, vel, 6.0, IMPACT_CHUNKS));
                }
            }
        }
        out
    }

    /// Crack every breakable cell of `which` within `r` of (cx, cy) whose
    /// `toughness` is beaten by `force_at(distance)`. Returns the splinters.
    #[allow(clippy::too_many_arguments)]
    fn fracture(
        &mut self,
        world: &mut World,
        mats: &Materials,
        owners: &HashMap<u32, usize>,
        which: Which,
        cx: f32,
        cy: f32,
        r: f32,
        force_at: impl Fn(f32) -> f32,
        vel: (f32, f32),
        push: f32,
        coarse: f32,
    ) -> Vec<BodySpec> {
        // Cells that break into loose pieces, with how much the hit beat them by.
        let mut breaking: Vec<(usize, f32)> = Vec::new();
        let ri = r.ceil() as i32;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                if !world.in_bounds(x, y) {
                    continue;
                }
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                if d > r {
                    continue;
                }
                let i = world.idx(x as usize, y as usize);
                if !which.matches(world, mats, owners, i) {
                    continue;
                }
                // Pieces that only just broke off can't be broken again yet
                // (otherwise a chain of blasts grinds them into dust).
                if let Some(&n) = owners.get(&(i as u32)) {
                    let b = &self.bodies[n];
                    if b.shard && self.tick.wrapping_sub(b.born) < FRESH_TICKS {
                        continue;
                    }
                }
                let m = mats.get(world.mat[i]);
                let Some(tough) = m.toughness else { continue };
                let tough = tough * m.firmness(world.temp[i]).max(0.02);
                if force_at(d) <= tough {
                    continue;
                }
                match m.breaks_into {
                    Some(into) => world.transform(mats, i, into),
                    None => breaking.push((i, force_at(d) / tough)),
                }
            }
        }
        // Break into jagged chunks rather than dust: each broken cell joins the
        // nearest of some scattered "crack centres". Harder hits (compared with
        // the material's toughness) scatter the centres closer together, so
        // they make smaller pieces. Pieces never mix different objects.
        let seed = self.tick.wrapping_mul(2654435761);
        let mut groups: std::collections::BTreeMap<(usize, u8, i32, i32), Vec<usize>> = Default::default();
        for &(i, ratio) in &breaking {
            let tier: u8 = if ratio > 30.0 { 0 } else if ratio > 6.0 { 1 } else { 2 };
            let spacing = [4.0f32, 6.5, 10.0][tier as usize] * coarse;
            let (x, y) = ((i % world.w) as f32 + 0.5, (i / world.w) as f32 + 0.5);
            let (gx, gy) = ((x / spacing).floor() as i32, (y / spacing).floor() as i32);
            let mut best = (f32::MAX, gx, gy);
            for oy in -1..=1 {
                for ox in -1..=1 {
                    let (sx, sy) = (gx + ox, gy + oy);
                    let h = hash((sx as u32).wrapping_mul(73856093) ^ (sy as u32).wrapping_mul(19349663) ^ seed ^ tier as u32);
                    let jx = (h & 0xffff) as f32 / 65535.0;
                    let jy = (h >> 16) as f32 / 65535.0;
                    let (px, py) = ((sx as f32 + jx) * spacing, (sy as f32 + jy) * spacing);
                    let d = (px - x).powi(2) + (py - y).powi(2);
                    if d < best.0 {
                        best = (d, sx, sy);
                    }
                }
            }
            let owner = owners.get(&(i as u32)).copied().unwrap_or(usize::MAX);
            groups.entry((owner, tier, best.1, best.2)).or_default().push(i);
        }
        // A chunk can come out in separate bits (a crack centre's area wraps
        // around a hole): each connected bit is its own piece.
        let mut pieces: Vec<Vec<usize>> = Vec::new();
        for (_, cells) in groups {
            let set: std::collections::HashSet<usize> = cells.iter().copied().collect();
            let mut seen: std::collections::HashSet<usize> = std::collections::HashSet::new();
            for &start in &cells {
                if !seen.insert(start) {
                    continue;
                }
                let mut piece = vec![start];
                let mut stack = vec![start];
                while let Some(i) = stack.pop() {
                    let (x, y) = (i % world.w, i / world.w);
                    let mut near = Vec::with_capacity(4);
                    if x > 0 { near.push(i - 1); }
                    if x + 1 < world.w { near.push(i + 1); }
                    if y > 0 { near.push(i - world.w); }
                    if y + 1 < world.h { near.push(i + world.w); }
                    for j in near {
                        if set.contains(&j) && seen.insert(j) {
                            piece.push(j);
                            stack.push(j);
                        }
                    }
                }
                pieces.push(piece);
            }
        }
        // Glue tiny offcuts onto a neighbouring piece, so things break into
        // chunks with a few crumbs, not a cloud of single pixels.
        let mut which_piece: HashMap<usize, usize> = HashMap::new();
        for (n, p) in pieces.iter().enumerate() {
            for &i in p {
                which_piece.insert(i, n);
            }
        }
        for n in 0..pieces.len() {
            if pieces[n].is_empty() || pieces[n].len() >= MIN_PIECE {
                continue;
            }
            let mut target = None;
            'find: for &i in &pieces[n] {
                let (x, y) = (i % world.w, i / world.w);
                for (ok, j) in [(x > 0, i.wrapping_sub(1)), (x + 1 < world.w, i + 1), (y > 0, i.wrapping_sub(world.w)), (y + 1 < world.h, i + world.w)] {
                    if !ok {
                        continue;
                    }
                    if let Some(&m) = which_piece.get(&j) {
                        let same_owner = owners.get(&(i as u32)) == owners.get(&(j as u32));
                        if m != n && same_owner && !pieces[m].is_empty() {
                            target = Some(m);
                            break 'find;
                        }
                    }
                }
            }
            if let Some(m) = target {
                let moved = std::mem::take(&mut pieces[n]);
                for &i in &moved {
                    which_piece.insert(i, m);
                }
                pieces[m].extend(moved);
            }
        }
        pieces.retain(|p| !p.is_empty());

        // Pieces fly outward from the hit.
        let mut out = Vec::new();
        for piece in pieces {
            if self.bodies.len() + out.len() >= MAX_OBJECTS {
                for i in piece {
                    world.make_air(i); // too many pieces already: it's dust
                }
                continue;
            }
            for &i in &piece {
                if let Some(&n) = owners.get(&(i as u32)) {
                    let b = &mut self.bodies[n];
                    if let Some(&(_, li)) = b.stamped.iter().find(|&&(wi, _)| wi as usize == i) {
                        b.cells[li as usize] = None;
                        b.dirty = true;
                    }
                }
            }
            let mut spec = self.detach(world, &piece);
            let (dx, dy) = (spec.x - cx, spec.y - cy);
            let len = (dx * dx + dy * dy).sqrt().max(0.5);
            let jitter = 0.6 + (hash(piece[0] as u32 ^ self.tick) % 100) as f32 / 100.0;
            spec.vx = vel.0 + dx / len * push * jitter;
            spec.vy = vel.1 + dy / len * push * jitter;
            spec.spin = ((hash(piece[0] as u32 ^ self.tick ^ 77) % 200) as f32 / 100.0 - 1.0) * 4.0;
            out.push(spec);
        }
        out
    }

    /// Explosions from this frame: nearby ones merge into one bigger blast.
    /// Each blast cracks things by toughness, throws objects, and heats and
    /// lights everything around it.
    fn explosions(&mut self, world: &mut World, mats: &Materials) -> Vec<BodySpec> {
        if world.blasts.is_empty() {
            return Vec::new();
        }
        // Merge blasts within 8 cells of each other.
        let mut groups: std::collections::BTreeMap<(i32, i32), (f32, f32, f32, f32, f32, Option<MatId>)> = Default::default();
        for b in world.blasts.drain(..) {
            let e = groups.entry(((b.x / 8.0) as i32, (b.y / 8.0) as i32)).or_insert((0.0, 0.0, 0.0, 0.0, 0.0, b.fire));
            e.0 += b.x;
            e.1 += b.y;
            e.2 += 1.0;
            e.3 = e.3.max(b.size);
            e.4 = e.4.max(b.power);
        }
        let mut out = Vec::new();
        for (_, (sx, sy, n, size, power, fire)) in groups {
            let (cx, cy) = (sx / n, sy / n);
            let r = (size * (1.0 + 0.35 * n.ln())).min(48.0);
            let p0 = power * (1.0 + n.ln());
            let force_at = move |d: f32| p0 * (1.0 - d / r).max(0.0).powi(2);

            // Heat and fire.
            let ri = r.ceil() as i32;
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                    if !world.in_bounds(x, y) {
                        continue;
                    }
                    let d = ((dx * dx + dy * dy) as f32).sqrt();
                    if d > r {
                        continue;
                    }
                    let i = world.idx(x as usize, y as usize);
                    let k = 1.0 - d / r;
                    // A quick hot flash: enough to light things, not melt rock.
                    world.temp[i] = world.temp[i].max((world.temp[i] + 350.0 * k * k).min(800.0));
                    world.wake_at(i);
                    if let Some(f) = fire {
                        let gas = mats.get(world.mat[i]).state == State::Gas && !world.is_owned(i);
                        if gas && d < r * 0.7 && hash(i as u32 ^ self.tick) % 100 < 45 {
                            world.transform(mats, i, f);
                        }
                    }
                }
            }

            // Throw loose stuff (sand, gravel, water, oil...) outwards. All of
            // it leaves at once, so the inside of a pile doesn't just bump
            // into the outside.
            let reach = r * 0.85;
            let rt = reach.ceil() as i32;
            let strength = (p0 / 10_000.0).sqrt().clamp(0.5, 1.5);
            for dy in -rt..=rt {
                for dx in -rt..=rt {
                    let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                    if !world.in_bounds(x, y) {
                        continue;
                    }
                    let d = ((dx * dx + dy * dy) as f32).sqrt();
                    if d > reach {
                        continue;
                    }
                    let i = world.idx(x as usize, y as usize);
                    let mat = mats.get(world.mat[i]);
                    if world.is_owned(i) || !matches!(mat.state, State::Powder | State::Liquid) {
                        continue;
                    }
                    // Other explosives caught in a blast go off too (they'd
                    // be set off before they could be thrown clear).
                    if mat.explosion_size > 0.0 {
                        if let Some(c) = mat.catches_fire_at {
                            world.temp[i] = world.temp[i].max(c);
                            world.wake_at(i);
                        }
                        continue;
                    }
                    let k = 1.0 - d / reach;
                    let h = hash(i as u32 ^ self.tick.wrapping_mul(2_654_435_761));
                    if (h % 1000) as f32 / 1000.0 > 0.35 + 0.65 * k {
                        continue;
                    }
                    let jitter = 0.7 + ((h >> 10) % 600) as f32 / 1000.0;
                    let speed = (1.0 + 5.0 * k) * jitter * strength;
                    let (ux, uy) = if d > 0.01 { (dx as f32 / d, dy as f32 / d) } else { (0.0, -1.0) };
                    // A little extra kick upwards: blasts against the ground spray up.
                    world.throw(mats, x as usize, y as usize, ux * speed, uy * speed - 0.6 * k);
                }
            }

            // Crack things (objects and scenery).
            let owners = self.owners();
            let first = out.len();
            out.extend(self.fracture(world, mats, &owners, Which::Any, cx, cy, r, force_at, (0.0, 0.0), BLAST_MAX_SPEED / CELL * 0.8, 1.0));
            for spec in out[first..].iter_mut() {
                // Splinters near the centre fly faster.
                let d = ((spec.x - cx).powi(2) + (spec.y - cy).powi(2)).sqrt();
                let k = (1.0 - d / r).clamp(0.25, 1.0);
                spec.vx *= k;
                spec.vy *= k;
            }

            // Shove objects away from the blast.
            for b in &self.bodies {
                let rb = &mut self.set[b.handle];
                let c = rb.center_of_mass();
                let (dx, dy) = (c.x / CELL - cx, c.y / CELL - cy);
                let d = (dx * dx + dy * dy).sqrt();
                let reach = r + (b.w.max(b.h) as f32) * 0.5;
                if d >= reach {
                    continue;
                }
                let p = force_at((d - (b.w.max(b.h) as f32) * 0.5).max(0.0));
                let cells = b.cells.iter().filter(|c| c.is_some()).count() as f32;
                let j = (p * cells.sqrt() * BLAST_PUSH).min(rb.mass() * BLAST_MAX_SPEED); // newton-seconds
                let (ux, uy) = if d > 0.01 { (dx / d, dy / d) } else { (0.0, -1.0) };
                rb.apply_impulse(Vector::new(ux * j, uy * j), true);
                rb.apply_torque_impulse(j * 0.02 * if (b.handle.into_raw_parts().0 & 1) == 0 { 1.0 } else { -1.0 }, true);
            }
        }
        out
    }

    /// Make a new spec for each piece of a broken object, moving like the original.
    fn split(&self, b: &Body, pieces: &[Vec<usize>]) -> Vec<BodySpec> {
        let rb = &self.set[b.handle];
        let (cx, cy, angle) = self.pose(b);
        let (c, s) = (angle.cos(), angle.sin());
        let mut out = Vec::new();
        for piece in pieces {
            let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
            for &i in piece {
                let (x, y) = (i % b.w, i / b.w);
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
            let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
            let mut cells = vec![None; w * h];
            for &i in piece {
                let (x, y) = (i % b.w, i / b.w);
                cells[(y - y0) * w + (x - x0)] = b.cells[i];
            }
            // Centre of the piece, in the old object's frame, then in the world.
            let lx = x0 as f32 + w as f32 / 2.0 - b.w as f32 / 2.0;
            let ly = y0 as f32 + h as f32 / 2.0 - b.h as f32 / 2.0;
            let (dx, dy) = rot(c, s, lx, ly);
            let (px, py) = (cx + dx, cy + dy);
            let v = rb.velocity_at_point(Vector::new(px * CELL, py * CELL));
            out.push(BodySpec {
                assembly: b.assembly.clone(),
                depth: b.depth,
                w,
                h,
                cells,
                x: px,
                y: py,
                angle,
                vx: v.x / CELL,
                vy: v.y / CELL,
                spin: rb.angvel(),
            });
        }
        out
    }

    fn is_terrain(world: &World, mats: &Materials, i: usize) -> bool {
        let m = mats.get(world.mat[i]);
        !world.is_owned(i) && !m.crushable && matches!(m.state, State::Solid | State::Powder)
    }

    fn update_terrain(&mut self, world: &World, mats: &Materials) {
        let mut rebuilt: Vec<(usize, usize)> = Vec::new();
        for cy in 0..self.ch {
            for cx in 0..self.cw {
                // Nothing changed here for a while (the world's sleeping
                // blocks are these same 32x32 chunks): the ground is as it was.
                if CHUNK == 32 && self.chunks[cy * self.cw + cx].sig != 0 && world.block_still(cy * self.cw + cx) {
                    continue;
                }
                let (x0, y0) = (cx * CHUNK, cy * CHUNK);
                let (x1, y1) = ((x0 + CHUNK).min(self.w), (y0 + CHUNK).min(self.h));
                // Fingerprint of which cells are solid.
                let mut sig: u64 = 0xcbf2_9ce4_8422_2325;
                for y in y0..(y1 + 1).min(self.h) {
                    for x in x0..(x1 + 1).min(self.w) {
                        let i = y * self.w + x;
                        let v = if Self::is_terrain(world, mats, i) { world.mat[i] as u64 + 1 } else { 0 };
                        sig = (sig ^ v).wrapping_mul(0x0100_0000_01b3);
                    }
                }
                let k = cy * self.cw + cx;
                if self.chunks[k].sig == sig {
                    continue;
                }
                self.chunks[k].sig = sig;
                for c in std::mem::take(&mut self.chunks[k].colliders) {
                    self.colliders.remove(c, &mut self.islands, &mut self.set, &mut self.soft, false);
                }
                // Trace the outline of the ground (marching squares between cell
                // centres), so slopes are smooth and things can slide and roll.
                let solid = |x: usize, y: usize| -> Option<MatId> {
                    let i = y * self.w + x;
                    Self::is_terrain(world, mats, i).then(|| world.mat[i])
                };
                let mut by_mat: std::collections::BTreeMap<MatId, Vec<((f32, f32), (f32, f32))>> = Default::default();
                for sy in y0..y1.min(self.h - 1) {
                    for sx in x0..x1.min(self.w - 1) {
                        let (a, b, c, d) = (solid(sx, sy), solid(sx + 1, sy), solid(sx + 1, sy + 1), solid(sx, sy + 1));
                        let case = (a.is_some() as u8) << 3 | (b.is_some() as u8) << 2 | (c.is_some() as u8) << 1 | d.is_some() as u8;
                        if case == 0 || case == 15 {
                            continue;
                        }
                        let (fx, fy) = (sx as f32, sy as f32);
                        let top = (fx + 1.0, fy + 0.5);
                        let right = (fx + 1.5, fy + 1.0);
                        let bottom = (fx + 1.0, fy + 1.5);
                        let left = (fx + 0.5, fy + 1.0);
                        let segs: &[((f32, f32), (f32, f32))] = match case {
                            1 | 14 => &[(left, bottom)],
                            2 | 13 => &[(bottom, right)],
                            3 | 12 => &[(left, right)],
                            4 | 11 => &[(top, right)],
                            6 | 9 => &[(top, bottom)],
                            7 | 8 => &[(left, top)],
                            5 => &[(left, top), (bottom, right)],
                            10 => &[(top, right), (left, bottom)],
                            _ => &[],
                        };
                        let mat = a.or(b).or(c).or(d).unwrap();
                        by_mat.entry(mat).or_default().extend_from_slice(segs);
                    }
                }
                for (mat, segs) in by_mat {
                    let m = mats.get(mat);
                    // Pixel slopes are tiny staircases; straighten them into
                    // proper slopes (within a cell) so things slide and roll.
                    let mut verts = Vec::new();
                    let mut idx = Vec::new();
                    for chain in chains(&segs) {
                        let pts = simplify(&chain, SMOOTHING);
                        for w in pts.windows(2) {
                            let n = verts.len() as u32;
                            verts.push(Vector::new(w[0].0 * CELL, w[0].1 * CELL));
                            verts.push(Vector::new(w[1].0 * CELL, w[1].1 * CELL));
                            idx.push([n, n + 1]);
                        }
                    }
                    if idx.is_empty() {
                        continue;
                    }
                    // The slipperier surface wins (anything slides on ice), and
                    // the bouncier one does too.
                    let col = ColliderBuilder::polyline(verts, Some(idx))
                        .user_data(if m.state == State::Powder { POWDER_TAG } else { 0 })
                        .friction(m.friction)
                        .restitution(m.bounciness)
                        .friction_combine_rule(CoefficientCombineRule::Min)
                        .restitution_combine_rule(CoefficientCombineRule::Max);
                    let h = self.colliders.insert_with_parent(col, self.ground, &mut self.set);
                    self.chunks[k].colliders.push(h);
                }
                // Solid blocks just inside the outline: anything forced past
                // the smooth surface (a wheel pushed by its axle, say) meets
                // solid ground and is pushed back out, instead of being
                // trapped inside a hollow shell.
                let (w, h) = (self.w, self.h);
                let deep = |x: usize, y: usize| -> Option<MatId> {
                    let (gx, gy) = (x0 + x, y0 + y);
                    if gx == 0 || gy == 0 || gx + 1 >= w || gy + 1 >= h {
                        return None;
                    }
                    let i = gy * w + gx;
                    let all = [i, i - 1, i + 1, i - w, i + w].iter().all(|&j| Self::is_terrain(world, mats, j));
                    all.then(|| world.mat[i])
                };
                for (x, y, rw, rh, mat) in rects(x1 - x0, y1 - y0, deep) {
                    let m = mats.get(mat);
                    let col = ColliderBuilder::cuboid(rw as f32 * CELL / 2.0, rh as f32 * CELL / 2.0)
                        .translation(Vector::new(
                            (x0 + x) as f32 * CELL + rw as f32 * CELL / 2.0,
                            (y0 + y) as f32 * CELL + rh as f32 * CELL / 2.0,
                        ))
                        .user_data(if m.state == State::Powder { POWDER_TAG } else { 0 })
                        .friction(m.friction)
                        .restitution(m.bounciness)
                        .friction_combine_rule(CoefficientCombineRule::Min)
                        .restitution_combine_rule(CoefficientCombineRule::Max);
                    let hd = self.colliders.insert_with_parent(col, self.ground, &mut self.set);
                    self.chunks[k].colliders.push(hd);
                }
                rebuilt.push((cx, cy));
            }
        }
        // Wake objects near changed terrain so they notice the ground is gone.
        if !rebuilt.is_empty() {
            for b in &self.bodies {
                let (bx, by, _) = self.pose(b);
                let r = (b.w.max(b.h) as f32) * 0.75 + 2.0;
                let near = rebuilt.iter().any(|&(cx, cy)| {
                    let (x0, y0) = ((cx * CHUNK) as f32, (cy * CHUNK) as f32);
                    bx + r >= x0 && bx - r <= x0 + CHUNK as f32 && by + r >= y0 && by - r <= y0 + CHUNK as f32
                });
                if near {
                    self.set[b.handle].wake_up(true);
                }
            }
        }
    }

    /// Liquids push objects up (by the weight of liquid they displace) and slow them down.
    /// Upward force (newtons) and where it acts, if the object is in liquid.
    fn buoyancy(b: &Body, world: &World, mats: &Materials) -> Option<(f32, Vector)> {
        if b.stamped.is_empty() {
            return None;
        }
        // Bounding box of the object in the world.
        let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0, usize::MAX, 0);
        for &(wi, _) in &b.stamped {
            let (x, y) = (wi as usize % world.w, wi as usize / world.w);
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
        let liquid = |x: i32, y: i32| -> Option<f32> {
            if !world.in_bounds(x, y) {
                return None;
            }
            let i = world.idx(x as usize, y as usize);
            let m = mats.get(world.mat[i]);
            (m.state == State::Liquid && !world.is_owned(i)).then_some(m.density)
        };
        // Water level just outside each side: climb from the object's bottom
        // up through the liquid to its surface.
        let level = |x: i32| -> Option<(f32, f32)> {
            let start = (y1 as i32 + 1..=y1 as i32 + 2).chain((y0 as i32..=y1 as i32).rev()).find(|&y| liquid(x, y).is_some())?;
            let rho = liquid(x, start)?;
            let mut top = start;
            while top > 0 && liquid(x, top - 1).is_some() {
                top -= 1;
            }
            Some((top as f32, rho))
        };
        let left = level(x0 as i32 - 1);
        let right = level(x1 as i32 + 1);
        let ((sl, rl), (sr, rr)) = match (left, right) {
            (Some(l), Some(r)) => (l, r),
            (Some(l), None) => (l, l),
            (None, Some(r)) => (r, r),
            (None, None) => return None,
        };
        let rho = (rl + rr) / 2.0;
        let span = (x1 as f32 - x0 as f32 + 2.0).max(1.0);
        // Cells below the (straight-line) water surface are under water.
        let (mut n, mut sx, mut sy) = (0usize, 0.0f32, 0.0f32);
        for &(wi, _) in &b.stamped {
            let (x, y) = ((wi as usize % world.w) as f32, (wi as usize / world.w) as f32);
            let t = (x - x0 as f32 + 1.0) / span;
            let surface = sl + (sr - sl) * t;
            if y >= surface {
                n += 1;
                sx += x + 0.5;
                sy += y + 0.5;
            }
        }
        if n == 0 {
            return None;
        }
        let force = n as f32 * CELL * CELL * rho * GRAVITY * b.depth;
        Some((force, Vector::new(sx / n as f32 * CELL, sy / n as f32 * CELL)))
    }

    fn is_scenery(world: &World, mats: &Materials, i: usize) -> bool {
        !world.is_owned(i) && mats.get(world.mat[i]).state == State::Solid
    }

    /// Find solid scenery that just lost its connection to the edge of the
    /// world, lift it out of the grid, and return it as new objects.
    ///
    /// Only pieces next to a cell that disappeared this frame are checked, so
    /// things painted floating in the air stay put until they're damaged.
    fn collapse(&mut self, world: &mut World, mats: &Materials) -> Vec<BodySpec> {
        let n = world.w * world.h;
        self.tick = self.tick.wrapping_add(1);
        if self.solid_prev.len() != n {
            self.solid_prev = (0..n).map(|i| Self::is_scenery(world, mats, i)).collect();
            self.strength_dirty = true;
            return self.snap_overhangs(world, mats);
        }
        // What scenery appeared or went away (only where the world isn't
        // asleep: nothing changes in sleeping blocks).
        let mut lost: Vec<usize> = Vec::new();
        let mut gained = false;
        for b in 0..world.blocks() {
            if world.block_still(b) {
                continue;
            }
            let (x0, x1, y0, y1) = world.block_cells(b);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = y * world.w + x;
                    let now = Self::is_scenery(world, mats, i);
                    let was = self.solid_prev[i];
                    if was && !now {
                        lost.push(i);
                    }
                    gained |= now && !was;
                    self.solid_prev[i] = now;
                }
            }
        }
        lost.sort_unstable();
        if !lost.is_empty() || gained {
            self.strength_dirty = true;
        }
        let mut out = Vec::new();
        // Strength is checked a few times a second, not every frame.
        // Hot scenery that softens (red-hot iron) is checked again now and
        // then, so a heated beam sags and gives way.
        if self.tick.is_multiple_of(30) && !self.strength_dirty {
            let soft: Vec<Option<f32>> = mats.list.iter().map(|m| m.softens_at).collect();
            self.strength_dirty = (0..world.blocks()).filter(|&b| !world.block_still(b)).any(|b| {
                let (x0, x1, y0, y1) = world.block_cells(b);
                (y0..y1).any(|y| (x0..x1).any(|x| {
                    let i = y * world.w + x;
                    self.solid_prev[i] && soft[world.mat[i] as usize].is_some_and(|t| world.temp[i] > t)
                }))
            });
        }
        if self.strength_dirty && self.tick.is_multiple_of(4) {
            out.extend(self.snap_overhangs(world, mats));
        }
        if lost.is_empty() {
            return out;
        }

        let (w, h) = (world.w, world.h);
        // Epoch marks: `seen` = visited this frame, `held` = known to be supported.
        self.epoch = self.epoch.wrapping_add(2).max(2);
        let (seen, held) = (self.epoch, self.epoch + 1);
        const MAX_FLOOD: usize = 40_000; // bigger than this: assume it's held up

        for &gone in &lost {
            let (gx, gy) = (gone % w, gone / w);
            let mut starts = Vec::with_capacity(4);
            if gx > 0 { starts.push(gone - 1); }
            if gx + 1 < w { starts.push(gone + 1); }
            if gy > 0 { starts.push(gone - w); }
            if gy + 1 < h { starts.push(gone + w); }
            for start in starts {
                if !self.solid_prev[start] || self.mark[start] == seen || self.mark[start] == held {
                    continue;
                }
                // Flood the connected piece; stop early if it reaches support.
                let mut piece = vec![start];
                let mut stack = vec![start];
                self.mark[start] = seen;
                let mut supported = false;
                while let Some(i) = stack.pop() {
                    let (x, y) = (i % w, i / w);
                    let anchored = world.flags[i] & crate::world::ANCHORED != 0;
                    if x == 0 || y == 0 || x + 1 == w || y + 1 == h || anchored || piece.len() > MAX_FLOOD {
                        supported = true;
                        break;
                    }
                    for j in [i - 1, i + 1, i - w, i + w] {
                        if !self.solid_prev[j] {
                            continue;
                        }
                        if self.mark[j] == held {
                            supported = true;
                            break;
                        }
                        if self.mark[j] != seen {
                            self.mark[j] = seen;
                            piece.push(j);
                            stack.push(j);
                        }
                    }
                    if supported {
                        break;
                    }
                }
                if supported {
                    for &i in &piece {
                        self.mark[i] = held;
                    }
                    continue;
                }
                // Cut loose: lift it out of the grid as an object.
                out.push(self.detach(world, &piece));
            }
        }
        out
    }

    /// Lift a piece of scenery out of the grid and describe it as an object.
    fn detach(&mut self, world: &mut World, piece: &[usize]) -> BodySpec {
        let w = world.w;
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        for &i in piece {
            x0 = x0.min(i % w);
            y0 = y0.min(i / w);
            x1 = x1.max(i % w);
            y1 = y1.max(i / w);
        }
        let (pw, ph) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut cells = vec![None; pw * ph];
        for &i in piece {
            let (x, y) = (i % w, i / w);
            cells[(y - y0) * pw + (x - x0)] = Some(BodyCell {
                mat: world.mat[i],
                temp: world.temp[i],
                burn: world.burn[i],
                life: world.life[i],
                flags: world.flags[i] & !OWNED,
                shade: world.shade[i],
                paint: world.paint[i],
            });
            world.make_air(i);
            if i < self.solid_prev.len() {
                self.solid_prev[i] = false;
            }
        }
        BodySpec {
            assembly: None,
            depth: 1.0,
            w: pw,
            h: ph,
            cells,
            x: x0 as f32 + pw as f32 / 2.0,
            y: y0 as f32 + ph as f32 / 2.0,
            angle: 0.0,
            vx: 0.0,
            vy: 0.0,
            spin: 0.0,
        }
    }

    /// Strength: how far each solid can stick out from what holds it up.
    ///
    /// Anything touching the edge of the world is held up fully, and so is
    /// anything standing on a column that goes down to it (a wall on the ground).
    /// From there, each step sideways or hanging underneath uses up one point,
    /// things sitting on top carry the same points, and no cell can have more
    /// points than its material's `strength`. Cells that run out snap off.
    /// Pieces not connected to the edge at all are left alone here (see
    /// `collapse`).
    fn snap_overhangs(&mut self, world: &mut World, mats: &Materials) -> Vec<BodySpec> {
        self.strength_dirty = false;
        let mut out = Vec::new();
        for region in Self::snap_regions(world, mats) {
            out.extend(self.snap_region(world, mats, region));
        }
        out
    }

    /// The parts of the world worth re-checking: where something has lately
    /// changed (awake sleeping-area blocks), padded by as far as the
    /// strongest material can reach, merged into boxes (x, y, w, h). In a big
    /// level that's a few small boxes, not the whole world.
    fn snap_regions(world: &World, mats: &Materials) -> Vec<(usize, usize, usize, usize)> {
        let (bw, bh) = world.block_dims();
        let reach = mats.list.iter().map(|m| m.strength).fold(0.0f32, f32::max);
        let pad = (((reach + 8.0) / 32.0).ceil() as usize).clamp(2, 8);
        let mut need = vec![false; bw * bh];
        let mut any = false;
        for b in 0..bw * bh {
            if world.block_still(b) {
                continue;
            }
            any = true;
            let (bx, by) = (b % bw, b / bw);
            for y in by.saturating_sub(pad)..=(by + pad).min(bh - 1) {
                for x in bx.saturating_sub(pad)..=(bx + pad).min(bw - 1) {
                    need[y * bw + x] = true;
                }
            }
        }
        if !any {
            return Vec::new();
        }
        if need.iter().all(|&n| n) {
            return vec![(0, 0, world.w, world.h)];
        }
        // Connected groups of needed blocks, each as its bounding box.
        let mut regions = Vec::new();
        let mut seen = vec![false; bw * bh];
        for start in 0..bw * bh {
            if !need[start] || seen[start] {
                continue;
            }
            let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
            let mut stack = vec![start];
            seen[start] = true;
            while let Some(b) = stack.pop() {
                let (bx, by) = (b % bw, b / bw);
                (x0, x1, y0, y1) = (x0.min(bx), x1.max(bx), y0.min(by), y1.max(by));
                for (ok, nb) in [(bx > 0, b.wrapping_sub(1)), (bx + 1 < bw, b + 1), (by > 0, b.wrapping_sub(bw)), (by + 1 < bh, b + bw)] {
                    if ok && need[nb] && !seen[nb] {
                        seen[nb] = true;
                        stack.push(nb);
                    }
                }
            }
            regions.push((x0 * 32, y0 * 32, ((x1 + 1) * 32).min(world.w) - x0 * 32, ((y1 + 1) * 32).min(world.h) - y0 * 32));
        }
        regions
    }

    /// `snap_overhangs` for one box of the world. The box's own edge counts
    /// as solid ground (as the world's edge does): things that really hang
    /// from further out than the padding aren't judged here.
    fn snap_region(&mut self, world: &mut World, mats: &Materials, (rx, ry, w, h): (usize, usize, usize, usize)) -> Vec<BodySpec> {
        use std::collections::BinaryHeap;
        let n = w * h;
        let ww = world.w;
        let at = move |i: usize| (ry + i / w) * ww + rx + i % w; // box cell -> world cell
        // Which cells are scenery (loose in the box: fixed solids), and each
        // one's strength, weaker when hot (`softens_at`). Per-material tables,
        // and empty air (most of the box) costs nothing.
        let solid: Vec<bool> = mats.list.iter().map(|m| m.state == State::Solid).collect();
        let base: Vec<f32> = mats.list.iter().map(|m| m.strength.max(0.0)).collect();
        let mut scenery = vec![false; n];
        let mut strength = vec![0i32; n];
        for y in 0..h {
            let row = (ry + y) * ww + rx;
            for x in 0..w {
                let wi = row + x;
                let mat = world.mat[wi] as usize;
                if solid[mat] && world.flags[wi] & OWNED == 0 {
                    scenery[y * w + x] = true;
                    let m = &mats.list[mat];
                    strength[y * w + x] = if m.softens_at.is_some() { (base[mat] * m.firmness(world.temp[wi])).round() as i32 } else { base[mat] as i32 };
                }
            }
        }
        let mut left = vec![i32::MIN; n];
        let mut heap = BinaryHeap::new();
        // The box's edge counts as ground (as the world's edge does), but only
        // where the structure there really is connected to the world's ground
        // or edge: a floating island that crosses the box's edge must not
        // look supported (it would then be judged, and snap).
        let mut edge_ground = vec![false; n];
        let (world_w, world_h) = (world.w, world.h);
        self.epoch = self.epoch.wrapping_add(4).max(4);
        let (visiting, supported, island) = (self.epoch, self.epoch + 1, self.epoch + 2);
        let ring = (0..w).map(|x| (x, 0)).chain((0..w).map(|x| (x, h - 1))).chain((1..h.saturating_sub(1)).flat_map(|y| [(0, y), (w - 1, y)]));
        for (x, y) in ring {
            let i = y * w + x;
            if !scenery[i] {
                continue;
            }
            let start = at(i);
            let real_edge = |c: usize| c.is_multiple_of(world_w) || c / world_w == 0 || c % world_w + 1 == world_w || c / world_w + 1 == world_h;
            edge_ground[i] = if real_edge(start) {
                true
            } else {
                match self.mark[start] {
                    m if m == supported => true,
                    m if m == island => false,
                    _ => {
                        // Flood out through connected scenery until it reaches
                        // the world's edge or an anchor (supported), runs out
                        // (an island), or is clearly huge (supported).
                        let mut seen = vec![start];
                        let mut stack = vec![start];
                        self.mark[start] = visiting;
                        let mut verdict = None;
                        'flood: while let Some(c) = stack.pop() {
                            if real_edge(c) || world.flags[c] & crate::world::ANCHORED != 0 {
                                verdict = Some(true);
                                break;
                            }
                            let (cx, cy) = (c % world_w, c / world_w);
                            for (ok, nb) in [(cx > 0, c.wrapping_sub(1)), (cx + 1 < world_w, c + 1), (cy > 0, c.wrapping_sub(world_w)), (cy + 1 < world_h, c + world_w)] {
                                if !ok || !Self::is_scenery(world, mats, nb) {
                                    continue;
                                }
                                match self.mark[nb] {
                                    m if m == supported => {
                                        verdict = Some(true);
                                        break 'flood;
                                    }
                                    m if m == island => {
                                        verdict = Some(false);
                                        break 'flood;
                                    }
                                    m if m == visiting => {}
                                    _ => {
                                        self.mark[nb] = visiting;
                                        seen.push(nb);
                                        stack.push(nb);
                                    }
                                }
                            }
                            if seen.len() > 3_000_000 {
                                verdict = Some(true);
                                break;
                            }
                        }
                        let ok = verdict.unwrap_or(false);
                        for c in seen {
                            self.mark[c] = if ok { supported } else { island };
                        }
                        ok
                    }
                }
            };
        }
        // Grounded: touching the edge, or standing on something grounded.
        let mut grounded = vec![false; n];
        for y in (0..h).rev() {
            for x in 0..w {
                let i = y * w + x;
                let edge = edge_ground[i];
                let anchored = world.flags[at(i)] & crate::world::ANCHORED != 0;
                if scenery[i] && (edge || anchored || grounded[i + w]) {
                    grounded[i] = true;
                    left[i] = strength[i];
                }
            }
        }
        // Only grounded cells touching scenery that isn't grounded can pass
        // anything on: the rest (the inside of the ground) needn't be queued.
        for i in 0..n {
            if !grounded[i] {
                continue;
            }
            let (x, y) = (i % w, i / w);
            let open = |j: usize| scenery[j] && !grounded[j];
            if (y > 0 && open(i - w)) || (y + 1 < h && open(i + w)) || (x > 0 && open(i - 1)) || (x + 1 < w && open(i + 1)) {
                heap.push((left[i], i));
            }
        }
        while let Some((s, i)) = heap.pop() {
            if s < left[i] {
                continue;
            }
            let (x, y) = (i % w, i / w);
            let mut try_cell = |j: usize, resting_on_me: bool| {
                if !scenery[j] {
                    return;
                }
                let cap = strength[j];
                // Attached straight to solid ground (a beam on a pillar): it
                // reaches as far as its own material allows.
                let s = if grounded[i] { cap.max(s) } else { s };
                let v = if resting_on_me { cap.min(s) } else { cap.min(s.saturating_sub(1)) };
                if v > left[j] {
                    left[j] = v;
                    heap.push((v, j));
                }
            };
            if y > 0 {
                try_cell(i - w, true); // the cell above rests on this one
            }
            if y + 1 < h {
                try_cell(i + w, false); // hanging underneath
            }
            if x > 0 {
                try_cell(i - 1, false);
            }
            if x + 1 < w {
                try_cell(i + 1, false);
            }
        }
        // Cells that ran out of strength (but are connected), grouped into pieces.
        let mut out = Vec::new();
        let mut done = vec![false; n];
        for start in 0..n {
            if done[start] || left[start] == i32::MIN || left[start] >= 0 {
                continue;
            }
            let mut piece = vec![start];
            let mut stack = vec![start];
            done[start] = true;
            while let Some(i) = stack.pop() {
                let (x, y) = (i % w, i / w);
                let mut visit = |j: usize| {
                    if !done[j] && left[j] != i32::MIN && left[j] < 0 {
                        done[j] = true;
                        piece.push(j);
                        stack.push(j);
                    }
                };
                if x > 0 {
                    visit(i - 1);
                }
                if x + 1 < w {
                    visit(i + 1);
                }
                if y > 0 {
                    visit(i - w);
                }
                if y + 1 < h {
                    visit(i + w);
                }
            }
            let piece: Vec<usize> = piece.into_iter().map(at).collect();
            out.push(self.detach(world, &piece));
        }
        out
    }

    /// Turn every solid or powder cell inside a rectangle into objects
    /// (one object per connected piece).
    pub fn make_objects(&mut self, world: &mut World, mats: &Materials, x0: i32, y0: i32, x1: i32, y1: i32) -> usize {
        let (x0, x1) = (x0.min(x1).max(0) as usize, (x0.max(x1).max(0) as usize).min(world.w - 1));
        let (y0, y1) = (y0.min(y1).max(0) as usize, (y0.max(y1).max(0) as usize).min(world.h - 1));
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut grid: Vec<Option<BodyCell>> = vec![None; w * h];
        for y in 0..h {
            for x in 0..w {
                let i = world.idx(x0 + x, y0 + y);
                if Self::is_terrain(world, mats, i) {
                    grid[y * w + x] = Some(BodyCell {
                        mat: world.mat[i],
                        temp: world.temp[i],
                        burn: world.burn[i],
                        life: world.life[i],
                        flags: world.flags[i],
                        shade: world.shade[i],
                paint: world.paint[i],
                    });
                }
            }
        }
        let pieces = components(w, h, &grid);
        let n = pieces.len();
        for piece in pieces {
            let (mut px0, mut py0, mut px1, mut py1) = (usize::MAX, usize::MAX, 0, 0);
            for &i in &piece {
                px0 = px0.min(i % w);
                py0 = py0.min(i / w);
                px1 = px1.max(i % w);
                py1 = py1.max(i / w);
            }
            let (pw, ph) = (px1 - px0 + 1, py1 - py0 + 1);
            let mut cells = vec![None; pw * ph];
            for &i in &piece {
                let (x, y) = (i % w, i / w);
                cells[(y - py0) * pw + (x - px0)] = grid[i];
                let wi = world.idx(x0 + x, y0 + y);
                world.make_air(wi);
            }
            let cx = (x0 + px0) as f32 + pw as f32 / 2.0;
            let cy = (y0 + py0) as f32 + ph as f32 / 2.0;
            self.add(world, mats, BodySpec { assembly: None, depth: 1.0, w: pw, h: ph, cells, x: cx, y: cy, angle: 0.0, vx: 0.0, vy: 0.0, spin: 0.0 });
        }
        n
    }

    // ------------------------------------------------------------ editing --

    fn handle_of(&self, id: u64) -> Option<RigidBodyHandle> {
        if id == GROUND {
            return Some(self.ground);
        }
        self.find(id).map(|n| self.bodies[n].handle)
    }

    /// Add a joint exactly as described (anchors already in each object's frame).
    pub fn add_joint(&mut self, mats: &Materials, spec: JointSpec) -> bool {
        self.insert_joint(mats, spec)
    }

    /// Make the rapier joint for a spec. Returns false if either end is gone.
    fn insert_joint(&mut self, mats: &Materials, spec: JointSpec) -> bool {
        let (Some(h1), Some(h2)) = (self.handle_of(spec.a), self.handle_of(spec.b)) else { return false };
        if h1 == h2 {
            return false;
        }
        // Wheels roll on round shapes.
        if matches!(spec.kind, JointKind::Wheel { .. })
            && let Some(n) = self.find(spec.b)
                && !self.bodies[n].round {
                    let mut b = std::mem::replace(
                        &mut self.bodies[n],
                        Body { bounds:(0,0,0,0), assembly: None, depth: 1.0, w: 0, h: 0, cells: Vec::new(), handle: h2, stamped: Vec::new(), born: 0, dirty: false, shard: false, round: false, id: 0 },
                    );
                    b.round = true;
                    self.build_colliders(&mut b, mats);
                    self.bodies[n] = b;
                }
        let a1 = Vector::new(spec.anchor_a.0 * CELL, spec.anchor_a.1 * CELL);
        let a2 = Vector::new(spec.anchor_b.0 * CELL, spec.anchor_b.1 * CELL);
        let motor = spec.motor.clone();
        let target = |m: &Motor, throttle: f32| -> f32 {
            let turns = if m.drive { m.speed * throttle } else { m.speed };
            turns * std::f32::consts::TAU
        };
        let joint: GenericJoint = match &spec.kind {
            JointKind::Hinge => {
                let mut j = GenericJointBuilder::new(JointAxesMask::LOCKED_REVOLUTE_AXES)
                    .local_anchor1(a1)
                    .local_anchor2(a2)
                    .contacts_enabled(false);
                if let Some(m) = &motor {
                    j = j.motor_velocity(JointAxis::AngX, target(m, self.throttle), MOTOR_RESPONSE).motor_max_force(JointAxis::AngX, motor_force(m, self.throttle));
                }
                j.build()
            }
            JointKind::Wheel { stiffness, damping, travel } => {
                let mut j = GenericJointBuilder::new(JointAxesMask::LIN_X)
                    .local_anchor1(a1)
                    .local_anchor2(a2)
                    .contacts_enabled(false)
                    .motor_position(JointAxis::LinY, 0.0, stiffness * 1000.0, damping * 1000.0)
                    .limits(JointAxis::LinY, [-travel * CELL, travel * CELL]);
                if let Some(m) = &motor {
                    j = j.motor_velocity(JointAxis::AngX, target(m, self.throttle), MOTOR_RESPONSE).motor_max_force(JointAxis::AngX, motor_force(m, self.throttle));
                }
                j.build()
            }
            JointKind::Weld => GenericJointBuilder::new(JointAxesMask::LOCKED_FIXED_AXES)
                .local_anchor1(a1)
                .local_anchor2(a2)
                .contacts_enabled(false)
                .build(),
            JointKind::Rope { length } => {
                let mut j: GenericJoint = RopeJointBuilder::new(length * CELL).local_anchor1(a1).local_anchor2(a2).build().into();
                j.set_contacts_enabled(true);
                j
            }
        };
        let handle = self.impulse_joints.insert(h1, h2, joint, true);
        self.joints.push(Joint { spec, handle });
        true
    }

    /// Join two things (object ids, or `GROUND`) at a point in the world
    /// (cells). For a rope, `at` is on `a` and `at_b` on `b`; otherwise both
    /// ends are at `at`. Returns false if either end doesn't exist.
    pub fn join(&mut self, mats: &Materials, a: u64, b: u64, kind: JointKind, at: (f32, f32), at_b: Option<(f32, f32)>, motor: Option<Motor>) -> bool {
        let local = |this: &Self, id: u64, p: (f32, f32)| -> Option<(f32, f32)> {
            if id == GROUND {
                return Some(p);
            }
            let n = this.find(id)?;
            let (cx, cy, ang) = this.pose(&this.bodies[n]);
            let (dx, dy) = (p.0 - cx, p.1 - cy);
            let (c, s) = (ang.cos(), ang.sin());
            Some((c * dx + s * dy, -s * dx + c * dy))
        };
        let (Some(anchor_a), Some(anchor_b)) = (local(self, a, at), local(self, b, at_b.unwrap_or(at))) else { return false };
        self.insert_joint(mats, JointSpec { a, b, kind, anchor_a, anchor_b, motor, break_force: None })
    }

    /// Set a joint failure load (kN). Invalid or non-positive values are rejected.
    pub fn set_joint_break_force(&mut self, a: u64, b: u64, force: f32) -> bool {
        if !force.is_finite() || force <= 0.0 { return false; }
        let Some(j) = self.joints.iter_mut().find(|j| (j.spec.a == a && j.spec.b == b) || (j.spec.a == b && j.spec.b == a)) else { return false };
        j.spec.break_force = Some(force);
        true
    }

    /// Release overloaded attachments after solving; detached bodies keep momentum.
    fn break_overloaded_joints(&mut self) {
        let (gone, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.joints).into_iter().partition(|j| {
            let Some(limit) = j.spec.break_force else { return false };
            self.impulse_joints.get(j.handle).is_some_and(|joint| {
                let f = joint.impulses[0].hypot(joint.impulses[1]) / DT / 1000.0;
                f > limit
            })
        });
        for j in gone { self.impulse_joints.remove(j.handle, true); }
        self.joints = keep;
    }

    /// All joints.
    pub fn joints(&self) -> Vec<JointSpec> {
        self.joints.iter().map(|j| j.spec.clone()).collect()
    }

    /// Joints attached to an object.
    pub fn joints_of(&self, id: u64) -> Vec<JointSpec> {
        self.joints.iter().filter(|j| j.spec.a == id || j.spec.b == id).map(|j| j.spec.clone()).collect()
    }

    /// Where a joint is in the world (cells), for drawing it: (end on a, end on b).
    pub fn joint_ends(&self, spec: &JointSpec) -> Option<((f32, f32), (f32, f32))> {
        let world = |id: u64, p: (f32, f32)| -> Option<(f32, f32)> {
            if id == GROUND {
                return Some(p);
            }
            let n = self.find(id)?;
            let (cx, cy, ang) = self.pose(&self.bodies[n]);
            let (dx, dy) = rot(ang.cos(), ang.sin(), p.0, p.1);
            Some((cx + dx, cy + dy))
        };
        Some((world(spec.a, spec.anchor_a)?, world(spec.b, spec.anchor_b)?))
    }

    /// Remove every joint attached to an object.
    pub fn unjoin(&mut self, id: u64) {
        let (gone, keep): (Vec<Joint>, Vec<Joint>) = std::mem::take(&mut self.joints).into_iter().partition(|j| j.spec.a == id || j.spec.b == id);
        for j in gone {
            self.impulse_joints.remove(j.handle, true);
        }
        self.joints = keep;
    }

    /// Set the player's throttle (-1 = full reverse, 1 = full ahead) for
    /// every `drive` motor.
    pub fn set_throttle(&mut self, throttle: f32) {
        let throttle = throttle.clamp(-1.0, 1.0);
        if (throttle - self.throttle).abs() < 1e-4 {
            return;
        }
        self.throttle = throttle;
        for j in &self.joints {
            if let Some(m) = j.spec.motor.as_ref().filter(|m| m.drive) {
                let v = m.speed * throttle * std::f32::consts::TAU;
                if let Some(joint) = self.impulse_joints.get_mut(j.handle, true) {
                    joint.data.set_motor_velocity(JointAxis::AngX, v, MOTOR_RESPONSE);
                    joint.data.set_motor_max_force(JointAxis::AngX, motor_force(m, throttle));
                }
            }
        }
    }

    /// Runtime arcade control: commanded velocity is not deflected by contacts.
    /// Useful for a player ship; damage and material cells still simulate.
    pub fn set_kinematic(&mut self, id: u64, enabled: bool, lock_rotation: bool) -> bool {
        let Some(n) = self.find(id) else { return false; };
        let rb = &mut self.set[self.bodies[n].handle];
        let kind = if enabled { RigidBodyType::KinematicVelocityBased } else { RigidBodyType::Dynamic };
        if rb.body_type() != kind { rb.set_body_type(kind, true); }
        rb.lock_rotations(lock_rotation, true);
        if lock_rotation { rb.set_rotation(Rotation::new(0.0), true);rb.set_angvel(0.0,true); }
        true
    }

    /// Set an object's velocity without rebuilding its cells or attachments.
    /// Speeds are cells per second; spin is radians per second.
    pub fn set_velocity(&mut self, id: u64, vx: f32, vy: f32, spin: f32) -> bool {
        if !vx.is_finite() || !vy.is_finite() || !spin.is_finite() { return false; }
        let Some(n) = self.find(id) else { return false; };
        let rb = &mut self.set[self.bodies[n].handle];
        rb.set_linvel(Vector::new(vx * CELL, vy * CELL), true);
        rb.set_angvel(spin, true);
        true
    }

    /// Apply horizontal drag to a group without rebuilding its bodies or joints.
    /// A factor of one preserves speed; zero stops horizontal movement.
    pub fn damp_horizontal(&mut self, ids: &[u64], factor: f32) {
        if !factor.is_finite() { return; }
        let factor = factor.clamp(0.0, 1.0);
        for id in ids {
            if let Some(n) = self.find(*id) {
                let rb = &mut self.set[self.bodies[n].handle];
                let v = rb.linvel();
                rb.set_linvel(Vector::new(v.x * factor, v.y), true);
            }
        }
    }

    /// Apply a bounded angular acceleration to an object (radians/s²).
    /// Useful for rider balance and other game controls; keeps joints intact.
    pub fn steer_rotation(&mut self, id: u64, acceleration: f32, max_speed: f32) {
        if !acceleration.is_finite() || !max_speed.is_finite() { return; }
        let Some(n) = self.find(id) else { return };
        let root = &self.set[self.bodies[n].handle];
        let delta = (root.angvel() + acceleration * DT).clamp(-max_speed.abs(), max_speed.abs()) - root.angvel();
        let mut rigid = std::collections::BTreeSet::from([id]);
        for _ in 0..self.joints.len() { for j in &self.joints {
            if matches!(j.spec.kind, JointKind::Weld) && j.spec.a != GROUND && j.spec.b != GROUND && (rigid.contains(&j.spec.a)||rigid.contains(&j.spec.b)) {
                rigid.insert(j.spec.a);rigid.insert(j.spec.b);
            }
        }}
        let mut assembly=rigid.clone();
        for j in &self.joints {if matches!(j.spec.kind,JointKind::Wheel{..}) && rigid.contains(&j.spec.a) {assembly.insert(j.spec.b);}}
        let handles: Vec<_> = self.bodies.iter().filter(|b|assembly.contains(&b.id)).map(|b|(b.handle,rigid.contains(&b.id))).collect();
        let mass: f32 = handles.iter().map(|(h,_)|self.set[*h].mass()).sum();
        let centre = handles.iter().fold(Vector::ZERO, |v,(h,_)|v+self.set[*h].center_of_mass()*self.set[*h].mass())/mass.max(0.001);
        for (h,is_rigid) in handles {
            let rb=&mut self.set[h];let p=rb.center_of_mass()-centre;
            rb.set_linvel(rb.linvel()+Vector::new(-p.y,p.x)*delta,true);
            if is_rigid {rb.set_angvel(rb.angvel()+delta,true);}
        }
    }

    pub fn throttle(&self) -> f32 {
        self.throttle
    }

    /// Joints whose objects are gone (broke apart, deleted) are dropped.
    fn tidy_joints(&mut self) {
        let alive: std::collections::HashSet<u64> = self.bodies.iter().map(|b| b.id).collect();
        let (keep, gone): (Vec<Joint>, Vec<Joint>) = std::mem::take(&mut self.joints)
            .into_iter()
            .partition(|j| (j.spec.a == GROUND || alive.contains(&j.spec.a)) && (j.spec.b == GROUND || alive.contains(&j.spec.b)));
        for j in gone {
            self.impulse_joints.remove(j.handle, false);
        }
        self.joints = keep;
    }

    /// Objects that lost cells (burned, chipped, broke) in the last update.
    pub fn damaged(&self) -> &[u64] {
        &self.damaged
    }

    /// Ids of all objects.
    pub fn ids(&self) -> Vec<u64> {
        self.bodies.iter().map(|b| b.id).collect()
    }

    fn find(&self, id: u64) -> Option<usize> {
        self.bodies.iter().position(|b| b.id == id)
    }

    /// Which object (if any) is drawn at this world cell.
    pub fn id_at(&self, world: &World, x: usize, y: usize) -> Option<u64> {
        let wi = world.idx(x, y) as u32;
        if !world.is_owned(wi as usize) {
            return None;
        }
        self.bodies.iter().find(|b| b.stamped.iter().any(|&(w, _)| w == wi)).map(|b| b.id)
    }

    /// The current state of one object.
    pub fn spec_of(&self, world: &World, id: u64) -> Option<BodySpec> {
        let n = self.find(id)?;
        let b = &self.bodies[n];
        let mut cells = b.cells.clone();
        for &(wi, li) in &b.stamped {
            let (wi, li) = (wi as usize, li as usize);
            if let Some(c) = cells[li].as_mut()
                && world.is_owned(wi) && world.mat[wi] == c.mat {
                    c.temp = world.temp[wi];
                    c.burn = world.burn[wi];
                    c.life = world.life[wi];
                    c.flags = world.flags[wi] & !OWNED;
                }
        }
        let (x, y, angle) = self.pose(b);
        let rb = &self.set[b.handle];
        Some(BodySpec {
            assembly: b.assembly.clone(),
            depth: b.depth,
            w: b.w,
            h: b.h,
            cells,
            x,
            y,
            angle,
            vx: rb.linvel().x / CELL,
            vy: rb.linvel().y / CELL,
            spin: rb.angvel(),
        })
    }

    /// Mass in kilograms.
    pub fn mass(&self, id: u64) -> Option<f32> {
        self.find(id).map(|n| self.set[self.bodies[n].handle].mass())
    }

    /// Take an object out of the world entirely. Returns what it was.
    pub fn remove(&mut self, world: &mut World, id: u64) -> Option<BodySpec> {
        let spec = self.spec_of(world, id)?;
        let n = self.find(id)?;
        self.unjoin(id);
        let mut b = self.bodies.remove(n);
        Self::read_back(&mut b, world, true);
        self.set.remove(
            b.handle,
            &mut self.islands,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.soft,
            true,
        );
        Some(spec)
    }

    /// Change an object: move it, turn it, resize it, recolour it... `change`
    /// gets its current state to edit; the object keeps its id. Returns false
    /// if there's no such object (or the change left it empty).
    pub fn edit(&mut self, world: &mut World, mats: &Materials, id: u64, change: impl FnOnce(&mut BodySpec)) -> bool {
        let joints = self.joints_of(id);
        let round = self.find(id).is_some_and(|n| self.bodies[n].round);
        let Some(mut spec) = self.remove(world, id) else { return false };
        change(&mut spec);
        let before = self.next_id;
        self.next_id = id;
        let added = self.add_raw(world, mats, spec);
        self.next_id = before;
        if let Some(b) = self.bodies.last_mut()
            && added.is_some() {
                b.born = self.tick.wrapping_sub(SETTLE_TICKS); // edits aren't "new pieces"
            }
        if added.is_some() {
            if round {
                let n = self.bodies.len() - 1;
                let mut b = std::mem::replace(
                    &mut self.bodies[n],
                    Body { bounds:(0,0,0,0), assembly: None, depth: 1.0, w: 0, h: 0, cells: Vec::new(), handle: RigidBodyHandle::invalid(), stamped: Vec::new(), born: 0, dirty: false, shard: false, round: false, id: 0 },
                );
                b.round = true;
                self.build_colliders(&mut b, mats);
                self.bodies[n] = b;
            }
            for j in joints {
                self.insert_joint(mats, j);
            }
        }
        added.is_some()
    }

    /// Material files changed: rebuild collision shapes so new densities,
    /// friction and bounce take effect.
    pub fn materials_changed(&mut self, mats: &Materials) {
        for c in self.chunks.iter_mut() {
            c.sig = 0;
        }
        let mut bodies = std::mem::take(&mut self.bodies);
        for b in bodies.iter_mut() {
            self.build_colliders(b, mats);
        }
        self.bodies = bodies;
        self.strength_dirty = true;
    }

    /// Current state of every object (for saving and Reset).
    pub fn export(&self, world: &mut World) -> Vec<BodySpec> {
        let mut out = Vec::new();
        for b in &self.bodies {
            let mut copy = Body {
                bounds: b.bounds,
                assembly: b.assembly.clone(),
                depth: b.depth,
                w: b.w,
                h: b.h,
                cells: b.cells.clone(),
                handle: b.handle,
                stamped: b.stamped.clone(),
                born: b.born,
                dirty: false,
                shard: false,
                round: b.round,
                id: b.id,
            };
            Self::read_back(&mut copy, world, false);
            let (x, y, angle) = self.pose(b);
            let rb = &self.set[b.handle];
            out.push(BodySpec {
                assembly: b.assembly.clone(),
                depth: b.depth,
                w: b.w,
                h: b.h,
                cells: copy.cells,
                x,
                y,
                angle,
                vx: rb.linvel().x / CELL,
                vy: rb.linvel().y / CELL,
                spin: rb.angvel(),
            });
        }
        out
    }

    /// Which object (if any) owns this world cell.
    pub fn body_at(&self, wi: usize) -> Option<usize> {
        self.bodies.iter().position(|b| b.stamped.iter().any(|&(w, _)| w as usize == wi))
    }

    /// Remove all objects from the world.
    pub fn clear(&mut self, world: &mut World) {
        for i in 0..world.mat.len() {
            if world.is_owned(i) {
                world.make_air(i);
            }
        }
        *self = Physics::new(self.w, self.h);
    }
}

/// How hard a motor may push (newton-metres). A driven motor with no throttle
/// goes nearly slack, so cars coast instead of braking.
fn motor_force(m: &Motor, throttle: f32) -> f32 {
    if m.drive && throttle.abs() < 1e-3 { m.strength * 1000.0 * 0.02 } else { m.strength * 1000.0 }
}

/// Join outline segments that share end points into chains of points.
fn chains(segs: &[((f32, f32), (f32, f32))]) -> Vec<Vec<(f32, f32)>> {
    use std::collections::VecDeque;
    let key = |p: (f32, f32)| ((p.0 * 2.0).round() as i32, (p.1 * 2.0).round() as i32);
    let mut at: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (n, &(a, b)) in segs.iter().enumerate() {
        at.entry(key(a)).or_default().push(n);
        at.entry(key(b)).or_default().push(n);
    }
    let mut used = vec![false; segs.len()];
    let mut out = Vec::new();
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut chain: VecDeque<(f32, f32)> = VecDeque::from([segs[start].0, segs[start].1]);
        // Grow the back end, then the front end, along unused segments.
        for front in [false, true] {
            loop {
                let end = if front { chain[0] } else { chain[chain.len() - 1] };
                let Some(n) = at[&key(end)].iter().copied().find(|&n| !used[n]) else { break };
                used[n] = true;
                let (p, q) = segs[n];
                let other = if key(p) == key(end) { q } else { p };
                if front {
                    chain.push_front(other);
                } else {
                    chain.push_back(other);
                }
            }
        }
        out.push(chain.into_iter().collect());
    }
    out
}

/// Straighten a chain of points (Douglas-Peucker): drop points that are
/// within `eps` cells of the straight line between their neighbours.
fn simplify(pts: &[(f32, f32)], eps: f32) -> Vec<(f32, f32)> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let (a, b) = (pts[0], pts[pts.len() - 1]);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt();
    let mut far = (0.0, 0);
    for (n, p) in pts.iter().enumerate().take(pts.len() - 1).skip(1) {
        let d = if len < 1e-6 {
            ((p.0 - a.0).powi(2) + (p.1 - a.1).powi(2)).sqrt()
        } else {
            ((p.0 - a.0) * dy - (p.1 - a.1) * dx).abs() / len
        };
        if d > far.0 {
            far = (d, n);
        }
    }
    if far.0 <= eps && len > 1e-6 {
        return vec![a, b];
    }
    let mut left = simplify(&pts[..=far.1], eps);
    let right = simplify(&pts[far.1..], eps);
    left.pop();
    left.extend(right);
    left
}

/// A hard contact: total force (kN), the touching points (cells), and the two
/// things touching (the ground or an object).
/// A hard contact: force (kN), the touching points, the two bodies, and the
/// push direction (from the first body towards the second, times force).
type Hit = (f32, Vec<(f32, f32)>, Option<RigidBodyHandle>, Option<RigidBodyHandle>, (f32, f32));

/// Which cells a crack can affect.
#[derive(Clone, Copy, PartialEq)]
enum Which {
    /// Fixed solid scenery.
    Scenery,
    /// One object's cells.
    Body(usize),
    /// Any solid: scenery or objects (explosions).
    Any,
}

impl Which {
    fn matches(self, world: &World, mats: &Materials, owners: &HashMap<u32, usize>, i: usize) -> bool {
        match self {
            Which::Scenery => !world.is_owned(i) && mats.get(world.mat[i]).state == State::Solid,
            Which::Body(n) => owners.get(&(i as u32)) == Some(&n),
            Which::Any => world.is_owned(i) || mats.get(world.mat[i]).state == State::Solid,
        }
    }
}

/// The closest cell of `which` to a point (in cells), within `reach`.
fn nearest_cell(world: &World, mats: &Materials, owners: &HashMap<u32, usize>, which: Which, px: f32, py: f32, reach: f32) -> Option<usize> {
    let ri = reach.ceil() as i32;
    let mut best: Option<(f32, usize)> = None;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let (x, y) = (px as i32 + dx, py as i32 + dy);
            if !world.in_bounds(x, y) {
                continue;
            }
            let i = world.idx(x as usize, y as usize);
            if !which.matches(world, mats, owners, i) {
                continue;
            }
            let d = (x as f32 + 0.5 - px).powi(2) + (y as f32 + 0.5 - py).powi(2);
            if d <= reach * reach && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, i));
            }
        }
    }
    best.map(|(_, i)| i)
}

/// Move a liquid cell out of an object's way: to the nearest open space
/// (usually the spot the object just left), or else up to the surface.
/// Returns where the liquid went (None if it had nowhere to go).
fn push_liquid_up(world: &mut World, mats: &Materials, x: i32, y: i32) -> Option<usize> {
    push_aside(world, mats, x, y, false)
}

/// Move the cell at (x, y) out of the way, to the nearest gas. `down_first`:
/// try the rows below first (a character climbing up through sand leaves
/// the sand behind it, under its feet).
fn push_aside(world: &mut World, mats: &Materials, x: i32, y: i32, down_first: bool) -> Option<usize> {
    let from = world.idx(x as usize, y as usize);
    let free = |world: &World, nx: i32, ny: i32| -> Option<usize> {
        if !world.in_bounds(nx, ny) {
            return None;
        }
        let i = world.idx(nx as usize, ny as usize);
        (!world.is_owned(i) && mats.get(world.mat[i]).state == State::Gas).then_some(i)
    };
    // Rings of growing size, checking upper rows first (or lower).
    for r in 1..=6i32 {
        for k in 0..=2 * r {
            let dy = if down_first { r - k } else { k - r };
            for k in 0..=r {
                for dx in [-k, k] {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    if let Some(i) = free(world, x + dx, y + dy) {
                        world.swap_cells(from, i);
                        return Some(i);
                    }
                }
            }
        }
    }
    // Deep under water: up the column to the surface.
    for ny in (0..y).rev() {
        let i = world.idx(x as usize, ny as usize);
        if world.is_owned(i) {
            continue;
        }
        match mats.get(world.mat[i]).state {
            State::Gas => {
                world.swap_cells(from, i);
                return Some(i);
            }
            State::Liquid | State::Powder => continue,
            _ => break,
        }
    }
    world.make_air(from); // nowhere to go
    None
}

/// Connected pieces (4-way) of a cell grid, as lists of indices.
fn components(w: usize, h: usize, cells: &[Option<BodyCell>]) -> Vec<Vec<usize>> {
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    for start in 0..w * h {
        if seen[start] || cells[start].is_none() {
            continue;
        }
        let mut piece = Vec::new();
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            piece.push(i);
            let (x, y) = (i % w, i / w);
            let mut visit = |j: usize| {
                if !seen[j] && cells[j].is_some() {
                    seen[j] = true;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(i - 1);
            }
            if x + 1 < w {
                visit(i + 1);
            }
            if y > 0 {
                visit(i - w);
            }
            if y + 1 < h {
                visit(i + w);
            }
        }
        out.push(piece);
    }
    out
}
