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
        self.tmp.resize(n, [0.0; 3]);
        self.scratch.resize(n, [0.0; 3]);
        self.rgba.resize(n * 4, 255);
        let air = c3(mats.get(0).color);
        let flicker_frame = world.frame / 3;

        for i in 0..n {
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
            self.tmp[i] = col;
        }

        if !view.heat {
            blur(&mut self.light, &mut self.scratch, world.w, world.h, 5);
            blur(&mut self.light, &mut self.scratch, world.w, world.h, 5);
        }

        for i in 0..n {
            let c = self.tmp[i];
            let l = self.light[i];
            let (r, g, b) = if view.heat {
                (c[0], c[1], c[2])
            } else {
                // Light brightens what it falls on, and adds a soft halo in the air.
                let lit = [
                    c[0] * (1.0 + l[0] / 255.0) + l[0] * 1.1,
                    c[1] * (1.0 + l[1] / 255.0) + l[1] * 1.1,
                    c[2] * (1.0 + l[2] / 255.0) + l[2] * 1.1,
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
fn blur(buf: &mut [[f32; 3]], tmp: &mut [[f32; 3]], w: usize, h: usize, r: usize) {
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
