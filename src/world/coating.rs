//! Coatings: a thin film of liquid left on solid ground (a wet road after
//! rain, an oil slick). Water or oil touching solid scenery wets it; the film
//! stays after the liquid runs off and dries over the liquid's `coat_lasts`
//! seconds. A coated surface grips less (the liquid's `coat_grip`, a share
//! of the dry grip), so tyres skid and boxes slide. A film of something
//! flammable (oil) catches fire when the ground gets hot enough; a film
//! that boils (water on hot metal) is gone at once.
//!
//! How dry a film is is worked out from the clock (`coat_until`), so a wet
//! road doesn't keep its area awake while it dries. Physics re-traces the
//! ground's grip in steps (`coat_level`: 3 = soaked .. 0 = dry).

use super::{BURNING, OWNED, World};
use crate::material::{AIR, MatId, Materials, State};

/// `coat_until` for a film that never dries (painted with `coat ... forever`).
pub const FOREVER: u32 = u32::MAX;

/// How many grip steps a drying film goes through (soaked, damp, nearly dry).
const LEVELS: f32 = 3.0;

impl World {
    /// How long a film of `liquid` lasts, in frames.
    fn film_frames(mats: &Materials, liquid: MatId) -> f32 {
        mats.get(liquid).coat_lasts * 60.0
    }

    /// Coat solid cell `i` with `liquid`: fresh (`FOREVER` to never dry).
    pub fn coat_cell(&mut self, mats: &Materials, i: usize, liquid: MatId, forever: bool) {
        if mats.get(self.mat[i]).state != State::Solid || self.flags[i] & OWNED != 0 {
            return;
        }
        let until = if forever { FOREVER } else { self.frame.saturating_add(Self::film_frames(mats, liquid) as u32).min(FOREVER - 1) };
        if self.coat[i] != liquid {
            self.wake_at(i);
            self.coat[i] = liquid;
            self.coat_until[i] = until;
        } else if self.coat_until[i] != FOREVER {
            // Topped up (never turns a lasting film back into a drying one).
            self.coat_until[i] = until;
        }
    }

    /// Is cell `i`'s surface open to the air or a liquid (somewhere a film
    /// can be)? Cells buried inside the ground aren't.
    pub fn exposed(&self, mats: &Materials, i: usize) -> bool {
        let (x, y) = (i % self.w, i / self.w);
        let open = |j: usize| self.flags[j] & OWNED != 0 || mats.get(self.mat[j]).state != State::Solid;
        (y > 0 && open(i - self.w)) || (y + 1 < self.h && open(i + self.w)) || (x > 0 && open(i - 1)) || (x + 1 < self.w && open(i + 1))
    }

    /// Take any film off cell `i`.
    pub fn uncoat(&mut self, i: usize) {
        if self.coat[i] != AIR {
            self.coat[i] = AIR;
            self.coat_until[i] = 0;
            self.wake_at(i);
        }
    }

    /// How wet cell `i` is with its film, 0 (dry) to 1 (soaked). Still
    /// touching its liquid (a puddle sitting there) counts as soaked.
    pub fn wetness(&self, mats: &Materials, i: usize) -> f32 {
        let c = self.coat[i];
        if c == AIR {
            return 0.0;
        }
        let until = self.coat_until[i];
        if until == FOREVER {
            return 1.0;
        }
        let (x, y) = (i % self.w, i / self.w);
        let touching = (y > 0 && self.mat[i - self.w] == c)
            || (y + 1 < self.h && self.mat[i + self.w] == c)
            || (x > 0 && self.mat[i - 1] == c)
            || (x + 1 < self.w && self.mat[i + 1] == c);
        if touching {
            return 1.0;
        }
        (until.saturating_sub(self.frame) as f32 / Self::film_frames(mats, c).max(1.0)).clamp(0.0, 1.0)
    }

    /// The film's grip step: 3 soaked, 2 damp, 1 nearly dry, 0 dry.
    pub fn coat_level(&self, mats: &Materials, i: usize) -> u8 {
        (self.wetness(mats, i) * LEVELS).ceil() as u8
    }

    /// When cell `i`'s film next steps down a level (frame), if it's drying.
    pub fn coat_changes_at(&self, mats: &Materials, i: usize) -> Option<u32> {
        let c = self.coat[i];
        if c == AIR || self.coat_until[i] == FOREVER {
            return None;
        }
        let level = self.coat_level(mats, i);
        if level == 0 {
            return None;
        }
        let step = Self::film_frames(mats, c) / LEVELS;
        Some(self.coat_until[i].saturating_sub((step * (level - 1) as f32) as u32).max(self.frame + 1))
    }

    /// How much grip a surface keeps under a film of `liquid` at this level
    /// (a share of its dry grip: 1 = no change).
    pub fn film_grip(mats: &Materials, liquid: MatId, level: u8) -> f32 {
        if liquid == AIR || level == 0 {
            return 1.0;
        }
        let lose = 1.0 - mats.get(liquid).coat_grip;
        1.0 - lose * (level as f32 / LEVELS).min(1.0)
    }

    /// Each frame, for the awake parts of the world: liquids wet the solid
    /// ground they touch; films on ground that's gone, hot or dry go.
    pub(super) fn coatings(&mut self, mats: &Materials) {
        let film: Vec<bool> = mats.list.iter().map(|m| m.leaves_film()).collect();
        let solid: Vec<bool> = mats.list.iter().map(|m| m.state == State::Solid).collect();
        let w = self.w;
        for b in 0..self.bw * self.bh {
            if !self.active[b] {
                continue;
            }
            let (x0, x1, y0, y1) = self.block_cells(b);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = y * w + x;
                    let id = self.mat[i];
                    if film[id as usize] && self.flags[i] & OWNED == 0 {
                        let around = [(y > 0).then(|| i - w), (y + 1 < self.h).then(|| i + w), (x > 0).then(|| i - 1), (x + 1 < w).then(|| i + 1)];
                        for j in around.into_iter().flatten() {
                            let mj = self.mat[j] as usize;
                            // A film is only replaced by more of the same, or once
                            // it has dried (water doesn't wash an oil slick away).
                            if solid[mj] && self.flags[j] & OWNED == 0 && (self.coat[j] == id || self.coat[j] == AIR || self.wetness(mats, j) == 0.0) {
                                self.coat_cell(mats, j, id, false);
                            }
                        }
                    } else if self.coat[i] != AIR {
                        self.keep_film(mats, i, solid[id as usize]);
                    }
                }
            }
        }
    }

    /// A coated cell: does its film last this frame?
    fn keep_film(&mut self, mats: &Materials, i: usize, solid: bool) {
        if !solid || self.flags[i] & OWNED != 0 || (self.coat_until[i] != FOREVER && self.frame >= self.coat_until[i] && self.wetness(mats, i) == 0.0) {
            self.coat[i] = AIR;
            self.coat_until[i] = 0;
            return;
        }
        let c = mats.get(self.coat[i]);
        // The film is on the surface: hot air or flames just above it count.
        let (x, y) = (i % self.w, i / self.w);
        let mut t = self.temp[i];
        for (dx, dy) in [(0i32, -1i32), (-1, 0), (1, 0)] {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if self.in_bounds(nx, ny) {
                t = t.max(self.temp[ny as usize * self.w + nx as usize]);
            }
        }
        // Hot enough to set the film alight: it burns off in a lick of flame.
        if c.catches_fire_at.is_some_and(|f| t >= f) || (self.flags[i] & BURNING != 0 && c.catches_fire_at.is_some()) {
            let flame = c.gives_off;
            let heat = c.burn_temperature;
            self.uncoat(i);
            if let Some(flame) = flame {
                let (x, y) = (i % self.w, i / self.w);
                let open = [(0i32, -1i32), (-1, 0), (1, 0), (0, 1)].iter().find_map(|&(dx, dy)| {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if !self.in_bounds(nx, ny) {
                        return None;
                    }
                    let j = ny as usize * self.w + nx as usize;
                    (self.mat[j] == AIR && self.flags[j] & OWNED == 0).then_some(j)
                });
                if let Some(j) = open {
                    self.become_(mats, j, flame);
                    self.temp[j] = heat;
                }
            }
            // (A thin film warms the ground, it doesn't make rock glow.)
            self.temp[i] = self.temp[i].max(heat * 0.25);
            return;
        }
        // Hot enough to boil it: gone at once.
        if c.boils.as_ref().is_some_and(|b| t >= b.at) {
            self.uncoat(i);
        }
    }
}
