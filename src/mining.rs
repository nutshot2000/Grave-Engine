//! Focused surface drilling. Work belongs to terrain positions, never objects.
//! Sparse fractures travel with snapshots, scene saves and editor rewind.
use crate::{
    World,
    material::{MatId, Materials, State},
    sound::{Kind, Sound},
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Fracture {
    pub material: MatId,
    pub work: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Impact {
    pub x: f32,
    pub y: f32,
    pub material: MatId,
    pub frame: u32,
    pub blocked: bool,
    pub broken: bool,
    pub aim: (f32, f32),
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mining {
    pub fractures: BTreeMap<usize, Fracture>,
    pub impact: Option<Impact>,
}

/// Tool values are shared by editor and games. Higher grade can cut denser rock.
#[derive(Clone, Copy)]
pub struct Drill {
    pub radius: f32,
    pub reach: f32,
    pub power: f32,
    pub grade: f32,
}
impl Default for Drill {
    fn default() -> Self {
        Self {
            radius: 6.0,
            reach: 19.0,
            power: 1.0,
            grade: 3.0,
        }
    }
}
pub fn hardness(m: &crate::material::Material) -> f32 {
    m.mining_hardness
        .unwrap_or_else(|| m.toughness.map_or(0.15, |t| (t / 7000.0).max(0.05)))
}
fn obstacle(w: &World, m: &Materials, i: usize) -> bool {
    !w.is_owned(i) && matches!(m.get(w.mat[i]).state, State::Solid | State::Powder)
}
/// First reachable terrain surface. Owned bodies and liquids block the tool too.
pub fn surface(
    w: &World,
    m: &Materials,
    from: (f32, f32),
    aim: (f32, f32),
    reach: f32,
) -> Option<usize> {
    let n = aim.0.hypot(aim.1);
    if n < 0.01 {
        return None;
    }
    let mut outside = false;
    for step in 0..=(reach.clamp(4.0, 32.0) * 2.0) as usize {
        let d = step as f32 * 0.5;
        let (x, y) = (
            (from.0 + aim.0 / n * d).floor() as isize,
            (from.1 + aim.1 / n * d).floor() as isize,
        );
        if x < 1 || y < 1 || x >= w.w as isize - 1 || y >= w.h as isize - 1 {
            return None;
        }
        let i = w.idx(x as usize, y as usize);
        // Ignore the user's own body near the origin, never drill through other bodies.
        if w.is_owned(i) {
            if !outside {
                continue;
            }
            return None;
        }
        outside = true;
        if obstacle(w, m, i) {
            return Some(i);
        }
        if m.get(w.mat[i]).state == State::Liquid {
            return None;
        }
    }
    None
}
impl Mining {
    pub fn prune(&mut self, w: &World) {
        self.fractures
            .retain(|&i, f| i < w.mat.len() && w.mat[i] == f.material && !w.is_owned(i));
    }
    pub fn shift(&mut self, dx: i32, dy: i32, w: usize, h: usize) {
        self.fractures = std::mem::take(&mut self.fractures)
            .into_iter()
            .filter_map(|(i, f)| {
                let (x, y) = (i as i32 % w as i32 + dx, i as i32 / w as i32 + dy);
                (x >= 0 && y >= 0 && x < w as i32 && y < h as i32)
                    .then_some(((y.max(0) as usize) * w + x.max(0) as usize, f))
            })
            .collect();
        self.impact = None;
    }
    /// One 0.1-second strike. Only the visible skin is worked, so a hard facing
    /// shields softer rock and rewards cannot be collected through walls.
    pub fn strike(
        &mut self,
        w: &mut World,
        m: &Materials,
        from: (f32, f32),
        aim: (f32, f32),
        tool: Drill,
    ) -> BTreeMap<MatId, usize> {
        let mut removed = BTreeMap::new();
        self.prune(w);
        let Some(hit) = surface(w, m, from, aim, tool.reach) else {
            self.impact = None;
            return removed;
        };
        let n = aim.0.hypot(aim.1);
        let aim = (aim.0 / n, aim.1 / n);
        let (x, y) = ((hit % w.w) as f32, (hit / w.w) as f32);
        let material = w.mat[hit];
        let blocked = hardness(m.get(material)) > tool.grade;
        let candidates = candidates(w,m,from,hit,tool);
        for (i, weight) in candidates {
            let id = w.mat[i];
            let f = self.fractures.entry(i).or_insert(Fracture {
                material: id,
                work: 0.0,
            });
            f.work += 0.1 * tool.power.max(0.0) * weight;
            if f.work >= hardness(m.get(id)) {
                self.fractures.remove(&i);
                w.make_air(i);
                *removed.entry(id).or_default() += 1;
            }
        }
        // Bound old marks during very long runs; no simulation random numbers.
        while self.fractures.len() > 8192 {
            self.fractures.pop_first();
        }
        let broken = !removed.is_empty();
        self.impact = Some(Impact {
            x,
            y,
            material,
            frame: w.frame,
            blocked,
            broken,
            aim,
        });
        w.sound(Sound {
            kind: if broken { Kind::Break } else { Kind::Hit },
            x,
            y,
            mat: material,
            other: material,
            loud: if broken { 0.55 } else { 0.18 },
            size: if broken { 3.0 } else { 1.0 },
        });
        removed
    }
}

/// The exact exposed pixels that the next strike will work. Shared with aiming UI.
pub fn candidates(w:&World,m:&Materials,from:(f32,f32),hit:usize,tool:Drill)->Vec<(usize,f32)> {
    let (x,y)=((hit%w.w) as f32,(hit/w.w) as f32);
        let mut candidates = Vec::new();
        let r = tool.radius.clamp(2.0, 8.0);
        for cy in (y - r).floor().max(1.0) as usize..=((y + r).ceil() as usize).min(w.h - 2) {
            for cx in (x - r).floor().max(1.0) as usize..=((x + r).ceil() as usize).min(w.w - 2) {
                let i = w.idx(cx, cy);
                let d = (cx as f32 - x).hypot(cy as f32 - y);
                if d > r || !obstacle(w, m, i) || hardness(m.get(w.mat[i])) > tool.grade {
                    continue;
                }
                // Trace the face before removing anything: at most two layers
                // receive work during a strike, regardless of map iteration order.
                let (vx, vy) = (cx as f32 + 0.5 - from.0, cy as f32 + 0.5 - from.1);
                let len = vx.hypot(vy);
                if len > tool.reach + r * 0.25 {
                    continue;
                }
                let mut depth = 0;
                let mut last = usize::MAX;
                let mut covered = false;
                let mut outside = false;
                for step in 0..=(len * 2.0) as usize {
                    let t = step as f32 * 0.5 / len;
                    let (sx, sy) = (
                        (from.0 + vx * t).floor() as usize,
                        (from.1 + vy * t).floor() as usize,
                    );
                    if sx >= w.w || sy >= w.h {
                        covered = true;
                        break;
                    }
                    let j = w.idx(sx, sy);
                    if j == last {
                        continue;
                    }
                    last = j;
                    if w.is_owned(j) {
                        if !outside {
                            continue;
                        }
                        covered = true;
                        break;
                    }
                    outside = true;
                    if m.get(w.mat[j]).state == State::Liquid {
                        covered = true;
                        break;
                    }
                    if obstacle(w, m, j) {
                        depth += 1;
                        if j != i && hardness(m.get(w.mat[j])) > tool.grade {
                            covered = true;
                            break;
                        }
                        if depth > 2 {
                            covered = true;
                            break;
                        }
                    }
                }
                if !covered {
                    candidates.push((i, (1.0 - d / r * 0.55).max(0.35)));
                }
            }
        }
    candidates
}
