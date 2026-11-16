//! Changes (melting, boiling, freezing, running out, explosives going off,
//! fire, reactions), shared between the CPU's cores the same way movement
//! is (movement.rs): 64x64 patches in four checkerboard rounds. A cell only
//! ever touches itself and its four neighbours here, so far-apart patches
//! never meet.
//!
//! Things that affect the whole world (a blast going off, a spark thrown
//! into the air) are noted by each patch and added afterwards, patch by
//! patch in a fixed order, so the result is the same on any number of cores.

use super::movement::{PATCH, patch_seed};
use super::*;
use crate::material::Material;

/// Raw access to the cell layers this touches (see movement.rs `Grid`).
struct Cells {
    mat: *mut MatId,
    temp: *mut f32,
    life: *mut u16,
    burn: *mut u16,
    flags: *mut u8,
    shade: *mut u8,
    paint: *mut u32,
    coat: *mut MatId,
    awake: *mut u8,
    slow: *mut bool,
}
// Safety: as for movement's `Grid`: patches worked on together are 64 cells
// apart and a cell only touches its direct neighbours.
unsafe impl Send for Cells {}
unsafe impl Sync for Cells {}

/// What one patch wants done to the whole world afterwards.
#[derive(Default)]
struct Outbox {
    blasts: Vec<Blast>,
    /// Sparks to throw: (material, x, y, vx, vy).
    sparks: Vec<(MatId, f32, f32, f32, f32)>,
}

struct Changer<'a> {
    c: &'a Cells,
    ml: &'a [Material],
    active: &'a [bool],
    depth_air: &'a [bool],
    w: usize,
    h: usize,
    bw: usize,
    frame: u32,
    rng: u32,
    out: Outbox,
}

impl Changer<'_> {
    #[inline]
    fn mat(&self, i: usize) -> MatId {
        unsafe { *self.c.mat.add(i) }
    }
    #[inline]
    fn temp(&self, i: usize) -> f32 {
        unsafe { *self.c.temp.add(i) }
    }
    #[inline]
    fn set_temp(&self, i: usize, t: f32) {
        unsafe { *self.c.temp.add(i) = t }
    }
    #[inline]
    fn flags(&self, i: usize) -> u8 {
        unsafe { *self.c.flags.add(i) }
    }
    #[inline]
    fn set_flags(&self, i: usize, f: u8) {
        unsafe { *self.c.flags.add(i) = f }
    }
    #[inline]
    fn life(&self, i: usize) -> u16 {
        unsafe { *self.c.life.add(i) }
    }
    #[inline]
    fn burn(&self, i: usize) -> u16 {
        unsafe { *self.c.burn.add(i) }
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
    fn block_of(&self, i: usize) -> usize {
        (i / self.w / BLOCK) * self.bw + (i % self.w) / BLOCK
    }
    #[inline]
    fn wake_at(&self, i: usize) {
        unsafe { *self.c.awake.add(self.block_of(i)) = AWAKE_FRAMES };
    }
    fn neighbours(&self, i: usize) -> [Option<usize>; 4] {
        let (x, y, w, h) = (i % self.w, i / self.w, self.w, self.h);
        [(y > 0).then(|| i - w), (y + 1 < h).then(|| i + w), (x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1)]
    }

    /// Change a cell's material, keeping its temperature (as `World::become_`).
    fn become_(&mut self, i: usize, id: MatId) {
        self.wake_at(i);
        let life = match self.ml[id as usize].lifetime {
            Some(l) => {
                let jitter = 0.5 + (self.rand() % 1000) as f32 / 1000.0;
                ((l as f32 * jitter) as u16).max(1)
            }
            None => 0,
        };
        unsafe {
            *self.c.mat.add(i) = id;
            *self.c.paint.add(i) = 0;
            *self.c.coat.add(i) = AIR;
            *self.c.flags.add(i) = 0;
            *self.c.burn.add(i) = 0;
            *self.c.life.add(i) = life;
        }
    }

    /// Every awake block of patch (px, py), block by block.
    fn patch(&mut self, px: usize, py: usize) {
        for by in 2 * py..(2 * py + 2).min(self.h.div_ceil(BLOCK)) {
            for bx in 2 * px..(2 * px + 2).min(self.bw) {
                let b = by * self.bw + bx;
                if !self.active[b] {
                    continue;
                }
                // (Cells set it again if something is still waiting.)
                unsafe { *self.c.slow.add(b) = false };
                let (x0, x1) = (bx * BLOCK, ((bx + 1) * BLOCK).min(self.w));
                let (y0, y1) = (by * BLOCK, ((by + 1) * BLOCK).min(self.h));
                for y in y0..y1 {
                    for x in x0..x1 {
                        self.change(y * self.w + x);
                    }
                }
            }
        }
    }

    /// Temperature changes, running out, explosions, fire and reactions for
    /// one cell.
    fn change(&mut self, i: usize) {
        let ml = self.ml;
        let id = self.mat(i);
        let m = &ml[id as usize];
        let t = self.temp(i);

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
            self.become_(i, into);
            return;
        }

        // Geysers: in bursts, spit lumps up into the air above. The block is
        // visited every SLOW_EVERY frames even asleep, and spits on those
        // visits, so an eruption comes in pulses.
        if let Some(lump) = m.spits {
            let block = self.block_of(i);
            unsafe { *self.c.slow.add(block) = true };
            let turn = self.frame % SLOW_EVERY == block as u32 % SLOW_EVERY;
            if turn && i >= self.w && ml[self.mat(i - self.w) as usize].state == State::Gas {
                let (x, y) = ((i % self.w) as u32, (i / self.w) as u32);
                // Each vent (16x16 cells of it) keeps its own rhythm.
                let mut h = (x / 16).wrapping_mul(0x9E37_79B9) ^ (y / 16).wrapping_mul(0x85EB_CA6B);
                h ^= h >> 15;
                let every = m.spits_every.max(30);
                if (self.frame + h % every) % every < every / 5 {
                    for _ in 0..4 {
                        if !self.chance(m.spits_chance) {
                            continue;
                        }
                        let r = |s: &mut Self| (s.rand() % 1000) as f32 / 1000.0;
                        let vx = (r(self) - 0.5) * 0.9;
                        let vy = -m.spits_speed * (0.6 + r(self) * 0.5);
                        self.out.sparks.push((lump, x as f32 + 0.5, y as f32 - 0.5, vx, vy));
                    }
                }
            }
        }

        // Short-lived things (fire, smoke) run out.
        if m.lifetime.is_some() {
            self.wake_at(i);
            if self.life(i) <= 1 {
                let into = if self.chance(m.decays_into_chance) { m.decays_into } else { AIR };
                self.become_(i, into);
                return;
            }
            unsafe { *self.c.life.add(i) -= 1 };
        }

        // Explosives go off as soon as they're hot enough. They carry their
        // own oxygen, so they don't need air (buried gunpowder still blows).
        if m.explosion_size > 0.0
            && let Some(ignite) = m.catches_fire_at
            && (t >= ignite || self.flags(i) & BURNING != 0)
        {
            self.out.blasts.push(Blast {
                x: (i % self.w) as f32 + 0.5,
                y: (i / self.w) as f32 + 0.5,
                size: m.explosion_size,
                power: m.explosion_power,
                fire: m.gives_off,
            });
            let (into, bt) = (m.burns_into, m.burn_temperature);
            self.become_(i, into);
            self.set_temp(i, self.temp(i).max(bt));
            return;
        }

        // Burning.
        if let Some(ignite) = m.catches_fire_at {
            let burning = self.flags(i) & BURNING != 0;
            if burning || t >= ignite {
                self.wake_at(i);
                // Fire needs something to feed it (air).
                let mut air = [0usize; 4];
                let mut n_air = 0;
                let mut doused = false;
                let mut n_gas = 0;
                for j in self.neighbours(i).into_iter().flatten() {
                    let nm = &ml[self.mat(j) as usize];
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
                    self.set_flags(i, self.flags(i) & !BURNING); // smothered or doused
                } else if !burning {
                    self.set_flags(i, self.flags(i) | BURNING);
                    let jitter = 0.7 + (self.rand() % 600) as f32 / 1000.0;
                    unsafe { *self.c.burn.add(i) = ((m.burns_for as f32 * jitter) as u16).max(1) };
                } else {
                    if let Some(puff) = m.gives_off
                        && n_gas > 0
                        && self.chance(m.gives_off_chance)
                    {
                        let j = air[self.rand() as usize % n_gas];
                        let bt = m.burn_temperature;
                        self.become_(j, puff);
                        self.set_temp(j, self.temp(j).max(bt));
                        let shade = self.rand() as u8;
                        unsafe { *self.c.shade.add(j) = shade };
                    }
                    // Embers: now and then a glowing bit flies up and away.
                    if let Some(spark) = m.sparks
                        && n_gas > 0
                        && self.chance(m.sparks_chance)
                    {
                        let j = air[self.rand() as usize % n_gas];
                        let vx = ((self.rand() % 1000) as f32 / 1000.0 - 0.5) * 0.9;
                        let vy = -(0.35 + (self.rand() % 1000) as f32 / 1000.0 * 0.8);
                        self.out.sparks.push((spark, (j % self.w) as f32 + 0.5, (j / self.w) as f32 + 0.5, vx, vy));
                    }
                    if self.burn(i) <= 1 {
                        let into = if self.chance(m.burns_into_chance) { m.burns_into } else { AIR };
                        self.become_(i, into);
                        return;
                    }
                    unsafe { *self.c.burn.add(i) -= 1 };
                }
            }
        }

        // Reactions with one random neighbour.
        if !m.reactions.is_empty() {
            let dir = self.rand() as usize % 4;
            if let Some(j) = self.neighbours(i)[dir] {
                let other = self.mat(j);
                if let Some(r) = m.reactions.iter().find(|r| r.with == other && r.chance >= SLOW_CHANCE) {
                    // Something that could react soon stays awake.
                    self.wake_at(i);
                    let (me, it, p) = (r.me_into, r.other_into, r.chance);
                    if self.chance(p) {
                        // Products start at their own normal temperature.
                        self.become_(i, me);
                        self.set_temp(i, ml[me as usize].temperature);
                        self.become_(j, it);
                        self.set_temp(j, ml[it as usize].temperature);
                    }
                }
            }
        }
        // Slow reactions (drying mud...): the block is visited every
        // SLOW_EVERY frames; each neighbour is tried with the chance
        // scaled up to match what every-frame trying would give.
        if m.reactions.iter().any(|r| r.chance < SLOW_CHANCE) {
            let block = self.block_of(i);
            let turn = self.frame % SLOW_EVERY == block as u32 % SLOW_EVERY;
            for j in self.neighbours(i).into_iter().flatten() {
                let other = self.mat(j);
                let Some(r) = m.reactions.iter().find(|r| r.with == other && r.chance < SLOW_CHANCE) else { continue };
                unsafe { *self.c.slow.add(block) = true };
                let (me, it, p) = (r.me_into, r.other_into, r.chance);
                if turn && self.chance((p * SLOW_EVERY as f32 / 4.0).min(1.0)) {
                    self.become_(i, me);
                    self.set_temp(i, ml[me as usize].temperature);
                    self.become_(j, it);
                    self.set_temp(j, ml[it as usize].temperature);
                    break;
                }
            }
        }
    }
}

impl World {
    pub(super) fn changes(&mut self, mats: &Materials) {
        let base = self.rand();
        let rounds = self.patch_rounds();
        let awake_cells: usize = rounds.iter().map(|r| r.len()).sum::<usize>() * PATCH * PATCH;
        let threads = if awake_cells >= PARALLEL_CELLS { worker_threads() } else { 1 };
        let cells = Cells {
            mat: self.mat.as_mut_ptr(),
            temp: self.temp.as_mut_ptr(),
            life: self.life.as_mut_ptr(),
            burn: self.burn.as_mut_ptr(),
            flags: self.flags.as_mut_ptr(),
            shade: self.shade.as_mut_ptr(),
            paint: self.paint.as_mut_ptr(),
            coat: self.coat.as_mut_ptr(),
            awake: self.awake.as_mut_ptr(),
            slow: self.slow.as_mut_ptr(),
        };
        let (w, h, bw, frame) = (self.w, self.h, self.bw, self.frame);
        let (active, depth_air, ml) = (&self.active, &self.depth_air, &mats.list[..]);
        let mut outboxes: Vec<Outbox> = Vec::new();
        for round in &rounds {
            if round.is_empty() {
                continue;
            }
            let per = round.len().div_ceil(threads);
            let cells = &cells;
            let parts = run_bands(round.chunks(per).map(|part| {
                move || {
                    part.iter()
                        .map(|&(px, py)| {
                            let mut c = Changer { c: cells, ml, active, depth_air, w, h, bw, frame, rng: patch_seed(base, px, py), out: Outbox::default() };
                            c.patch(px, py);
                            c.out
                        })
                        .collect::<Vec<_>>()
                }
            }));
            outboxes.extend(parts.into_iter().flatten());
        }
        // Now the world-wide things, in patch order.
        for out in outboxes {
            self.blasts.extend(out.blasts);
            for (spark, x, y, vx, vy) in out.sparks {
                self.spawn_flyer(mats, spark, x, y, vx, vy);
            }
        }
    }
}
