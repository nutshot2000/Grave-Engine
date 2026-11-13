//! Software isometric projection and depth-sorted material voxels. Projection,
//! picking and arbitrary visual instances are shared by all overhead games.
use super::{Point, Volume};
use crate::{Materials, State, material::AIR, world::BURNING};
use std::collections::BTreeMap;
#[derive(Clone, Copy)]
pub struct Projection {
    pub origin: [f32; 2],
    pub scale: f32,
}
impl Projection {
    pub fn project(&self, p: Point) -> [f32; 2] {
        [
            self.origin[0] + (p.x - p.y) * self.scale,
            self.origin[1] + ((p.x + p.y) * 0.5 - p.z) * self.scale,
        ]
    }
    pub fn unproject(&self, p: [f32; 2], z: f32) -> Point {
        let a = (p[0] - self.origin[0]) / self.scale;
        let b = (p[1] - self.origin[1]) / self.scale + z;
        Point::new(b + a * 0.5, b - a * 0.5, z)
    }
    /// Pick the foremost solid surface along the viewing ray, not an assumed
    /// flat ground plane. Aiming at a roof therefore targets its actual height.
    pub fn pick(&self, p: [f32; 2], v: &Volume, m: &Materials) -> Point {
        for z in (0..v.height).rev() {
            let q = self.unproject(p, z as f32 + 0.5);
            if v.solid_at(q, m) {
                return q;
            }
        }
        self.unproject(p, 2.)
    }
}
#[derive(Clone, Copy)]
pub struct VisualCell {
    pub at: Point,
    pub color: [u8; 3],
}
pub struct Instance<'a> {
    pub at: Point,
    pub angle: f32,
    pub cells: &'a [VisualCell],
    pub shadow_radius: f32,
}
struct Draw {
    p: Point,
    color: [u8; 3],
    faces: [bool; 3],
    gas: u8,
    alpha: f32,
}
/// Reused buffers; a game only supplies camera, instances and projectile points.
pub struct Viewport {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub projection: Projection,
    /// Optional continuous scenery under a finite simulation volume.
    pub ground_color: Option<[u8; 3]>,
    glow: Vec<[f32; 3]>,
    glow_scratch: Vec<[f32; 3]>,
    frame: u32,
}
impl Viewport {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height * 4],
            glow: vec![[0.; 3]; width * height],
            glow_scratch: vec![[0.; 3]; width * height],
            frame: 0,
            ground_color: None,
            projection: Projection {
                origin: [width as f32 * 0.5, height as f32 * 0.5],
                scale: 2.5,
            },
        }
    }
    fn pixel(&mut self, x: i32, y: i32, c: [u8; 3], alpha: f32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let i = (y as usize * self.width + x as usize) * 4;
        for k in 0..3 {
            self.pixels[i + k] = (self.pixels[i + k] as f32 * (1. - alpha) + c[k] as f32 * alpha)
                .clamp(0., 255.) as u8;
        }
        self.pixels[i + 3] = 255;
    }
    fn polygon(&mut self, points: &[[f32; 2]], c: [u8; 3], alpha: f32) {
        let lo = points
            .iter()
            .map(|p| p[1])
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.) as i32;
        let hi = points
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            .min(self.height as f32) as i32;
        for y in lo..hi {
            let scan = y as f32 + 0.5;
            let mut cuts = [0.; 8];
            let mut n = 0;
            for i in 0..points.len() {
                let a = points[i];
                let b = points[(i + 1) % points.len()];
                if (a[1] <= scan && b[1] > scan) || (b[1] <= scan && a[1] > scan) {
                    cuts[n] = a[0] + (scan - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
                    n += 1;
                }
            }
            if n >= 2 {
                let x0 = cuts[..n]
                    .iter()
                    .copied()
                    .fold(f32::INFINITY, f32::min)
                    .ceil() as i32;
                let x1 = cuts[..n]
                    .iter()
                    .copied()
                    .fold(f32::NEG_INFINITY, f32::max)
                    .ceil() as i32;
                for x in x0.max(0)..x1.min(self.width as i32) {
                    self.pixel(x, y, c, alpha);
                }
            }
        }
    }
    fn cube(&mut self, d: &Draw) {
        let p = d.p;
        let project = |x: f32, y: f32, z: f32| {
            self.projection
                .project(Point::new(p.x + x, p.y + y, p.z + z))
        };
        let o = project(0., 0., 1.);
        let a = project(1., 0., 1.);
        let b = project(1., 1., 1.);
        let c = project(0., 1., 1.);
        let f = project(1., 0., 0.);
        let g = project(1., 1., 0.);
        let h = project(0., 1., 0.);
        if d.gas == 1 {
            self.flame(d);
            return;
        }
        if d.gas == 2 {
            self.smoke(d);
            return;
        }
        let shade = |v: f32| d.color.map(|c| (c as f32 * v).min(255.) as u8);
        if d.faces[1] {
            self.polygon(&[a, f, g, b], shade(0.64), 1.);
        }
        if d.faces[2] {
            self.polygon(&[b, g, h, c], shade(0.8), 1.);
        }
        if d.faces[0] {
            self.polygon(&[o, a, b, c], d.color, 1.);
        }
    }
    // Gas is upright and camera-facing. Flattening it onto the ground-plane
    // diamond is what made overhead fire look like scattered orange dashes.
    fn flame(&mut self, d: &Draw) {
        let base = self
            .projection
            .project(Point::new(d.p.x + 0.5, d.p.y + 0.5, d.p.z + 0.2));
        let seed = noise(
            d.p.x.to_bits() ^ d.p.y.to_bits().rotate_left(11) ^ d.p.z.to_bits().rotate_left(21),
        );
        let f = noise(seed ^ (self.frame / 3).wrapping_mul(7919)) as f32 / u32::MAX as f32;
        let scale = self.projection.scale;
        let h = scale * (2.4 + f * 2.2);
        let width = scale * (0.62 + f * 0.3);
        let sway = ((self.frame as f32 * 0.18 + seed as f32 * 0.00001).sin()) * scale * 0.65;
        for dy in 0..h.ceil() as i32 {
            let t = dy as f32 / h;
            let wave = noise(seed ^ (dy as u32).wrapping_mul(31337) ^ self.frame / 3) as f32
                / u32::MAX as f32;
            let radius = (width * (1. - t).powf(0.7) * (0.7 + 0.35 * (t * 3.14).sin())
                + wave * 0.3)
                .max(0.3);
            let cx = base[0] + sway * t * t + (wave - 0.5) * 0.7;
            for x in (cx - radius).floor() as i32..=(cx + radius).ceil() as i32 {
                let q = ((x as f32 + 0.5 - cx) / radius).abs();
                if q > 1. {
                    continue;
                }
                let core = (1. - q) * (1. - t);
                let color = if core > 0.72 {
                    [d.color[0], (d.color[1] as f32 * 1.32).min(205.) as u8, 58]
                } else if core > 0.28 {
                    d.color
                } else {
                    [
                        d.color[0].saturating_sub(40),
                        (d.color[1] as f32 * 0.46) as u8,
                        (d.color[2] as f32 * 0.5) as u8,
                    ]
                };
                self.pixel(
                    x,
                    (base[1] - dy as f32) as i32,
                    color,
                    d.alpha * (0.55 + core * 0.4),
                );
            }
        }
        let x = base[0] as i32;
        let y = (base[1] - h * 0.3) as i32;
        if x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32 {
            let i = y as usize * self.width + x as usize;
            for k in 0..3 {
                self.glow[i][k] = self.glow[i][k].max(d.color[k] as f32 * d.alpha * 1.3);
            }
        }
    }
    fn smoke(&mut self, d: &Draw) {
        let centre = self
            .projection
            .project(Point::new(d.p.x + 0.5, d.p.y + 0.5, d.p.z + 1.));
        let seed = noise(d.p.x.to_bits() ^ d.p.y.to_bits() ^ d.p.z.to_bits());
        let radius = self.projection.scale * (1.6 + (seed % 100) as f32 * 0.009);
        for dy in -radius.ceil() as i32..=radius.ceil() as i32 {
            for dx in -radius.ceil() as i32..=radius.ceil() as i32 {
                let r = (dx * dx + dy * dy) as f32 / (radius * radius);
                if r >= 1. {
                    continue;
                }
                self.pixel(
                    centre[0] as i32 + dx,
                    centre[1] as i32 + dy,
                    d.color,
                    d.alpha * (1. - r).powi(2),
                );
            }
        }
    }
    pub fn draw(
        &mut self,
        v: &Volume,
        m: &Materials,
        camera: Point,
        instances: &[Instance<'_>],
        tracers: &[Point],
    ) -> &[u8] {
        self.frame = v.frame;
        self.glow.fill([0.; 3]);
        self.projection.origin = [
            self.width as f32 * 0.5 - (camera.x - camera.y) * self.projection.scale,
            self.height as f32 * 0.56
                - ((camera.x + camera.y) * 0.5 - camera.z) * self.projection.scale,
        ];
        for (i, p) in self.pixels.chunks_exact_mut(4).enumerate() {
            let n = noise(i as u32) % 7;
            if let Some(c) = self.ground_color {
                let factor = 0.81 + n as f32 * 0.012;
                p.copy_from_slice(&[
                    (c[0] as f32 * factor) as u8,
                    (c[1] as f32 * factor) as u8,
                    (c[2] as f32 * factor) as u8,
                    255,
                ]);
            } else {
                p.copy_from_slice(&[35 + n as u8, 32 + n as u8, 29 + n as u8, 255]);
            }
        }
        let mut draw = Vec::new();
        let mut effects = BTreeMap::<(u8, i32, i32, i32), Emitter>::new();
        for (y, s) in v.sections.iter().enumerate() {
            let w = &s.world;
            for row in 0..v.height {
                let z = v.height - 1 - row;
                for x in 0..v.width {
                    let i = row * v.width + x;
                    let id = w.mat[i];
                    if id == AIR || v.debris.is_stamped(y, i) {
                        continue;
                    }
                    let mat = m.get(id);
                    let gas = mat.state == State::Gas;
                    let p = Point::new(x as f32, y as f32, z as f32);
                    let screen = self.projection.project(p);
                    if screen[0] < -8.
                        || screen[0] > self.width as f32 + 8.
                        || screen[1] < -8.
                        || screen[1] > self.height as f32 + 12.
                    {
                        continue;
                    }
                    let visible = |p: Point| {
                        let other = v.material_at(p);
                        other == AIR || m.get(other).state == State::Gas
                    };
                    let faces = [
                        visible(Point::new(p.x, p.y, p.z + 1.)),
                        visible(Point::new(p.x + 1., p.y, p.z)),
                        visible(Point::new(p.x, p.y + 1., p.z)),
                    ];
                    if !gas && !faces.iter().any(|v| *v) {
                        continue;
                    }
                    if gas {
                        let age = mat
                            .lifetime
                            .map_or(1., |l| (w.life[i] as f32 / l.max(1) as f32).clamp(0., 1.));
                        add_emitter(
                            &mut effects,
                            p,
                            mat.color,
                            if mat.glow > 0. { 1 } else { 2 },
                            age,
                        );
                        continue;
                    }
                    let base = crate::asset::paint_rgb(w.paint[i]).unwrap_or(mat.color);
                    let f = noise(
                        (i + y * w.mat.len()) as u32
                            ^ if gas || w.flags[i] & BURNING != 0 {
                                v.frame / 3 * 7919
                            } else {
                                0
                            },
                    ) as f32
                        / u32::MAX as f32;
                    let mut color = base.map(|c| (c as f32 * (0.93 + f * 0.14)).min(255.) as u8);
                    if w.flags[i] & BURNING != 0 {
                        color = burning_color(color, f);
                        if faces.iter().any(|v| *v) {
                            add_emitter(
                                &mut effects,
                                Point::new(p.x, p.y, p.z + 1.),
                                [255, 132, 25],
                                1,
                                0.85,
                            );
                        }
                    }
                    if !gas {
                        for a in instances {
                            let dx = p.x - a.at.x;
                            let dy = p.y - a.at.y;
                            if a.at.z > p.z && dx * dx + dy * dy < a.shadow_radius.powi(2) {
                                color = color.map(|c| (c as f32 * 0.57) as u8);
                            }
                        }
                    }
                    if gas && mat.glow == 0. {
                        color = color.map(|c| (c as f32 * 0.65) as u8);
                    }
                    draw.push(Draw {
                        p,
                        color,
                        faces,
                        gas: 0,
                        alpha: if gas {
                            if mat.glow > 0. { 0.7 } else { 0.25 }
                        } else {
                            1.
                        },
                    });
                }
            }
        }
        for plume in &v.plumes {
            let mat = m.get(plume.material);
            let age = mat
                .lifetime
                .map_or(1., |l| (plume.life as f32 / l.max(1) as f32).clamp(0., 1.));
            add_emitter(
                &mut effects,
                plume.at,
                mat.color,
                if mat.glow > 0. { 1 } else { 2 },
                age,
            );
        }
        for (centre, cell) in v.debris.voxels() {
            let mat = m.get(cell.mat);
            let p = Point::new(centre.x - 0.5, centre.y - 0.5, centre.z - 0.5);
            let base = crate::asset::paint_rgb(cell.paint).unwrap_or(mat.color);
            let f = noise(
                cell.shade as u32
                    ^ if cell.flags & BURNING != 0 {
                        v.frame / 3 * 7919
                    } else {
                        0
                    },
            ) as f32
                / u32::MAX as f32;
            let mut color = base.map(|c| (c as f32 * (0.93 + 0.14 * f)).min(255.) as u8);
            if cell.flags & BURNING != 0 {
                color = burning_color(color, f);
                add_emitter(
                    &mut effects,
                    Point::new(p.x, p.y, p.z + 1.),
                    [255, 132, 25],
                    1,
                    0.85,
                );
            }
            draw.push(Draw {
                p,
                color,
                faces: [true; 3],
                gas: 0,
                alpha: 1.,
            });
        }

        for a in instances {
            let (s, c) = a.angle.sin_cos();
            for cell in a.cells {
                let p = Point::new(
                    a.at.x + cell.at.x * c - cell.at.y * s,
                    a.at.y + cell.at.x * s + cell.at.y * c,
                    a.at.z + cell.at.z,
                );
                draw.push(Draw {
                    p,
                    color: cell.color,
                    faces: [true; 3],
                    gas: 0,
                    alpha: 1.,
                });
            }
        }
        for p in tracers {
            draw.push(Draw {
                p: *p,
                color: [255, 224, 110],
                faces: [true; 3],
                gas: 0,
                alpha: 1.,
            });
        }
        for ((kind, _, _, _), e) in effects {
            let n = e.count as f32;
            let p = Point::new(e.p.x / n, e.p.y / n, e.p.z / n);
            let color = e.color.map(|v| (v / n).clamp(0., 255.) as u8);
            draw.push(Draw {
                p,
                color,
                faces: [false; 3],
                gas: kind,
                alpha: if kind == 1 {
                    (0.4 + (n / 12.).min(0.3)) * e.age.max(0.25)
                } else {
                    (0.16 + (n / 25.).min(0.18)) * e.age
                },
            });
        }
        draw.sort_by(|a, b| (a.p.x + a.p.y + a.p.z).total_cmp(&(b.p.x + b.p.y + b.p.z)));
        for d in draw {
            self.cube(&d);
        }
        // The same box-blur used by the 2D renderer, with a small capped halo
        // so a large fire lights its surroundings without bleaching the fuel.
        crate::render::blur(
            &mut self.glow,
            &mut self.glow_scratch,
            self.width,
            self.height,
            4,
        );
        crate::render::blur(
            &mut self.glow,
            &mut self.glow_scratch,
            self.width,
            self.height,
            4,
        );
        for (i, p) in self.pixels.chunks_exact_mut(4).enumerate() {
            for k in 0..3 {
                let halo = (self.glow[i][k] * 0.8).min([24., 13., 4.][k]);
                p[k] = (p[k] as f32 + halo).min(255.) as u8;
            }
        }
        &self.pixels
    }
}
fn noise(mut n: u32) -> u32 {
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb352d);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846ca68b);
    n ^ (n >> 16)
}

struct Emitter {
    p: Point,
    color: [f32; 3],
    count: u32,
    age: f32,
}
fn add_emitter(
    out: &mut BTreeMap<(u8, i32, i32, i32), Emitter>,
    p: Point,
    color: [u8; 3],
    kind: u8,
    age: f32,
) {
    let size = if kind == 1 {
        [2., 2., 3.]
    } else {
        [3., 3., 4.]
    };
    let key = (
        kind,
        (p.x / size[0]).floor() as i32,
        (p.y / size[1]).floor() as i32,
        (p.z / size[2]).floor() as i32,
    );
    let e = out.entry(key).or_insert(Emitter {
        p: Point::default(),
        color: [0.; 3],
        count: 0,
        age: 0.,
    });
    e.p.x += p.x;
    e.p.y += p.y;
    e.p.z += p.z;
    e.count += 1;
    e.age = e.age.max(age);
    for k in 0..3 {
        e.color[k] += color[k] as f32;
    }
}
fn burning_color(base: [u8; 3], f: f32) -> [u8; 3] {
    let amount = if f > 0.8 { 0.5 } else { 0.12 + f * 0.12 };
    let flame = [255., 85. + 85. * f, 15. + 20. * f];
    std::array::from_fn(|k| {
        (base[k] as f32 * 0.62 * (1. - amount) + flame[k] * amount).min(255.) as u8
    })
}
