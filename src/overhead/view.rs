//! Software isometric projection and depth-sorted material voxels. Projection,
//! picking and arbitrary visual instances are shared by all overhead games.
use super::{Point, Volume, space::Space};
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
    pub fn pick(&self, p: [f32; 2], v: &impl Space, m: &Materials) -> Point {
        for z in (0..v.extent()[2]).rev() {
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
    size: [f32; 3],
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
    frame: u32,
    terrain_texture: bool,
}
impl Viewport {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height * 4],
            glow: vec![[0.; 3]; width * height],
            frame: 0,
            terrain_texture: false,
            ground_color: None,
            projection: Projection {
                origin: [width as f32 * 0.5, height as f32 * 0.5],
                scale: 2.5,
            },
        }
    }
    fn pixel(&mut self, x: i32, y: i32, mut c: [u8; 3], alpha: f32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        if self.terrain_texture {
            let wx = (x as f32 - self.projection.origin[0]) as i32;
            let wy = (y as f32 - self.projection.origin[1]) as i32;
            let hash =
                noise((wx as u32).wrapping_mul(73856093) ^ (wy as u32).wrapping_mul(19349663));
            let shade = 0.92 + (hash % 257) as f32 / 256. * 0.16;
            c = c.map(|a| (a as f32 * shade).min(255.) as u8);
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
            self.projection.project(Point::new(
                p.x + x * d.size[0],
                p.y + y * d.size[1],
                p.z + z * d.size[2],
            ))
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
        self.draw_many(
            &[(v, Point::default())],
            None,
            m,
            camera,
            instances,
            tracers,
        )
    }
    pub fn draw_map(
        &mut self,
        map: &super::map::WorldMap,
        m: &Materials,
        camera: Point,
        instances: &[Instance<'_>],
        tracers: &[Point],
    ) -> &[u8] {
        let scale = self.projection.scale;
        let project = |p: Point| {
            [
                self.width as f32 * 0.5 + ((p.x - p.y) - (camera.x - camera.y)) * scale,
                self.height as f32 * 0.56
                    + ((p.x + p.y - camera.x - camera.y) * 0.5 - p.z + camera.z) * scale,
            ]
        };
        let visible = |p: [f32; 2], r: f32| {
            p[0] >= -r
                && p[0] < self.width as f32 + r
                && p[1] >= -r
                && p[1] < self.height as f32 + r
        };
        let regions: Vec<_> = map
            .regions
            .iter()
            .filter_map(|(&key, r)| {
                let o = super::map::WorldMap::offset(key);
                let mut lo = [f32::INFINITY; 2];
                let mut hi = [f32::NEG_INFINITY; 2];
                for x in [o.x, o.x + 64.] {
                    for y in [o.y, o.y + 64.] {
                        for z in [0., map.height as f32] {
                            let p = project(Point::new(x, y, z));
                            for k in 0..2 {
                                lo[k] = lo[k].min(p[k]);
                                hi[k] = hi[k].max(p[k]);
                            }
                        }
                    }
                }
                let mut show = hi[0] >= -24.
                    && lo[0] < self.width as f32 + 24.
                    && hi[1] >= -24.
                    && lo[1] < self.height as f32 + 24.;
                if !show && !r.volume.debris.is_empty() {
                    show = r.volume.debris.states().iter().any(|f| {
                        visible(
                            project(Point::new(f.at.x + o.x, f.at.y + o.y, f.at.z)),
                            f.extent.distance(Point::default()) * scale * 1.5 + 16.,
                        )
                    });
                }
                if !show {
                    show = r.volume.plumes.iter().any(|p| {
                        visible(project(Point::new(p.at.x + o.x, p.at.y + o.y, p.at.z)), 24.)
                    });
                }
                show.then_some((&r.volume, o))
            })
            .collect();
        self.draw_many(&regions, Some(map), m, camera, instances, tracers)
    }
    fn draw_many(
        &mut self,
        volumes: &[(&Volume, Point)],
        map: Option<&super::map::WorldMap>,
        m: &Materials,
        camera: Point,
        instances: &[Instance<'_>],
        tracers: &[Point],
    ) -> &[u8] {
        self.frame = map.map_or_else(|| volumes.first().map_or(0, |v| v.0.frame), |v| v.frame);

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
        if let Some(map) = map {
            let reach = (self.width as f32 * 0.25 + self.height as f32 * 0.5)
                / self.projection.scale
                + map.height as f32;
            let x0 = ((camera.x - reach).max(0.) as usize / 8) * 8;
            let x1 = ((camera.x + reach) as usize).min(map.width);
            let y0 = ((camera.y - reach).max(0.) as usize / 8) * 8;
            let y1 = ((camera.y + reach) as usize).min(map.depth);
            for y in (y0..y1).step_by(8) {
                for x in (x0..x1).step_by(8) {
                    if map.regions.contains_key(&(x / 64, y / 64)) {
                        continue;
                    }
                    let h = map.ground_height(x as f32, y as f32);
                    let p = Point::new(x as f32, y as f32, 0.);
                    let screen = self.projection.project(Point::new(p.x, p.y, h as f32));
                    if screen[0] < -32.
                        || screen[0] > self.width as f32 + 32.
                        || screen[1] < -32.
                        || screen[1] > self.height as f32 + 64.
                    {
                        continue;
                    }
                    draw.push(Draw {
                        p,
                        size: [8., 8., h as f32 + 1.],
                        color: map.ground_color(x as f32 + 4., y as f32 + 4.),
                        faces: [
                            true,
                            h > map.ground_height((x + 8) as f32, y as f32),
                            h > map.ground_height(x as f32, (y + 8) as f32),
                        ],
                        gas: 0,
                        alpha: 1.,
                    });
                }
            }
        }
        for &(v, offset) in volumes {
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
                        let p = Point::new(x as f32 + offset.x, y as f32 + offset.y, z as f32);
                        let screen = self.projection.project(p);
                        if screen[0] < -8.
                            || screen[0] > self.width as f32 + 8.
                            || screen[1] < -8.
                            || screen[1] > self.height as f32 + 12.
                        {
                            continue;
                        }
                        let visible = |p: Point| {
                            let other = map.map_or_else(
                                || v.material_at(Point::new(p.x - offset.x, p.y - offset.y, p.z)),
                                |map| map.material_at(p),
                            );
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
                        let mut color =
                            base.map(|c| (c as f32 * (0.93 + f * 0.14)).min(255.) as u8);
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
                            size: [1.; 3],
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
                let projected = self.projection.project(Point::new(
                    plume.at.x + offset.x,
                    plume.at.y + offset.y,
                    plume.at.z,
                ));
                if projected[0] < -16.
                    || projected[0] > self.width as f32 + 16.
                    || projected[1] < -16.
                    || projected[1] > self.height as f32 + 24.
                {
                    continue;
                }
                let mat = m.get(plume.material);
                let age = mat
                    .lifetime
                    .map_or(1., |l| (plume.life as f32 / l.max(1) as f32).clamp(0., 1.));
                add_emitter(
                    &mut effects,
                    Point::new(plume.at.x + offset.x, plume.at.y + offset.y, plume.at.z),
                    mat.color,
                    if mat.glow > 0. { 1 } else { 2 },
                    age,
                );
            }
            for (centre, cell) in v.debris.voxels() {
                let mat = m.get(cell.mat);
                let p = Point::new(
                    centre.x + offset.x - 0.5,
                    centre.y + offset.y - 0.5,
                    centre.z - 0.5,
                );
                let projected = self.projection.project(p);
                if projected[0] < -16.
                    || projected[0] > self.width as f32 + 16.
                    || projected[1] < -16.
                    || projected[1] > self.height as f32 + 24.
                {
                    continue;
                }
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
                    size: [1.; 3],
                    p,
                    color,
                    faces: [true; 3],
                    gas: 0,
                    alpha: 1.,
                });
            }
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
                    size: [1.; 3],
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
                size: [1.; 3],
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
                size: [1.; 3],
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
        draw.sort_by(|a, b| {
            let key = |d: &Draw| d.p.x + d.p.y + d.p.z + d.size.iter().sum::<f32>();
            key(a).total_cmp(&key(b))
        });
        for d in draw {
            self.terrain_texture = d.size[0] > 1.;
            self.cube(&d);
        }
        self.terrain_texture = false;
        let mut x0 = self.width;
        let mut y0 = self.height;
        let mut x1 = 0;
        let mut y1 = 0;
        for (i, g) in self.glow.iter().enumerate() {
            if g[0] > 0. {
                x0 = x0.min(i % self.width);
                y0 = y0.min(i / self.width);
                x1 = x1.max(i % self.width + 1);
                y1 = y1.max(i / self.width + 1);
            }
        }
        if x1 > x0 && y1 > y0 {
            x0 = x0.saturating_sub(10);
            y0 = y0.saturating_sub(10);
            x1 = (x1 + 10).min(self.width);
            y1 = (y1 + 10).min(self.height);
            let (w, h) = (x1 - x0, y1 - y0);
            let mut glow = vec![[0.; 3]; w * h];
            let mut scratch = vec![[0.; 3]; w * h];
            for y in 0..h {
                glow[y * w..(y + 1) * w].copy_from_slice(
                    &self.glow[(y + y0) * self.width + x0..(y + y0) * self.width + x1],
                );
            }
            for _ in 0..2 {
                crate::render::blur(&mut glow, &mut scratch, w, h, 4);
            }
            for y in 0..h {
                for x in 0..w {
                    let i = (y + y0) * self.width + x + x0;
                    for k in 0..3 {
                        let halo = (glow[y * w + x][k] * 0.8).min([24., 13., 4.][k]);
                        self.pixels[i * 4 + k] =
                            (self.pixels[i * 4 + k] as f32 + halo).min(255.) as u8;
                    }
                }
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
