//! Sliding the whole physics world by a few cells, in place (endless maps
//! scroll this way: see endless.rs). Objects and characters keep their
//! speed and spin; the rock outlines already traced move with the ground,
//! so only the strip that comes in gets traced (instead of everything).
//! The world's cells must slide by the same amount (`World::shift`).

use super::*;

impl Physics {
    /// Move everything (dx, dy) cells. Rock outlines move with it when the
    /// shift is a whole number of chunks (else they're all traced again).
    pub fn shift(&mut self, dx: i32, dy: i32) {
        let (w, h) = (self.w as i64, self.h as i64);
        let delta = Vector::new(dx as f32 * CELL, dy as f32 * CELL);
        // A world cell index, moved; None if it's now outside.
        let moved = |i: u32| -> Option<u32> {
            let (x, y) = (i as i64 % w + dx as i64, i as i64 / w + dy as i64);
            (x >= 0 && y >= 0 && x < w && y < h).then(|| (y * w + x) as u32)
        };
        for b in self.bodies.iter_mut() {
            let rb = &mut self.set[b.handle];
            let t = rb.translation();
            rb.set_translation(t + delta, true);
            b.stamped = b.stamped.iter().filter_map(|&(wi, li)| Some((moved(wi)?, li))).collect();
        }
        self.shift_actors(dx, dy, &moved);
        // Joints fixed to the ground: their ground end is in world cells.
        for j in self.joints.iter_mut() {
            let Some(joint) = self.impulse_joints.get_mut(j.handle, true) else { continue };
            if j.spec.a == GROUND {
                j.spec.anchor_a = (j.spec.anchor_a.0 + dx as f32, j.spec.anchor_a.1 + dy as f32);
                let a1 = joint.data.local_anchor1();
                joint.data.set_local_anchor1(a1 + delta);
            }
            if j.spec.b == GROUND {
                j.spec.anchor_b = (j.spec.anchor_b.0 + dx as f32, j.spec.anchor_b.1 + dy as f32);
                let a2 = joint.data.local_anchor2();
                joint.data.set_local_anchor2(a2 + delta);
            }
        }
        // Rock outlines: chunk by chunk, moved along; chunks that slide off
        // are dropped, the ones coming in are traced next update.
        let c = CHUNK as i32;
        if dx % c == 0 && dy % c == 0 {
            let (cdx, cdy) = (dx / c, dy / c);
            let (cw, ch) = (self.cw as i32, self.ch as i32);
            let mut old: Vec<Chunk> = self.chunks.drain(..).collect();
            let mut fresh: Vec<Chunk> = (0..cw * ch).map(|_| Chunk { sig: 0, colliders: Vec::new(), recheck: 0, lazy: true }).collect();
            for cy in 0..ch {
                for cx in 0..cw {
                    let chunk = std::mem::replace(&mut old[(cy * cw + cx) as usize], Chunk { sig: 0, colliders: Vec::new(), recheck: 0, lazy: false });
                    let (nx, ny) = (cx + cdx, cy + cdy);
                    // Chunks on the far edge are traced again too: their
                    // outline may have run on into what's now gone.
                    let inside = nx >= 1 && ny >= 1 && nx < cw - 1 && ny < ch - 1;
                    if inside {
                        for &hnd in &chunk.colliders {
                            if let Some(col) = self.colliders.get_mut(hnd) {
                                let t = col.position_wrt_parent().map(|p| p.translation).unwrap_or(Vector::new(0.0, 0.0));
                                col.set_translation_wrt_parent(t + delta);
                            }
                        }
                        fresh[(ny * cw + nx) as usize] = chunk;
                    } else {
                        // Switched off now, removed a few per frame later.
                        for hnd in chunk.colliders {
                            if let Some(col) = self.colliders.get_mut(hnd) {
                                col.set_enabled(false);
                                self.graveyard.push(hnd);
                            }
                        }
                    }
                }
            }
            self.chunks = fresh;
        } else {
            for chunk in self.chunks.iter_mut() {
                for hnd in chunk.colliders.drain(..) {
                    self.colliders.remove(hnd, &mut self.islands, &mut self.set, &mut self.soft, false);
                }
                chunk.sig = 0;
            }
        }
        // What was solid last frame, moved along (so nothing looks like it
        // vanished and set off a collapse).
        let n = self.solid_prev.len();
        if n == (w * h) as usize {
            let (wu, hu) = (w as usize, h as usize);
            let v = &mut self.solid_prev;
            if dy > 0 {
                let d = dy as usize * wu;
                v.copy_within(0..n - d, d);
                v[..d].fill(false);
            } else if dy < 0 {
                let d = (-dy) as usize * wu;
                v.copy_within(d.., 0);
                v[n - d..].fill(false);
            }
            if dx != 0 {
                let d = (dx.unsigned_abs() as usize).min(wu);
                for row in v.chunks_mut(wu).take(hu) {
                    if dx > 0 {
                        row.copy_within(0..wu - d, d);
                        row[..d].fill(false);
                    } else {
                        row.copy_within(d.., 0);
                        row[wu - d..].fill(false);
                    }
                }
            }
        }
        self.strength_dirty = true;
        self.heard.clear();
    }
}
