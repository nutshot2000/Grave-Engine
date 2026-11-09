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
//!
//! Units: one cell is 10 cm, so a 10-cell crate is a metre wide.

use crate::material::{MatId, Materials, State};
use crate::world::{OWNED, World};
use rapier2d::prelude::*;

/// Metres per cell.
pub const CELL: f32 = 0.1;
const CHUNK: usize = 32;
const GRAVITY: f32 = 9.81;
const DT: f32 = 1.0 / 60.0;

/// One cell of an object, with all its simulation state.
#[derive(Clone, Copy, Debug)]
pub struct BodyCell {
    pub mat: MatId,
    pub temp: f32,
    pub burn: u16,
    pub life: u16,
    pub flags: u8,
    pub shade: u8,
}

/// An object's shape and where it is. Used to create objects, save them and
/// restore them (for Reset).
#[derive(Clone, Debug)]
pub struct BodySpec {
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
                    shade: (hash(i as u32 ^ (cx as u32).wrapping_mul(7919) ^ (cy as u32).wrapping_mul(104729)) >> 24) as u8,
                })
            })
            .collect();
        BodySpec { w, h, cells, x: cx, y: cy, angle: 0.0, vx: 0.0, vy: 0.0, spin: 0.0 }
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
    w: usize,
    h: usize,
    cells: Vec<Option<BodyCell>>,
    handle: RigidBodyHandle,
    /// (world cell index, local cell index) written by the last stamp.
    stamped: Vec<(u32, u32)>,
}

struct Chunk {
    sig: u64,
    colliders: Vec<ColliderHandle>,
}

pub struct Physics {
    bodies: Vec<Body>,
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

    pub fn body_count(&self) -> usize {
        self.bodies.len()
    }

    /// Add an object and put it into the world straight away.
    pub fn add(&mut self, world: &mut World, mats: &Materials, spec: BodySpec) {
        if spec.cell_count() == 0 {
            return;
        }
        let rb = RigidBodyBuilder::dynamic()
            .translation(Vector::new(spec.x * CELL, spec.y * CELL))
            .rotation(spec.angle)
            .linvel(Vector::new(spec.vx * CELL, spec.vy * CELL))
            .angvel(spec.spin)
            .ccd_enabled(true);
        let handle = self.set.insert(rb);
        let mut body = Body { w: spec.w, h: spec.h, cells: spec.cells, handle, stamped: Vec::new() };
        self.build_colliders(&mut body, mats);
        self.stamp(&mut body, world, mats);
        self.bodies.push(body);
    }

    fn build_colliders(&mut self, body: &mut Body, mats: &Materials) {
        let old: Vec<ColliderHandle> = self.set[body.handle].colliders().to_vec();
        for c in old {
            self.colliders.remove(c, &mut self.islands, &mut self.set, &mut self.soft, true);
        }
        let (w, h) = (body.w, body.h);
        let cells = &body.cells;
        for (x, y, rw, rh, mat) in rects(w, h, |x, y| cells[y * w + x].map(|c| c.mat)) {
            let cx = (x as f32 + rw as f32 / 2.0 - w as f32 / 2.0) * CELL;
            let cy = (y as f32 + rh as f32 / 2.0 - h as f32 / 2.0) * CELL;
            let m = mats.get(mat);
            let col = ColliderBuilder::cuboid(rw as f32 * CELL / 2.0, rh as f32 * CELL / 2.0)
                .translation(Vector::new(cx, cy))
                .density(m.density.max(50.0))
                .friction(0.7)
                .restitution(0.05);
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
        let r = (hw * hw + hh * hh).sqrt() + 1.0;
        let (x0, x1) = ((cx - r).floor() as i32, (cx + r).ceil() as i32);
        let (y0, y1) = ((cy - r).floor() as i32, (cy + r).ceil() as i32);
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
                match mats.get(world.mat[wi]).state {
                    State::Solid | State::Powder => continue, // terrain wins
                    State::Liquid => push_liquid_up(world, mats, wx, wy),
                    State::Gas => {}
                }
                world.mat[wi] = cell.mat;
                world.temp[wi] = cell.temp;
                world.burn[wi] = cell.burn;
                world.life[wi] = cell.life;
                world.flags[wi] = cell.flags | OWNED;
                world.shade[wi] = cell.shade;
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
        // 1. Read back and lift every object out of the grid.
        let mut bodies = std::mem::take(&mut self.bodies);
        let mut changed = Vec::new();
        for (n, b) in bodies.iter_mut().enumerate() {
            if Self::read_back(b, world, true) {
                changed.push(n);
            }
        }
        // Objects that lost cells may have split into pieces.
        let mut keep = Vec::with_capacity(bodies.len());
        let mut new_specs = Vec::new();
        for (n, mut b) in bodies.into_iter().enumerate() {
            if !changed.contains(&n) {
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

        // 2. Scenery that lost its support breaks loose.
        new_specs.extend(self.collapse(world, mats));

        // 3. Terrain colliders for whatever changed.
        self.update_terrain(world, mats);

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
            &(),
            &(),
        );

        // 4. Put everything back into the grid (new pieces too).
        let mut bodies = std::mem::take(&mut self.bodies);
        for b in bodies.iter_mut() {
            let rb = &mut self.set[b.handle];
            rb.reset_forces(false);
            rb.reset_torques(false);
            self.stamp(b, world, mats);
        }
        self.bodies = bodies;
        for spec in new_specs {
            self.add(world, mats, spec);
        }
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
        !world.is_owned(i) && matches!(mats.get(world.mat[i]).state, State::Solid | State::Powder)
    }

    fn update_terrain(&mut self, world: &World, mats: &Materials) {
        let mut rebuilt: Vec<(usize, usize)> = Vec::new();
        for cy in 0..self.ch {
            for cx in 0..self.cw {
                let (x0, y0) = (cx * CHUNK, cy * CHUNK);
                let (x1, y1) = ((x0 + CHUNK).min(self.w), (y0 + CHUNK).min(self.h));
                // Fingerprint of which cells are solid.
                let mut sig: u64 = 0xcbf2_9ce4_8422_2325;
                for y in y0..y1 {
                    let mut bits: u64 = 0;
                    for x in x0..x1 {
                        bits = (bits << 1) | Self::is_terrain(world, mats, y * self.w + x) as u64;
                    }
                    sig = (sig ^ bits).wrapping_mul(0x0100_0000_01b3);
                }
                let k = cy * self.cw + cx;
                if self.chunks[k].sig == sig {
                    continue;
                }
                self.chunks[k].sig = sig;
                for c in std::mem::take(&mut self.chunks[k].colliders) {
                    self.colliders.remove(c, &mut self.islands, &mut self.set, &mut self.soft, false);
                }
                let (cw, chh) = (x1 - x0, y1 - y0);
                let ww = self.w;
                for (x, y, rw, rh, _) in rects(cw, chh, |x, y| Self::is_terrain(world, mats, (y0 + y) * ww + x0 + x).then_some(())) {
                    let col = ColliderBuilder::cuboid(rw as f32 * CELL / 2.0, rh as f32 * CELL / 2.0)
                        .translation(Vector::new(
                            (x0 + x) as f32 * CELL + rw as f32 * CELL / 2.0,
                            (y0 + y) as f32 * CELL + rh as f32 * CELL / 2.0,
                        ))
                        .friction(0.8);
                    let h = self.colliders.insert_with_parent(col, self.ground, &mut self.set);
                    self.chunks[k].colliders.push(h);
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
        let force = n as f32 * CELL * CELL * rho * GRAVITY;
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
        let now: Vec<bool> = (0..n).map(|i| Self::is_scenery(world, mats, i)).collect();
        self.tick = self.tick.wrapping_add(1);
        if self.solid_prev.len() != n {
            self.solid_prev = now;
            self.strength_dirty = true;
            return self.snap_overhangs(world, mats);
        }
        let lost: Vec<usize> = (0..n).filter(|&i| self.solid_prev[i] && !now[i]).collect();
        if lost.len() > 0 || (0..n).any(|i| now[i] && !self.solid_prev[i]) {
            self.strength_dirty = true;
        }
        self.solid_prev = now;
        let mut out = Vec::new();
        // Strength is checked a few times a second, not every frame.
        if self.strength_dirty && self.tick % 4 == 0 {
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
                    if x == 0 || y == 0 || x + 1 == w || y + 1 == h || piece.len() > MAX_FLOOD {
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
                flags: world.flags[i],
                shade: world.shade[i],
            });
            world.make_air(i);
            if i < self.solid_prev.len() {
                self.solid_prev[i] = false;
            }
        }
        BodySpec {
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
        use std::collections::BinaryHeap;
        self.strength_dirty = false;
        let (w, h) = (world.w, world.h);
        let n = w * h;
        let strength: Vec<i32> = mats.list.iter().map(|m| m.strength.max(0.0).round() as i32).collect();
        let scenery: Vec<bool> = (0..n).map(|i| Self::is_scenery(world, mats, i)).collect();
        let mut left = vec![i32::MIN; n];
        let mut heap = BinaryHeap::new();
        // Grounded: touching the edge, or standing on something grounded.
        let mut grounded = vec![false; n];
        for y in (0..h).rev() {
            for x in 0..w {
                let i = y * w + x;
                let edge = x == 0 || y == 0 || x + 1 == w || y + 1 == h;
                if scenery[i] && (edge || grounded[i + w]) {
                    grounded[i] = true;
                    left[i] = strength[world.mat[i] as usize];
                    heap.push((left[i], i));
                }
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
                let cap = strength[world.mat[j] as usize];
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
            self.add(world, mats, BodySpec { w: pw, h: ph, cells, x: cx, y: cy, angle: 0.0, vx: 0.0, vy: 0.0, spin: 0.0 });
        }
        n
    }

    /// Current state of every object (for saving and Reset).
    pub fn export(&self, world: &mut World) -> Vec<BodySpec> {
        let mut out = Vec::new();
        for b in &self.bodies {
            let mut copy = Body { w: b.w, h: b.h, cells: b.cells.clone(), handle: b.handle, stamped: b.stamped.clone() };
            Self::read_back(&mut copy, world, false);
            let (x, y, angle) = self.pose(b);
            let rb = &self.set[b.handle];
            out.push(BodySpec {
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

/// Move a liquid cell out of an object's way: to the nearest open space
/// (usually the spot the object just left), or else up to the surface.
fn push_liquid_up(world: &mut World, mats: &Materials, x: i32, y: i32) {
    let from = world.idx(x as usize, y as usize);
    let free = |world: &World, nx: i32, ny: i32| -> Option<usize> {
        if !world.in_bounds(nx, ny) {
            return None;
        }
        let i = world.idx(nx as usize, ny as usize);
        (!world.is_owned(i) && mats.get(world.mat[i]).state == State::Gas).then_some(i)
    };
    // Rings of growing size, checking upper rows first.
    for r in 1..=6i32 {
        for dy in -r..=r {
            for k in 0..=r {
                for dx in [-k, k] {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    if let Some(i) = free(world, x + dx, y + dy) {
                        world.swap_cells(from, i);
                        return;
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
                return;
            }
            State::Liquid => continue,
            _ => break,
        }
    }
    world.make_air(from); // nowhere to go
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
