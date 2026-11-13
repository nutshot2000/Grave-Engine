//! Common material access for a bounded volume or a sparse scrolling map.
use super::{Explosion, Hit, Point, Volume};
use crate::{MatId, Materials, State, material::AIR, physics::BodyCell};
pub trait Space {
    fn extent(&self) -> [usize; 3];
    fn frame(&self) -> u32;
    fn explosions(&self) -> &[Explosion];
    fn cell(&self, p: Point) -> Option<BodyCell>;
    fn take_cell(&mut self, p: Point) -> Option<BodyCell>;
    fn put_cell(&mut self, p: Point, c: Option<BodyCell>, m: &Materials);
    fn heat(&mut self, p: Point, r: f32, t: f32);
    fn impact(&mut self, p: Point, r: f32, power: f32, m: &Materials);
    fn explode(&mut self, e: Explosion);
    fn material_at(&self, p: Point) -> MatId {
        self.cell(p).map_or(AIR, |c| c.mat)
    }
    fn contains(&self, p: Point) -> bool {
        let n = self.extent();
        [p.x, p.y, p.z]
            .iter()
            .enumerate()
            .all(|(i, a)| a.is_finite() && *a >= 0. && *a < n[i] as f32)
    }
    fn solid_at(&self, p: Point, m: &Materials) -> bool {
        let id = self.material_at(p);
        id != AIR && m.get(id).state != State::Gas
    }
    fn raycast(&self, a: Point, b: Point, m: &Materials) -> Option<Hit> {
        let n = (a.distance(b) / 0.35).ceil().clamp(1., 4096.) as usize;
        for k in 0..=n {
            let p = a.lerp(b, k as f32 / n as f32);
            if self.solid_at(p, m) {
                return Some(Hit {
                    at: p,
                    material: self.material_at(p),
                });
            }
        }
        None
    }
}
impl Space for Volume {
    fn extent(&self) -> [usize; 3] {
        [self.width, self.depth, self.height]
    }
    fn frame(&self) -> u32 {
        self.frame
    }
    fn explosions(&self) -> &[Explosion] {
        &self.explosions
    }
    fn cell(&self, p: Point) -> Option<BodyCell> {
        let (y, i) = self.index(p)?;
        let w = &self.sections[y].world;
        Some(BodyCell {
            mat: w.mat[i],
            temp: w.temp[i],
            burn: w.burn[i],
            life: w.life[i],
            flags: w.flags[i],
            shade: w.shade[i],
            paint: w.paint[i],
        })
    }
    fn take_cell(&mut self, p: Point) -> Option<BodyCell> {
        let c = self.cell(p)?;
        let (y, i) = self.index(p)?;
        self.sections[y].world.make_air(i);
        Some(c)
    }
    fn put_cell(&mut self, p: Point, c: Option<BodyCell>, m: &Materials) {
        if let Some((y, i)) = self.index(p) {
            let w = &mut self.sections[y].world;
            if let Some(c) = c {
                w.set(m, i % self.width, i / self.width, c.mat);
                w.temp[i] = c.temp;
                w.burn[i] = c.burn;
                w.life[i] = c.life;
                w.flags[i] = c.flags;
                w.shade[i] = c.shade;
                w.paint[i] = c.paint;
            } else {
                w.make_air(i);
            }
        }
    }
    fn heat(&mut self, p: Point, r: f32, t: f32) {
        Volume::heat(self, p, r, t)
    }
    fn impact(&mut self, p: Point, r: f32, power: f32, m: &Materials) {
        Volume::impact(self, p, r, power, m)
    }
    fn explode(&mut self, e: Explosion) {
        Volume::explode(self, e)
    }
}
