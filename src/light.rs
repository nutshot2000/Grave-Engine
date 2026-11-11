//! Lighting: how bright each cell is, from the sky, a sun and lamps, with
//! real shadows.
//!
//! ```text
//! lighting 0.25 #7080c0          # overall light (0 = pitch dark, 1 = full) and its tint: a blue night
//! sun 35 0.9 #ffe0b0             # sunlight from 35° left of straight down, strength 0.9, warm
//! light 120 80 60                # a lamp at 120 80 lighting 60 cells around
//! light 120 80 60 strength 1.5 color #ffb060 flicker 0.3      # a flickering torch
//! light 300 150 90 cone 0 25 on bike                          # a headlight on `bike`, pointing right, 25° either side
//! ```
//!
//! - **Shadows:** sunlight and lamplight stop at solid things; water, ice,
//!   glass and smoke let some through (material key `lets_light_through`),
//!   tinted by their colour, so light under water turns blue-green.
//! - **Glow** (fire, lava, red-hot metal) still lights things up around it.
//! - In the dark, lamps and sunbeams show as a soft haze in the air.
//! - Lights `on` an object move and turn with it (cones point where it points).
//! - Triggers can fade the overall light: `lighting 0.1 2` (to 0.1 over 2 s).
//! - A scene with no lighting lines is fully lit, as before.

use crate::material::Materials;
use crate::world::World;

/// A sun: light from one direction across the whole sky.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sun {
    /// Degrees from straight down; positive = the sun is on the left (light
    /// travels right).
    pub angle: f32,
    pub strength: f32,
    pub color: [u8; 3],
}

/// A lamp.
#[derive(Clone, Debug, PartialEq)]
pub struct Light {
    /// False for a projected facade/fill light positioned in front of the 2D plane.
    pub shadows: bool,
    /// Where (cells): in the world, or from the centre of the object it's
    /// `on` (in that object's own unturned frame).
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub strength: f32,
    pub color: [u8; 3],
    /// A cone: which way it points (degrees, 0 = right, 90 = down) and how
    /// wide (degrees either side). None = all round.
    pub cone: Option<(f32, f32)>,
    /// How much it flickers, 0..1.
    pub flicker: f32,
    /// The object it rides on: its id while running, or its number N+1 in a
    /// scene or snapshot (like joints).
    pub on: Option<u64>,
}

impl Light {
    pub fn at(x: f32, y: f32, radius: f32) -> Light {
        Light { shadows: true, x, y, radius, strength: 1.0, color: [255, 236, 200], cone: None, flicker: 0.0, on: None }
    }
}

/// A scene's lighting.
#[derive(Clone, Debug, PartialEq)]
pub struct Lighting {
    /// Light everywhere, before sun and lamps: 1 = full daylight.
    pub ambient: f32,
    pub ambient_color: [u8; 3],
    pub sun: Option<Sun>,
    pub lights: Vec<Light>,
    /// Fading the overall light: (to, frames left).
    pub fade: Option<(f32, u32)>,
    /// Moving the sun: (to, frames left). Sunset in a level.
    pub sun_fade: Option<(Sun, u32)>,
    /// Changing the overall tint: (to, frames left).
    pub tint_fade: Option<([u8; 3], u32)>,
}

impl Default for Lighting {
    fn default() -> Self {
        Lighting { ambient: 1.0, ambient_color: [255, 255, 255], sun: None, lights: Vec::new(), fade: None, sun_fade: None, tint_fade: None }
    }
}

/// A lamp where it is right now (world cells, radians).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub shadows: bool,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub strength: f32,
    pub color: [u8; 3],
    /// (direction, half width), radians.
    pub cone: Option<(f32, f32)>,
}

fn hash(a: u32) -> u32 {
    let mut x = a.wrapping_mul(0x9E37_79B1);
    x ^= x >> 15;
    x = x.wrapping_mul(0x85EB_CA77);
    x ^= x >> 13;
    x
}

impl Lighting {
    /// Plain full daylight (draws exactly like no lighting at all).
    pub fn is_plain(&self) -> bool {
        self.ambient >= 0.999 && self.ambient_color == [255, 255, 255] && self.sun.is_none() && self.lights.is_empty()
    }

    /// Fade the overall light to `to` over `seconds` (0 = at once).
    pub fn fade_to(&mut self, to: f32, seconds: f32) {
        let to = to.clamp(0.0, 1.5);
        let frames = (seconds * 60.0).round() as u32;
        if frames == 0 {
            self.ambient = to;
            self.fade = None;
        } else {
            self.fade = Some((to, frames));
        }
    }

    /// Fade the overall tint to `to` over `seconds` (0 = at once).
    pub fn tint_to(&mut self, to: [u8; 3], seconds: f32) {
        let frames = (seconds * 60.0).round() as u32;
        if frames == 0 {
            self.ambient_color = to;
            self.tint_fade = None;
        } else {
            self.tint_fade = Some((to, frames));
        }
    }

    /// Move the sun to this angle, strength and colour over `seconds` (0 =
    /// at once): a sunset, the sun going behind a mountain.
    pub fn sun_to(&mut self, to: Sun, seconds: f32) {
        let frames = (seconds * 60.0).round() as u32;
        if frames == 0 || self.sun.is_none() {
            self.sun = Some(to);
            self.sun_fade = None;
        } else {
            self.sun_fade = Some((to, frames));
        }
    }

    /// One simulation frame.
    pub fn step(&mut self) {
        if let Some((to, left)) = self.tint_fade {
            let k = 1.0 / left.max(1) as f32;
            for c in 0..3 {
                self.ambient_color[c] = (self.ambient_color[c] as f32 + (to[c] as f32 - self.ambient_color[c] as f32) * k).round() as u8;
            }
            self.tint_fade = (left > 1).then_some((to, left - 1));
        }
        if let (Some((to, left)), Some(sun)) = (self.sun_fade, self.sun.as_mut()) {
            let k = 1.0 / left.max(1) as f32;
            sun.angle += (to.angle - sun.angle) * k;
            sun.strength += (to.strength - sun.strength) * k;
            for c in 0..3 {
                sun.color[c] = (sun.color[c] as f32 + (to.color[c] as f32 - sun.color[c] as f32) * k).round() as u8;
            }
            self.sun_fade = (left > 1).then_some((to, left - 1));
        }
        if let Some((to, left)) = self.fade {
            self.ambient += (to - self.ambient) / left.max(1) as f32;
            self.fade = (left > 1).then_some((to, left - 1));
        }
    }

    /// Where every lamp is now. `pose(id)` gives an object's centre and angle.
    pub fn place(&self, frame: u32, pose: impl Fn(u64) -> Option<(f32, f32, f32)>) -> Vec<Placed> {
        let mut out = Vec::new();
        for (n, l) in self.lights.iter().enumerate() {
            let (x, y, turn) = match l.on {
                None => (l.x, l.y, 0.0),
                Some(id) => {
                    let Some((cx, cy, a)) = pose(id) else { continue };
                    let (c, s) = (a.cos(), a.sin());
                    (cx + c * l.x - s * l.y, cy + s * l.x + c * l.y, a)
                }
            };
            let mut strength = l.strength;
            if l.flicker > 0.0 {
                // Smooth-ish flicker: two noise values a few frames apart.
                let f = |k: u32| hash(k.wrapping_mul(7919) ^ (n as u32).wrapping_mul(104729)) as f32 / u32::MAX as f32;
                let t = frame / 4;
                let k = (frame % 4) as f32 / 4.0;
                let noise = f(t) * (1.0 - k) + f(t + 1) * k;
                strength *= 1.0 - l.flicker.clamp(0.0, 1.0) * noise;
            }
            let cone = l.cone.map(|(d, w)| (d.to_radians() + turn, w.to_radians().max(0.01)));
            out.push(Placed { shadows: l.shadows, x, y, radius: l.radius.max(1.0), strength, color: l.color, cone });
        }
        out
    }

    /// The scene lines for this lighting. `name(n)` names object number n.
    pub fn to_lines(&self, name: impl Fn(u64) -> String, pose: impl Fn(u64) -> Option<(f32, f32, f32)>) -> String {
        if self.is_plain() {
            return String::new();
        }
        let hex = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
        let mut s = String::from("\n# Lighting\n");
        let ambient = self.fade.map_or(self.ambient, |f| f.0);
        s += &format!("lighting {} {}\n", ambient, hex(self.ambient_color));
        if let Some(sun) = self.sun {
            s += &format!("sun {} {} {}\n", sun.angle, sun.strength, hex(sun.color));
        }
        for l in &self.lights {
            let (x, y, extra) = match l.on.and_then(|n| Some((n, pose(n)?))) {
                Some((n, (cx, cy, a))) => {
                    let (c, si) = (a.cos(), a.sin());
                    (cx + c * l.x - si * l.y, cy + si * l.x + c * l.y, format!(" on {}", name(n)))
                }
                None => (l.x, l.y, String::new()),
            };
            s += &format!("light {x:.1} {y:.1} {} strength {} color {}", l.radius, l.strength, hex(l.color));
            if let Some((d, w)) = l.cone {
                // Saved in the world's frame: the object's turn is added back on load.
                let turn = l.on.and_then(&pose).map_or(0.0, |p| p.2.to_degrees());
                s += &format!(" cone {:.1} {w}", d + turn);
            }
            if l.flicker > 0.0 {
                s += &format!(" flicker {}", l.flicker);
            }
            if !l.shadows {s += " unshadowed";}
            s += &extra;
            s.push('\n');
        }
        s
    }
}

/// How far above the window sun shadows are worked out (rows): things
/// higher than this don't shade it.
const MAX_SHADOW_ROWS: usize = 360;

/// How much light each material lets through, per cell, per colour.
fn passes(mats: &Materials) -> Vec<[f32; 3]> {
    mats.list
        .iter()
        .map(|m| {
            let p = m.light_through();
            if p <= 0.0 {
                return [0.0; 3];
            }
            // Coloured things tint the light passing through (water: blue-green).
            let c = m.color;
            let tint = |v: u8| 0.55 + 0.45 * (v as f32 / 255.0);
            if p >= 0.999 {
                [1.0; 3]
            } else {
                [p * tint(c[0]), p * tint(c[1]), p * tint(c[2])]
            }
        })
        .collect()
}

/// Work out the light falling on every cell: `out[i]` = how lit (1 = full
/// white daylight), per colour. Glow from hot things is added by the renderer.
/// `lamp_only` gets just the lamps' share (for the haze lamps make in the air).
///
/// Only for the window `win` = (x, y, w, h) of the world (what's being
/// drawn): `out` is `w * h`, row by row. Shadows from outside it still count.
#[allow(clippy::too_many_arguments)]
pub fn illuminate(world: &World, mats: &Materials, lighting: &Lighting, lamps: &[Placed], win: (usize, usize, usize, usize), out: &mut Vec<[f32; 3]>, scratch: &mut Vec<[f32; 3]>, lamp_only: &mut Vec<[f32; 3]>) {
    let (wx0, wy0, ww, wh) = win;
    let n = ww * wh;
    let inside = |x: i32, y: i32| -> Option<usize> {
        (x >= wx0 as i32 && y >= wy0 as i32 && x < (wx0 + ww) as i32 && y < (wy0 + wh) as i32).then(|| (y as usize - wy0) * ww + x as usize - wx0)
    };
    // Sun and lamps go into `out` first, get softened a little (light spills
    // just round edges), then the overall light is added.
    out.clear();
    out.resize(n, [0.0; 3]);
    let a = lighting.ambient.max(0.0);
    let ac = lighting.ambient_color;
    let ambient = [a * ac[0] as f32 / 255.0, a * ac[1] as f32 / 255.0, a * ac[2] as f32 / 255.0];
    let pass = passes(mats);
    let through = |i: usize| -> [f32; 3] {
        if world.flags[i] & crate::world::HIDDEN != 0 {
            [1.0; 3] // a character's hidden body cells (they cast no shadow)
        } else {
            pass[world.mat[i] as usize]
        }
    };

    // Sunlight, swept down row by row at the sun's slant.
    if let Some(sun) = lighting.sun.filter(|s| s.strength > 0.0) {
        let slope = sun.angle.to_radians().tan().clamp(-3.0, 3.0);
        scratch.clear();
        scratch.resize(n, [0.0; 3]);
        // Sweep down from open sky over just the slanted band that can reach
        // the window: wide at the top, narrowing to the window's width at its
        // bottom. Light from more than `MAX_SHADOW_ROWS` above is taken as
        // open sky (so tall worlds cost the same as short ones).
        let (w, bottom) = (world.w, wy0 + wh);
        let top = bottom.saturating_sub(MAX_SHADOW_ROWS);
        let mut prev = vec![[1.0f32; 3]; w]; // light leaving the row above (open sky)
        let mut row = vec![[0.0f32; 3]; w];
        for y in top..bottom {
            let reach = (slope.abs() * (bottom - y) as f32).ceil() as usize + 2;
            let (c0, c1) = (wx0.saturating_sub(reach), (wx0 + ww + reach).min(w));
            for x in c0..c1 {
                let src = x as f32 - slope;
                let x0 = src.floor();
                let k = src - x0;
                let get = |xi: f32| -> [f32; 3] {
                    if xi < 0.0 || xi >= w as f32 { [1.0; 3] } else { prev[xi as usize] }
                };
                let (l0, l1) = (get(x0), get(x0 + 1.0));
                row[x] = [l0[0] * (1.0 - k) + l1[0] * k, l0[1] * (1.0 - k) + l1[1] * k, l0[2] * (1.0 - k) + l1[2] * k];
            }
            for x in c0..c1 {
                let i = y * w + x;
                if y >= wy0 && x >= wx0 && x < wx0 + ww {
                    scratch[(y - wy0) * ww + x - wx0] = row[x];
                }
                let t = through(i);
                prev[x] = [row[x][0] * t[0], row[x][1] * t[1], row[x][2] * t[2]];
            }
        }
        let c = [sun.color[0] as f32 / 255.0 * sun.strength, sun.color[1] as f32 / 255.0 * sun.strength, sun.color[2] as f32 / 255.0 * sun.strength];
        for i in 0..n {
            for k in 0..3 {
                out[i][k] += scratch[i][k] * c[k];
            }
        }
    }

    // Remember the sunlight so the lamps' share can be told apart.
    lamp_only.clear();
    lamp_only.extend_from_slice(out);
    // Lamps: rays outward from each, stopping at solid things.
    let mut local: Vec<f32> = Vec::new();
    let mut tint: Vec<[f32; 3]> = Vec::new();
    for lamp in lamps {
        let r = lamp.radius.min(400.0);
        // Too far from the window to light any of it.
        if lamp.x + r < wx0 as f32 || lamp.y + r < wy0 as f32 || lamp.x - r > (wx0 + ww) as f32 || lamp.y - r > (wy0 + wh) as f32 {
            continue;
        }
        let ri = r.ceil() as i32;
        let side = (2 * ri + 1) as usize;
        local.clear();
        local.resize(side * side, 0.0);
        tint.clear();
        tint.resize(side * side, [1.0; 3]);
        let rays = ((std::f32::consts::TAU * r * 1.6).ceil() as usize).max(16);
        for ray in 0..rays {
            let ang = ray as f32 / rays as f32 * std::f32::consts::TAU;
            let mut cone_k = 1.0;
            if let Some((dir, half)) = lamp.cone {
                let mut d = (ang - dir).rem_euclid(std::f32::consts::TAU);
                if d > std::f32::consts::PI {
                    d = std::f32::consts::TAU - d;
                }
                if d > half {
                    continue;
                }
                cone_k = (1.0 - (d / half).powi(2)).max(0.0); // soft edge
            }
            let (dx, dy) = (ang.cos() * 0.5, ang.sin() * 0.5);
            let mut t = [1.0f32; 3];
            let (mut px, mut py) = (lamp.x, lamp.y);
            let mut last = usize::MAX;
            let steps = (r * 2.0) as usize;
            for s in 0..steps {
                px += dx;
                py += dy;
                let (cx, cy) = (px.floor() as i32, py.floor() as i32);
                if !world.in_bounds(cx, cy) {
                    break;
                }
                let i = world.idx(cx as usize, cy as usize);
                if i == last {
                    continue;
                }
                last = i;
                let d = (s + 1) as f32 * 0.5 / r;
                let fall = (1.0 - d).max(0.0).powf(1.4) * cone_k;
                let (lx, ly) = ((cx - lamp.x.floor() as i32 + ri) as usize, (cy - lamp.y.floor() as i32 + ri) as usize);
                if lx < side && ly < side {
                    let li = ly * side + lx;
                    let v = fall * (t[0] + t[1] + t[2]) / 3.0;
                    if v > local[li] {
                        local[li] = v;
                        let m = t[0].max(t[1]).max(t[2]).max(1e-4);
                        tint[li] = [t[0] / m, t[1] / m, t[2] / m];
                    }
                }
                // (The lamp's own spot doesn't block it: a lamp on an object.)
                if s >= 3 && lamp.shadows {
                    let p = through(i);
                    t = [t[0] * p[0], t[1] * p[1], t[2] * p[2]];
                }
                if t[0] + t[1] + t[2] < 0.01 {
                    break;
                }
            }
        }
        let c = [lamp.color[0] as f32 / 255.0 * lamp.strength, lamp.color[1] as f32 / 255.0 * lamp.strength, lamp.color[2] as f32 / 255.0 * lamp.strength];
        let (ox, oy) = (lamp.x.floor() as i32 - ri, lamp.y.floor() as i32 - ri);
        for ly in 0..side {
            for lx in 0..side {
                let v = local[ly * side + lx];
                if v <= 0.0 {
                    continue;
                }
                let (x, y) = (ox + lx as i32, oy + ly as i32);
                let Some(i) = inside(x, y) else { continue };
                let tn = tint[ly * side + lx];
                for k in 0..3 {
                    out[i][k] += v * c[k] * tn[k];
                }
            }
        }
    }
    for (l, o) in lamp_only.iter_mut().zip(out.iter()) {
        *l = [(o[0] - l[0]).max(0.0), (o[1] - l[1]).max(0.0), (o[2] - l[2]).max(0.0)];
    }
    if !lamps.is_empty() || lighting.sun.is_some() {
        scratch.resize(n, [0.0; 3]);
        crate::render::blur(out, scratch, ww, wh, 1);
    }
    for v in out.iter_mut() {
        for k in 0..3 {
            v[k] += ambient[k];
        }
    }
}
