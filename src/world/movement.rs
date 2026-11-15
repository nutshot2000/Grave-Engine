//! Moving powders, liquids and gases, shared between the CPU's cores.
//!
//! The world is cut into 64x64 patches (2x2 sleeping blocks) and worked on
//! in four rounds, like the squares of a checkerboard: in each round the
//! patches being worked on are at least 64 cells apart. Nothing moves more
//! than 17 cells in a frame (a liquid spreads up to 16 sideways), so two
//! cores never touch the same cell, or the same sleeping block.
//!
//! Each patch rolls its own dice, seeded from the frame and where it is, so
//! the answer is exactly the same on any number of cores (and with one).
//! (The idea is from Noita's "Falling Everything" engine, GDC 2019.)

use super::*;

/// Patch size: two sleeping blocks across.
const PATCH: usize = 2 * BLOCK;
/// The farthest a cell can reach in one frame (cells). Must stay under
/// `PATCH / 2` so neighbouring same-colour patches never meet.
const REACH: i32 = 17;
const _: () = assert!((REACH as usize) < PATCH / 2);

/// Raw access to the cell layers, for workers that each keep to their own
/// part of the world (see the module notes for why they never overlap).
struct Grid {
    mat: *mut MatId,
    temp: *mut f32,
    life: *mut u16,
    burn: *mut u16,
    flags: *mut u8,
    shade: *mut u8,
    paint: *mut u32,
    tex: *mut u16,
    moved: *mut u32,
    awake: *mut u8,
}
// Safety: workers only touch cells (and blocks) in their own patch and its
// margin, and patches worked on at the same time are far enough apart.
unsafe impl Send for Grid {}
unsafe impl Sync for Grid {}

/// One worker moving the cells of one patch.
struct Mover<'a> {
    g: &'a Grid,
    fast: &'a [Fast],
    active: &'a [bool],
    flow: &'a [(f32, f32)],
    flow_w: usize,
    w: usize,
    h: usize,
    bw: usize,
    frame: u32,
    rng: u32,
}

impl Mover<'_> {
    #[inline]
    fn mat(&self, i: usize) -> MatId {
        unsafe { *self.g.mat.add(i) }
    }
    #[inline]
    fn temp(&self, i: usize) -> f32 {
        unsafe { *self.g.temp.add(i) }
    }
    #[inline]
    fn owned(&self, i: usize) -> bool {
        unsafe { *self.g.flags.add(i) & OWNED != 0 }
    }
    #[inline]
    fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h
    }
    #[inline]
    fn rand(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }
    #[inline]
    fn chance(&mut self, p: f32) -> bool {
        p >= 1.0 || (self.rand() >> 8) as f32 * (1.0 / 16_777_216.0) < p
    }
    #[inline]
    fn wake_at(&self, i: usize) {
        let b = (i / self.w / BLOCK) * self.bw + (i % self.w) / BLOCK;
        unsafe { *self.g.awake.add(b) = AWAKE_FRAMES };
    }

    /// The air current at a cell (cells per frame), smoothly blended.
    #[inline]
    fn flow_at(&self, x: i32, y: i32) -> (f32, f32) {
        let (fx, fy) = (x as f32 / FLOW_GRID as f32 + 1.0, y as f32 / FLOW_GRID as f32 + 1.0);
        let (gx, gy) = (fx as usize, fy as usize);
        let (tx, ty) = (fx - gx as f32, fy - gy as f32);
        let w = self.flow_w;
        let at = |gx: usize, gy: usize| self.flow[gy * w + gx];
        let (a, b, c, d) = (at(gx, gy), at(gx + 1, gy), at(gx, gy + 1), at(gx + 1, gy + 1));
        let top = (a.0 + (b.0 - a.0) * tx, a.1 + (b.1 - a.1) * tx);
        let bot = (c.0 + (d.0 - c.0) * tx, c.1 + (d.1 - c.1) * tx);
        (top.0 + (bot.0 - top.0) * ty, top.1 + (bot.1 - top.1) * ty)
    }

    /// How heavy a cell is right now. Gases get lighter as they heat up.
    #[inline]
    fn weight(&self, i: usize) -> f32 {
        let f = &self.fast[self.mat(i) as usize];
        if f.state == State::Gas { f.density * 293.0 / (self.temp(i) + 273.0).max(1.0) } else { f.density }
    }

    /// Can the cell at `i` (with weight `wi`) sink into `j`?
    #[inline]
    fn can_sink(&self, wi: f32, j: usize) -> bool {
        let s = self.fast[self.mat(j) as usize].state;
        (s == State::Gas || s == State::Liquid) && !self.owned(j) && self.weight(j) < wi
    }

    /// Can a gas at `i` (weight `wi`) rise into `j`? Needs a clear difference
    /// so still air doesn't shimmer.
    #[inline]
    fn can_rise(&self, wi: f32, j: usize) -> bool {
        let s = self.fast[self.mat(j) as usize].state;
        (s == State::Gas || s == State::Liquid) && !self.owned(j) && wi < self.weight(j) * 0.97
    }

    /// Can a gas be pushed into `j` by the air currents? (Swaps with other gases.)
    #[inline]
    fn can_drift(&self, i: usize, j: usize) -> bool {
        self.fast[self.mat(j) as usize].state == State::Gas && !self.owned(j) && self.mat(j) != self.mat(i)
    }

    fn swap(&mut self, a: usize, b: usize) {
        // Swapping two identical cells (air with air) changes nothing.
        if self.mat(a) != self.mat(b) || (self.temp(a) - self.temp(b)).abs() > 0.05 {
            self.wake_at(a);
            self.wake_at(b);
        }
        let g = self.g;
        unsafe {
            std::ptr::swap(g.mat.add(a), g.mat.add(b));
            std::ptr::swap(g.temp.add(a), g.temp.add(b));
            std::ptr::swap(g.life.add(a), g.life.add(b));
            std::ptr::swap(g.burn.add(a), g.burn.add(b));
            std::ptr::swap(g.flags.add(a), g.flags.add(b));
            std::ptr::swap(g.shade.add(a), g.shade.add(b));
            std::ptr::swap(g.paint.add(a), g.paint.add(b));
            std::ptr::swap(g.tex.add(a), g.tex.add(b));
            *g.moved.add(a) = self.frame;
            *g.moved.add(b) = self.frame;
        }
    }

    fn try_sink(&mut self, i: usize, wi: f32, x: i32, y: i32) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        let j = y as usize * self.w + x as usize;
        if self.can_sink(wi, j) {
            self.swap(i, j);
            return true;
        }
        false
    }

    /// Move every awake cell of patch (px, py): bottom to top, so falling
    /// things only move once, each row in a random direction so nothing
    /// drifts one way.
    fn patch(&mut self, px: usize, py: usize) {
        let (x0, x1) = (px * PATCH, ((px + 1) * PATCH).min(self.w));
        let (y0, y1) = (py * PATCH, ((py + 1) * PATCH).min(self.h));
        for y in (y0..y1).rev() {
            let left_to_right = self.rand() & 1 == 0;
            let row_blocks = (y / BLOCK) * self.bw;
            for k in 0..x1 - x0 {
                let x = if left_to_right { x0 + k } else { x1 - 1 - k };
                if !self.active[row_blocks + x / BLOCK] {
                    continue;
                }
                let i = y * self.w + x;
                if unsafe { *self.g.moved.add(i) } == self.frame || self.owned(i) {
                    continue;
                }
                let f = self.fast[self.mat(i) as usize];
                let (x, y) = (x as i32, y as i32);
                match f.state {
                    State::Solid => {}
                    State::Powder => self.move_powder(x, y),
                    State::Liquid => self.move_liquid(x, y, (f.flows as i32).min(REACH - 1)),
                    State::Gas => self.move_gas(x, y, (f.flows as i32).min(REACH - 1)),
                }
            }
        }
    }

    fn move_powder(&mut self, x: i32, y: i32) {
        let i = y as usize * self.w + x as usize;
        let wi = self.weight(i);
        if self.try_sink(i, wi, x, y + 1) {
            return;
        }
        // Sliding down the side of the pile (damp, sticky powders rarely do).
        let slides = self.fast[self.mat(i) as usize].slides;
        if slides < 1.0 && !self.chance(slides) {
            return;
        }
        let d = if self.rand() & 1 == 0 { 1 } else { -1 };
        if self.try_sink(i, wi, x + d, y + 1) {
            return;
        }
        self.try_sink(i, wi, x - d, y + 1);
    }

    fn move_liquid(&mut self, x: i32, y: i32, flows: i32) {
        let i = y as usize * self.w + x as usize;
        let wi = self.weight(i);
        if self.try_sink(i, wi, x, y + 1) {
            return;
        }
        let d = if self.rand() & 1 == 0 { 1 } else { -1 };
        if self.try_sink(i, wi, x + d, y + 1) || self.try_sink(i, wi, x - d, y + 1) {
            return;
        }
        // Spread sideways: slide as far as `flows` cells into lighter stuff.
        for dir in [d, -d] {
            let mut best = None;
            for step in 1..=flows {
                let nx = x + dir * step;
                if !self.in_bounds(nx, y) {
                    break;
                }
                let j = y as usize * self.w + nx as usize;
                if !self.can_sink(wi, j) {
                    break;
                }
                best = Some(j);
                // Stop early if we can fall from here.
                if self.in_bounds(nx, y + 1) && self.can_sink(wi, j + self.w) {
                    break;
                }
            }
            if let Some(j) = best {
                self.swap(i, j);
                return;
            }
        }
    }

    fn move_gas(&mut self, x: i32, y: i32, flows: i32) {
        let i = y as usize * self.w + x as usize;
        // Smoke, steam and flames get carried around by the air currents.
        // (Plain air moving through plain air would change nothing.)
        if self.mat(i) != AIR {
            let (u, v) = self.flow_at(x, y);
            if self.chance(u.abs().min(0.9)) {
                let dir = if u > 0.0 { 1 } else { -1 };
                // Blown into something solid? The air goes up and over it.
                for (dx, dy) in [(dir, 0), (dir, -1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if self.in_bounds(nx, ny) {
                        let j = ny as usize * self.w + nx as usize;
                        if self.can_drift(i, j) {
                            self.swap(i, j);
                            return;
                        }
                        if self.fast[self.mat(j) as usize].state == State::Gas {
                            break; // open air, just the same gas: nothing to do
                        }
                    }
                }
            }
            if self.chance(v.abs().min(0.9)) {
                let ny = y + if v > 0.0 { 1 } else { -1 };
                if self.in_bounds(x, ny) {
                    let j = ny as usize * self.w + x as usize;
                    if self.can_drift(i, j) {
                        self.swap(i, j);
                        return;
                    }
                }
            }
        }
        let wi = self.weight(i);
        // Still air with nowhere to rise to (the usual case: millions of sky
        // cells) has nothing to do: skip the dice. Same outcome, much faster.
        if self.mat(i) == AIR {
            let open = [(0, -1), (-1, -1), (1, -1), (-1, 0), (1, 0)].iter().any(|&(dx, dy)| {
                let (nx, ny) = (x + dx, y + dy);
                self.in_bounds(nx, ny) && self.can_rise(wi, ny as usize * self.w + nx as usize)
            });
            if !open {
                return;
            }
        }
        // Rising isn't perfectly steady, so plumes break up.
        if self.rand().is_multiple_of(8) {
            return;
        }
        let d = if self.rand() & 1 == 0 { 1 } else { -1 };
        for (dx, dy) in [(0, -1), (d, -1), (-d, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if self.in_bounds(nx, ny) {
                let j = ny as usize * self.w + nx as usize;
                if self.can_rise(wi, j) {
                    self.swap(i, j);
                    return;
                }
            }
        }
        // Drift sideways under ceilings, a little at random.
        if flows > 0 && !self.rand().is_multiple_of(3) {
            let reach = 1 + (self.rand() as i32).rem_euclid(flows);
            let mut best = None;
            for step in 1..=reach {
                let nx = x + d * step;
                if !self.in_bounds(nx, y) {
                    break;
                }
                let j = y as usize * self.w + nx as usize;
                if !self.can_rise(wi, j) {
                    break;
                }
                best = Some(j);
            }
            if let Some(j) = best {
                self.swap(i, j);
            }
        }
    }
}

/// A patch's dice: from this frame's roll and where the patch is.
fn patch_seed(base: u32, px: usize, py: usize) -> u32 {
    let mut x = base ^ (px as u32).wrapping_mul(0x9E37_79B9) ^ (py as u32).wrapping_mul(0x85EB_CA6B);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    x.max(1)
}

impl World {
    pub(super) fn movement(&mut self, mats: &Materials) {
        self.update_flow();
        let fast: Vec<Fast> = mats
            .list
            .iter()
            .map(|m| Fast { state: m.state, density: m.density, flows: m.flows, slides: m.slides })
            .collect();
        let base = self.rand();
        let (pw, ph) = (self.w.div_ceil(PATCH), self.h.div_ceil(PATCH));
        let (bw, bh) = (self.bw, self.bh);
        let patch_on = |px: usize, py: usize| {
            (2 * py..(2 * py + 2).min(bh)).any(|by| (2 * px..(2 * px + 2).min(bw)).any(|bx| self.active[by * bw + bx]))
        };
        // Lower rows first in each round (falling things land in patches
        // already done, which keeps a pour smooth).
        let rounds: Vec<Vec<(usize, usize)>> = [(0, 1), (1, 1), (0, 0), (1, 0)]
            .iter()
            .map(|&(ox, oy)| {
                let mut list = Vec::new();
                for py in (0..ph).rev().filter(|py| py % 2 == oy) {
                    for px in (0..pw).filter(|px| px % 2 == ox) {
                        if patch_on(px, py) {
                            list.push((px, py));
                        }
                    }
                }
                list
            })
            .collect();
        let awake_cells: usize = rounds.iter().map(|r| r.len()).sum::<usize>() * PATCH * PATCH;
        let threads = if awake_cells >= PARALLEL_CELLS { worker_threads() } else { 1 };
        let grid = Grid {
            mat: self.mat.as_mut_ptr(),
            temp: self.temp.as_mut_ptr(),
            life: self.life.as_mut_ptr(),
            burn: self.burn.as_mut_ptr(),
            flags: self.flags.as_mut_ptr(),
            shade: self.shade.as_mut_ptr(),
            paint: self.paint.as_mut_ptr(),
            tex: self.tex.as_mut_ptr(),
            moved: self.moved.as_mut_ptr(),
            awake: self.awake.as_mut_ptr(),
        };
        let (w, h, frame) = (self.w, self.h, self.frame);
        let (active, flow, flow_w) = (&self.active, &self.flow, self.flow_w);
        for round in &rounds {
            if round.is_empty() {
                continue;
            }
            let per = round.len().div_ceil(threads);
            let (grid, fast) = (&grid, &fast);
            run_bands(round.chunks(per).map(|part| {
                move || {
                    for &(px, py) in part {
                        let mut m = Mover { g: grid, fast, active, flow, flow_w, w, h, bw, frame, rng: patch_seed(base, px, py) };
                        m.patch(px, py);
                    }
                }
            }));
        }
    }
}
