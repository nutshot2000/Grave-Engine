//! The simulation: a grid of cells, each with a material and a temperature.
//!
//! Every frame does three things, all driven by the material files:
//! 1. **Heat** spreads between neighbours (by `conducts_heat` / `heat_capacity`).
//! 2. **Changes**: temperature crossing a material's thresholds makes it catch
//!    fire, melt, boil, freeze or condense; short-lived things decay; touching
//!    materials react.
//! 3. **Movement**: powders fall, liquids fall and spread, gases rise. Heavier
//!    things sink below lighter ones, and hot gas is lighter than cold gas.
//!
//! There are no special cases like "fire burns wood": wood burns because it
//! got hotter than its `catches_fire_at` while touching air.

use crate::material::{AIR, MatId, Materials, State};

pub const BURNING: u8 = 1;
/// This cell belongs to a rigid object (see `physics.rs`). The cell simulation
/// still heats, burns and melts it, but never moves it.
pub const OWNED: u8 = 2;
/// Drawn some other way (a character's pictures): the renderer shows what's
/// behind instead of this cell.
pub const HIDDEN: u8 = 4;
/// Anchored scenery: counts as held up (like the world's edge), so it never
/// collapses or snaps under its own weight. Set by a scene's `anchor` line;
/// lost if the cell turns into something else (burns, melts, breaks).
pub const ANCHORED: u8 = 8;

/// An explosive cell went off here this frame (handled by `physics.rs`).
#[derive(Clone, Copy, Debug)]
pub struct Blast {
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub power: f32,
    /// What the blast fills nearby air with (usually fire).
    pub fire: Option<MatId>,
}

/// A cell thrown into the air (by a blast, a splash...). It flies on its own,
/// off the grid, until it hits something, then lands back in as a normal cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flyer {
    /// Position and speed, in cells and cells per frame.
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub mat: MatId,
    pub temp: f32,
    pub life: u16,
    pub burn: u16,
    pub flags: u8,
    pub shade: u8,
    pub paint: u32,
}

/// Gravity for flying cells: 9.81 m/s² at 10 cells a metre, 60 frames a second.
pub const FLY_GRAVITY: f32 = 9.81 * 10.0 / 3600.0;
/// Air slows flying cells a little every frame.
const FLY_DRAG: f32 = 0.995;
/// Flying things lighter than this (kg/m³) get carried by the air, the
/// lighter the more (embers 150, snow 300, ash 600: not much).
const AIR_CARRIES: f32 = 800.0;
/// Fastest a flying cell can go (cells per frame; 6 = 36 m/s).
const FLY_MAX_SPEED: f32 = 6.0;
/// A flying cell this fast (cells per frame) knocks loose stuff it hits
/// flying too, passing on this share of its speed.
const KNOCK_SPEED: f32 = 1.0;
const KNOCK_KEEP: f32 = 0.9;
/// At most this many cells in the air at once (more just stay put).
pub const MAX_FLYING: usize = 30_000;

/// Air currents are worked out on a grid this many cells apart.
const FLOW_GRID: usize = 8;

/// Smooth random noise in 3D (x, y, time), roughly 0..1.
fn value_noise(x: f32, y: f32, t: f32) -> f32 {
    fn h(x: i32, y: i32, z: i32) -> f32 {
        let mut n = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ (z as u32).wrapping_mul(0xcb1a_b31f);
        n ^= n >> 13;
        n = n.wrapping_mul(0x5bd1_e995);
        n ^= n >> 15;
        (n & 0xffff) as f32 / 65535.0
    }
    let (x0, y0, z0) = (x.floor(), y.floor(), t.floor());
    let s = |v: f32| v * v * (3.0 - 2.0 * v);
    let (fx, fy, fz) = (s(x - x0), s(y - y0), s(t - z0));
    let (ix, iy, iz) = (x0 as i32, y0 as i32, z0 as i32);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let layer = |z: i32| {
        let a = lerp(h(ix, iy, z), h(ix + 1, iy, z), fx);
        let b = lerp(h(ix, iy + 1, z), h(ix + 1, iy + 1, z), fx);
        lerp(a, b, fy)
    };
    lerp(layer(iz), layer(iz + 1), fz)
}

/// Pack a texture position into a cell's `tex`.
#[inline]
pub fn pack_tex(x: usize, y: usize) -> u16 {
    (x as u16 & 0xff) | ((y as u16 & 0xff) << 8)
}

/// Room temperature, °C.
pub const ROOM_TEMP: f32 = 20.0;

#[derive(Clone)]
pub struct World {
    pub w: usize,
    pub h: usize,
    pub mat: Vec<MatId>,
    pub temp: Vec<f32>,
    /// Frames left for things with a `lifetime`.
    pub life: Vec<u16>,
    /// Frames left of burning.
    pub burn: Vec<u16>,
    pub flags: Vec<u8>,
    /// Air touching the face from a neighbouring depth plane; refreshed by Sim.
    pub depth_air: Vec<bool>,
    /// Per-cell random number for colour speckle.
    pub shade: Vec<u8>,
    /// Opaque 0xffRRGGBB surface paint; zero means the native material appearance.
    pub paint: Vec<u32>,
    /// Where in its material's texture picture each cell is (x in the low
    /// byte, y in the high byte). Scenery uses its world position; objects
    /// use their position within the object, so patterns move with them.
    pub tex: Vec<u16>,
    /// The frame a cell last moved in (so nothing moves twice per frame).
    moved: Vec<u32>,
    pub frame: u32,
    pub ambient: f32,
    /// Explosions that happened during the last step.
    pub blasts: Vec<Blast>,
    /// Cells flying through the air.
    pub flying: Vec<Flyer>,
    /// Steady wind blowing gases sideways, in metres per second (+ = right).
    pub wind: f32,
    /// How swirly the air is (0 = still, 1 = normal, 3 = stormy).
    pub turbulence: f32,
    /// Air currents for this frame, on a coarse grid (cells per frame).
    flow: Vec<(f32, f32)>,
    flow_w: usize,
    rng: u32,
    // Scratch buffers, reused every frame.
    new_temp: Vec<f32>,
    k: Vec<f32>,
    inv_cap: Vec<f32>,
    /// Sleeping areas: the world in 32x32 blocks. Each block counts down
    /// frames since something last happened in it (0 = asleep); a block is
    /// worked on while it or a neighbour is awake (`active`). Anything that
    /// changes a cell wakes its block (`wake_at`).
    awake: Vec<u8>,
    active: Vec<bool>,
    /// Blocks holding something with a slow reaction waiting to happen: they
    /// get a short visit now and then even when asleep.
    slow: Vec<bool>,
    /// Let quiet areas sleep (on by default; off works on everything every
    /// frame, for comparing).
    pub sleeping: bool,
    bw: usize,
    bh: usize,
    /// A number unique to this world (copies share it), so things worked
    /// out from it and kept between frames (sunlight) know it's the same one.
    pub id: u64,
}

static NEXT_WORLD: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Block size for sleeping areas (cells).
const BLOCK: usize = 32;
/// How long a block stays awake after something happens in it (frames).
const AWAKE_FRAMES: u8 = 60;
/// Reactions with a chance below this per frame are "slow" (mud drying, wet
/// sand drying): they don't keep an area awake. Instead its block is visited
/// every `SLOW_EVERY` frames and the chance scaled up to match.
const SLOW_CHANCE: f32 = 0.02;
const SLOW_EVERY: u32 = 16;

/// Per-material numbers the hot loops need, gathered once per frame.
struct Fast {
    state: State,
    density: f32,
    flows: u8,
    slides: f32,
}

impl World {
    pub fn new(w: usize, h: usize) -> Self {
        let n = w * h;
        let mut world = World {
            w,
            h,
            mat: vec![AIR; n],
            temp: vec![ROOM_TEMP; n],
            life: vec![0; n],
            burn: vec![0; n],
            flags: vec![0; n],
            depth_air: vec![false; n],
            shade: vec![0; n],
            paint: vec![0; n],
            tex: (0..n).map(|i| pack_tex(i % w, i / w)).collect(),
            moved: vec![0; n],
            frame: 1,
            ambient: ROOM_TEMP,
            blasts: Vec::new(),
            flying: Vec::new(),
            wind: 0.0,
            turbulence: 1.0,
            flow: Vec::new(),
            flow_w: 0,
            rng: 0x9E37_79B9,
            new_temp: vec![0.0; n],
            k: vec![0.0; n],
            inv_cap: vec![0.0; n],
            awake: vec![AWAKE_FRAMES; w.div_ceil(BLOCK) * h.div_ceil(BLOCK)],
            active: vec![true; w.div_ceil(BLOCK) * h.div_ceil(BLOCK)],
            slow: vec![false; w.div_ceil(BLOCK) * h.div_ceil(BLOCK)],
            bw: w.div_ceil(BLOCK),
            bh: h.div_ceil(BLOCK),
            sleeping: true,
            id: NEXT_WORLD.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        };
        for i in 0..n {
            world.shade[i] = world.rand() as u8;
        }
        world
    }

    /// Something happened at cell `i`: wake its block.
    #[inline]
    pub fn wake_at(&mut self, i: usize) {
        let b = (i / self.w / BLOCK) * self.bw + (i % self.w) / BLOCK;
        self.awake[b] = AWAKE_FRAMES;
    }

    /// Wake every block touching a box of cells (inclusive, clipped).
    pub fn wake_box(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let (x0, x1, y0, y1) = self.clip(x0, x1, y0, y1);
        if x0 > x1 || y0 > y1 {
            return;
        }
        for by in y0 as usize / BLOCK..=y1 as usize / BLOCK {
            for bx in x0 as usize / BLOCK..=x1 as usize / BLOCK {
                self.awake[by * self.bw + bx] = AWAKE_FRAMES;
            }
        }
    }

    /// Wake the whole world (after editing it from outside).
    pub fn wake_all(&mut self) {
        self.awake.fill(AWAKE_FRAMES);
    }

    /// How much of the world is being worked on (0..1): the rest is asleep.
    pub fn awake_fraction(&self) -> f32 {
        self.active.iter().filter(|a| **a).count() as f32 / self.active.len().max(1) as f32
    }

    /// Which blocks get worked on this frame: awake ones and their neighbours.
    fn plan_blocks(&mut self) {
        if !self.sleeping {
            self.awake.fill(AWAKE_FRAMES);
        }
        let (bw, bh) = (self.bw, self.bh);
        for by in 0..bh {
            for bx in 0..bw {
                let mut on = false;
                for dy in by.saturating_sub(1)..=(by + 1).min(bh - 1) {
                    for dx in bx.saturating_sub(1)..=(bx + 1).min(bw - 1) {
                        on |= self.awake[dy * bw + dx] > 0;
                    }
                }
                self.active[by * bw + bx] = on;
            }
        }
        // A sleeping block with a slow reaction waiting gets its turn.
        for b in 0..bw * bh {
            if self.slow[b] && !self.active[b] && self.frame % SLOW_EVERY == b as u32 % SLOW_EVERY {
                self.active[b] = true;
            }
        }
    }

    /// The size of the sleeping-area grid: (blocks across, blocks down).
    pub fn block_dims(&self) -> (usize, usize) {
        (self.bw, self.bh)
    }

    /// How many sleeping-area blocks there are (32x32 cells, row by row).
    pub fn blocks(&self) -> usize {
        self.bw * self.bh
    }

    /// Has nothing in block `b` or next to it changed for a while (so
    /// anything worked out from its cells last time still holds)?
    pub fn block_still(&self, b: usize) -> bool {
        if self.active[b] {
            return false;
        }
        let (bx, by) = (b % self.bw, b / self.bw);
        for dy in by.saturating_sub(1)..=(by + 1).min(self.bh - 1) {
            for dx in bx.saturating_sub(1)..=(bx + 1).min(self.bw - 1) {
                if self.awake[dy * self.bw + dx] > 0 {
                    return false;
                }
            }
        }
        true
    }

    /// The cells of block `b`: x range and y range.
    #[inline]
    pub fn block_cells(&self, b: usize) -> (usize, usize, usize, usize) {
        let (bx, by) = (b % self.bw, b / self.bw);
        (bx * BLOCK, ((bx + 1) * BLOCK).min(self.w), by * BLOCK, ((by + 1) * BLOCK).min(self.h))
    }

    pub fn seed(&mut self, seed: u32) {
        self.rng = seed.max(1);
    }

    #[inline]
    pub fn idx(&self, x: usize, y: usize) -> usize {
        y * self.w + x
    }

    #[inline]
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h
    }

    #[inline]
    fn rand(&mut self) -> u32 {
        // xorshift32: fast and good enough for sand.
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

    /// Put a fresh cell of `id` at its normal starting temperature.
    pub fn set(&mut self, mats: &Materials, x: usize, y: usize, id: MatId) {
        let i = self.idx(x, y);
        let t = mats.get(id).temperature;
        self.become_(mats, i, id);
        self.temp[i] = t;
        self.shade[i] = self.rand() as u8;
        self.tex[i] = pack_tex(x, y);
    }

    /// Change a cell's material, keeping its temperature.
    fn become_(&mut self, mats: &Materials, i: usize, id: MatId) {
        self.wake_at(i);
        self.mat[i] = id;
        self.paint[i] = 0;
        self.flags[i] = 0;
        self.burn[i] = 0;
        self.life[i] = match mats.get(id).lifetime {
            Some(l) => {
                let jitter = 0.5 + (self.rand() % 1000) as f32 / 1000.0;
                ((l as f32 * jitter) as u16).max(1)
            }
            None => 0,
        };
    }

    pub fn fill(&mut self, mats: &Materials, id: MatId) {
        for y in 0..self.h {
            for x in 0..self.w {
                self.set(mats, x, y, id);
            }
        }
    }

    /// Paint a filled circle of material.
    pub fn paint_circle(&mut self, mats: &Materials, cx: i32, cy: i32, r: i32, id: MatId) {
        let r = r.clamp(0, 8192);
        let (x0, x1, y0, y1) = self.clip(cx - r, cx + r, cy - r, cy + r);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (dx, dy) = (x - cx, y - cy);
                if dx * dx + dy * dy <= r * r + r / 2 && self.in_bounds(x, y) {
                    self.set(mats, x as usize, y as usize, id);
                }
            }
        }
    }

    /// Add heat (or remove it, with a negative amount) in a circle.
    pub fn heat_circle(&mut self, cx: i32, cy: i32, r: i32, amount: f32) {
        let r = r.clamp(0, 8192);
        let (x0, x1, y0, y1) = self.clip(cx - r, cx + r, cy - r, cy + r);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (dx, dy) = (x - cx, y - cy);
                if dx * dx + dy * dy <= r * r + r / 2 && self.in_bounds(x, y) {
                    let i = self.idx(x as usize, y as usize);
                    self.temp[i] = (self.temp[i] + amount).clamp(-273.0, 5000.0);
                    self.wake_at(i);
                }
            }
        }
    }

    /// A box (inclusive) cut down to the world, so loops never visit
    /// millions of cells outside it. (May come out empty: x0 > x1.)
    pub fn clip(&self, x0: i32, x1: i32, y0: i32, y1: i32) -> (i32, i32, i32, i32) {
        (x0.max(0), x1.min(self.w as i32 - 1), y0.max(0), y1.min(self.h as i32 - 1))
    }

    pub fn is_owned(&self, i: usize) -> bool {
        self.flags[i] & OWNED != 0
    }

    /// Turn a cell into air, keeping its temperature.
    pub fn make_air(&mut self, i: usize) {
        self.wake_at(i);
        self.mat[i] = AIR;
        self.paint[i] = 0;
        self.flags[i] = 0;
        self.life[i] = 0;
        self.burn[i] = 0;
    }

    /// Change a cell into another material, keeping its temperature.
    pub fn transform(&mut self, mats: &Materials, i: usize, id: MatId) {
        self.become_(mats, i, id);
    }

    /// Swap two cells completely.
    pub fn swap_cells(&mut self, a: usize, b: usize) {
        self.swap(a, b);
    }

    pub fn is_burning(&self, i: usize) -> bool {
        self.flags[i] & BURNING != 0
    }

    /// How many cells of each material there are.
    pub fn census(&self, mats: &Materials) -> Vec<(String, usize, usize)> {
        let mut count = vec![0usize; mats.list.len()];
        let mut burning = vec![0usize; mats.list.len()];
        for i in 0..self.mat.len() {
            let m = self.mat[i] as usize;
            if m < count.len() {
                count[m] += 1;
                if self.is_burning(i) {
                    burning[m] += 1;
                }
            }
        }
        // Things in the air still count.
        for f in &self.flying {
            if let Some(c) = count.get_mut(f.mat as usize) {
                *c += 1;
                if f.flags & BURNING != 0 {
                    burning[f.mat as usize] += 1;
                }
            }
        }
        mats.list
            .iter()
            .enumerate()
            .filter(|(i, _)| count[*i] > 0)
            .map(|(i, m)| (m.name.clone(), count[i], burning[i]))
            .collect()
    }

    /// Advance the simulation by one frame.
    pub fn step(&mut self, mats: &Materials) {
        self.frame = self.frame.wrapping_add(1).max(1);
        self.blasts.clear();
        self.plan_blocks();
        self.heat(mats);
        self.changes(mats);
        self.movement(mats);
        self.fly(mats);
        // Blasts wake everything they can reach.
        for k in 0..self.blasts.len() {
            let b = &self.blasts[k];
            let r = (b.size * 2.0) as i32 + 2;
            let (x, y) = (b.x as i32, b.y as i32);
            self.wake_box(x - r, y - r, x + r, y + r);
        }
        for a in self.awake.iter_mut() {
            *a = a.saturating_sub(1);
        }
    }

    // -------------------------------------------------------------- flying --

    /// Can a flying cell pass through cell `i`? (Gases yes; objects and
    /// anything solid, powdery or liquid, no.)
    fn open(&self, mats: &Materials, i: usize) -> bool {
        self.flags[i] & OWNED == 0 && mats.get(self.mat[i]).state == State::Gas
    }

    /// Throw cell `(x, y)` into the air with this speed (cells per frame).
    /// Returns false if it can't fly (a gas, part of an object, or too many
    /// things already flying).
    pub fn throw(&mut self, mats: &Materials, x: usize, y: usize, vx: f32, vy: f32) -> bool {
        if x >= self.w || y >= self.h {
            return false;
        }
        let i = self.idx(x, y);
        if self.flying.len() >= MAX_FLYING || self.open(mats, i) || self.is_owned(i) {
            return false;
        }
        self.flying.push(Flyer {
            x: x as f32 + 0.5,
            y: y as f32 + 0.5,
            vx,
            vy,
            mat: self.mat[i],
            temp: self.temp[i],
            life: self.life[i],
            burn: self.burn[i],
            flags: self.flags[i],
            shade: self.shade[i],
            paint: self.paint[i],
        });
        self.make_air(i);
        true
    }

    /// Like `throw`, but the cell starts flying from somewhere else (`fx`,
    /// `fy`, which must be open air): a tyre carrying sand round and
    /// flinging it off the top. Returns false if it can't.
    #[allow(clippy::too_many_arguments)]
    pub fn throw_from(&mut self, mats: &Materials, x: usize, y: usize, fx: f32, fy: f32, vx: f32, vy: f32) -> bool {
        if fx < 0.0 || fy < 0.0 || fx >= self.w as f32 || fy >= self.h as f32 {
            return false;
        }
        let start = self.idx(fx as usize, fy as usize);
        if !self.open(mats, start) || !self.throw(mats, x, y, vx, vy) {
            return false;
        }
        let f = self.flying.last_mut().unwrap();
        (f.x, f.y) = (fx, fy);
        true
    }

    /// Send a brand-new flying cell of `mat` (at its own starting
    /// temperature) from (x, y) at this speed: a fire's ember. Returns false
    /// if too many things are flying already.
    pub fn spawn_flyer(&mut self, mats: &Materials, mat: MatId, x: f32, y: f32, vx: f32, vy: f32) -> bool {
        if self.flying.len() >= MAX_FLYING || x < 0.0 || y < 0.0 || x >= self.w as f32 || y >= self.h as f32 {
            return false;
        }
        let m = mats.get(mat);
        let life = m.lifetime.map_or(0, |l| {
            let jitter = 0.5 + (self.rand() % 1000) as f32 / 1000.0;
            ((l as f32 * jitter) as u16).max(1)
        });
        let shade = self.rand() as u8;
        self.flying.push(Flyer { x, y, vx, vy, mat, temp: m.temperature, life, burn: 0, flags: 0, shade, paint: 0 });
        true
    }

    /// Move everything that's flying; whatever hits something lands.
    fn fly(&mut self, mats: &Materials) {
        if self.flying.is_empty() {
            return;
        }
        let list = std::mem::take(&mut self.flying);
        let mut keep = Vec::with_capacity(list.len());
        let (w, h) = (self.w as f32, self.h as f32);
        for mut f in list {
            let m = mats.get(f.mat);
            f.vy += FLY_GRAVITY;
            // Light things (embers, ash, snow) are carried by the air: its
            // swirls and the wind. Hot ones float up a little on their heat.
            let light = ((AIR_CARRIES - m.density) / AIR_CARRIES).clamp(0.0, 1.0);
            if light > 0.0 && f.y >= 0.0 {
                let (u, v) = self.flow_at(f.x as i32, f.y as i32);
                // Every flake is a little different (how fast it settles, when
                // it sways), so snow drifts down in a haze, not in lines.
                let me = f.shade as f32 / 255.0;
                let k = 0.12 * light * (0.55 + 0.9 * me);
                let sway = ((self.frame as f32) * (0.035 + 0.03 * me) + me * 40.0).sin() * 0.16 * light * light;
                f.vx += sway * 0.12;
                // Heavier flakes (snow) feel the wind far less than embers do:
                // they come down, drifting, instead of streaming sideways.
                f.vx += (u * 2.0 * light * light - f.vx) * k;
                f.vy += (v * 2.0 - f.vy) * k;
                if f.temp > 300.0 {
                    f.vy -= FLY_GRAVITY * 0.85 * light;
                }
            }
            f.vx = (f.vx * FLY_DRAG).clamp(-FLY_MAX_SPEED, FLY_MAX_SPEED);
            f.vy = (f.vy * FLY_DRAG).clamp(-FLY_MAX_SPEED, FLY_MAX_SPEED);
            // Explosives hot enough to go off do it right here: drop back
            // onto the grid where the normal rules set them off.
            if f.y >= 0.0 && m.explosion_size > 0.0 && (f.flags & BURNING != 0 || m.catches_fire_at.is_some_and(|c| f.temp >= c)) {
                self.land(mats, f);
                continue;
            }
            // Things that run out (fire, steam) still run out in the air.
            if m.lifetime.is_some() {
                if f.life <= 1 {
                    continue;
                }
                f.life -= 1;
            }
            // Move at most one cell at a time so nothing tunnels through walls.
            let steps = f.vx.abs().max(f.vy.abs()).ceil().max(1.0);
            let (sx, sy) = (f.vx / steps, f.vy / steps);
            let mut landed = false;
            let mut gone = false;
            for _ in 0..steps as usize {
                let (nx, ny) = (f.x + sx, f.y + sy);
                if nx < 0.0 || nx >= w || ny >= h {
                    gone = true; // off the side or bottom of the world
                    break;
                }
                if ny < 0.0 {
                    // Above the top: keep going, it'll come back down.
                    (f.x, f.y) = (nx, ny);
                    continue;
                }
                let j = self.idx(nx as usize, ny as usize);
                if self.open(mats, j) {
                    (f.x, f.y) = (nx, ny);
                } else {
                    // Hitting loose stuff (sand, water) fast knocks it flying
                    // in turn, like snooker balls: that's how a buried charge
                    // throws up the whole pile above it, not just its neighbours.
                    let speed2 = f.vx * f.vx + f.vy * f.vy;
                    let loose = matches!(mats.get(self.mat[j]).state, State::Powder | State::Liquid);
                    if loose && speed2 > KNOCK_SPEED * KNOCK_SPEED && !self.is_owned(j) {
                        let spread = ((self.rand() % 1000) as f32 / 1000.0 - 0.5) * 0.5;
                        let (kx, ky) = (f.vx * KNOCK_KEEP, f.vy * KNOCK_KEEP);
                        let (bx, by) = (kx - ky * spread, ky + kx * spread);
                        self.throw(mats, nx as usize, ny as usize, bx, by);
                    }
                    landed = true;
                    break;
                }
            }
            if gone {
                continue;
            }
            if landed {
                self.land(mats, f);
            } else {
                keep.push(f);
            }
        }
        // (Anything thrown while we were busy goes on the end.)
        keep.append(&mut self.flying);
        self.flying = keep;
    }

    /// Put a flying cell back on the grid where it is (or just above, if
    /// something else landed there first).
    fn land(&mut self, mats: &Materials, f: Flyer) {
        if f.y < 0.0 {
            return;
        }
        let x0 = (f.x as usize).min(self.w - 1);
        let y0 = (f.y as usize).min(self.h - 1);
        // Straight up first (the pile shifted under it), then nearby columns.
        let columns = (0..=16i32).flat_map(|d| if d == 0 { vec![0] } else { vec![-d, d] });
        for dx in columns {
            let x = x0 as i32 + dx;
            if x < 0 || x as usize >= self.w {
                continue;
            }
            let x = x as usize;
            // Up through loose stuff (it shifted under the cell), but never
            // through something solid: no popping out above a cave roof.
            let mut found = None;
            for y in (0..=y0).rev().take(96) {
                let i = self.idx(x, y);
                if self.open(mats, i) {
                    found = Some(y);
                    break;
                }
                if self.is_owned(i) || !matches!(mats.get(self.mat[i]).state, State::Powder | State::Liquid) {
                    break;
                }
            }
            let Some(y) = found else { continue };
            let i = self.idx(x, y);
            {
                self.wake_at(i);
                self.mat[i] = f.mat;
                self.temp[i] = f.temp;
                self.life[i] = f.life;
                self.burn[i] = f.burn;
                self.flags[i] = f.flags & !OWNED;
                self.shade[i] = f.shade;
                self.paint[i] = f.paint;
                self.tex[i] = pack_tex(x, y);
                return;
            }
        }
        // Nowhere to go (a sealed pocket): it's lost, like spray on a wall.
    }

    // ---------------------------------------------------------------- heat --

    fn heat(&mut self, mats: &Materials) {
        let (w, h) = (self.w, self.h);
        let ml = &mats.list;
        let blocks = self.bw * self.bh;
        // Heat flows across each edge between two cells. The edge conducts as
        // well as the worse of the two (like insulation), and each side's
        // temperature moves by flow / heat_capacity. Sleeping blocks keep
        // their temperatures as they are.
        const RATE: f32 = 0.24;
        for b in 0..blocks {
            let (x0, x1, y0, y1) = self.block_cells(b);
            if !self.active[b] {
                continue;
            }
            // (Block edges read their neighbours' conductivity: work it out
            // one cell further out.)
            let (ex0, ex1, ey0, ey1) = (x0.saturating_sub(1), (x1 + 1).min(w), y0.saturating_sub(1), (y1 + 1).min(h));
            for y in ey0..ey1 {
                for x in ex0..ex1 {
                    let i = y * w + x;
                    let m = &ml[self.mat[i] as usize];
                    self.k[i] = m.conducts_heat;
                    self.inv_cap[i] = 1.0 / m.heat_capacity;
                }
            }
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = y * w + x;
                    let t = self.temp[i];
                    let ki = self.k[i];
                    let mut flow = 0.0;
                    let mut edge = |j: usize| {
                        let kj = self.k[j];
                        let k = if ki < kj { ki } else { kj };
                        flow += k * (self.temp[j] - t);
                    };
                    if x > 0 {
                        edge(i - 1);
                    }
                    if x + 1 < w {
                        edge(i + 1);
                    }
                    if y > 0 {
                        edge(i - w);
                    }
                    if y + 1 < h {
                        edge(i + w);
                    }
                    let coef = (RATE * self.inv_cap[i]).min(0.24);
                    self.new_temp[i] = t + coef * flow;
                }
            }
        }
        // New temperatures in, for awake blocks only (`new_temp` keeps the old
        // ones, to see what changed).
        for b in 0..blocks {
            if !self.active[b] {
                continue;
            }
            let (x0, x1, y0, y1) = self.block_cells(b);
            for y in y0..y1 {
                let r = y * w;
                self.temp[r + x0..r + x1].swap_with_slice(&mut self.new_temp[r + x0..r + x1]);
            }
        }

        // Heat sources, burning, and slowly settling back to room temperature.
        // A block whose temperatures are still changing stays awake.
        let ambient = self.ambient;
        for b in 0..blocks {
            if !self.active[b] {
                continue;
            }
            let (x0, x1, y0, y1) = self.block_cells(b);
            let mut busy = false;
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = y * w + x;
                    let m = &ml[self.mat[i] as usize];
                    let t = &mut self.temp[i];
                    if let Some(s) = m.stays_at {
                        *t = s;
                    } else if self.flags[i] & BURNING != 0 {
                        // Burning makes its own heat: it stays at least this hot.
                        if *t < m.burn_temperature {
                            *t = m.burn_temperature;
                        }
                    } else if m.cools_off > 0.0 {
                        *t += (ambient - *t) * m.cools_off;
                    }
                    // (`new_temp` now holds last frame's temperatures.)
                    busy |= (*t - self.new_temp[i]).abs() > 0.02;
                }
            }
            if busy {
                self.awake[b] = AWAKE_FRAMES;
            }
        }
    }

    // ------------------------------------------------------------- changes --

    fn neighbours(&self, i: usize) -> [Option<usize>; 4] {
        let (x, y, w, h) = (i % self.w, i / self.w, self.w, self.h);
        [
            (y > 0).then(|| i - w),
            (y + 1 < h).then(|| i + w),
            (x > 0).then(|| i - 1),
            (x + 1 < w).then(|| i + 1),
        ]
    }

    fn changes(&mut self, mats: &Materials) {
        let ml = &mats.list;
        let w = self.w;
        for b in 0..self.bw * self.bh {
            if !self.active[b] {
                continue;
            }
            let (x0, x1, y0, y1) = self.block_cells(b);
            self.slow[b] = false; // (cells set it again if something is still waiting)
            for y in y0..y1 {
                for x in x0..x1 {
                    self.change(mats, ml, y * w + x);
                }
            }
        }
    }

    /// Temperature changes, running out, explosions, fire and reactions for
    /// one cell.
    fn change(&mut self, mats: &Materials, ml: &[crate::material::Material], i: usize) {
        {
            let id = self.mat[i];
            let m = &ml[id as usize];
            let t = self.temp[i];

            // Temperature thresholds: melt / boil when hot, freeze / condense when cold.
            let mut changed = None;
            if let Some(c) = m.melts.filter(|c| t >= c.at) {
                // Fluffy stuff (snow) is mostly air: most of it just vanishes.
                changed = Some(if m.melt_yield < 1.0 && !self.chance(m.melt_yield) { AIR } else { c.into });
            } else if let Some(c) = m.boils.filter(|c| t >= c.at) {
                changed = Some(c.into);
            } else if let Some(c) = m.freezes.filter(|c| t <= c.at) {
                changed = Some(c.into);
            } else if let Some(c) = m.condenses.filter(|c| t <= c.at) {
                changed = Some(c.into);
            }
            if let Some(into) = changed {
                self.become_(mats, i, into);
                return;
            }

            // Short-lived things (fire, smoke) run out.
            if m.lifetime.is_some() {
                self.wake_at(i);
                if self.life[i] <= 1 {
                    let into = if self.chance(m.decays_into_chance) { m.decays_into } else { AIR };
                    self.become_(mats, i, into);
                    return;
                }
                self.life[i] -= 1;
            }

            // Explosives go off as soon as they're hot enough. They carry their
            // own oxygen, so they don't need air (buried gunpowder still blows).
            if m.explosion_size > 0.0
                && let Some(ignite) = m.catches_fire_at
                    && (t >= ignite || self.flags[i] & BURNING != 0) {
                        self.blasts.push(Blast {
                            x: (i % self.w) as f32 + 0.5,
                            y: (i / self.w) as f32 + 0.5,
                            size: m.explosion_size,
                            power: m.explosion_power,
                            fire: m.gives_off,
                        });
                        let (into, bt) = (m.burns_into, m.burn_temperature);
                        self.become_(mats, i, into);
                        self.temp[i] = self.temp[i].max(bt);
                        return;
                    }

            // Burning.
            if let Some(ignite) = m.catches_fire_at {
                let burning = self.flags[i] & BURNING != 0;
                if burning || t >= ignite {
                    self.wake_at(i);
                    // Fire needs something to feed it (air).
                    let mut air = [0usize; 4];
                    let mut n_air = 0;
                    let mut doused = false;
                    let mut n_gas = 0;
                    for j in self.neighbours(i).into_iter().flatten() {
                        let nm = &ml[self.mat[j] as usize];
                        if nm.feeds_fire {
                            // Remember open gas cells first: flames only puff into those.
                            if nm.state == State::Gas {
                                air.copy_within(0..n_air, 1);
                                air[0] = j;
                                n_gas += 1;
                            } else {
                                air[n_air] = j;
                            }
                            n_air += 1;
                        }
                        doused |= nm.puts_out_fire;
                    }
                    if (n_air == 0 && !self.depth_air[i]) || doused {
                        self.flags[i] &= !BURNING; // smothered or doused
                    } else if !burning {
                        self.flags[i] |= BURNING;
                        let jitter = 0.7 + (self.rand() % 600) as f32 / 1000.0;
                        self.burn[i] = ((m.burns_for as f32 * jitter) as u16).max(1);
                    } else {
                        if let Some(puff) = m.gives_off
                            && n_gas > 0 && self.chance(m.gives_off_chance) {
                                let j = air[self.rand() as usize % n_gas];
                                let bt = m.burn_temperature;
                                self.become_(mats, j, puff);
                                self.temp[j] = self.temp[j].max(bt);
                                self.shade[j] = self.rand() as u8;
                            }
                        // Embers: now and then a glowing bit flies up and away.
                        if let Some(spark) = m.sparks
                            && n_gas > 0 && self.chance(m.sparks_chance) {
                                let j = air[self.rand() as usize % n_gas];
                                let vx = ((self.rand() % 1000) as f32 / 1000.0 - 0.5) * 0.9;
                                let vy = -(0.35 + (self.rand() % 1000) as f32 / 1000.0 * 0.8);
                                self.spawn_flyer(mats, spark, (j % self.w) as f32 + 0.5, (j / self.w) as f32 + 0.5, vx, vy);
                            }
                        if self.burn[i] <= 1 {
                            let into = if self.chance(m.burns_into_chance) { m.burns_into } else { AIR };
                            self.become_(mats, i, into);
                            return;
                        }
                        self.burn[i] -= 1;
                    }
                }
            }

            // Reactions with one random neighbour.
            if !m.reactions.is_empty() {
                let dir = self.rand() as usize % 4;
                if let Some(j) = self.neighbours(i)[dir] {
                    let other = self.mat[j];
                    if let Some(r) = m.reactions.iter().find(|r| r.with == other && r.chance >= SLOW_CHANCE) {
                        // Something that could react soon stays awake.
                        self.wake_at(i);
                        let (me, it, p) = (r.me_into, r.other_into, r.chance);
                        if self.chance(p) {
                            // Products start at their own normal temperature.
                            self.become_(mats, i, me);
                            self.temp[i] = ml[me as usize].temperature;
                            self.become_(mats, j, it);
                            self.temp[j] = ml[it as usize].temperature;
                        }
                    }
                }
            }
            // Slow reactions (drying mud...): the block is visited every
            // SLOW_EVERY frames; each neighbour is tried with the chance
            // scaled up to match what every-frame trying would give.
            if m.reactions.iter().any(|r| r.chance < SLOW_CHANCE) {
                let block = (i / self.w / BLOCK) * self.bw + (i % self.w) / BLOCK;
                let turn = self.frame % SLOW_EVERY == block as u32 % SLOW_EVERY;
                for j in self.neighbours(i).into_iter().flatten() {
                    let other = self.mat[j];
                    let Some(r) = m.reactions.iter().find(|r| r.with == other && r.chance < SLOW_CHANCE) else { continue };
                    self.slow[block] = true;
                    let (me, it, p) = (r.me_into, r.other_into, r.chance);
                    if turn && self.chance((p * SLOW_EVERY as f32 / 4.0).min(1.0)) {
                        self.become_(mats, i, me);
                        self.temp[i] = ml[me as usize].temperature;
                        self.become_(mats, j, it);
                        self.temp[j] = ml[it as usize].temperature;
                        break;
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------ movement --

    /// Work out this frame's air currents: slowly drifting swirls (curl of a
    /// smooth noise field, so the air goes round in eddies instead of
    /// piling up), plus the wind.
    fn update_flow(&mut self) {
        let (gw, gh) = (self.w / FLOW_GRID + 3, self.h / FLOW_GRID + 3);
        let t = self.frame as f32 / 300.0;
        // Smooth noise at each grid node (two sizes of swirl).
        let mut n = vec![0.0f32; gw * gh];
        for gy in 0..gh {
            for gx in 0..gw {
                let (fx, fy) = (gx as f32, gy as f32);
                n[gy * gw + gx] = value_noise(fx * 0.28, fy * 0.28, t) + 0.5 * value_noise(fx * 0.7 + 31.0, fy * 0.7 + 17.0, t * 1.7);
            }
        }
        let wind = self.wind * 10.0 / 60.0; // m/s -> cells per frame (10 cells per metre, 60 fps)
        let k = 1.6 * self.turbulence;
        self.flow.clear();
        self.flow.resize(gw * gh, (wind, 0.0));
        for gy in 1..gh - 1 {
            for gx in 1..gw - 1 {
                let dndy = (n[(gy + 1) * gw + gx] - n[(gy - 1) * gw + gx]) * 0.5;
                let dndx = (n[gy * gw + gx + 1] - n[gy * gw + gx - 1]) * 0.5;
                self.flow[gy * gw + gx] = (wind + dndy * k, -dndx * k);
            }
        }
        self.flow_w = gw;
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

    fn movement(&mut self, mats: &Materials) {
        self.update_flow();
        let fast: Vec<Fast> = mats
            .list
            .iter()
            .map(|m| Fast { state: m.state, density: m.density, flows: m.flows, slides: m.slides })
            .collect();
        let (w, h) = (self.w as i32, self.h as i32);
        let frame = self.frame;
        // Bottom to top, so falling things only move once. Alternate the
        // sideways direction so nothing drifts one way.
        for y in (0..h).rev() {
            let left_to_right = self.rand() & 1 == 0;
            let row_blocks = (y as usize / BLOCK) * self.bw;
            let mut xi = 0;
            while xi < w {
                let x = if left_to_right { xi } else { w - 1 - xi };
                if !self.active[row_blocks + x as usize / BLOCK] {
                    // Jump to the next block in this direction.
                    let bx = x as usize / BLOCK;
                    let edge = if left_to_right { ((bx + 1) * BLOCK) as i32 } else { w - (bx * BLOCK) as i32 };
                    xi = edge.max(xi + 1);
                    continue;
                }
                xi += 1;
                let i = (y * w + x) as usize;
                if self.moved[i] == frame || self.flags[i] & OWNED != 0 {
                    continue;
                }
                let f = &fast[self.mat[i] as usize];
                match f.state {
                    State::Solid => {}
                    State::Powder => self.move_powder(&fast, x, y),
                    State::Liquid => self.move_liquid(&fast, x, y, f.flows as i32),
                    State::Gas => self.move_gas(&fast, x, y, f.flows as i32),
                }
            }
        }
    }

    /// How heavy a cell is right now. Gases get lighter as they heat up.
    #[inline]
    fn weight(&self, fast: &[Fast], i: usize) -> f32 {
        let f = &fast[self.mat[i] as usize];
        if f.state == State::Gas {
            f.density * 293.0 / (self.temp[i] + 273.0).max(1.0)
        } else {
            f.density
        }
    }

    /// Can the cell at `i` (with weight `wi`) sink into `j`?
    #[inline]
    fn can_sink(&self, fast: &[Fast], wi: f32, j: usize) -> bool {
        let s = fast[self.mat[j] as usize].state;
        (s == State::Gas || s == State::Liquid) && self.flags[j] & OWNED == 0 && self.weight(fast, j) < wi
    }

    /// Can a gas at `i` (weight `wi`) rise into `j`? Needs a clear difference
    /// so still air doesn't shimmer.
    #[inline]
    fn can_rise(&self, fast: &[Fast], wi: f32, j: usize) -> bool {
        let s = fast[self.mat[j] as usize].state;
        (s == State::Gas || s == State::Liquid) && self.flags[j] & OWNED == 0 && wi < self.weight(fast, j) * 0.97
    }

    fn swap(&mut self, a: usize, b: usize) {
        // Swapping two identical cells (air with air) changes nothing.
        if self.mat[a] != self.mat[b] || (self.temp[a] - self.temp[b]).abs() > 0.05 {
            self.wake_at(a);
            self.wake_at(b);
        }
        self.mat.swap(a, b);
        self.temp.swap(a, b);
        self.life.swap(a, b);
        self.burn.swap(a, b);
        self.flags.swap(a, b);
        self.shade.swap(a, b);
        self.paint.swap(a, b);
        self.tex.swap(a, b);
        self.moved[a] = self.frame;
        self.moved[b] = self.frame;
    }

    fn try_sink(&mut self, fast: &[Fast], i: usize, wi: f32, x: i32, y: i32) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        let j = y as usize * self.w + x as usize;
        if self.can_sink(fast, wi, j) {
            self.swap(i, j);
            return true;
        }
        false
    }

    fn move_powder(&mut self, fast: &[Fast], x: i32, y: i32) {
        let i = y as usize * self.w + x as usize;
        let wi = self.weight(fast, i);
        if self.try_sink(fast, i, wi, x, y + 1) {
            return;
        }
        // Sliding down the side of the pile (damp, sticky powders rarely do).
        let slides = fast[self.mat[i] as usize].slides;
        if slides < 1.0 && !self.chance(slides) {
            return;
        }
        let d = if self.rand() & 1 == 0 { 1 } else { -1 };
        if self.try_sink(fast, i, wi, x + d, y + 1) {
            return;
        }
        self.try_sink(fast, i, wi, x - d, y + 1);
    }

    fn move_liquid(&mut self, fast: &[Fast], x: i32, y: i32, flows: i32) {
        let i = y as usize * self.w + x as usize;
        let wi = self.weight(fast, i);
        if self.try_sink(fast, i, wi, x, y + 1) {
            return;
        }
        let d = if self.rand() & 1 == 0 { 1 } else { -1 };
        if self.try_sink(fast, i, wi, x + d, y + 1) || self.try_sink(fast, i, wi, x - d, y + 1) {
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
                if !self.can_sink(fast, wi, j) {
                    break;
                }
                best = Some(j);
                // Stop early if we can fall from here.
                if self.in_bounds(nx, y + 1) && self.can_sink(fast, wi, j + self.w) {
                    break;
                }
            }
            if let Some(j) = best {
                self.swap(i, j);
                return;
            }
        }
    }

    /// Can a gas be pushed into `j` by the air currents? (Swaps with other gases.)
    #[inline]
    fn can_drift(&self, fast: &[Fast], i: usize, j: usize) -> bool {
        fast[self.mat[j] as usize].state == State::Gas && self.flags[j] & OWNED == 0 && self.mat[j] != self.mat[i]
    }

    fn move_gas(&mut self, fast: &[Fast], x: i32, y: i32, flows: i32) {
        let i = y as usize * self.w + x as usize;
        // Smoke, steam and flames get carried around by the air currents.
        // (Plain air moving through plain air would change nothing.)
        if self.mat[i] != AIR {
            let (u, v) = self.flow_at(x, y);
            if self.chance(u.abs().min(0.9)) {
                let dir = if u > 0.0 { 1 } else { -1 };
                // Blown into something solid? The air goes up and over it.
                for (dx, dy) in [(dir, 0), (dir, -1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if self.in_bounds(nx, ny) {
                        let j = ny as usize * self.w + nx as usize;
                        if self.can_drift(fast, i, j) {
                            self.swap(i, j);
                            return;
                        }
                        if fast[self.mat[j] as usize].state == State::Gas {
                            break; // open air, just the same gas: nothing to do
                        }
                    }
                }
            }
            if self.chance(v.abs().min(0.9)) {
                let ny = y + if v > 0.0 { 1 } else { -1 };
                if self.in_bounds(x, ny) {
                    let j = ny as usize * self.w + x as usize;
                    if self.can_drift(fast, i, j) {
                        self.swap(i, j);
                        return;
                    }
                }
            }
        }
        let wi = self.weight(fast, i);
        // Rising isn't perfectly steady, so plumes break up.
        if self.rand().is_multiple_of(8) {
            return;
        }
        let d = if self.rand() & 1 == 0 { 1 } else { -1 };
        for (dx, dy) in [(0, -1), (d, -1), (-d, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if self.in_bounds(nx, ny) {
                let j = ny as usize * self.w + nx as usize;
                if self.can_rise(fast, wi, j) {
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
                if !self.can_rise(fast, wi, j) {
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
