//! Terrain grappling: elastic tension, corner wrapping and a sagging segmented line.
//! Lengths are cells; forces run through the character's normal collision solver.
use super::*;
type Point = (f32, f32);
#[derive(Clone, Debug, PartialEq)]
pub struct Rope {
    pub range: f32,
    pub length: f32,
    pub elasticity: f32,
    /// First point is the fixed hook; later points are terrain bends.
    pub pins: Vec<Point>,
    pub reel: f32,
}
impl Default for Rope {
    fn default() -> Self {
        Self {
            range: 150.,
            length: 60.,
            elasticity: 0.06,
            pins: Vec::new(),
            reel: 0.,
        }
    }
}
fn distance(a: Point, b: Point) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}
fn solid(w: &World, m: &Materials, p: Point) -> bool {
    let (x, y) = (p.0.floor() as i32, p.1.floor() as i32);
    if !w.in_bounds(x, y) {
        return true;
    }
    let i = w.idx(x as usize, y as usize);
    !w.is_owned(i) && m.get(w.mat[i]).state == crate::material::State::Solid
}
fn obstruction(w: &World, m: &Materials, a: Point, b: Point) -> Option<Point> {
    let n = (distance(a, b) * 2.).ceil() as usize;
    for k in 2..n {
        let t = k as f32 / n as f32;
        let p = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        if solid(w, m, p) {
            return Some(p);
        }
    }
    None
}
impl Physics {
    /// Press once to launch along aim; press again to detach, preserving velocity.
    pub fn grapple(&mut self, w: &World, m: &Materials, aim: Point) -> bool {
        let Some(a) = self.actors.iter_mut().find(|a| a.spec.creature.is_none()) else {
            return false;
        };
        let Some(r) = a.spec.rope.as_mut() else {
            return false;
        };
        if !r.pins.is_empty() {
            r.pins.clear();
            return true;
        }
        if a.spec.dead_for > 0 {
            return false;
        }
        let rb = &self.set[a.handle];
        let from = (rb.translation().x / CELL, rb.translation().y / CELL - 3.);
        let n = aim.0.hypot(aim.1);
        if n < 0.01 {
            return false;
        }
        for k in 1..=(r.range * 2.) as usize {
            let d = k as f32 * 0.5;
            let p = (from.0 + aim.0 / n * d, from.1 + aim.1 / n * d);
            if !w.in_bounds(p.0 as i32, p.1 as i32) {
                break;
            }
            let i = w.idx(p.0 as usize, p.1 as usize);
            if w.is_owned(i) {
                continue;
            }
            let mat = m.get(w.mat[i]);
            if mat.state == crate::material::State::Solid {
                r.pins.push(p);
                r.length = d.max(12.);
                return true;
            }
        }
        false
    }
    pub fn reel_rope(&mut self, reel: f32) {
        if let Some(a) = self.actors.iter_mut().find(|a| a.spec.creature.is_none()) {
            if let Some(r) = a.spec.rope.as_mut() {
                r.reel = reel.clamp(-1., 1.);
            }
        }
    }
    pub(in crate::physics) fn ropes_think(&mut self, w: &World, m: &Materials) {
        for a in &mut self.actors {
            let Some(r) = a.spec.rope.as_mut().filter(|r| !r.pins.is_empty()) else {
                continue;
            };
            if a.spec.dead_for > 0 || a.spec.ledge.is_some() || !solid(w, m, r.pins[0]) {
                r.pins.clear();
                continue;
            }
            let rb = &mut self.set[a.handle];
            let p = (rb.translation().x / CELL, rb.translation().y / CELL - 3.);
            // Unwrap as soon as the preceding bend has a clear sightline.
            while r.pins.len() > 1 && obstruction(w, m, r.pins[r.pins.len() - 2], p).is_none() {
                r.pins.pop();
            }
            let pin = *r.pins.last().unwrap();
            if let Some(hit) = obstruction(w, m, pin, p) {
                if r.pins.len() < 24 {
                    // Find a visible, air-side rock corner, preferring the shortest detour.
                    let mut best = None;
                    let mut cost = f32::MAX;
                    for y in -8..=8 {
                        for x in -8..=8 {
                            let q = (
                                hit.0.floor() + x as f32 + 0.5,
                                hit.1.floor() + y as f32 + 0.5,
                            );
                            if solid(w, m, q) || distance(q, pin) < 1.5 || distance(q, p) < 2. {
                                continue;
                            }
                            let near = [(-0.9, 0.), (0.9, 0.), (0., -0.9), (0., 0.9)]
                                .iter()
                                .any(|d| solid(w, m, (q.0 + d.0, q.1 + d.1)));
                            if !near
                                || obstruction(w, m, pin, q).is_some()
                                || obstruction(w, m, q, p).is_some()
                            {
                                continue;
                            }
                            let c = distance(q, pin) + distance(q, p);
                            if c < cost {
                                best = Some(q);
                                cost = c;
                            }
                        }
                    }
                    if let Some(q) = best {
                        r.pins.push(q);
                    }
                }
            }
            let fixed: f32 = r.pins.windows(2).map(|s| distance(s[0], s[1])).sum();
            r.length = (r.length - r.reel * 28. * DT).clamp((fixed + 10.).min(r.range), r.range);
            let pin = *r.pins.last().unwrap();
            let d = distance(pin, p);
            let free = (r.length - fixed).max(8.);
            if d > free && d > 0.1 {
                let n = Vector::new((pin.0 - p.0) / d, (pin.1 - p.1) / d);
                let v = rb.linvel();
                let radial = v.dot(n);
                // A damped elastic constraint catches falls without a teleport or a rigid rod.
                let correction = ((d - free) * CELL / (DT * (1. + r.elasticity * 60.))).min(18.);
                let pull = (correction - radial).max(0.);
                rb.set_linvel(v + n * pull, true);
                a.v_before = rb.linvel();
            }
        }
    }
}
