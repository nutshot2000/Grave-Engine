//! Open upper atmosphere for overhead worlds. Gas leaves the bounded material
//! grid instead of flattening against its top, then rises and fades freely.
use super::{Point, Volume};
use crate::{MatId, Materials, State, material::AIR};
#[derive(Clone, Copy, Debug)]
pub struct Plume {
    pub at: Point,
    pub velocity: Point,
    pub material: MatId,
    pub life: u16,
    pub temperature: f32,
}
impl Volume {
    pub(crate) fn vent_sky(&mut self, m: &Materials) {
        for p in &mut self.plumes {
            p.at.x += p.velocity.x / 60.;
            p.at.y += p.velocity.y / 60.;
            p.at.z += p.velocity.z / 60.;
            p.life = p.life.saturating_sub(1);
            p.temperature += (20. - p.temperature) * m.get(p.material).cools_off;
            if p.life == 0 {
                let mat = m.get(p.material);
                if mat.decays_into != AIR {
                    p.material = mat.decays_into;
                    p.life = m.get(p.material).lifetime.unwrap_or(60);
                    p.velocity.z *= 0.65;
                }
            }
        }
        self.plumes
            .retain(|p| p.life > 0 && p.at.z < self.height as f32 + 100.);
        for (y, s) in self.sections.iter_mut().enumerate() {
            for row in 0..2 {
                for x in 0..self.width {
                    let i = row * self.width + x;
                    let id = s.world.mat[i];
                    if id == AIR || m.get(id).state != State::Gas {
                        continue;
                    }
                    if self.plumes.len() < 16000 {
                        let phase = ((x * 13 + y * 7 + self.frame as usize) % 19) as f32 / 19.;
                        self.plumes.push(Plume {
                            at: Point::new(
                                x as f32 + 0.5,
                                y as f32 + 0.5,
                                self.height as f32 - row as f32 - 0.5,
                            ),
                            velocity: Point::new(
                                (phase - 0.5) * 2.,
                                0.7,
                                (if m.get(id).glow > 0. { 40. } else { 22. }) + phase * 3.,
                            ),
                            material: id,
                            life: s.world.life[i].max(3),
                            temperature: s.world.temp[i],
                        });
                    }
                    s.world.make_air(i);
                }
            }
        }
    }
}
