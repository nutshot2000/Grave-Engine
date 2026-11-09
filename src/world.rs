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
    /// Per-cell random number for colour speckle.
    pub shade: Vec<u8>,
    /// The frame a cell last moved in (so nothing moves twice per frame).
    moved: Vec<u32>,
    pub frame: u32,
    pub ambient: f32,
    rng: u32,
    // Scratch buffers, reused every frame.
    new_temp: Vec<f32>,
    k: Vec<f32>,
    inv_cap: Vec<f32>,
}

/// Per-material numbers the hot loops need, gathered once per frame.
struct Fast {
    state: State,
    density: f32,
    flows: u8,
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
            shade: vec![0; n],
            moved: vec![0; n],
            frame: 1,
            ambient: ROOM_TEMP,
            rng: 0x9E37_79B9,
            new_temp: vec![0.0; n],
            k: vec![0.0; n],
            inv_cap: vec![0.0; n],
        };
        for i in 0..n {
            world.shade[i] = world.rand() as u8;
        }
        world
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
    }

    /// Change a cell's material, keeping its temperature.
    fn become_(&mut self, mats: &Materials, i: usize, id: MatId) {
        self.mat[i] = id;
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
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                let (dx, dy) = (x - cx, y - cy);
                if dx * dx + dy * dy <= r * r + r / 2 && self.in_bounds(x, y) {
                    self.set(mats, x as usize, y as usize, id);
                }
            }
        }
    }

    /// Add heat (or remove it, with a negative amount) in a circle.
    pub fn heat_circle(&mut self, cx: i32, cy: i32, r: i32, amount: f32) {
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                let (dx, dy) = (x - cx, y - cy);
                if dx * dx + dy * dy <= r * r + r / 2 && self.in_bounds(x, y) {
                    let i = self.idx(x as usize, y as usize);
                    self.temp[i] = (self.temp[i] + amount).clamp(-273.0, 5000.0);
                }
            }
        }
    }

    pub fn is_owned(&self, i: usize) -> bool {
        self.flags[i] & OWNED != 0
    }

    /// Turn a cell into air, keeping its temperature.
    pub fn make_air(&mut self, i: usize) {
        self.mat[i] = AIR;
        self.flags[i] = 0;
        self.life[i] = 0;
        self.burn[i] = 0;
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
        self.heat(mats);
        self.changes(mats);
        self.movement(mats);
    }

    // ---------------------------------------------------------------- heat --

    fn heat(&mut self, mats: &Materials) {
        let (w, h) = (self.w, self.h);
        let ml = &mats.list;
        for i in 0..w * h {
            let m = &ml[self.mat[i] as usize];
            self.k[i] = m.conducts_heat;
            self.inv_cap[i] = 1.0 / m.heat_capacity;
        }
        // Heat flows across each edge between two cells. The edge conducts as
        // well as the worse of the two (like insulation), and each side's
        // temperature moves by flow / heat_capacity.
        const RATE: f32 = 0.24;
        for y in 0..h {
            for x in 0..w {
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
        std::mem::swap(&mut self.temp, &mut self.new_temp);

        // Heat sources, burning, and slowly settling back to room temperature.
        let ambient = self.ambient;
        for i in 0..w * h {
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
        let n = self.w * self.h;
        let ml = &mats.list;
        for i in 0..n {
            let id = self.mat[i];
            let m = &ml[id as usize];
            let t = self.temp[i];

            // Temperature thresholds: melt / boil when hot, freeze / condense when cold.
            let mut changed = None;
            if let Some(c) = m.melts.filter(|c| t >= c.at) {
                changed = Some(c.into);
            } else if let Some(c) = m.boils.filter(|c| t >= c.at) {
                changed = Some(c.into);
            } else if let Some(c) = m.freezes.filter(|c| t <= c.at) {
                changed = Some(c.into);
            } else if let Some(c) = m.condenses.filter(|c| t <= c.at) {
                changed = Some(c.into);
            }
            if let Some(into) = changed {
                self.become_(mats, i, into);
                continue;
            }

            // Short-lived things (fire, smoke) run out.
            if m.lifetime.is_some() {
                if self.life[i] <= 1 {
                    let into = m.decays_into;
                    self.become_(mats, i, into);
                    continue;
                }
                self.life[i] -= 1;
            }

            // Burning.
            if let Some(ignite) = m.catches_fire_at {
                let burning = self.flags[i] & BURNING != 0;
                if burning || t >= ignite {
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
                    if n_air == 0 || doused {
                        self.flags[i] &= !BURNING; // smothered or doused
                    } else if !burning {
                        self.flags[i] |= BURNING;
                        let jitter = 0.7 + (self.rand() % 600) as f32 / 1000.0;
                        self.burn[i] = ((m.burns_for as f32 * jitter) as u16).max(1);
                    } else {
                        if let Some(puff) = m.gives_off {
                            if n_gas > 0 && self.chance(m.gives_off_chance) {
                                let j = air[self.rand() as usize % n_gas];
                                let bt = m.burn_temperature;
                                self.become_(mats, j, puff);
                                self.temp[j] = self.temp[j].max(bt);
                                self.shade[j] = self.rand() as u8;
                            }
                        }
                        if self.burn[i] <= 1 {
                            let into = m.burns_into;
                            self.become_(mats, i, into);
                            continue;
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
                    if let Some(r) = m.reactions.iter().find(|r| r.with == other) {
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
        }
    }

    // ------------------------------------------------------------ movement --

    fn movement(&mut self, mats: &Materials) {
        let fast: Vec<Fast> = mats
            .list
            .iter()
            .map(|m| Fast { state: m.state, density: m.density, flows: m.flows })
            .collect();
        let (w, h) = (self.w as i32, self.h as i32);
        let frame = self.frame;
        // Bottom to top, so falling things only move once. Alternate the
        // sideways direction so nothing drifts one way.
        for y in (0..h).rev() {
            let left_to_right = self.rand() & 1 == 0;
            for xi in 0..w {
                let x = if left_to_right { xi } else { w - 1 - xi };
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
        self.mat.swap(a, b);
        self.temp.swap(a, b);
        self.life.swap(a, b);
        self.burn.swap(a, b);
        self.flags.swap(a, b);
        self.shade.swap(a, b);
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

    fn move_gas(&mut self, fast: &[Fast], x: i32, y: i32, flows: i32) {
        let i = y as usize * self.w + x as usize;
        let wi = self.weight(fast, i);
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
        if flows > 0 && self.rand() % 3 != 0 {
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
