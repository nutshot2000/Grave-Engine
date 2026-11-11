//! Squad tactics and a carrier built from separately destructible material bodies.
use super::*;
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum PartRole {
    Generator,
    Gun,
    Cooler,
    Reactor,
}
pub(super) struct BossPart {
    pub id: u64,
    pub role: PartRole,
    pub dead: bool,
    offset: (f32, f32),
    initial: usize,
}
pub(super) struct BossRig {
    root: u64,
    born: u32,
    x: f32,
    y: f32,
    pub parts: Vec<BossPart>,
}
impl Game {
    pub(super) fn spawn_squad(&mut self, leader: Kind, y: f32) {
        let squad = self.next_squad;
        self.next_squad += 1;
        let y = y.clamp(85., H as f32 - 85.);
        let x = W as f32 - 80.;
        let members = if leader == Kind::Interceptor {
            vec![
                (Kind::Interceptor, x - 60., y),
                (Kind::Interceptor, x - 20., y - 45.),
                (Kind::Interceptor, x - 20., y + 45.),
            ]
        } else {
            vec![
                (leader, x, y),
                (Kind::ShieldDrone, x - 52., y - 40.),
                (Kind::ShieldDrone, x - 52., y + 40.),
                (Kind::RepairDrone, x + 48., y),
            ]
        };
        for (k, x, y) in members {
            let id = self.spawn(k, x, y);
            self.ships.iter_mut().find(|s| s.id == id).unwrap().squad = squad;
            self.sim.physics.set_kinematic(id, true, true);
        }
        if leader == Kind::Frigate {
            let id = self.spawn(Kind::Bomber, x - 95., (y + 85.).min(H as f32 - 35.));
            self.ships.iter_mut().find(|s| s.id == id).unwrap().squad = squad;
        }
        self.notices.push(Notice {
            x: W as f32 - 140.,
            y: 45.,
            ttl: 150,
            text: if leader == Kind::Interceptor {
                "HUNTER WING / FLANKING"
            } else if leader == Kind::Frigate {
                "SIEGE GROUP / PRIORITY TARGETS"
            } else {
                "SHIELDED CONVOY / KILL THE ESCORTS"
            }
            .into(),
        });
    }
    pub(super) fn formation_velocity(&self, s: &Ship) -> Option<(f32, f32)> {
        if s.squad == 0 || !matches!(s.kind, Kind::ShieldDrone | Kind::RepairDrone) {
            return None;
        }
        let (x, y) = self.sim.physics.centre(s.id)?;
        let leader = self.ships.iter().find(|l| {
            !l.dead && l.squad == s.squad && matches!(l.kind, Kind::Tanker | Kind::Frigate)
        });
        let (tx, ty) = leader.and_then(|l| self.sim.physics.centre(l.id)).map_or(
            (x - 40., s.base_y),
            |(lx, ly)| {
                if s.kind == Kind::RepairDrone {
                    (lx + 48., ly)
                } else {
                    (lx - 52., ly + if s.base_y < ly { -40. } else { 40. })
                }
            },
        );
        Some(((tx - x) * 3. - 15., (ty - y) * 3.))
    }
    pub(super) fn protected_target(&self, id: u64) -> bool {
        if let Some(b) = &self.boss {
            if let Some(part) = b.parts.iter().find(|p| p.id == id && !p.dead) {
                let shields = b
                    .parts
                    .iter()
                    .any(|p| !p.dead && p.role == PartRole::Generator);
                if part.role == PartRole::Generator {
                    return false;
                }
                if shields {
                    return true;
                }
                if part.role == PartRole::Reactor {
                    return b
                        .parts
                        .iter()
                        .any(|p| !p.dead && matches!(p.role, PartRole::Gun | PartRole::Cooler));
                }
                return false;
            }
            if b.root == id {
                return b
                    .parts
                    .iter()
                    .any(|p| !p.dead && p.role == PartRole::Generator);
            }
        }
        let Some(ship) = self.ships.iter().find(|s| s.id == id && !s.dead) else {
            return false;
        };
        if ship.squad == 0 || ship.kind == Kind::ShieldDrone {
            return false;
        }
        let Some((x, y)) = self.sim.physics.centre(id) else {
            return false;
        };
        self.ships.iter().any(|d| {
            !d.dead
                && d.kind == Kind::ShieldDrone
                && d.squad == ship.squad
                && self
                    .sim
                    .physics
                    .centre(d.id)
                    .is_some_and(|(dx, dy)| (x - dx).hypot(y - dy) < 100.)
        })
    }
    fn part_blueprint(&self, role: PartRole, x: f32, y: f32) -> BodySpec {
        let (w, h) = match role {
            PartRole::Generator => (24, 22),
            PartRole::Gun => (34, 18),
            PartRole::Cooler => (30, 16),
            PartRole::Reactor => (32, 36),
        };
        let mut cells = vec![None; w * h];
        let mut rect = |name: &str, x0: usize, y0: usize, x1: usize, y1: usize| {
            let m = self.mats.id(name).unwrap();
            for yy in y0..y1 {
                for xx in x0..x1 {
                    cells[yy * w + xx] = Some(m);
                }
            }
        };
        rect("steel", 0, 0, w, h);
        rect("aluminium", 2, 2, w - 2, h - 2);
        match role {
            PartRole::Generator => {
                rect("copper", 5, 3, w - 4, h - 3);
                rect("glass", 3, 5, w - 6, h - 5);
                rect("ceramic", 8, 7, w - 8, h - 7);
            }
            PartRole::Gun => {
                rect("iron", 0, 4, w - 3, h - 4);
                rect("copper", 0, 6, w - 7, h - 6);
                rect("plastic", w - 10, 3, w - 3, h - 3);
            }
            PartRole::Cooler => {
                rect("copper", 3, 3, w - 3, h - 3);
                rect("water", 5, 5, w - 5, h - 5);
                rect("glass", 2, 5, 5, h - 5);
            }
            PartRole::Reactor => {
                rect("copper", 4, 4, w - 4, h - 4);
                rect("glass", 2, 8, 12, h - 8);
                rect("gunpowder", 12, 12, 17, 24);
            }
        }
        let mut b = BodySpec::from_mats(w, h, &cells, &self.mats, x, y);
        b.depth = 0.025;
        b
    }
    pub(super) fn assemble_boss(&mut self, root: u64, x: f32, y: f32) {
        self.sim.physics.set_kinematic(root, true, true);
        let layout = [
            (PartRole::Generator, (-55., -65.)),
            (PartRole::Generator, (-55., 65.)),
            (PartRole::Gun, (-65., -30.)),
            (PartRole::Gun, (-65., 30.)),
            (PartRole::Cooler, (-10., -55.)),
            (PartRole::Cooler, (-10., 55.)),
            (PartRole::Reactor, (0., 0.)),
        ];
        let mut parts = vec![];
        for (role, offset) in layout {
            let spec = self.part_blueprint(role, x + offset.0, y + offset.1);
            let initial = spec.cell_count();
            let id = self
                .sim
                .physics
                .add_exact(&mut self.sim.world, &self.mats, spec)
                .unwrap();
            self.sim.physics.set_kinematic(id, true, true);
            self.sim.physics.join(
                &self.mats,
                root,
                id,
                grave_engine::physics::JointKind::Weld,
                (x + offset.0, y + offset.1),
                None,
                None,
            );
            parts.push(BossPart {
                id,
                role,
                dead: false,
                offset,
                initial,
            });
        }
        self.boss = Some(BossRig {
            root,
            born: self.frame,
            x,
            y,
            parts,
        });
    }
    pub(super) fn boss_targets(&self) -> Vec<(f32, f32)> {
        self.boss
            .iter()
            .flat_map(|b| b.parts.iter())
            .filter(|p| !p.dead && !self.protected_target(p.id))
            .filter_map(|p| {
                self.sim.physics.centre(p.id).or_else(|| {
                    if p.role == PartRole::Reactor {
                        self.reactor_position()
                    } else {
                        None
                    }
                })
            })
            .collect()
    }
    pub(super) fn boss_objective(&self) -> &'static str {
        let Some(b) = &self.boss else {
            return "CARRIER APPROACHING";
        };
        if b.parts
            .iter()
            .any(|p| !p.dead && p.role == PartRole::Generator)
        {
            "1 / BREAK THE BLUE SHIELD GENERATORS"
        } else if b.parts.iter().any(|p| !p.dead && p.role == PartRole::Gun) {
            "2 / SHOOT OFF THE GUN PODS"
        } else if b
            .parts
            .iter()
            .any(|p| !p.dead && p.role == PartRole::Cooler)
        {
            "3 / RUPTURE THE COOLING TANKS"
        } else {
            "4 / REACTOR EXPOSED — FINISH IT"
        }
    }
    pub(super) fn boss_defeated(&self) -> bool {
        self.boss
            .as_ref()
            .is_some_and(|b| b.parts.iter().all(|p| p.dead))
    }
    pub(super) fn read_boss_damage(&mut self) {
        let Some(mut b) = self.boss.take() else {
            return;
        };
        let mut broken = vec![];
        let remaining_fuel = self.remaining_reactors();
        for part in &mut b.parts {
            if part.dead {
                continue;
            }
            let alive = self.sim.physics.spec_of(&self.sim.world, part.id);
            let failed = if part.role == PartRole::Reactor {
                remaining_fuel == 0
            } else {
                alive
                    .as_ref()
                    .is_none_or(|s| s.cell_count() < part.initial * 55 / 100)
            };
            if failed {
                part.dead = true;
                self.sim.physics.unjoin(part.id);
                self.sim.physics.set_kinematic(part.id, false, false);
                self.sim
                    .physics
                    .set_velocity(part.id, -20., part.offset.1.signum() * 40., 0.8);
                broken.push((b.x + part.offset.0, b.y + part.offset.1, part.role));
                // Functional destruction also cancels that module's pending weapons.
                self.lasers.retain(|l| l.owner != part.id);
            }
        }
        self.boss = Some(b);
        for (x, y, role) in broken {
            self.burst(x, y, 2, 22);
            self.sounds.push(2);
            self.score += 750;
            self.notices.push(Notice {
                x,
                y,
                ttl: 120,
                text: match role {
                    PartRole::Generator => "SHIELD GENERATOR LOST",
                    PartRole::Gun => "GUN POD DETACHED",
                    PartRole::Cooler => "COOLANT LEAK",
                    PartRole::Reactor => "REACTOR DETONATED",
                }
                .into(),
            });
        }
    }
    pub(super) fn update_encounters(&mut self, px: f32, py: f32) {
        // Support drones move with their own leader, rather than independent waves.
        let drones: Vec<_> = self
            .ships
            .iter()
            .filter(|s| !s.dead && matches!(s.kind, Kind::RepairDrone | Kind::ShieldDrone))
            .filter_map(|s| self.formation_velocity(s).map(|v| (s.id, v)))
            .collect();
        for (id, (vx, vy)) in drones {
            self.sim.physics.set_velocity(id, vx, vy, 0.);
        }
        if self.frame % 120 == 0 {
            let repairs: Vec<_> = self
                .ships
                .iter()
                .filter(|s| !s.dead && s.kind == Kind::RepairDrone)
                .filter_map(|s| {
                    let (rx, ry) = self.sim.physics.centre(s.id)?;
                    self.ships
                        .iter()
                        .filter(|t| {
                            !t.dead
                                && t.id != s.id
                                && t.squad == s.squad
                                && t.squad > 0
                                && t.kind != Kind::ShieldDrone
                        })
                        .filter_map(|t| {
                            let body = self.sim.physics.spec_of(&self.sim.world, t.id)?;
                            if body.cell_count() >= t.cells
                                || (body.x - rx).hypot(body.y - ry) > 120.
                            {
                                return None;
                            }
                            Some((t.id, t.kind, body))
                        })
                        .next()
                })
                .collect();
            for (id, kind, body) in repairs {
                let original = self.blueprint(kind, body.x, body.y);
                if original.w == body.w && original.h == body.h {
                    let fuel = self.mats.id("gunpowder").unwrap();
                    self.sim
                        .physics
                        .edit(&mut self.sim.world, &self.mats, id, |b| {
                            let mut count = 0;
                            for (c, src) in b.cells.iter_mut().zip(original.cells.iter()) {
                                if c.is_none() && src.is_some_and(|c| c.mat != fuel) && count < 18 {
                                    *c = *src;
                                    count += 1;
                                }
                            }
                        });
                    self.notices.push(Notice {
                        x: body.x,
                        y: body.y - 20.,
                        ttl: 55,
                        text: "REPAIR LINK".into(),
                    });
                }
            }
        }
        let mines: Vec<_> = self
            .ships
            .iter()
            .filter(|s| {
                !s.dead
                    && s.kind == Kind::Bomber
                    && (self.frame - s.born) > 60
                    && (self.frame - s.born) % 140 == 70
            })
            .filter_map(|s| self.sim.physics.centre(s.id))
            .collect();
        let mine_count = self
            .ships
            .iter()
            .filter(|s| s.kind == Kind::Mine && !s.dead)
            .count();
        if mine_count < 8 {
            for (x, y) in mines {
                self.spawn(Kind::Mine, (x + 38.).min(W as f32 - 14.), y);
            }
        }
        let mut detonate = vec![];
        for s in &self.ships {
            if s.dead || s.kind != Kind::Mine {
                continue;
            }
            self.sim.physics.set_velocity(s.id, -25., 0., 0.15);
            if self.frame - s.born > 60 {
                if let Some((x, y)) = self.sim.physics.centre(s.id) {
                    if (x - px).hypot(y - py) < 28. {
                        detonate.push((x, y));
                    }
                }
            }
        }
        for (x, y) in detonate {
            self.sim.world.heat_circle(x as i32, y as i32, 8, 1200.);
            self.damage_player(28.);
        }
        let Some(mut b) = self.boss.take() else {
            return;
        };
        let age = self.frame - b.born;
        b.x += (W as f32 - 108. - b.x) * 0.025;
        b.y = H as f32 / 2. + (age as f32 * 0.007).sin() * 18.;
        if let Some((x, y)) = self.sim.physics.centre(b.root) {
            self.sim
                .physics
                .set_velocity(b.root, (b.x - x) * 4., (b.y - y) * 4., 0.);
        }
        let mut fire = vec![];
        let mut beams = vec![];
        let generators = b
            .parts
            .iter()
            .any(|p| !p.dead && p.role == PartRole::Generator);
        let coolers = b
            .parts
            .iter()
            .filter(|p| !p.dead && p.role == PartRole::Cooler)
            .count();
        for part in &b.parts {
            if part.dead {
                continue;
            }
            let Some((x, y)) = self.sim.physics.centre(part.id).or_else(|| {
                if part.role == PartRole::Reactor {
                    self.reactor_position()
                } else {
                    None
                }
            }) else {
                continue;
            };
            self.sim.physics.set_velocity(
                part.id,
                (b.x + part.offset.0 - x) * 4.,
                (b.y + part.offset.1 - y) * 4.,
                0.,
            );
            if part.role == PartRole::Gun {
                if age > 100 && age % if generators { 110 } else { 85 } == 0 {
                    let aim = (py - y).atan2(px - x);
                    for n in -2..=2 {
                        fire.push((x - 18., y, aim + n as f32 * 0.14, 90.));
                    }
                }
                if age > 60 && age % 240 == 90 {
                    beams.push(Laser {
                        owner: part.id,
                        x: x - 18.,
                        y,
                        angle: (py - y).atan2(px - x),
                        age: 0,
                    });
                }
            }
            if part.role == PartRole::Reactor && coolers == 0 && !generators && age % 65 == 0 {
                for n in -3..=3 {
                    fire.push((
                        x - 18.,
                        y,
                        std::f32::consts::PI + n as f32 * 0.2 + (age as f32 * 0.02).sin() * 0.4,
                        85.,
                    ));
                }
            }
        }
        self.boss = Some(b);
        for (x, y, a, v) in fire {
            self.shoot(x, y, a, v, false, 1);
        }
        if !beams.is_empty() {
            self.sounds.push(4);
        }
        self.lasers.extend(beams);
    }
    pub(super) fn draw_encounters(&self, p: &egui::Painter, rect: Rect, scale: f32) {
        let to = |x: f32, y: f32| Pos2::new(rect.left() + x * scale, rect.top() + y * scale);
        for s in &self.ships {
            if s.dead {
                continue;
            }
            let Some((x, y)) = self.sim.physics.centre(s.id) else {
                continue;
            };
            let label = match s.kind {
                Kind::ShieldDrone => "SHIELD",
                Kind::RepairDrone => "REPAIR",
                Kind::Bomber => "MINELAYER",
                Kind::Mine => "MINE",
                _ => continue,
            };
            let color = if s.kind == Kind::RepairDrone {
                Color32::from_rgb(85, 240, 155)
            } else {
                Color32::from_rgb(75, 175, 255)
            };
            p.text(
                to(x, y - 20.),
                egui::Align2::CENTER_BOTTOM,
                label,
                egui::FontId::proportional(6. * scale),
                color,
            );
            if s.kind == Kind::Mine {
                p.circle_stroke(
                    to(x, y),
                    13. * scale,
                    Stroke::new(scale, Color32::from_rgb(255, 110, 150)),
                );
            }
            for t in self.ships.iter().filter(|t| {
                !t.dead
                    && t.id != s.id
                    && t.squad == s.squad
                    && t.squad > 0
                    && matches!(t.kind, Kind::Tanker | Kind::Frigate)
            }) {
                if let Some((tx, ty)) = self.sim.physics.centre(t.id) {
                    p.line_segment(
                        [to(x, y), to(tx, ty)],
                        Stroke::new(0.6 * scale, color.gamma_multiply(0.45)),
                    );
                    if s.kind == Kind::ShieldDrone {
                        p.circle_stroke(to(tx, ty), 46. * scale, Stroke::new(0.6 * scale, color));
                    }
                }
            }
        }
        if let Some(b) = &self.boss {
            for part in &b.parts {
                if part.dead {
                    continue;
                }
                if let Some((x, y)) = self.sim.physics.centre(part.id).or_else(|| {
                    if part.role == PartRole::Reactor {
                        self.reactor_position()
                    } else {
                        None
                    }
                }) {
                    let locked = self.protected_target(part.id);
                    let c = if locked {
                        Color32::from_rgb(50, 140, 230)
                    } else {
                        Color32::from_rgb(255, 185, 65)
                    };
                    p.circle_stroke(
                        to(x, y),
                        match part.role {
                            PartRole::Reactor => 24.,
                            _ => 18.,
                        } * scale,
                        Stroke::new(0.7 * scale, c),
                    );
                    p.text(
                        to(x, y - 22.),
                        egui::Align2::CENTER_BOTTOM,
                        match part.role {
                            PartRole::Generator => "GENERATOR",
                            PartRole::Gun => "GUN POD",
                            PartRole::Cooler => "COOLING",
                            PartRole::Reactor => {
                                if locked {
                                    "CORE LOCKED"
                                } else {
                                    "CORE EXPOSED"
                                }
                            }
                        },
                        egui::FontId::proportional(5.5 * scale),
                        c,
                    );
                }
            }
        }
    }
}
