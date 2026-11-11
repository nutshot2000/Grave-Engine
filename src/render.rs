//! Turns the world into pixels (RGBA, one pixel per cell). Runs on the CPU so
//! the same picture comes out in the window and in headless screenshots.

use crate::material::{Materials, State};
use crate::world::World;

#[derive(Clone, Copy, Default, PartialEq)]
pub struct View {
    /// Show temperature instead of normal colours.
    pub heat: bool,
}

/// Reusable buffers for drawing.
#[derive(Default)]
pub struct Renderer {
    /// Material textures, loaded the first time they're needed.
    textures: std::collections::HashMap<std::path::PathBuf, Option<crate::import::Picture>>,
    light: Vec<[f32; 3]>,
    tmp: Vec<[f32; 3]>,
    scratch: Vec<[f32; 3]>,
    pub rgba: Vec<u8>,
    /// The lighting to draw with (see light.rs; set it with `light`).
    lighting: crate::light::Lighting,
    lamps: Vec<crate::light::Placed>,
    illum: Vec<[f32; 3]>,
    sun_scratch: Vec<[f32; 3]>,
    /// Cells that give off their own light (fire, lava, red-hot metal,
    /// burning embers): drawn at full brightness however dark it is.
    self_lit: Vec<bool>,
    /// Glow spread wide (fires light up a big area in the dark).
    wide: Vec<[f32; 3]>,
    lamp_only: Vec<[f32; 3]>,
    /// Sky and scenery layers behind the world, and where the camera is
    /// (their parallax depends on it).
    backdrop: crate::backdrop::Backdrop,
    camera: Option<(f32, f32)>,
    bg: Vec<[f32; 3]>,
    bg_glow: Vec<[f32; 3]>,
    pictures: std::collections::HashMap<std::path::PathBuf, Option<crate::import::Picture>>,
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn c3(c: [u8; 3]) -> [f32; 3] {
    [c[0] as f32, c[1] as f32, c[2] as f32]
}

/// The colour of something glowing hot (dull red -> orange -> yellow-white).
pub fn hot_color(t: f32) -> [f32; 3] {
    let stops: [(f32, [f32; 3]); 4] = [
        (450.0, [90.0, 8.0, 0.0]),
        (750.0, [230.0, 70.0, 10.0]),
        (1100.0, [255.0, 170.0, 50.0]),
        (1600.0, [255.0, 240.0, 200.0]),
    ];
    if t <= stops[0].0 {
        return stops[0].1;
    }
    for w in stops.windows(2) {
        if t <= w[1].0 {
            return lerp3(w[0].1, w[1].1, (t - w[0].0) / (w[1].0 - w[0].0));
        }
    }
    stops[3].1
}

/// Colour for the heat view.
pub fn heat_color(t: f32) -> [f32; 3] {
    if t < 15.0 {
        let k = ((15.0 - t) / 60.0).clamp(0.0, 1.0);
        return lerp3([20.0, 20.0, 30.0], [80.0, 170.0, 255.0], k);
    }
    let stops: [(f32, [f32; 3]); 6] = [
        (25.0, [20.0, 20.0, 30.0]),
        (100.0, [120.0, 20.0, 90.0]),
        (300.0, [220.0, 30.0, 30.0]),
        (600.0, [255.0, 140.0, 0.0]),
        (1000.0, [255.0, 240.0, 60.0]),
        (1600.0, [255.0, 255.0, 255.0]),
    ];
    if t <= stops[0].0 {
        return stops[0].1;
    }
    for w in stops.windows(2) {
        if t <= w[1].0 {
            return lerp3(w[0].1, w[1].1, (t - w[0].0) / (w[1].0 - w[0].0));
        }
    }
    stops[5].1
}

#[inline]
fn hash(a: u32) -> u32 {
    let mut x = a.wrapping_mul(0x9E37_79B1);
    x ^= x >> 15;
    x = x.wrapping_mul(0x85EB_CA77);
    x ^= x >> 13;
    x
}

impl Renderer {
    /// Forget loaded textures (material files changed).
    pub fn reload_textures(&mut self) {
        self.textures.clear();
    }

    /// Draw with a simulation's lighting (its sun, lamps and overall light)
    /// from now on.
    pub fn light(&mut self, sim: &crate::sim::Sim) {
        self.lighting = sim.lighting.clone();
        self.lamps = sim.lamps();
        self.backdrop = sim.backdrop.clone();
    }

    /// Where the camera is centred (cells), for the backdrop's parallax.
    /// Not set = the middle of the world.
    pub fn set_camera(&mut self, x: f32, y: f32) {
        self.camera = Some((x, y));
    }

    /// Draw with this lighting from now on.
    pub fn set_lighting(&mut self, lighting: crate::light::Lighting, lamps: Vec<crate::light::Placed>) {
        self.lighting = lighting;
        self.lamps = lamps;
    }

    pub fn draw(&mut self, world: &World, mats: &Materials, view: View) -> &[u8] {
        // Load any textures not seen yet.
        let pics: Vec<Option<crate::import::Picture>> = mats
            .list
            .iter()
            .map(|m| {
                let p = m.texture.as_ref()?;
                self.textures.entry(p.clone()).or_insert_with(|| crate::import::Picture::open(p).ok()).clone()
            })
            .collect();
        let n = world.w * world.h;
        self.light.resize(n, [0.0; 3]);
        self.self_lit.clear();
        self.self_lit.resize(n, false);
        self.tmp.resize(n, [0.0; 3]);
        self.scratch.resize(n, [0.0; 3]);
        self.rgba.resize(n * 4, 255);
        let air = c3(mats.get(0).color);
        let flicker_frame = world.frame / 3;

        for i in 0..n {
            if world.flags[i] & crate::world::HIDDEN != 0 {
                self.light[i] = [0.0; 3];
                self.tmp[i] = if view.heat { lerp3(air, heat_color(world.temp[i]), 0.8) } else { air };
                continue;
            }
            let m = mats.get(world.mat[i]);
            let t = world.temp[i];
            let shade = (world.shade[i] as f32 / 255.0) * 2.0 - 1.0;
            let mut col = match &pics[world.mat[i] as usize] {
                Some(p) => {
                    let t = world.tex[i];
                    let (x, y) = ((t & 0xff) as usize % p.w, (t >> 8) as usize % p.h);
                    let o = (y * p.w + x) * 4;
                    [p.rgba[o] as f32, p.rgba[o + 1] as f32, p.rgba[o + 2] as f32]
                }
                None => c3(m.color),
            };
            let mut emit = [0.0f32; 3];

            if view.heat {
                let base = lerp3(col, [0.0; 3], 0.75);
                let h = heat_color(t);
                col = [base[0] + h[0], base[1] + h[1], base[2] + h[2]];
            } else {
                // Speckle; glowing things also flicker over time.
                let mut s = shade;
                if m.glow > 0.0 {
                    s = (hash(i as u32 ^ flicker_frame.wrapping_mul(7919)) as f32 / u32::MAX as f32) * 2.0 - 1.0;
                }
                let k = 1.0 + m.color_variation * s;
                col = [col[0] * k, col[1] * k, col[2] * k];

                // Things that only last a while fade out as they run out.
                if let Some(l) = m.lifetime {
                    let a = (world.life[i] as f32 / l as f32 * 1.6).clamp(0.0, 1.0);
                    if m.glow > 0.0 {
                        // Flames go from yellow to deep red as they die.
                        col = lerp3([200.0, 30.0, 10.0], col, a);
                        col = lerp3(air, col, (a * 2.0).min(1.0));
                    } else {
                        col = lerp3(air, col, a);
                    }
                }
                if m.glow > 0.0 {
                    emit = [col[0] * m.glow, col[1] * m.glow, col[2] * m.glow];
                }

                // Burning: flickering flames on the surface.
                if world.is_burning(i) {
                    let f = hash(i as u32 ^ flicker_frame.wrapping_mul(31337)) as f32 / u32::MAX as f32;
                    let flame = [255.0, 90.0 + 120.0 * f, 20.0 + 30.0 * f];
                    col = lerp3(col, flame, 0.55 + 0.35 * f);
                    emit = [emit[0] + flame[0] * 0.6, emit[1] + flame[1] * 0.6, emit[2] + flame[2] * 0.6];
                } else if t > 450.0 && m.state != State::Gas && m.glow == 0.0 {
                    // Anything hot enough glows (red hot iron, embers...).
                    let k = ((t - 450.0) / 700.0).clamp(0.0, 1.0);
                    let hc = hot_color(t);
                    col = lerp3(col, hc, k * 0.9);
                    emit = [emit[0] + hc[0] * k * 0.5, emit[1] + hc[1] * k * 0.5, emit[2] + hc[2] * k * 0.5];
                }
            }
            self.light[i] = emit;
            self.self_lit[i] = emit != [0.0; 3];
            self.tmp[i] = col;
        }

        // Hidden cells (under a character's pictures) take the look of
        // whatever is next to them in the row, so a swimmer sits in water,
        // not in a dark hole.
        for y in 0..world.h {
            let row = y * world.w;
            for x in 0..world.w {
                let i = row + x;
                if world.flags[i] & crate::world::HIDDEN == 0 {
                    continue;
                }
                let shown = |j: usize| world.flags[j] & crate::world::HIDDEN == 0;
                let left = (1..=24).take_while(|d| *d <= x).map(|d| i - d).find(|&j| shown(j));
                let right = (1..=24).take_while(|d| x + d < world.w).map(|d| i + d).find(|&j| shown(j));
                let pick = match (left, right) {
                    (Some(l), Some(r)) => Some(if i - l <= r - i { l } else { r }),
                    (l, r) => l.or(r),
                };
                if let Some(j) = pick {
                    self.tmp[i] = self.tmp[j];
                    self.light[i] = self.light[j];
                }
            }
        }

        // Cells flying through the air, drawn on top.
        for f in &world.flying {
            if f.x < 0.0 || f.y < 0.0 || f.x >= world.w as f32 || f.y >= world.h as f32 {
                continue;
            }
            let i = world.idx(f.x as usize, f.y as usize);
            let m = mats.get(f.mat);
            let s = (f.shade as f32 / 255.0) * 2.0 - 1.0;
            let k = 1.0 + m.color_variation * s;
            let mut col = c3(m.color);
            col = [col[0] * k, col[1] * k, col[2] * k];
            if view.heat {
                col = lerp3(col, heat_color(f.temp), 0.8);
            } else if f.flags & crate::world::BURNING != 0 || m.glow > 0.0 {
                let flame = [255.0, 150.0, 40.0];
                col = lerp3(col, flame, 0.7);
                self.light[i] = [flame[0] * 0.5, flame[1] * 0.5, flame[2] * 0.5];
                self.self_lit[i] = true;
            } else if f.temp > 450.0 {
                let k = ((f.temp - 450.0) / 700.0).clamp(0.0, 1.0);
                col = lerp3(col, hot_color(f.temp), k * 0.9);
            }
            self.tmp[i] = col;
        }

        if !view.heat {
            blur(&mut self.light, &mut self.scratch, world.w, world.h, 5);
            blur(&mut self.light, &mut self.scratch, world.w, world.h, 5);
        }
        let lit = !view.heat && !(self.lighting.is_plain() && self.lamps.is_empty());
        // In the dark, glow also reaches far: a wide, soft spread of it,
        // stronger the darker the scene.
        let dark = if lit { (1.0 - self.lighting.ambient).clamp(0.0, 1.0) } else { 0.0 };
        if dark > 0.05 {
            self.wide.clear();
            self.wide.extend_from_slice(&self.light);
            blur(&mut self.wide, &mut self.scratch, world.w, world.h, 16);
            blur(&mut self.wide, &mut self.scratch, world.w, world.h, 16);
        }
        if lit {
            crate::light::illuminate(world, mats, &self.lighting, &self.lamps, &mut self.illum, &mut self.sun_scratch, &mut self.lamp_only);
        }
        let sun_full = self.lighting.sun.map_or([0.0; 3], |s| [s.color[0] as f32 / 255.0 * s.strength, s.color[1] as f32 / 255.0 * s.strength, s.color[2] as f32 / 255.0 * s.strength]);
        let backdrop = !view.heat && !self.backdrop.is_empty();
        if backdrop {
            let (cx, cy) = self.camera.unwrap_or((world.w as f32 / 2.0, world.h as f32 / 2.0));
            self.backdrop.draw(world.w, world.h, cx, cy, world.frame, &mut self.pictures, &mut self.bg, &mut self.bg_glow);
        }
        let amb = {
            let (a, c) = (self.lighting.ambient.max(0.0), self.lighting.ambient_color);
            [a * c[0] as f32 / 255.0, a * c[1] as f32 / 255.0, a * c[2] as f32 / 255.0]
        };

        for i in 0..n {
            // Empty air shows the backdrop (not flying cells drawn there).
            let open = backdrop && world.mat[i] == crate::material::AIR && self.tmp[i] == air;
            let c = if open { self.bg[i] } else { self.tmp[i] };
            let l = self.light[i];
            let own = if open { self.bg_glow[i] } else { [0.0; 3] };
            let (r, g, b) = if view.heat {
                (c[0], c[1], c[2])
            } else if lit {
                // Sky, sun and lamps light what they fall on; glow adds its
                // own light and halo; in the air, beams show as a soft haze.
                let e = self.illum[i];
                let gas = mats.get(world.mat[i]).state == State::Gas;
                let haze = |k: usize| if gas { (e[k] - amb[k]).max(0.0) * 12.0 } else { 0.0 };
                // The far backdrop isn't shadowed by the world: it gets the
                // overall light and the sun; lamps only haze the air in front.
                let lamp = self.lamp_only[i];
                let haze = |k: usize| if open { lamp[k] * 30.0 } else { haze(k) };
                let e = if open { [(amb[0] + sun_full[0]).min(1.0), (amb[1] + sun_full[1]).min(1.0), (amb[2] + sun_full[2]).min(1.0)] } else { e };
                // Fire and glowing things shine at full strength in the dark,
                // and their glow lights up what's around them strongly.
                let e = if self.self_lit[i] { [e[0].max(1.0), e[1].max(1.0), e[2].max(1.0)] } else { e };
                let far = if dark > 0.05 { self.wide[i] } else { [0.0; 3] };
                let px = |k: usize| c[k] * (e[k] + (l[k] * 2.0 + far[k] * 8.0 * dark) / 255.0) + l[k] * 1.1 + far[k] * 2.5 * dark + haze(k) + own[k];
                (px(0), px(1), px(2))
            } else {
                // Light brightens what it falls on, and adds a soft halo in the air.
                let lit = [
                    c[0] * (1.0 + l[0] / 255.0) + l[0] * 1.1 + own[0],
                    c[1] * (1.0 + l[1] / 255.0) + l[1] * 1.1 + own[1],
                    c[2] * (1.0 + l[2] / 255.0) + l[2] * 1.1 + own[2],
                ];
                (lit[0], lit[1], lit[2])
            };
            let o = i * 4;
            self.rgba[o] = r.clamp(0.0, 255.0) as u8;
            self.rgba[o + 1] = g.clamp(0.0, 255.0) as u8;
            self.rgba[o + 2] = b.clamp(0.0, 255.0) as u8;
            self.rgba[o + 3] = 255;
        }
        &self.rgba
    }
}

/// Box blur, horizontal then vertical, radius `r`.
pub(crate) fn blur(buf: &mut [[f32; 3]], tmp: &mut [[f32; 3]], w: usize, h: usize, r: usize) {
    let norm = 1.0 / (2 * r + 1) as f32;
    for y in 0..h {
        let row = &buf[y * w..(y + 1) * w];
        let mut acc = [0.0f32; 3];
        for x in 0..=r.min(w - 1) {
            for c in 0..3 {
                acc[c] += row[x][c];
            }
        }
        for x in 0..w {
            for c in 0..3 {
                tmp[y * w + x][c] = acc[c] * norm;
            }
            if x + r + 1 < w {
                for c in 0..3 {
                    acc[c] += row[x + r + 1][c];
                }
            }
            if x >= r {
                for c in 0..3 {
                    acc[c] -= row[x - r][c];
                }
            }
        }
    }
    for x in 0..w {
        let mut acc = [0.0f32; 3];
        for y in 0..=r.min(h - 1) {
            for c in 0..3 {
                acc[c] += tmp[y * w + x][c];
            }
        }
        for y in 0..h {
            for c in 0..3 {
                buf[y * w + x][c] = acc[c] * norm;
            }
            if y + r + 1 < h {
                for c in 0..3 {
                    acc[c] += tmp[(y + r + 1) * w + x][c];
                }
            }
            if y >= r {
                for c in 0..3 {
                    acc[c] -= tmp[(y - r) * w + x][c];
                }
            }
        }
    }
}
