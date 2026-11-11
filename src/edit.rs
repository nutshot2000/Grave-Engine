//! Editing: areas of scenery, flipping/turning/resizing, and undo history.
//!
//! Everything here works on a running `Sim`, so the editor (and later, AI
//! agents through the command channel) can change the world at any time.

use crate::material::{AIR, MatId, Materials, State};
use crate::physics::{BodyCell, BodySpec};
use crate::sim::{Sim, Snapshot};
use crate::template::Instance;
use crate::world::{OWNED, World};

/// A rectangle of cells, corners included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Region {
    /// From any two corners, clipped to the world.
    pub fn new(world: &World, ax: i32, ay: i32, bx: i32, by: i32) -> Self {
        let (w, h) = (world.w as i32 - 1, world.h as i32 - 1);
        Region {
            x0: ax.min(bx).clamp(0, w),
            y0: ay.min(by).clamp(0, h),
            x1: ax.max(bx).clamp(0, w),
            y1: ay.max(by).clamp(0, h),
        }
    }
    pub fn w(&self) -> usize {
        (self.x1 - self.x0 + 1) as usize
    }
    pub fn h(&self) -> usize {
        (self.y1 - self.y0 + 1) as usize
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1
    }
    pub fn moved(&self, dx: i32, dy: i32) -> Self {
        Region { x0: self.x0 + dx, y0: self.y0 + dy, x1: self.x1 + dx, y1: self.y1 + dy }
    }
}

/// A cut-out piece of the world (scenery, sand, water... not objects).
#[derive(Clone, Debug)]
pub struct Clip {
    pub w: usize,
    pub h: usize,
    pub cells: Vec<Option<BodyCell>>,
}

fn cell_at(world: &World, i: usize) -> BodyCell {
    BodyCell {
        mat: world.mat[i],
        temp: world.temp[i],
        burn: world.burn[i],
        life: world.life[i],
        flags: world.flags[i] & !OWNED,
        shade: world.shade[i],
    }
}

fn put(world: &mut World, i: usize, c: BodyCell) {
    world.wake_at(i);
    world.mat[i] = c.mat;
    world.temp[i] = c.temp;
    world.burn[i] = c.burn;
    world.life[i] = c.life;
    world.flags[i] = c.flags & !OWNED;
    world.shade[i] = c.shade;
}

/// Copy everything that isn't air (or an object) inside a region.
pub fn copy_region(world: &World, r: Region) -> Clip {
    let (w, h) = (r.w(), r.h());
    let mut cells = vec![None; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = world.idx(r.x0 as usize + x, r.y0 as usize + y);
            if world.mat[i] != AIR && !world.is_owned(i) {
                cells[y * w + x] = Some(cell_at(world, i));
            }
        }
    }
    Clip { w, h, cells }
}

/// Cut a region out, leaving air (objects are left alone).
pub fn cut_region(world: &mut World, r: Region) -> Clip {
    let clip = copy_region(world, r);
    for y in r.y0..=r.y1 {
        for x in r.x0..=r.x1 {
            let i = world.idx(x as usize, y as usize);
            if !world.is_owned(i) && world.mat[i] != AIR {
                let t = world.temp[i];
                world.make_air(i);
                world.temp[i] = t;
            }
        }
    }
    clip
}

/// Paste a clip with its top-left corner at (x0, y0). Empty parts of the
/// clip don't overwrite anything; objects aren't painted over.
pub fn paste_clip(world: &mut World, clip: &Clip, x0: i32, y0: i32) {
    for y in 0..clip.h {
        for x in 0..clip.w {
            let Some(c) = clip.cells[y * clip.w + x] else { continue };
            let (wx, wy) = (x0 + x as i32, y0 + y as i32);
            if !world.in_bounds(wx, wy) {
                continue;
            }
            let i = world.idx(wx as usize, wy as usize);
            if !world.is_owned(i) {
                put(world, i, c);
            }
        }
    }
}

/// Turn every non-air cell in a region (or of an object) into another material.
pub fn recolor(cells: &mut [Option<BodyCell>], mats: &Materials, to: MatId) {
    let t = mats.get(to).temperature;
    for c in cells.iter_mut().flatten() {
        c.mat = to;
        c.temp = t;
        c.burn = 0;
        c.life = 0;
        c.flags = 0;
    }
}

/// Flip, turn and resize grids of cells (clips and objects alike).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Transform {
    FlipH,
    FlipV,
    /// A quarter turn clockwise.
    Turn,
    /// Resize by this factor (nearest-cell, so pixel art stays crisp).
    Scale(f32),
}

pub fn transform_grid(w: usize, h: usize, cells: &[Option<BodyCell>], t: Transform) -> (usize, usize, Vec<Option<BodyCell>>) {
    match t {
        Transform::FlipH => {
            let mut out = vec![None; w * h];
            for y in 0..h {
                for x in 0..w {
                    out[y * w + (w - 1 - x)] = cells[y * w + x];
                }
            }
            (w, h, out)
        }
        Transform::FlipV => {
            let mut out = vec![None; w * h];
            for y in 0..h {
                for x in 0..w {
                    out[(h - 1 - y) * w + x] = cells[y * w + x];
                }
            }
            (w, h, out)
        }
        Transform::Turn => {
            // New grid is h wide and w tall.
            let mut out = vec![None; w * h];
            for y in 0..h {
                for x in 0..w {
                    out[x * h + (h - 1 - y)] = cells[y * w + x];
                }
            }
            (h, w, out)
        }
        Transform::Scale(f) => {
            let f = f.clamp(0.05, 20.0);
            let nw = ((w as f32 * f).round() as usize).clamp(1, 2048);
            let nh = ((h as f32 * f).round() as usize).clamp(1, 2048);
            let mut out = vec![None; nw * nh];
            for y in 0..nh {
                for x in 0..nw {
                    let sx = ((x as f32 + 0.5) * w as f32 / nw as f32) as usize;
                    let sy = ((y as f32 + 0.5) * h as f32 / nh as f32) as usize;
                    out[y * nw + x] = cells[sy.min(h - 1) * w + sx.min(w - 1)];
                }
            }
            (nw, nh, out)
        }
    }
}

impl Clip {
    pub fn transform(&mut self, t: Transform) {
        let (w, h, cells) = transform_grid(self.w, self.h, &self.cells, t);
        *self = Clip { w, h, cells };
    }
}

/// Apply a flip/turn/resize to an object, keeping its centre where it is.
pub fn transform_body(spec: &mut BodySpec, t: Transform) {
    let (w, h, cells) = transform_grid(spec.w, spec.h, &spec.cells, t);
    spec.w = w;
    spec.h = h;
    spec.cells = cells;
}

/// Flip, turn or resize an area of scenery in place (around its centre).
/// Returns the area the result now covers.
pub fn transform_region(world: &mut World, r: Region, t: Transform) -> Region {
    let mut clip = cut_region(world, r);
    clip.transform(t);
    let (cx, cy) = ((r.x0 + r.x1) as f32 / 2.0, (r.y0 + r.y1) as f32 / 2.0);
    let x0 = (cx - (clip.w as f32 - 1.0) / 2.0).round() as i32;
    let y0 = (cy - (clip.h as f32 - 1.0) / 2.0).round() as i32;
    paste_clip(world, &clip, x0, y0);
    Region::new(world, x0, y0, x0 + clip.w as i32 - 1, y0 + clip.h as i32 - 1)
}

/// What's in an area: (material name, cell count), most common first.
pub fn census(cells: impl Iterator<Item = MatId>, mats: &Materials) -> Vec<(String, usize)> {
    let mut count = vec![0usize; mats.list.len()];
    for m in cells {
        if (m as usize) < count.len() {
            count[m as usize] += 1;
        }
    }
    let mut out: Vec<(String, usize)> = count
        .iter()
        .enumerate()
        .filter(|(i, n)| **n > 0 && *i as MatId != AIR)
        .map(|(i, n)| (mats.list[i].name.clone(), *n))
        .collect();
    out.sort_by(|a, b| b.1.cmp(&a.1));
    out
}

/// Is a cell something the Select tool should treat as scenery (not air or gas)?
pub fn is_stuff(world: &World, mats: &Materials, i: usize) -> bool {
    !world.is_owned(i) && mats.get(world.mat[i]).state != State::Gas
}

// ------------------------------------------------------------------ undo --

/// One cell's full state, for undo.
#[derive(Clone, Copy, PartialEq)]
struct CellState {
    mat: MatId,
    temp: f32,
    burn: u16,
    life: u16,
    flags: u8,
    shade: u8,
    tex: u16,
}

fn state(w: &World, i: usize) -> CellState {
    CellState { mat: w.mat[i], temp: w.temp[i], burn: w.burn[i], life: w.life[i], flags: w.flags[i], shade: w.shade[i], tex: w.tex[i] }
}

fn restore_state(w: &mut World, i: usize, s: CellState) {
    w.mat[i] = s.mat;
    w.temp[i] = s.temp;
    w.burn[i] = s.burn;
    w.life[i] = s.life;
    w.flags[i] = s.flags;
    w.shade[i] = s.shade;
    w.tex[i] = s.tex;
}

/// One undoable step: only the cells that changed, plus the objects before
/// and after (objects are small).
struct Step {
    label: String,
    /// (cell, before, after)
    cells: Vec<(u32, CellState, CellState)>,
    /// A different-sized world (loading a scene): keep the whole thing.
    whole: Option<(Snapshot, Snapshot)>,
    bodies_before: Vec<BodySpec>,
    bodies_after: Vec<BodySpec>,
    links_before: Vec<Option<Instance>>,
    links_after: Vec<Option<Instance>>,
    joints_before: Vec<crate::physics::JointSpec>,
    joints_after: Vec<crate::physics::JointSpec>,
    follow_before: Option<u64>,
    follow_after: Option<u64>,
    /// Lighting (lamps on objects numbered like joints).
    lighting_before: crate::light::Lighting,
    lighting_after: crate::light::Lighting,
    backdrop_before: crate::backdrop::Backdrop,
    backdrop_after: crate::backdrop::Backdrop,
}

/// Undo and redo for everything the editor does.
///
/// Call `touch` just before changing the world. Changes are gathered into one
/// step until you call `commit` (or nothing changes for a moment and you call
/// `commit_if_idle`), so a whole paint stroke undoes in one go.
#[derive(Default)]
pub struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
    pending: Option<(Snapshot, String)>,
    last_touch: Option<std::time::Instant>,
}

/// Steps kept. Each step only stores what changed, so this is plenty.
const MAX_STEPS: usize = 2000;

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// About to change something: remember how things were (once per step).
    pub fn touch(&mut self, sim: &mut Sim, label: &str) {
        // Edited by hand: everything wakes up and gets looked at again.
        sim.world.wake_all();
        if self.pending.is_none() {
            self.pending = Some((sim.snapshot(), label.to_string()));
        }
        self.last_touch = Some(std::time::Instant::now());
    }

    /// Is a step being gathered?
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    /// Finish the current step if nothing has changed for `quiet` seconds.
    pub fn commit_if_idle(&mut self, sim: &mut Sim, quiet: f32) {
        if self.last_touch.is_some_and(|t| t.elapsed().as_secs_f32() >= quiet) {
            self.commit(sim);
        }
    }

    /// Finish the current step now.
    pub fn commit(&mut self, sim: &mut Sim) {
        let Some((before, label)) = self.pending.take() else { return };
        self.last_touch = None;
        let after = sim.snapshot();
        let step = if before.world.mat.len() != after.world.mat.len() {
            Step {
                label,
                cells: Vec::new(),
                whole: Some((before, after)),
                bodies_before: Vec::new(),
                bodies_after: Vec::new(),
                links_before: Vec::new(),
                links_after: Vec::new(),
                joints_before: Vec::new(),
                joints_after: Vec::new(),
                follow_before: None,
                follow_after: None,
                lighting_before: Default::default(),
                lighting_after: Default::default(),
                backdrop_before: Default::default(),
                backdrop_after: Default::default(),
            }
        } else {
            let mut cells = Vec::new();
            for i in 0..after.world.mat.len() {
                let (a, b) = (state(&before.world, i), state(&after.world, i));
                if a != b {
                    cells.push((i as u32, a, b));
                }
            }
            if cells.is_empty()
                && before.bodies.len() == after.bodies.len()
                && same_bodies(&before.bodies, &after.bodies)
                && before.instances == after.instances
                && before.joints == after.joints
                && before.follow == after.follow
                && before.lighting == after.lighting
                && before.backdrop == after.backdrop
            {
                return; // nothing actually changed
            }
            Step {
                label,
                cells,
                whole: None,
                bodies_before: before.bodies,
                bodies_after: after.bodies,
                links_before: before.instances,
                links_after: after.instances,
                joints_before: before.joints,
                joints_after: after.joints,
                follow_before: before.follow,
                follow_after: after.follow,
                lighting_before: before.lighting,
                lighting_after: after.lighting,
                backdrop_before: before.backdrop,
                backdrop_after: after.backdrop,
            }
        };
        self.undo.push(step);
        if self.undo.len() > MAX_STEPS {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.pending.is_some()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    /// What the next undo would undo.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|s| s.label.as_str())
    }
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|s| s.label.as_str())
    }
    pub fn len(&self) -> usize {
        self.undo.len()
    }
    pub fn is_empty(&self) -> bool {
        self.undo.is_empty()
    }

    /// Undo the last step. Returns its label.
    pub fn undo(&mut self, sim: &mut Sim, mats: &Materials) -> Option<String> {
        self.commit(sim);
        let step = self.undo.pop()?;
        apply(sim, mats, &step, false);
        let label = step.label.clone();
        self.redo.push(step);
        Some(label)
    }

    /// Redo the last undone step. Returns its label.
    pub fn redo(&mut self, sim: &mut Sim, mats: &Materials) -> Option<String> {
        self.commit(sim);
        let step = self.redo.pop()?;
        apply(sim, mats, &step, true);
        let label = step.label.clone();
        self.undo.push(step);
        Some(label)
    }
}

fn same_bodies(a: &[BodySpec], b: &[BodySpec]) -> bool {
    a.iter().zip(b).all(|(a, b)| {
        a.w == b.w
            && a.h == b.h
            && (a.x - b.x).abs() < 0.01
            && (a.y - b.y).abs() < 0.01
            && (a.angle - b.angle).abs() < 0.001
            && a.cells.iter().zip(&b.cells).all(|(p, q)| p.map(|c| c.mat) == q.map(|c| c.mat))
    })
}

/// Put the world into the before (or after) state of a step.
fn apply(sim: &mut Sim, mats: &Materials, step: &Step, forward: bool) {
    if let Some((before, after)) = &step.whole {
        *sim = Sim::restore(if forward { after } else { before }, mats);
        return;
    }
    let mut snap = sim.snapshot();
    for &(i, before, after) in &step.cells {
        restore_state(&mut snap.world, i as usize, if forward { after } else { before });
    }
    // Objects are restored as a whole set; the cells above never include them.
    for i in 0..snap.world.mat.len() {
        if snap.world.flags[i] & OWNED != 0 {
            snap.world.make_air(i);
        }
    }
    snap.bodies = if forward { step.bodies_after.clone() } else { step.bodies_before.clone() };
    snap.instances = if forward { step.links_after.clone() } else { step.links_before.clone() };
    snap.joints = if forward { step.joints_after.clone() } else { step.joints_before.clone() };
    snap.follow = if forward { step.follow_after } else { step.follow_before };
    snap.lighting = if forward { step.lighting_after.clone() } else { step.lighting_before.clone() };
    snap.backdrop = if forward { step.backdrop_after.clone() } else { step.backdrop_before.clone() };
    for b in snap.bodies.iter_mut() {
        b.vx = 0.0;
        b.vy = 0.0;
        b.spin = 0.0;
    }
    *sim = Sim::restore(&snap, mats);
}

/// Paint an object's cells into the world as fixed scenery, exactly where
/// it's drawn (used to "freeze" an object in place).
pub fn freeze(world: &mut World, spec: &BodySpec) {
    let (c, s) = (spec.angle.cos(), spec.angle.sin());
    let (hw, hh) = (spec.w as f32 / 2.0, spec.h as f32 / 2.0);
    let r = (hw * hw + hh * hh).sqrt() + 1.0;
    for wy in (spec.y - r).floor() as i32..=(spec.y + r).ceil() as i32 {
        for wx in (spec.x - r).floor() as i32..=(spec.x + r).ceil() as i32 {
            if !world.in_bounds(wx, wy) {
                continue;
            }
            let (dx, dy) = (wx as f32 + 0.5 - spec.x, wy as f32 + 0.5 - spec.y);
            let (lx, ly) = (c * dx + s * dy + hw, -s * dx + c * dy + hh);
            if lx < 0.0 || ly < 0.0 || lx as usize >= spec.w || ly as usize >= spec.h {
                continue;
            }
            if let Some(cell) = spec.cells[ly as usize * spec.w + lx as usize] {
                let i = world.idx(wx as usize, wy as usize);
                if !world.is_owned(i) {
                    put(world, i, cell);
                }
            }
        }
    }
}

// ---------------------------------------------------------------- rewind --

/// One recorded moment: the cells that changed since the moment before
/// (or every cell, for a keyframe), plus all the objects.
struct Moment {
    frame: u32,
    keyframe: Option<Vec<CellState>>,
    cells: Vec<(u32, CellState)>,
    flying: Vec<crate::world::Flyer>,
    actors: Vec<crate::physics::ActorSpec>,
    bodies: Vec<BodySpec>,
    links: Vec<Option<Instance>>,
    joints: Vec<crate::physics::JointSpec>,
    follow: Option<u64>,
}

/// Records what happens during Play so you can scrub back to any moment.
///
/// Every `every` frames a moment is stored. Normally only the cells that
/// changed are kept; every `KEYFRAME_EVERY` moments the whole grid is kept, so
/// scrubbing never has to replay far. When it gets too big, the oldest
/// stretch is dropped.
pub struct Timeline {
    moments: Vec<Moment>,
    /// The grid as of the newest moment (what changes are compared against).
    last: Vec<CellState>,
    pub every: u32,
    counter: u32,
    bytes: usize,
}

const KEYFRAME_EVERY: usize = 40;
/// Rough memory limit for the recording.
const TIMELINE_BYTES: usize = 400 * 1024 * 1024;
/// Temperature changes smaller than this (°C) aren't worth recording.
const TEMP_STEP: f32 = 1.0;

impl Default for Timeline {
    fn default() -> Self {
        Timeline { moments: Vec::new(), last: Vec::new(), every: 6, counter: 0, bytes: 0 }
    }
}

impl Timeline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.moments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.moments.is_empty()
    }

    /// The simulation frame number of a recorded moment.
    pub fn frame(&self, i: usize) -> Option<u32> {
        self.moments.get(i).map(|m| m.frame)
    }

    /// The newest moment's frame number.
    pub fn last_frame(&self) -> Option<u32> {
        self.moments.last().map(|m| m.frame)
    }

    pub fn clear(&mut self) {
        *self = Timeline { every: self.every, ..Timeline::default() };
    }

    /// Forget everything after moment `i` (you went back and carried on).
    pub fn truncate_after(&mut self, i: usize, sim: &mut Sim) {
        if i + 1 >= self.moments.len() {
            return;
        }
        self.moments.truncate(i + 1);
        self.bytes = self.moments.iter().map(moment_bytes).sum();
        // Rebuild `last` to match the moment we're continuing from.
        if let Some(cells) = self.cells_at(i) {
            self.last = cells;
        } else {
            self.last = (0..sim.world.mat.len()).map(|c| state(&sim.world, c)).collect();
        }
        self.counter = 0;
    }

    /// Call after every simulation step: stores a moment every `every` frames.
    pub fn record(&mut self, sim: &mut Sim) {
        let force = self.moments.is_empty();
        self.counter += 1;
        if !force && self.counter < self.every {
            return;
        }
        self.counter = 0;
        let snap = sim.snapshot();
        let w = &snap.world;
        let n = w.mat.len();
        let key = force || self.last.len() != n || self.moments.len() % KEYFRAME_EVERY == 0;
        let moment = if key {
            let all: Vec<CellState> = (0..n).map(|i| state(w, i)).collect();
            self.last = all.clone();
            Moment { frame: w.frame, keyframe: Some(all), cells: Vec::new(), flying: w.flying.clone(), actors: snap.actors.clone(), bodies: snap.bodies, links: snap.instances, joints: snap.joints, follow: snap.follow }
        } else {
            let mut cells = Vec::new();
            for i in 0..n {
                let now = state(w, i);
                let was = self.last[i];
                let same = now.mat == was.mat
                    && now.flags == was.flags
                    && now.burn == was.burn
                    && now.life == was.life
                    && now.shade == was.shade
                    && (now.temp - was.temp).abs() < TEMP_STEP;
                if !same {
                    cells.push((i as u32, now));
                    self.last[i] = now;
                }
            }
            Moment { frame: w.frame, keyframe: None, cells, flying: w.flying.clone(), actors: snap.actors.clone(), bodies: snap.bodies, links: snap.instances, joints: snap.joints, follow: snap.follow }
        };
        self.bytes += moment_bytes(&moment);
        self.moments.push(moment);
        // Too big? Drop the oldest stretch up to the next keyframe.
        while self.bytes > TIMELINE_BYTES && self.moments.len() > KEYFRAME_EVERY {
            let next_key = self.moments.iter().skip(1).position(|m| m.keyframe.is_some()).map(|p| p + 1);
            let Some(k) = next_key else { break };
            for m in self.moments.drain(..k) {
                self.bytes -= moment_bytes(&m);
            }
        }
    }

    /// The whole grid at moment `i`.
    fn cells_at(&self, i: usize) -> Option<Vec<CellState>> {
        let key = (0..=i).rev().find(|&k| self.moments[k].keyframe.is_some())?;
        let mut cells = self.moments[key].keyframe.clone()?;
        for m in &self.moments[key + 1..=i] {
            for &(c, s) in &m.cells {
                cells[c as usize] = s;
            }
        }
        Some(cells)
    }

    /// Put the simulation back to moment `i`.
    pub fn restore(&self, i: usize, sim: &mut Sim, mats: &Materials) -> bool {
        let (Some(m), Some(cells)) = (self.moments.get(i), self.cells_at(i.min(self.moments.len().saturating_sub(1)))) else {
            return false;
        };
        let mut snap = sim.snapshot();
        if cells.len() != snap.world.mat.len() {
            return false;
        }
        for (c, s) in cells.into_iter().enumerate() {
            restore_state(&mut snap.world, c, s);
        }
        snap.world.frame = m.frame;
        snap.world.flying = m.flying.clone();
        snap.actors = m.actors.clone();
        snap.bodies = m.bodies.clone();
        snap.instances = m.links.clone();
        snap.joints = m.joints.clone();
        snap.follow = m.follow;
        *sim = Sim::restore(&snap, mats);
        true
    }
}

fn moment_bytes(m: &Moment) -> usize {
    let cell = std::mem::size_of::<CellState>();
    m.keyframe.as_ref().map_or(0, |k| k.len() * cell)
        + m.cells.len() * (cell + 4)
        + m.flying.len() * std::mem::size_of::<crate::world::Flyer>()
        + m.bodies.iter().map(|b| b.cells.len() * std::mem::size_of::<Option<BodyCell>>() + 64).sum::<usize>()
}
