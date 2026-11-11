//! Bending: metal objects bend for good instead of only cracking.
//!
//! When something hits an object made of a material with a `yield` setting
//! (iron, steel, brass...), the hit levers the object around a bend point:
//! the nearest weld or hinge it hangs from (a fork bends where it meets the
//! frame), or its middle. If that turning push beats what the metal can take
//! there, the part past the bend point turns a little and stays turned:
//!
//! - what it can take = `yield` × thickness² × depth × how firm it still is
//!   (`softens_at`: red-hot metal bends easily), so thin parts bend first;
//! - harder hits bend it further (a few degrees up to about 20 per hit);
//! - whatever is attached past the bend (a wheel on a fork) moves with it;
//! - a huge hit, or bending back and forth too far in total, tears it in two.
//!
//! Bending works on the object's own cells: they're redrawn turned, so the
//! dent shows and the collision shape follows.

use super::*;

/// What a section can take, per (MPa × metre of depth × cell²), in kN·cells.
/// Tuned so a motorbike fork (2 cells thick, 2 cm deep) shrugs off scrapes
/// and bumps but bends in a hard crash.
const BEND_K: f32 = 100.0;
/// How far a hit bends (radians) for each doubling-and-a-bit (ln) of how
/// much it beat the metal by.
const BEND_RATE: f32 = 0.12;
/// The most one hit bends something (radians, about 20 degrees).
const MAX_BEND_PER_HIT: f32 = 0.35;
/// A hit this many times what the metal can take tears it in two.
const TEAR_RATIO: f32 = 12.0;
/// Bent this far in total (radians, about 70 degrees): it tears.
const TEAR_TOTAL: f32 = 1.2;
/// A bend point must have at least this many cells on each side.
const MIN_SIDE: usize = 3;

/// A hard hit on an object that might bend it.
#[derive(Clone, Copy, Debug)]
pub(super) struct BendHit {
    pub id: u64,
    /// kN.
    pub force: f32,
    /// Where (world cells), and which way the push goes (unit vector).
    pub at: (f32, f32),
    pub dir: (f32, f32),
}

impl Physics {
    /// Does this object have metal that bends at this cell? (For impacts.)
    pub(super) fn bendable(mats: &Materials, mat: MatId) -> bool {
        mats.get(mat).yield_strength.is_some()
    }

    /// Bend whatever was hit hard enough this frame (strongest hit per object).
    pub(super) fn bends(&mut self, world: &mut World, mats: &Materials) {
        let mut hits = std::mem::take(&mut self.bend_hits);
        if hits.is_empty() {
            return;
        }
        hits.sort_by(|a, b| b.force.total_cmp(&a.force));
        let mut done = std::collections::HashSet::new();
        for h in hits {
            if done.insert(h.id) {
                self.bend(world, mats, h);
            }
        }
    }

    /// How much an object has bent in total (radians), for games and tests.
    pub fn bent(&self, id: u64) -> f32 {
        self.bent.get(&id).copied().unwrap_or(0.0)
    }

    fn bend(&mut self, world: &mut World, mats: &Materials, hit: BendHit) {
        let Some(n) = self.find(hit.id) else { return };
        {
            let b = &self.bodies[n];
            if b.dirty || b.round || (b.shard && self.tick.wrapping_sub(b.born) < FRESH_TICKS) {
                return;
            }
        }
        let Some(spec) = self.spec_of(world, hit.id) else { return };
        let (w, h) = (spec.w as i32, spec.h as i32);
        let (c, s) = (spec.angle.cos(), spec.angle.sin());
        // Into the object's own (unturned) frame, in cells from its centre.
        let to_local = |p: (f32, f32)| {
            let (dx, dy) = (p.0 - spec.x, p.1 - spec.y);
            (c * dx + s * dy, -s * dx + c * dy)
        };
        let centre_of = |k: usize| ((k as i32 % w) as f32 + 0.5 - w as f32 / 2.0, (k as i32 / w) as f32 + 0.5 - h as f32 / 2.0);
        let cells: Vec<usize> = (0..spec.cells.len()).filter(|&k| spec.cells[k].is_some()).collect();
        if cells.len() < MIN_SIDE * 2 + 2 {
            return;
        }
        let hl = to_local(hit.at);
        let dl = (c * hit.dir.0 + s * hit.dir.1, -s * hit.dir.0 + c * hit.dir.1);

        // Its long axis (the way a bar runs).
        let nf = cells.len() as f32;
        let (mx, my) = cells.iter().fold((0.0, 0.0), |a, &k| {
            let p = centre_of(k);
            (a.0 + p.0 / nf, a.1 + p.1 / nf)
        });
        let (mut sxx, mut syy, mut sxy) = (0.0f32, 0.0f32, 0.0f32);
        for &k in &cells {
            let p = centre_of(k);
            let (dx, dy) = (p.0 - mx, p.1 - my);
            sxx += dx * dx;
            syy += dy * dy;
            sxy += dx * dy;
        }
        let a = 0.5 * (2.0 * sxy).atan2(sxx - syy);
        let u = (a.cos(), a.sin());
        let along = |p: (f32, f32)| (p.0 - mx) * u.0 + (p.1 - my) * u.1;
        let sh = along(hl);

        // The bend point: the nearest weld or hinge it hangs from (not right
        // at the hit), else its middle.
        let joints = self.joints_of(hit.id);
        let mut candidates: Vec<f32> = joints
            .iter()
            .filter(|j| matches!(j.kind, JointKind::Weld | JointKind::Hinge))
            .map(|j| along(if j.a == hit.id { j.anchor_a } else { j.anchor_b }))
            .collect();
        candidates.push(0.0);
        let sides = |sp: f32| {
            let hit_side = cells.iter().filter(|&&k| (along(centre_of(k)) - sp) * (sh - sp) > 0.0).count();
            (hit_side, cells.len() - hit_side)
        };
        let Some(sp) = candidates
            .into_iter()
            .filter(|&sp| (sp - sh).abs() >= 3.0)
            .filter(|&sp| {
                let (a, b) = sides(sp);
                a >= MIN_SIDE && b >= MIN_SIDE
            })
            .min_by(|a, b| (a - sh).abs().total_cmp(&(b - sh).abs()))
        else {
            return;
        };
        let side = (sh - sp).signum();

        // The cross-section there: how thick, how strong, how hot.
        let section: Vec<usize> = cells.iter().copied().filter(|&k| (-0.5..0.5).contains(&(along(centre_of(k)) - sp))).collect();
        if section.is_empty() {
            return;
        }
        let mut give = 0.0f32;
        let mut metal = 0;
        for &k in &section {
            let cell = spec.cells[k].unwrap();
            let m = mats.get(cell.mat);
            if let Some(y) = m.yield_strength {
                give += y * m.firmness(cell.temp);
                metal += 1;
            }
        }
        if metal * 2 < section.len() {
            return; // mostly not metal here: it cracks (toughness) rather than bends
        }
        let t = section.len() as f32;
        let holds = (BEND_K * spec.depth * give * t).max(1.0);
        let pivot = section.iter().fold((0.0, 0.0), |a, &k| {
            let p = centre_of(k);
            (a.0 + p.0 / t, a.1 + p.1 / t)
        });
        let r = (hl.0 - pivot.0, hl.1 - pivot.1);
        let turn = r.0 * dl.1 - r.1 * dl.0; // lever × push, signed
        let moment = hit.force * turn.abs();
        if moment <= holds {
            return;
        }
        let ratio = moment / holds;
        let theta = (BEND_RATE * ratio.ln()).min(MAX_BEND_PER_HIT) * turn.signum();
        let total = self.bent(hit.id) + theta.abs();
        let on_hit_side = |p: (f32, f32)| (along(p) - sp) * side > 0.0;

        if ratio > TEAR_RATIO || total > TEAR_TOTAL {
            self.tear(world, mats, &spec, &joints, hit.id, |k| on_hit_side(centre_of(k)), on_hit_side, hit.dir);
            return;
        }
        let lever = (sh - sp).abs();
        if theta.abs() * lever < 0.6 {
            return; // too small to show: it springs back
        }

        // Redraw it with the far side turned by theta around the pivot.
        let (tc, ts) = (theta.cos(), theta.sin());
        let turn_pt = |p: (f32, f32), c: f32, s: f32| {
            let (dx, dy) = (p.0 - pivot.0, p.1 - pivot.1);
            let (rx, ry) = rot(c, s, dx, dy);
            (pivot.0 + rx, pivot.1 + ry)
        };
        let old_at = |p: (f32, f32)| -> Option<usize> {
            let (x, y) = ((p.0 + w as f32 / 2.0).floor() as i32, (p.1 + h as f32 / 2.0).floor() as i32);
            (x >= 0 && y >= 0 && x < w && y < h).then(|| (y * w + x) as usize).filter(|&k| spec.cells[k].is_some())
        };
        // Bounds of the bent shape (in old grid coordinates).
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for &k in &cells {
            let p = centre_of(k);
            let q = if on_hit_side(p) { turn_pt(p, tc, ts) } else { p };
            let (gx, gy) = ((q.0 + w as f32 / 2.0).floor() as i32, (q.1 + h as f32 / 2.0).floor() as i32);
            x0 = x0.min(gx - 1);
            y0 = y0.min(gy - 1);
            x1 = x1.max(gx + 1);
            y1 = y1.max(gy + 1);
        }
        let (gw, gh) = ((x1 - x0 + 1) as usize, (y1 - y0 + 1) as usize);
        let mut grid: Vec<Option<BodyCell>> = vec![None; gw * gh];
        for gy in y0..=y1 {
            for gx in x0..=x1 {
                let q = (gx as f32 + 0.5 - w as f32 / 2.0, gy as f32 + 0.5 - h as f32 / 2.0);
                // The cell that stays here, or the turned cell that lands
                // here. Each side reaches a cell past the bend so the outside
                // of the bend doesn't open up.
                let stay = old_at(q).map(|k| (k, (along(centre_of(k)) - sp) * side));
                let moved = old_at(turn_pt(q, tc, -ts)).map(|k| (k, (along(centre_of(k)) - sp) * side));
                let pick = match (stay, moved) {
                    (Some((k, d)), _) if d <= 0.0 => Some(k),
                    (_, Some((k, d))) if d > 0.0 => Some(k),
                    (Some((k, d)), _) if d <= 1.0 => Some(k),
                    (_, Some((k, d))) if d > -1.0 => Some(k),
                    _ => None,
                };
                grid[((gy - y0) as usize) * gw + (gx - x0) as usize] = pick.and_then(|k| spec.cells[k]);
            }
        }
        // Trim empty edges.
        let filled: Vec<(usize, usize)> = (0..gw * gh).filter(|&i| grid[i].is_some()).map(|i| (i % gw, i / gw)).collect();
        let (fx0, fy0) = (filled.iter().map(|p| p.0).min().unwrap(), filled.iter().map(|p| p.1).min().unwrap());
        let (fx1, fy1) = (filled.iter().map(|p| p.0).max().unwrap(), filled.iter().map(|p| p.1).max().unwrap());
        let (nw, nh) = (fx1 - fx0 + 1, fy1 - fy0 + 1);
        let mut out = vec![None; nw * nh];
        for y in 0..nh {
            for x in 0..nw {
                out[y * nw + x] = grid[(y + fy0) * gw + x + fx0];
            }
        }
        // Where the new shape's centre is, in the old frame.
        let ox = (x0 + fx0 as i32) as f32 + nw as f32 / 2.0 - w as f32 / 2.0;
        let oy = (y0 + fy0 as i32) as f32 + nh as f32 / 2.0 - h as f32 / 2.0;
        let (wx, wy) = rot(c, s, ox, oy);
        let new = BodySpec { w: nw, h: nh, cells: out, x: spec.x + wx, y: spec.y + wy, ..spec.clone() };
        // Joints: ends past the bend move with it.
        let joints: Vec<JointSpec> = joints
            .into_iter()
            .map(|mut j| {
                let fix = |p: (f32, f32)| {
                    let q = if on_hit_side(p) { turn_pt(p, tc, ts) } else { p };
                    (q.0 - ox, q.1 - oy)
                };
                if j.a == hit.id {
                    j.anchor_a = fix(j.anchor_a);
                }
                if j.b == hit.id {
                    j.anchor_b = fix(j.anchor_b);
                }
                j
            })
            .collect();
        self.replace(world, mats, hit.id, new, joints);
        *self.bent.entry(hit.id).or_default() += theta.abs();
    }

    /// Put a reshaped object back with the same id and these joints.
    fn replace(&mut self, world: &mut World, mats: &Materials, id: u64, spec: BodySpec, joints: Vec<JointSpec>) {
        if self.remove(world, id).is_none() {
            return;
        }
        let before = self.next_id;
        self.next_id = id;
        let added = self.add_raw(world, mats, spec);
        self.next_id = before.max(id + 1);
        if added.is_some() {
            if let Some(b) = self.bodies.last_mut() {
                b.born = self.tick.wrapping_sub(SETTLE_TICKS);
            }
            for j in joints {
                self.insert_joint(mats, j);
            }
        }
    }

    /// Tear an object in two across its bend point. The bigger half keeps
    /// its id; joints go with the half they're on.
    #[allow(clippy::too_many_arguments)]
    fn tear(
        &mut self,
        world: &mut World,
        mats: &Materials,
        spec: &BodySpec,
        joints: &[JointSpec],
        id: u64,
        cell_on_hit_side: impl Fn(usize) -> bool,
        point_on_hit_side: impl Fn((f32, f32)) -> bool,
        dir: (f32, f32),
    ) {
        let (w, h) = (spec.w, spec.h);
        let (c, s) = (spec.angle.cos(), spec.angle.sin());
        let half = |far: bool| -> Option<(BodySpec, (f32, f32))> {
            let ks: Vec<usize> = (0..w * h).filter(|&k| spec.cells[k].is_some() && cell_on_hit_side(k) == far).collect();
            let (x0, y0) = (ks.iter().map(|k| k % w).min()?, ks.iter().map(|k| k / w).min()?);
            let (x1, y1) = (ks.iter().map(|k| k % w).max()?, ks.iter().map(|k| k / w).max()?);
            let (nw, nh) = (x1 - x0 + 1, y1 - y0 + 1);
            let mut cells = vec![None; nw * nh];
            for &k in &ks {
                cells[(k / w - y0) * nw + (k % w - x0)] = spec.cells[k];
            }
            let ox = x0 as f32 + nw as f32 / 2.0 - w as f32 / 2.0;
            let oy = y0 as f32 + nh as f32 / 2.0 - h as f32 / 2.0;
            let (wx, wy) = rot(c, s, ox, oy);
            let kick = if far { 1.5 } else { 0.0 };
            Some((
                BodySpec { w: nw, h: nh, cells, x: spec.x + wx, y: spec.y + wy, vx: spec.vx + dir.0 * kick, vy: spec.vy + dir.1 * kick, ..spec.clone() },
                (ox, oy),
            ))
        };
        let (Some(far), Some(near)) = (half(true), half(false)) else { return };
        if self.remove(world, id).is_none() {
            return;
        }
        self.bent.remove(&id);
        let far_bigger = far.0.cell_count() > near.0.cell_count();
        let mut ids = [0u64; 2];
        for (k, (piece, _)) in [&far, &near].into_iter().enumerate() {
            let keep_id = (k == 0) == far_bigger;
            let before = self.next_id;
            if keep_id {
                self.next_id = id;
            }
            let added = self.add_raw(world, mats, piece.clone());
            if keep_id {
                self.next_id = before.max(id + 1);
            }
            if let (Some(new), Some(b)) = (added, self.bodies.last_mut()) {
                b.born = self.tick.wrapping_sub(SETTLE_TICKS);
                ids[k] = new;
            }
        }
        for j in joints {
            let mut j = j.clone();
            let mut ok = true;
            for end in 0..2 {
                let (this, anchor) = if end == 0 { (j.a, &mut j.anchor_a) } else { (j.b, &mut j.anchor_b) };
                if this != id {
                    continue;
                }
                let k = if point_on_hit_side(*anchor) { 0 } else { 1 };
                let o = if k == 0 { far.1 } else { near.1 };
                *anchor = (anchor.0 - o.0, anchor.1 - o.1);
                if ids[k] == 0 {
                    ok = false;
                }
                if end == 0 {
                    j.a = ids[k];
                } else {
                    j.b = ids[k];
                }
            }
            if ok && j.a != j.b {
                self.insert_joint(mats, j);
            }
        }
    }
}
