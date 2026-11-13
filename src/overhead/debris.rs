//! Shared 3D rigid rubble for overhead worlds. Material cells keep using the
//! existing thermal solver; Rapier 3D handles free XYZ motion, spin and contacts.
use super::{Explosion, Point};
use crate::{
    MatId, Materials, Sim, State, World,
    material::AIR,
    physics::{BodyCell, CELL},
    world::OWNED,
};
use rapier3d::prelude::*;
use std::collections::{BTreeMap, HashMap};
const TILE: usize = 8;
#[derive(Clone, Copy, Debug)]
pub struct Voxel {
    pub offset: Point,
    pub cell: BodyCell,
}
/// Configurable physical crack spacing, not visual particle scaling. Longer X
/// spacing yields board-like pieces; jitter mixes large fragments and splinters.
#[derive(Clone, Copy, Debug)]
pub struct FractureSettings {
    pub spacing: Point,
    pub jitter: f32,
    pub seed: u32,
}
impl Default for FractureSettings {
    fn default() -> Self {
        Self {
            spacing: Point::new(7., 3.5, 7.),
            jitter: 0.85,
            seed: 73,
        }
    }
}
impl FractureSettings {
    fn region(&self, p: Point, material: MatId, frame: u64) -> (MatId, i32, i32, i32) {
        let safe = |x: f32, d: f32| if x.is_finite() { x.clamp(2., 16.) } else { d };
        let scale = [
            safe(self.spacing.x, 7.),
            safe(self.spacing.y, 3.5),
            safe(self.spacing.z, 7.),
        ];
        let q = [p.x / scale[0], p.y / scale[1], p.z / scale[2]];
        let g = q.map(|v| v.floor() as i32);
        let jitter = if self.jitter.is_finite() {
            self.jitter.clamp(0., 1.)
        } else {
            0.85
        };
        let mut best = (f32::INFINITY, g[0], g[1], g[2]);
        for z in g[2] - 1..=g[2] + 1 {
            for y in g[1] - 1..=g[1] + 1 {
                for x in g[0] - 1..=g[0] + 1 {
                    let seed = (x as u32).wrapping_mul(73856093)
                        ^ (y as u32).wrapping_mul(19349663)
                        ^ (z as u32).wrapping_mul(83492791)
                        ^ self.seed
                        ^ (frame as u32).wrapping_mul(7919)
                        ^ (material as u32).wrapping_mul(104729);
                    let random = |n: u32| {
                        let mut h = seed ^ n;
                        h ^= h >> 16;
                        h = h.wrapping_mul(0x7feb352d);
                        h ^= h >> 15;
                        h = h.wrapping_mul(0x846ca68b);
                        h ^= h >> 16;
                        h as f32 / u32::MAX as f32
                    };
                    let c = [
                        x as f32 + 0.5 + (random(11) - 0.5) * jitter,
                        y as f32 + 0.5 + (random(37) - 0.5) * jitter,
                        z as f32 + 0.5 + (random(83) - 0.5) * jitter,
                    ];
                    let weight = 1. + (random(131) - 0.5) * jitter;
                    let d = ((q[0] - c[0]).powi(2) + (q[1] - c[1]).powi(2) + (q[2] - c[2]).powi(2))
                        / weight;
                    if d < best.0 {
                        best = (d, x, y, z);
                    }
                }
            }
        }
        (material, best.1, best.2, best.3)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct FragmentState {
    pub id: u64,
    pub at: Point,
    pub velocity: Point,
    pub spin: Point,
    pub cells: usize,
    pub extent: Point,
    pub mass: f32,
    pub sleeping: bool,
}
struct Fragment {
    id: u64,
    born: u64,
    body: RigidBodyHandle,
    collider: ColliderHandle,
    cells: Vec<Option<Voxel>>,
    represented: Vec<bool>,
    dirty: bool,
}
struct Stamp {
    fragment: usize,
    cell: usize,
    section: usize,
    index: usize,
}
struct Terrain {
    hash: u64,
    handles: Vec<ColliderHandle>,
}
/// All games can use this solver through `Volume::debris`. Velocities are in
/// cells/second, spin in radians/second, masses in kg; world gravity is -Z.
pub struct Debris {
    pub fracture: FractureSettings,
    bodies: RigidBodySet,
    colliders: ColliderSet,
    islands: IslandManager,
    broad: BroadPhaseBvh,
    narrow: NarrowPhase,
    pipeline: PhysicsPipeline,
    joints: ImpulseJointSet,
    multibody: MultibodyJointSet,
    soft: SoftBodySet,
    ccd: CCDSolver,
    params: IntegrationParameters,
    fragments: Vec<Fragment>,
    stamps: Vec<Stamp>,
    owners: Vec<bool>,
    terrain: Vec<Terrain>,
    next_id: u64,
    tick: u64,
    thermal: Option<World>,
    width: usize,
    depth: usize,
    height: usize,
}
// Fresh pieces share a cut surface. Briefly suppress contacts only among
// pieces released together, so numerical corner contacts cannot weld them
// back into their original jigsaw. Terrain and older rubble still collide.
struct ReleaseContacts {
    born: HashMap<RigidBodyHandle, u64>,
    tick: u64,
}
impl PhysicsHooks for ReleaseContacts {
    fn filter_contact_pair(&self, c: &PairFilterContext) -> Option<SolverFlags> {
        if let (Some(a), Some(b)) = (c.rigid_body1, c.rigid_body2) {
            if let (Some(&x), Some(&y)) = (self.born.get(&a), self.born.get(&b)) {
                if x == y && self.tick.saturating_sub(x) < 4 {
                    return None;
                }
            }
        }
        Some(SolverFlags::COMPUTE_RIGID_IMPULSES)
    }
}
fn vector(p: Point) -> Vector {
    Vector::new(p.x, p.y, p.z)
}
fn point(v: Vector) -> Point {
    Point::new(v.x, v.y, v.z)
}
fn material_cell(w: &World, i: usize) -> BodyCell {
    BodyCell {
        mat: w.mat[i],
        temp: w.temp[i],
        burn: w.burn[i],
        life: w.life[i],
        flags: w.flags[i] & !OWNED,
        shade: w.shade[i],
        paint: w.paint[i],
    }
}
impl Debris {
    pub fn new(width: usize, depth: usize, height: usize) -> Self {
        let mut colliders = ColliderSet::new();
        // A catch floor beyond the map lets off-map rubble settle instead of
        // falling forever. The visible terrain still supplies its real height.
        colliders.insert(
            ColliderBuilder::cuboid(width as f32 * CELL + 30., depth as f32 * CELL + 30., 0.1)
                .translation(Vector::new(
                    width as f32 * CELL * 0.5,
                    depth as f32 * CELL * 0.5,
                    -0.1,
                ))
                .friction(0.8),
        );
        Self {
            fracture: FractureSettings::default(),
            bodies: RigidBodySet::new(),
            colliders,
            islands: IslandManager::new(),
            broad: BroadPhaseBvh::new(),
            narrow: NarrowPhase::new(),
            pipeline: PhysicsPipeline::new(),
            joints: ImpulseJointSet::new(),
            multibody: MultibodyJointSet::new(),
            soft: SoftBodySet::new(),
            ccd: CCDSolver::new(),
            params: IntegrationParameters {
                dt: 1. / 60.,
                ..Default::default()
            },
            fragments: vec![],
            stamps: vec![],
            owners: vec![false; width * depth * height],
            terrain: vec![],
            next_id: 1,
            tick: 0,
            thermal: None,
            width,
            depth,
            height,
        }
    }
    pub fn len(&self) -> usize {
        self.fragments.len()
    }
    pub fn is_empty(&self) -> bool {
        self.fragments.is_empty()
    }
    pub fn is_stamped(&self, section: usize, index: usize) -> bool {
        self.owners[section * self.width * self.height + index]
    }
    pub fn states(&self) -> Vec<FragmentState> {
        self.fragments
            .iter()
            .map(|f| {
                let b = &self.bodies[f.body];
                FragmentState {
                    id: f.id,
                    at: point(b.translation() / CELL),
                    velocity: point(b.linvel() / CELL),
                    spin: point(b.angvel()),
                    cells: f.cells.iter().flatten().count(),
                    extent: fragment_extent(&f.cells),
                    mass: b.mass(),
                    sleeping: b.is_sleeping(),
                }
            })
            .collect()
    }
    pub fn voxels(&self) -> impl Iterator<Item = (Point, BodyCell)> + '_ {
        self.fragments.iter().flat_map(|f| {
            let pose = *self.bodies[f.body].position();
            f.cells.iter().flatten().map(move |v| {
                (
                    point(pose.transform_point(vector(v.offset) * CELL) / CELL),
                    v.cell,
                )
            })
        })
    }
    pub fn count_unrepresented(&self, id: MatId) -> usize {
        self.fragments
            .iter()
            .map(|f| {
                f.cells
                    .iter()
                    .zip(&f.represented)
                    .filter(|(c, shown)| !**shown && c.is_some_and(|v| v.cell.mat == id))
                    .count()
            })
            .sum()
    }
    /// Spawn an arbitrary material fragment from world-space cell centres.
    /// Its shape, mass, paint and temperature are retained, not replaced by FX.
    pub fn spawn(
        &mut self,
        cells: Vec<(Point, BodyCell)>,
        velocity: Point,
        spin: Point,
        m: &Materials,
    ) -> Option<u64> {
        if ![velocity.x, velocity.y, velocity.z, spin.x, spin.y, spin.z]
            .iter()
            .all(|v| v.is_finite())
        {
            return None;
        }
        let cells: Vec<_> = cells
            .into_iter()
            .filter(|(p, c)| {
                p.x.is_finite()
                    && p.y.is_finite()
                    && p.z.is_finite()
                    && m.get(c.mat).state == State::Solid
            })
            .collect();
        if cells.is_empty() {
            return None;
        }
        let n = cells.len() as f32;
        let centre = cells.iter().fold(Point::default(), |s, (p, _)| {
            Point::new(s.x + p.x / n, s.y + p.y / n, s.z + p.z / n)
        });
        let voxels: Vec<_> = cells
            .into_iter()
            .map(|(p, cell)| {
                Some(Voxel {
                    offset: Point::new(p.x - centre.x, p.y - centre.y, p.z - centre.z),
                    cell,
                })
            })
            .collect();
        let (shape, mass, friction, bounce) = shape(&voxels, m)?;
        let body = self.bodies.insert(
            RigidBodyBuilder::dynamic()
                .translation(vector(centre) * CELL)
                .linvel(vector(velocity) * CELL)
                .angvel(vector(spin))
                .linear_damping(0.16)
                .angular_damping(0.45)
                .ccd_enabled(true),
        );
        let collider = self.colliders.insert_with_parent(
            ColliderBuilder::new(shape)
                .mass(mass)
                .friction(friction)
                .restitution(bounce)
                .active_hooks(ActiveHooks::FILTER_CONTACT_PAIRS),
            body,
            &mut self.bodies,
        );
        let id = self.next_id;
        self.next_id += 1;
        let count = voxels.len();
        self.fragments.push(Fragment {
            id,
            born: self.tick,
            body,
            collider,
            cells: voxels,
            represented: vec![false; count],
            dirty: false,
        });
        Some(id)
    }
    pub fn heat(&mut self, at: Point, radius: f32, temp: f32) {
        for f in &mut self.fragments {
            let pose = *self.bodies[f.body].position();
            for c in f.cells.iter_mut().flatten() {
                if point(pose.transform_point(vector(c.offset) * CELL) / CELL).distance(at)
                    <= radius
                {
                    c.cell.temp = c.cell.temp.max(temp);
                }
            }
        }
    }
    /// Read the native material step back, then remove only our own stamps.
    /// Melting/ash/gas stays in the native world and leaves the rigid fragment.
    pub(crate) fn read_back(&mut self, sections: &mut [Sim], m: &Materials) -> Vec<Explosion> {
        for s in self.stamps.drain(..) {
            let w = &mut sections[s.section].world;
            let f = &mut self.fragments[s.fragment];
            if let Some(v) = &mut f.cells[s.cell] {
                if m.get(w.mat[s.index]).state == State::Solid {
                    let next = material_cell(w, s.index);
                    if next.mat != v.cell.mat {
                        f.dirty = true;
                    }
                    v.cell = next;
                    w.make_air(s.index);
                } else {
                    f.cells[s.cell] = None;
                    f.dirty = true;
                }
            }
        }
        self.owners.fill(false);
        // Cells above/outside the bounded section grids still burn/cool using
        // the same World solver, in isolated air patches. They never freeze
        // their burn timers merely because a blast threw them above the roof.
        let mut missing = Vec::new();
        for (fi, f) in self.fragments.iter().enumerate() {
            for (ci, c) in f.cells.iter().enumerate() {
                if c.is_some() && !f.represented[ci] {
                    missing.push((fi, ci));
                }
            }
        }
        let mut events = Vec::new();
        if !missing.is_empty() {
            let h = missing.len().div_ceil(16) * 3 + 2;
            let w = self.thermal.get_or_insert_with(|| World::new(50, h));
            if w.h < h {
                *w = World::new(50, h);
            }
            w.fill(m, AIR);
            w.depth_air.fill(true);
            for (k, (fi, ci)) in missing.iter().enumerate() {
                let x = 1 + k % 16 * 3;
                let y = 1 + k / 16 * 3;
                let i = y * w.w + x;
                let c = self.fragments[*fi].cells[*ci].unwrap().cell;
                w.mat[i] = c.mat;
                w.temp[i] = c.temp;
                w.burn[i] = c.burn;
                w.life[i] = c.life;
                w.flags[i] = c.flags | OWNED;
                w.paint[i] = c.paint;
                w.shade[i] = c.shade;
            }
            w.wake_all();
            w.step(m);
            for (k, (fi, ci)) in missing.iter().enumerate() {
                let i = (1 + k / 16 * 3) * w.w + 1 + k % 16 * 3;
                let f = &mut self.fragments[*fi];
                if m.get(w.mat[i]).state == State::Solid {
                    f.cells[*ci].as_mut().unwrap().cell = material_cell(w, i);
                } else {
                    f.cells[*ci] = None;
                    f.dirty = true;
                }
            }
            for e in &w.blasts {
                let k = (e.y as usize / 3) * 16 + e.x as usize / 3;
                if let Some((fi, ci)) = missing.get(k) {
                    if let Some(c) = self.fragments[*fi].cells[*ci] {
                        events.push(Explosion {
                            at: point(
                                self.bodies[self.fragments[*fi].body]
                                    .position()
                                    .transform_point(vector(c.offset) * CELL)
                                    / CELL,
                            ),
                            radius: e.size,
                            power: e.power,
                            fire: e.fire,
                        });
                    }
                }
            }
        }
        for f in &mut self.fragments {
            f.represented.fill(false);
        }
        let mut removed = Vec::new();
        for (i, f) in self.fragments.iter_mut().enumerate() {
            if !f.dirty {
                continue;
            }
            if let Some((s, mass, friction, bounce)) = shape(&f.cells, m) {
                let c = &mut self.colliders[f.collider];
                c.set_shape(s);
                c.set_mass(mass);
                c.set_friction(friction);
                c.set_restitution(bounce);
                self.bodies[f.body].wake_up(true);
                f.dirty = false;
            } else {
                removed.push(i);
            }
        }
        for i in removed.into_iter().rev() {
            let f = self.fragments.remove(i);
            self.bodies.remove(
                f.body,
                &mut self.islands,
                &mut self.colliders,
                &mut self.joints,
                &mut self.multibody,
                &mut self.soft,
                true,
            );
        }
        events
    }
    /// Lift every detached native section body into shared XYZ space. New
    /// pieces are grouped in small connected 3D chunks, never a whole facade.
    pub(crate) fn promote(&mut self, sections: &mut [Sim], m: &Materials, events: &[Explosion]) {
        let mut bins = BTreeMap::<(MatId, i32, i32, i32), Vec<(Point, BodyCell, Point)>>::new();

        for (y, s) in sections.iter_mut().enumerate() {
            for id in s.physics.ids() {
                let Some(b) = s.physics.remove(&mut s.world, id) else {
                    continue;
                };
                let (sin, cos) = b.angle.sin_cos();
                for (i, c) in b.cells.into_iter().enumerate() {
                    let Some(c) = c else {
                        continue;
                    };
                    let (x, z) = (i % b.w, i / b.w);
                    let lx = x as f32 + 0.5 - b.w as f32 * 0.5;
                    let ly = z as f32 + 0.5 - b.h as f32 * 0.5;
                    let p = Point::new(
                        b.x + cos * lx - sin * ly,
                        y as f32 + 0.5,
                        self.height as f32 - (b.y + sin * lx + cos * ly),
                    );
                    bins.entry(self.fracture.region(p, c.mat, self.tick))
                        .or_default()
                        .push((p, c, Point::new(b.vx, 0., -b.vy)));
                }
            }
        }
        let mut groups = Vec::new();
        for (_, cells) in bins {
            let mut seen = vec![false; cells.len()];
            for first in 0..cells.len() {
                if seen[first] {
                    continue;
                }
                seen[first] = true;
                let mut queue = vec![first];
                let mut group = Vec::new();
                while let Some(i) = queue.pop() {
                    group.push(cells[i]);
                    for j in 0..cells.len() {
                        if !seen[j] && cells[i].0.distance(cells[j].0) <= 1.1 {
                            seen[j] = true;
                            queue.push(j);
                        }
                    }
                }
                groups.push(group);
            }
        }
        for cells in groups {
            let n = cells.len() as f32;
            let velocity = cells.iter().fold(Point::default(), |v, c| {
                Point::new(v.x + c.2.x / n, v.y, v.z + c.2.z / n)
            });
            let centre = cells[0].0;
            let nearby = events.iter().any(|e| centre.distance(e.at) < e.radius + 5.);
            let velocity = if nearby {
                Point::new(velocity.x * 0.15, 0., velocity.z * 0.15)
            } else {
                velocity
            };
            self.spawn(
                cells.into_iter().map(|(p, c, _)| (p, c)).collect(),
                velocity,
                Point::default(),
                m,
            );
        }
        // Both new and existing rubble receive the same full 3D blast impulse.
        for e in events {
            self.blast(*e);
        }
    }
    pub fn blast(&mut self, e: Explosion) {
        if ![e.at.x, e.at.y, e.at.z, e.radius, e.power]
            .iter()
            .all(|v| v.is_finite())
            || e.radius <= 0.
            || e.power <= 0.
        {
            return;
        }
        for f in &self.fragments {
            let b = &mut self.bodies[f.body];
            let centre = point(b.translation() / CELL);
            let d = centre.distance(e.at);
            let reach = e.radius + 3.;
            if d > reach {
                continue;
            }
            let dx = centre.x - e.at.x;
            let dy = centre.y - e.at.y;
            let dz = centre.z - e.at.z;
            let l = dx.hypot(dy).hypot(dz).max(0.5);
            let density =
                (b.mass() / (f.cells.iter().flatten().count() as f32 * CELL.powi(3))).max(100.);
            let speed = (100.
                * (e.power / 10000.).sqrt()
                * (700. / density).sqrt()
                * (1. - d / reach).max(0.1))
            .min(180.);
            let mut direction = Vector::new(dx / l, dy / l, dz / l + 0.2);
            // Pressure also pushes a thin panel out of its own plane. Purely
            // tangential impulses wedge tightly packed roof boards against
            // their neighbors instead of opening the blast hole.
            let mut lo = [f32::INFINITY; 3];
            let mut hi = [f32::NEG_INFINITY; 3];
            for v in f.cells.iter().flatten() {
                let p = [v.offset.x, v.offset.y, v.offset.z];
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            let span = [hi[0] - lo[0] + 1., hi[1] - lo[1] + 1., hi[2] - lo[2] + 1.];
            let axis = (0..3).min_by(|a, b| span[*a].total_cmp(&span[*b])).unwrap();
            if span[axis] < 3.5 && span.iter().copied().fold(0., f32::max) > 3.5 {
                let delta = [dx, dy, dz][axis];
                let sign = if axis == 2 && delta.abs() < 0.75 {
                    1.
                } else if delta < 0. {
                    -1.
                } else {
                    1.
                };
                let bias = sign * 0.9;
                match axis {
                    0 => direction.x += bias,
                    1 => direction.y += bias,
                    _ => direction.z += bias,
                }
            }
            b.apply_impulse(direction * (speed * CELL * b.mass()), true);
            let sign = if f.id % 2 == 0 { 1. } else { -1. };
            b.set_angvel(
                b.angvel()
                    + Vector::new(
                        sign * speed * 0.025,
                        sign * speed * 0.018,
                        -sign * speed * 0.035,
                    ),
                true,
            );
        }
    }
    /// Greedy solid cuboids, rebuilt only for changed 8³ terrain chunks.
    fn sync_terrain(&mut self, sections: &[Sim], m: &Materials) {
        let (nx, ny, nz) = (
            self.width.div_ceil(TILE),
            self.depth.div_ceil(TILE),
            self.height.div_ceil(TILE),
        );
        let count = nx * ny * nz;
        if self.terrain.len() != count {
            self.terrain = (0..count)
                .map(|_| Terrain {
                    hash: 0,
                    handles: vec![],
                })
                .collect();
        }
        for cz in 0..nz {
            for cy in 0..ny {
                for cx in 0..nx {
                    let mut grid = [AIR; TILE * TILE * TILE];
                    let mut hash = 14695981039346656037u64;
                    for z in 0..TILE {
                        for y in 0..TILE {
                            for x in 0..TILE {
                                let (wx, wy, wz) = (cx * TILE + x, cy * TILE + y, cz * TILE + z);
                                if wx < self.width && wy < self.depth && wz < self.height {
                                    let w = &sections[wy].world;
                                    let i = (self.height - 1 - wz) * self.width + wx;
                                    let id = w.mat[i];
                                    if !w.is_owned(i) && m.get(id).state == State::Solid {
                                        grid[(z * TILE + y) * TILE + x] = id;
                                    }
                                }
                                hash = (hash ^ grid[(z * TILE + y) * TILE + x] as u64)
                                    .wrapping_mul(1099511628211);
                            }
                        }
                    }
                    let idx = (cz * ny + cy) * nx + cx;
                    if self.terrain[idx].hash == hash {
                        continue;
                    }
                    for h in self.terrain[idx].handles.drain(..) {
                        self.colliders.remove(
                            h,
                            &mut self.islands,
                            &mut self.bodies,
                            &mut self.soft,
                            true,
                        );
                    }
                    self.terrain[idx].hash = hash;
                    for z in 0..TILE {
                        for y in 0..TILE {
                            for x in 0..TILE {
                                let i = (z * TILE + y) * TILE + x;
                                let id = grid[i];
                                if id == AIR {
                                    continue;
                                }
                                let mut sx = 1;
                                while x + sx < TILE && grid[i + sx] == id {
                                    sx += 1;
                                }
                                let mut sy = 1;
                                'dy: while y + sy < TILE {
                                    for xx in x..x + sx {
                                        if grid[(z * TILE + y + sy) * TILE + xx] != id {
                                            break 'dy;
                                        }
                                    }
                                    sy += 1;
                                }
                                let mut sz = 1;
                                'dz: while z + sz < TILE {
                                    for yy in y..y + sy {
                                        for xx in x..x + sx {
                                            if grid[((z + sz) * TILE + yy) * TILE + xx] != id {
                                                break 'dz;
                                            }
                                        }
                                    }
                                    sz += 1;
                                }
                                for zz in z..z + sz {
                                    for yy in y..y + sy {
                                        for xx in x..x + sx {
                                            grid[(zz * TILE + yy) * TILE + xx] = AIR;
                                        }
                                    }
                                }
                                let mat = m.get(id);
                                let h = self.colliders.insert(
                                    ColliderBuilder::cuboid(
                                        sx as f32 * CELL * 0.5,
                                        sy as f32 * CELL * 0.5,
                                        sz as f32 * CELL * 0.5,
                                    )
                                    .translation(
                                        Vector::new(
                                            (cx * TILE + x) as f32 + sx as f32 * 0.5,
                                            (cy * TILE + y) as f32 + sy as f32 * 0.5,
                                            (cz * TILE + z) as f32 + sz as f32 * 0.5,
                                        ) * CELL,
                                    )
                                    .friction(mat.friction)
                                    .restitution(mat.bounciness),
                                );
                                self.terrain[idx].handles.push(h);
                            }
                        }
                    }
                }
            }
        }
    }
    pub(crate) fn step(&mut self, sections: &mut [Sim], m: &Materials) {
        self.sync_terrain(sections, m);
        let hooks = ReleaseContacts {
            born: self.fragments.iter().map(|f| (f.body, f.born)).collect(),
            tick: self.tick,
        };
        self.tick = self.tick.wrapping_add(1);
        if !self.fragments.is_empty() {
            self.pipeline.step(
                Vector::new(0., 0., -9.81),
                &self.params,
                &mut self.islands,
                &mut self.broad,
                &mut self.narrow,
                &mut self.bodies,
                &mut self.colliders,
                &mut self.joints,
                &mut self.multibody,
                &mut self.soft,
                &mut self.ccd,
                &hooks,
                &(),
            );
        }
        for (fi, f) in self.fragments.iter_mut().enumerate() {
            let pose = *self.bodies[f.body].position();
            for (ci, c) in f.cells.iter().enumerate() {
                let Some(c) = c else {
                    continue;
                };
                let p = point(pose.transform_point(vector(c.offset) * CELL) / CELL);
                if p.x < 0.
                    || p.y < 0.
                    || p.z < 0.
                    || p.x >= self.width as f32
                    || p.y >= self.depth as f32
                    || p.z >= self.height as f32
                {
                    continue;
                }
                let y = p.y as usize;
                let i = (self.height - 1 - p.z as usize) * self.width + p.x as usize;
                let owner = y * self.width * self.height + i;
                let w = &mut sections[y].world;
                if self.owners[owner] || m.get(w.mat[i]).state != State::Gas {
                    continue;
                }
                w.mat[i] = c.cell.mat;
                w.temp[i] = c.cell.temp;
                w.burn[i] = c.cell.burn;
                w.life[i] = c.cell.life;
                w.flags[i] = c.cell.flags | OWNED;
                w.shade[i] = c.cell.shade;
                w.paint[i] = c.cell.paint;
                w.wake_at(i);
                f.represented[ci] = true;
                self.owners[owner] = true;
                self.stamps.push(Stamp {
                    fragment: fi,
                    cell: ci,
                    section: y,
                    index: i,
                });
            }
        }
    }
}
fn shape(cells: &[Option<Voxel>], m: &Materials) -> Option<(SharedShape, f32, f32, f32)> {
    let first = cells.iter().flatten().next()?.offset;
    let mut grid = BTreeMap::new();
    let mut aligned = true;
    let (mut mass, mut friction, mut bounce) = (0., 0., 0.);
    for c in cells.iter().flatten() {
        let mat = m.get(c.cell.mat);
        let kg = mat.density.max(1.) * CELL.powi(3);
        mass += kg;
        friction += kg * mat.friction;
        bounce += kg * mat.bounciness;
        let d = [
            c.offset.x - first.x,
            c.offset.y - first.y,
            c.offset.z - first.z,
        ];
        let k = (
            d[0].round() as i32,
            d[1].round() as i32,
            d[2].round() as i32,
        );
        aligned &= (d[0] - k.0 as f32).abs() < 0.02
            && (d[1] - k.1 as f32).abs() < 0.02
            && (d[2] - k.2 as f32).abs() < 0.02;
        grid.insert(k, c.cell.mat);
    }
    let mut shapes = Vec::new();
    if aligned {
        // Greedy voxel boxes have flat contact faces. One cube per voxel
        // produces internal edges that can lock neighboring roof fragments.
        while let Some((&(x, y, z), &id)) = grid.first_key_value() {
            let mut sx = 1;
            while grid.get(&(x + sx, y, z)) == Some(&id) {
                sx += 1;
            }
            let mut sy = 1;
            'dy: loop {
                for xx in x..x + sx {
                    if grid.get(&(xx, y + sy, z)) != Some(&id) {
                        break 'dy;
                    }
                }
                sy += 1;
            }
            let mut sz = 1;
            'dz: loop {
                for yy in y..y + sy {
                    for xx in x..x + sx {
                        if grid.get(&(xx, yy, z + sz)) != Some(&id) {
                            break 'dz;
                        }
                    }
                }
                sz += 1;
            }
            for zz in z..z + sz {
                for yy in y..y + sy {
                    for xx in x..x + sx {
                        grid.remove(&(xx, yy, zz));
                    }
                }
            }
            let centre = Vector::new(
                first.x + x as f32 + (sx - 1) as f32 * 0.5,
                first.y + y as f32 + (sy - 1) as f32 * 0.5,
                first.z + z as f32 + (sz - 1) as f32 * 0.5,
            ) * CELL;
            shapes.push((
                Pose::from_translation(centre),
                SharedShape::cuboid(
                    sx as f32 * CELL * 0.5 - 0.001,
                    sy as f32 * CELL * 0.5 - 0.001,
                    sz as f32 * CELL * 0.5 - 0.001,
                ),
            ));
        }
    } else {
        for c in cells.iter().flatten() {
            shapes.push((
                Pose::from_translation(vector(c.offset) * CELL),
                SharedShape::cuboid(CELL * 0.49, CELL * 0.49, CELL * 0.49),
            ));
        }
    }
    Some((
        SharedShape::compound(shapes),
        mass,
        friction / mass,
        bounce / mass,
    ))
}

fn fragment_extent(cells: &[Option<Voxel>]) -> Point {
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for c in cells.iter().flatten() {
        let p = [c.offset.x, c.offset.y, c.offset.z];
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    Point::new(hi[0] - lo[0] + 1., hi[1] - lo[1] + 1., hi[2] - lo[2] + 1.)
}
