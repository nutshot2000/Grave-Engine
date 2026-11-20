//! An Undertow station is a native Grave scene. Collision, damage, fire and
//! the background image all look at the same live material grid.
use eframe::egui::{Pos2, Vec2};
use grave_engine::{
    Material, Materials, Sim, State, material::AIR, scene, trigger::Action, world::BURNING,
};
use std::path::Path;

pub const PIXELS: f32 = 4.;
pub const WIDTH: usize = 192;
pub const HEIGHT: usize = 112;

pub struct Crossing {
    pub name: String,
    min: Pos2,
    max: Pos2,
}

impl Crossing {
    pub fn sign_position(&self) -> Pos2 {
        self.min - Vec2::new(19., 12.)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CrossingState {
    Sealed,
    Hot,
    Open,
}

pub struct Arena {
    pub crossings: Vec<Crossing>,
    pub sim: Sim,
    pub materials: Materials,
}

impl Arena {
    pub fn load(path: &Path, bundled: &str) -> (Self, Vec<(String, Pos2)>) {
        let materials = Materials::load_dir("materials");
        let mut markers = Vec::new();
        let mut load = |text: &str| {
            markers.clear();
            let (mut sim, issues) =
                scene::load(text, &materials, path.parent()).into_sim(&materials);
            if !issues.is_empty() {
                return Err(issues.join("\n"));
            }
            if (sim.world.w, sim.world.h) != (WIDTH, HEIGHT) {
                return Err(format!("Expected a {WIDTH} x {HEIGHT} station"));
            }
            let mut crossings = Vec::new();
            sim.triggers.retain(|zone| {
                for action in &zone.actions {
                    if let Action::Message(message) = action
                        && let Some(marker) = message.strip_prefix("undertow ")
                    {
                        if let Some(name) = marker.strip_prefix("route ") {
                            crossings.push(Crossing {
                                name: name.to_owned(),
                                min: Pos2::new(zone.x, zone.y) * PIXELS,
                                max: Pos2::new(zone.x + zone.w, zone.y + zone.h) * PIXELS,
                            });
                            return false;
                        }
                        markers.push((
                            marker.to_owned(),
                            Pos2::new(zone.x + zone.w / 2., zone.y + zone.h / 2.) * PIXELS,
                        ));
                        return false;
                    }
                }
                true
            });
            let arena = Self {
                crossings,
                sim,
                materials: materials.clone(),
            };
            if ["spawn", "exit"]
                .into_iter()
                .any(|tag| markers.iter().filter(|(name, _)| name == tag).count() != 1)
                || !markers
                    .iter()
                    .any(|(name, _)| matches!(name.as_str(), "runner" | "gunner"))
                || markers.iter().any(|(_, p)| !arena.clear(*p))
            {
                return Err(
                    "Check the spawn, exit and guard markers for missing markers or blocked floor"
                        .into(),
                );
            }
            Ok(arena)
        };

        let arena = match std::fs::read_to_string(path) {
            Ok(text) => load(&text).unwrap_or_else(|error| {
                eprintln!("{}: {error}. Loading the bundled station.", path.display());
                load(bundled).expect("bundled station must be valid")
            }),
            Err(_) => load(bundled).expect("bundled station must be valid"),
        };
        (arena, markers)
    }

    /// A route opens when the player's footprint fits through at least one
    /// lane across the marked gate. Report hot openings separately.
    pub fn crossing_state(&self, crossing: &Crossing) -> CrossingState {
        let left = ((crossing.min.x / PIXELS).floor() as i32).clamp(0, WIDTH as i32);
        let right = ((crossing.max.x / PIXELS).ceil() as i32).clamp(0, WIDTH as i32);
        let top = ((crossing.min.y / PIXELS).floor() as i32).clamp(0, HEIGHT as i32);
        let bottom = ((crossing.max.y / PIXELS).ceil() as i32).clamp(0, HEIGHT as i32);
        if left >= right || top >= bottom {
            return CrossingState::Sealed;
        }
        let mut open_hot = false;
        for row in top..bottom {
            let lane =
                || (left..right).map(|col| Pos2::new(col as f32 + 0.5, row as f32 + 0.5) * PIXELS);
            if lane().all(|pos| self.clear(pos)) {
                if !lane().any(|pos| self.heat(pos)) {
                    return CrossingState::Open;
                }
                open_hot = true;
            }
        }
        if open_hot {
            CrossingState::Hot
        } else {
            CrossingState::Sealed
        }
    }
    fn index(&self, p: Pos2) -> Option<usize> {
        let x = (p.x / PIXELS).floor() as i32;
        let y = (p.y / PIXELS).floor() as i32;
        self.sim
            .world
            .in_bounds(x, y)
            .then(|| self.sim.world.idx(x as usize, y as usize))
    }

    pub fn material(&self, p: Pos2) -> Option<&Material> {
        let mat = self.sim.world.mat[self.index(p)?];
        (mat != AIR).then(|| self.materials.get(mat))
    }

    pub fn wall(&self, p: Pos2) -> bool {
        self.index(p).is_none_or(|i| {
            let material = self.materials.get(self.sim.world.mat[i]);
            material.state == State::Solid && !Self::loose_shard(&self.sim, i, material)
        })
    }

    fn loose_shard(sim: &Sim, index: usize, material: &Material) -> bool {
        sim.world.is_owned(index) && material.toughness.is_some_and(|toughness| toughness < 250.)
    }
    pub fn clear(&self, p: Pos2) -> bool {
        [[0., 0.], [-7.5, -7.5], [7.5, -7.5], [-7.5, 7.5], [7.5, 7.5]]
            .into_iter()
            .all(|[x, y]| !self.wall(p + Vec2::new(x, y)))
    }

    pub fn heat(&self, p: Pos2) -> bool {
        self.index(p).is_some_and(|i| {
            self.sim.world.flags[i] & BURNING != 0 || self.sim.world.temp[i] >= 280.
        })
    }

    pub fn wet(&self, p: Pos2) -> bool {
        self.material(p).is_some_and(|m| m.state == State::Liquid)
    }

    pub fn strike(&mut self, p: Pos2, velocity: Vec2) -> bool {
        let sim = &mut self.sim;
        let world_pos = p / PIXELS;
        let x = world_pos.x as i32;
        let y = world_pos.y as i32;
        if !sim.world.in_bounds(x, y) {
            return false;
        }
        let i = sim.world.idx(x as usize, y as usize);
        let m = self.materials.get(sim.world.mat[i]);
        if m.explosion_size > 0. || m.state == State::Liquid && m.catches_fire_at.is_some() {
            sim.world.heat_circle(x, y, 3, 900.);
            return true;
        }
        if m.state != State::Solid || Self::loose_shard(sim, i, m) {
            return false;
        }
        // Centre the impulse a little beyond the surface so repeated rounds
        // can open a passage through a partition, using material toughness.
        let impact = world_pos + velocity.normalized() * 1.4;
        sim.physics.impact_circle(
            &mut sim.world,
            &self.materials,
            impact.x,
            impact.y,
            6.5,
            1300.,
        );
        true
    }
}
