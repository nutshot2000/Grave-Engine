//! Reusable sling loads: capture material cells, carry a suspended container,
//! release it ballistically, and deposit or rupture its real material contents.
use super::{Point, space::Space};
use crate::{Materials, State, World, material::AIR, physics::BodyCell, world::OWNED};
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CargoState {
    Ground,
    Slung,
    Falling,
    Deployed,
}
pub struct Cargo {
    pub id: u32,
    pub at: Point,
    pub velocity: Point,
    pub size: [usize; 3],
    pub cells: Vec<(Point, BodyCell)>,
    pub state: CargoState,
    pub rupture_speed: f32,
    thermal: Vec<World>,
}
impl Cargo {
    pub fn capture(id: u32, v: &mut impl Space, origin: [usize; 3], size: [usize; 3]) -> Self {
        let limits = v.extent();
        let origin = std::array::from_fn::<_, 3, _>(|i| origin[i].min(limits[i]));
        let size = std::array::from_fn::<_, 3, _>(|i| size[i].min(limits[i] - origin[i]));
        let mut cells = Vec::new();
        for y in 0..size[1] {
            for z in 0..size[2] {
                for x in 0..size[0] {
                    let p = Point::new(
                        (origin[0] + x) as f32,
                        (origin[1] + y) as f32,
                        (origin[2] + z) as f32,
                    );
                    if let Some(c) = v.take_cell(p).filter(|c| c.mat != AIR) {
                        cells.push((Point::new(x as f32, y as f32, z as f32), c));
                    }
                }
            }
        }
        Self {
            id,
            at: Point::new(origin[0] as f32, origin[1] as f32, origin[2] as f32),
            velocity: Point::default(),
            size,
            cells,
            state: CargoState::Slung,
            rupture_speed: 20.,
            thermal: (0..size[1])
                .map(|_| World::new(size[0] + 2, size[2] + 2))
                .collect(),
        }
    }
    pub fn centre(&self) -> Point {
        Point::new(
            self.at.x + self.size[0] as f32 * 0.5,
            self.at.y + self.size[1] as f32 * 0.5,
            self.at.z + self.size[2] as f32 * 0.5,
        )
    }
    pub fn release(&mut self, velocity: Point) {
        self.state = CargoState::Falling;
        self.velocity = velocity;
    }
    pub fn step(&mut self, v: &mut impl Space, m: &Materials, anchor: Option<Point>, dt: f32) {
        if !dt.is_finite() || dt <= 0. {
            return;
        }
        let dt = dt.min(0.1);
        if matches!(self.state, CargoState::Slung | CargoState::Falling) {
            self.material_step(v, m);
        }
        match self.state {
            CargoState::Slung => {
                if let Some(p) = anchor {
                    let mut desired = Point::new(
                        p.x - self.size[0] as f32 * 0.5,
                        p.y - self.size[1] as f32 * 0.5,
                        p.z - 22.,
                    );
                    // Shorten the cable near terrain instead of dragging a load through it.
                    let centre_x = desired.x + self.size[0] as f32 * 0.5;
                    let centre_y = desired.y + self.size[1] as f32 * 0.5;
                    for z in (0..p.z.max(0.) as usize).rev() {
                        if v.solid_at(Point::new(centre_x, centre_y, z as f32), m) {
                            desired.z = desired.z.max(z as f32 + 1.);
                            break;
                        }
                    }
                    let old = self.at;
                    self.at = self.at.lerp(desired, (dt * 8.).min(1.));
                    self.velocity = Point::new(
                        (self.at.x - old.x) / dt,
                        (self.at.y - old.y) / dt,
                        (self.at.z - old.z) / dt,
                    );
                }
            }
            CargoState::Falling => {
                self.velocity.z -= 98.1 * dt;
                let end = Point::new(
                    self.at.x + self.velocity.x * dt,
                    self.at.y + self.velocity.y * dt,
                    self.at.z + self.velocity.z * dt,
                );
                let bottom = Point::new(
                    self.at.x + self.size[0] as f32 * 0.5,
                    self.at.y + self.size[1] as f32 * 0.5,
                    self.at.z,
                );
                let next = Point::new(
                    end.x + self.size[0] as f32 * 0.5,
                    end.y + self.size[1] as f32 * 0.5,
                    end.z,
                );
                if let Some(hit) = v.raycast(bottom, next, m) {
                    self.at = Point::new(
                        hit.at.x - self.size[0] as f32 * 0.5,
                        hit.at.y - self.size[1] as f32 * 0.5,
                        hit.at.z + 1.,
                    );
                    let ruptured = self.velocity.z.abs() > self.rupture_speed;
                    self.deposit(v, m, ruptured);
                    self.state = CargoState::Deployed;
                } else {
                    self.at = end;
                    if self.at.z < 0. {
                        self.at.z = 2.;
                        self.deposit(v, m, true);
                        self.state = CargoState::Deployed;
                    }
                }
            }
            _ => {}
        }
    }
    /// Carried materials retain native heat, ignition and decay rules. OWNED
    /// keeps liquid in its suspended case; phase changes leave it into the world.
    fn material_step(&mut self, v: &mut impl Space, m: &Materials) {
        for w in &mut self.thermal {
            w.fill(m, AIR);
            w.depth_air.fill(true);
        }
        for (offset, c) in &self.cells {
            let y = offset.y as usize;
            let w = &mut self.thermal[y];
            let i = (self.size[2] - offset.z as usize) * w.w + offset.x as usize + 1;
            w.mat[i] = c.mat;
            w.temp[i] = c.temp;
            w.burn[i] = c.burn;
            w.life[i] = c.life;
            w.paint[i] = c.paint;
            w.shade[i] = c.shade;
            w.flags[i] = c.flags | OWNED;
            let at = Point::new(
                self.at.x + offset.x,
                self.at.y + offset.y,
                self.at.z + offset.z,
            );
            if let Some(cell) = v.cell(at) {
                let conduct = m
                    .get(c.mat)
                    .conducts_heat
                    .min(m.get(cell.mat).conducts_heat);
                w.temp[i] += (cell.temp - w.temp[i]) * conduct * 0.12;
            }
        }
        for y in 1..self.thermal.len() {
            let (a, b) = self.thermal.split_at_mut(y);
            crate::layers::exchange_heat(&mut a[y - 1], &mut b[0], m);
        }
        for w in &mut self.thermal {
            w.wake_all();
            w.step(m);
        }
        self.cells.retain_mut(|(offset, c)| {
            let w = &self.thermal[offset.y as usize];
            let i = (self.size[2] - offset.z as usize) * w.w + offset.x as usize + 1;
            let alive = m.get(w.mat[i]).state != State::Gas;
            if alive {
                c.mat = w.mat[i];
                c.temp = w.temp[i];
                c.burn = w.burn[i];
                c.life = w.life[i];
                c.paint = w.paint[i];
                c.shade = w.shade[i];
                c.flags = w.flags[i] & !OWNED;
            }
            alive
        });
        for (y, w) in self.thermal.iter().enumerate() {
            for (i, id) in w.mat.iter().copied().enumerate() {
                if id == AIR || m.get(id).state != State::Gas {
                    continue;
                }
                let at = Point::new(
                    self.at.x + i as f32 % w.w as f32 - 1.,
                    self.at.y + y as f32,
                    self.at.z + self.size[2] as f32 - (i / w.w) as f32,
                );
                if v.contains(at) && m.get(v.material_at(at)).state == State::Gas {
                    v.put_cell(
                        at,
                        Some(BodyCell {
                            mat: id,
                            temp: w.temp[i],
                            life: w.life[i],
                            flags: 0,
                            burn: 0,
                            shade: w.shade[i],
                            paint: 0,
                        }),
                        m,
                    );
                }
            }
            for e in &w.blasts {
                v.explode(super::Explosion {
                    at: Point::new(
                        self.at.x + e.x - 1.,
                        self.at.y + y as f32,
                        self.at.z + self.size[2] as f32 - e.y,
                    ),
                    radius: e.size,
                    power: e.power,
                    fire: e.fire,
                });
            }
        }
    }
    fn deposit(&self, v: &mut impl Space, m: &Materials, ruptured: bool) {
        for (n, (offset, c)) in self.cells.iter().enumerate() {
            let mut p = Point::new(
                self.at.x + offset.x,
                self.at.y + offset.y,
                self.at.z + offset.z,
            );
            if ruptured && m.get(c.mat).state == State::Liquid {
                let angle = n as f32 * 2.39996;
                let radius = 2. + (n % 13) as f32 * 0.42;
                p.x = self.centre().x + angle.cos() * radius;
                p.y = self.centre().y + angle.sin() * radius;
                p.z = self.at.z + 0.5 + (n % 3) as f32 * 0.3;
            }
            if ruptured
                && m.get(c.mat).state == State::Liquid
                && m.get(v.material_at(p)).state != State::Gas
            {
                'search: for radius in 1i32..=7 {
                    for dy in -radius..=radius {
                        for dx in -radius..=radius {
                            let q = Point::new(p.x + dx as f32, p.y + dy as f32, p.z);
                            if v.contains(q) && m.get(v.material_at(q)).state == State::Gas {
                                p = q;
                                break 'search;
                            }
                        }
                    }
                }
            }
            if v.contains(p) && m.get(v.material_at(p)).state == State::Gas {
                v.put_cell(p, Some(*c), m);
            }
        }
    }
}
