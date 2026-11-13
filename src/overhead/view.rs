//! Software isometric projection and depth-sorted material voxels. Projection,
//! picking and arbitrary visual instances are shared by all overhead games.
use super::{Point, Volume};
use crate::{Materials, State, material::AIR, world::BURNING};
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
    gas: bool,
    alpha: f32,
}
/// Reused buffers; a game only supplies camera, instances and projectile points.
pub struct Viewport {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub projection: Projection,
}
impl Viewport {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height * 4],
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
        if d.gas {
            self.polygon(&[o, a, b, c], d.color, d.alpha);
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
    pub fn draw(
        &mut self,
        v: &Volume,
        m: &Materials,
        camera: Point,
        instances: &[Instance<'_>],
        tracers: &[Point],
    ) -> &[u8] {
        self.projection.origin = [
            self.width as f32 * 0.5 - (camera.x - camera.y) * self.projection.scale,
            self.height as f32 * 0.56 - (camera.x + camera.y) * 0.5 * self.projection.scale,
        ];
        for (i, p) in self.pixels.chunks_exact_mut(4).enumerate() {
            let n = noise(i as u32) % 7;
            p.copy_from_slice(&[35 + n as u8, 32 + n as u8, 29 + n as u8, 255]);
        }
        let mut draw = Vec::new();
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
                        let flame = [255., 90. + 120. * f, 20. + 30. * f];
                        for k in 0..3 {
                            color[k] = (color[k] as f32 * 0.3 + flame[k] * 0.7).min(255.) as u8;
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
                        gas,
                        alpha: if gas {
                            if mat.glow > 0. { 0.7 } else { 0.25 }
                        } else {
                            1.
                        },
                    });
                }
            }
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
                let flame = [255., 90. + 120. * f, 20. + 30. * f];
                for k in 0..3 {
                    color[k] = (color[k] as f32 * 0.3 + flame[k] * 0.7) as u8;
                }
            }
            draw.push(Draw {
                p,
                color,
                faces: [true; 3],
                gas: false,
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
                    gas: false,
                    alpha: 1.,
                });
            }
        }
        for p in tracers {
            draw.push(Draw {
                p: *p,
                color: [255, 224, 110],
                faces: [true; 3],
                gas: false,
                alpha: 1.,
            });
        }
        draw.sort_by(|a, b| (a.p.x + a.p.y + a.p.z).total_cmp(&(b.p.x + b.p.y + b.p.z)));
        for d in draw {
            self.cube(&d);
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
