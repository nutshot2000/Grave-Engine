//! Persistent asset groups across physical depth planes. Physical joints stay
//! local to each plane; editor transformations operate on the entire instance.
use crate::{
    Materials, Sim,
    physics::{BodySpec, GROUND, JointSpec},
};
use std::collections::{BTreeMap, BTreeSet};

pub fn plane(s: &Sim, z: usize) -> &Sim {
    if z == 0 { s } else { &s.layers[z - 1] }
}
pub fn plane_mut(s: &mut Sim, z: usize) -> &mut Sim {
    if z == 0 { s } else { &mut s.layers[z - 1] }
}
pub fn all_ids(s: &Sim) -> Vec<Vec<u64>> {
    (0..=s.layers.len())
        .map(|z| plane(s, z).physics.ids())
        .collect()
}
pub fn names(s: &Sim) -> BTreeSet<String> {
    (0..=s.layers.len())
        .flat_map(|z| {
            let p = plane(s, z);
            p.physics
                .ids()
                .into_iter()
                .filter_map(|id| p.physics.assembly(id).map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .collect()
}
pub fn unique_name(s: &Sim, base: &str) -> String {
    let clean: String = base
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(60)
        .collect();
    let used = names(s);
    (1..)
        .map(|i| format!("{clean}_{i}"))
        .find(|n| !used.contains(n))
        .unwrap()
}
pub fn tag_new(s: &mut Sim, before: &[Vec<u64>], name: &str) {
    for z in 0..=s.layers.len() {
        let p = plane_mut(s, z);
        for id in p.physics.ids() {
            if !before.get(z).is_some_and(|v| v.contains(&id)) {
                p.physics.set_assembly(id, Some(name.into()));
            }
        }
    }
}
pub fn selected_names(s: &Sim, ids: &[u64]) -> BTreeSet<String> {
    ids.iter()
        .filter_map(|id| s.physics.assembly(*id).map(str::to_owned))
        .collect()
}
pub fn members(s: &Sim, ids: &[u64], whole: bool) -> Vec<Vec<u64>> {
    let groups = if whole {
        selected_names(s, ids)
    } else {
        BTreeSet::new()
    };
    (0..=s.layers.len())
        .map(|z| {
            let p = plane(s, z);
            p.physics
                .ids()
                .into_iter()
                .filter(|id| {
                    (z == 0 && ids.contains(id))
                        || p.physics.assembly(*id).is_some_and(|n| groups.contains(n))
                })
                .collect()
        })
        .collect()
}
pub fn expand_front(s: &Sim, ids: &[u64]) -> Vec<u64> {
    members(s, ids, true).remove(0)
}
pub fn transform(
    s: &mut Sim,
    m: &Materials,
    ids: &[u64],
    whole: bool,
    delta: (f32, f32),
    angle: f32,
    pivot: (f32, f32),
) {
    let members = members(s, ids, whole);
    let (c, sn) = (angle.cos(), angle.sin());
    let point = |p: (f32, f32)| {
        let (x, y) = (p.0 - pivot.0, p.1 - pivot.1);
        (
            pivot.0 + x * c - y * sn + delta.0,
            pivot.1 + x * sn + y * c + delta.1,
        )
    };
    for (z, ids) in members.iter().enumerate() {
        let p = plane_mut(s, z);
        let joints = p.physics.joints();
        for id in ids {
            p.physics.edit(&mut p.world, m, *id, |b| {
                (b.x, b.y) = point((b.x, b.y));
                b.angle += angle;
                b.vx = 0.;
                b.vy = 0.;
                b.spin = 0.;
            });
        }
        // Moving a grounded building also moves its attachment to the terrain.
        for id in ids {
            p.physics.unjoin(*id);
        }
        for mut j in joints {
            if ids.contains(&j.a) || ids.contains(&j.b) {
                if j.a == GROUND {
                    j.anchor_a = point(j.anchor_a);
                }
                if j.b == GROUND {
                    j.anchor_b = point(j.anchor_b);
                }
                p.physics.add_joint(m, j);
            }
        }
    }
}
pub fn remove(s: &mut Sim, ids: &[u64], whole: bool) {
    let groups = members(s, ids, whole);
    for (z, ids) in groups.into_iter().enumerate() {
        let p = plane_mut(s, z);
        for id in ids {
            p.physics.remove(&mut p.world, id);
        }
    }
}

#[derive(Clone)]
pub struct Clip {
    pub planes: Vec<PlaneClip>,
    pub centre: (f32, f32),
}
#[derive(Clone)]
pub struct PlaneClip {
    pub name: String,
    pub bodies: Vec<(u64, BodySpec)>,
    pub joints: Vec<JointSpec>,
    pub lights: Vec<crate::light::Light>,
}
impl Clip {
    pub fn capture(s: &Sim, ids: &[u64], whole: bool) -> Self {
        let groups = members(s, ids, whole);
        let mut count = 0.;
        let mut centre = (0., 0.);
        let planes = groups
            .iter()
            .enumerate()
            .map(|(z, ids)| {
                let p = plane(s, z);
                let bodies: Vec<_> = ids
                    .iter()
                    .filter_map(|id| p.physics.spec_of(&p.world, *id).map(|b| (*id, b)))
                    .collect();
                for (_, b) in &bodies {
                    centre.0 += b.x;
                    centre.1 += b.y;
                    count += 1.;
                }
                let joints = p
                    .physics
                    .joints()
                    .into_iter()
                    .filter(|j| {
                        (j.a == GROUND || ids.contains(&j.a))
                            && (j.b == GROUND || ids.contains(&j.b))
                    })
                    .collect();
                let lights = p
                    .lighting
                    .lights
                    .iter()
                    .filter(|l| l.on.is_some_and(|id| ids.contains(&id)))
                    .cloned()
                    .collect();
                PlaneClip {
                    name: if z == 0 {
                        "front".into()
                    } else {
                        s.layer_names[z - 1].clone()
                    },
                    bodies,
                    joints,
                    lights,
                }
            })
            .collect();
        if count > 0. {
            centre.0 /= count;
            centre.1 /= count;
        }
        Self { planes, centre }
    }
    pub fn paste(&self, s: &mut Sim, m: &Materials, x: f32, y: f32) -> Vec<u64> {
        let delta = (x - self.centre.0, y - self.centre.1);
        let mut new_groups = BTreeMap::new();
        for p in &self.planes {
            for (_, b) in &p.bodies {
                if let Some(name) = &b.assembly {
                    new_groups
                        .entry(name.clone())
                        .or_insert_with(|| unique_name(s, name));
                }
            }
        }
        let mut front = Vec::new();
        for (z, clip) in self.planes.iter().enumerate() {
            if clip.bodies.is_empty() {
                continue;
            }
            if z > 0 {
                s.ensure_layer(z, &clip.name, m);
            }
            let p = plane_mut(s, z);
            let mut remap = BTreeMap::from([(GROUND, GROUND)]);
            for (old, body) in &clip.bodies {
                let mut b = body.clone();
                b.x += delta.0;
                b.y += delta.1;
                b.vx = 0.;
                b.vy = 0.;
                b.spin = 0.;
                if let Some(name) = &b.assembly {
                    b.assembly = Some(new_groups[name].clone());
                }
                if let Some(id) = p.physics.add_exact(&mut p.world, m, b) {
                    remap.insert(*old, id);
                    if z == 0 {
                        front.push(id);
                    }
                }
            }
            for l in &clip.lights {
                if let Some(id) = l.on.and_then(|id| remap.get(&id).copied()) {
                    let mut l = l.clone();
                    l.on = Some(id);
                    p.lighting.lights.push(l);
                }
            }
            for j in &clip.joints {
                if let (Some(&a), Some(&b)) = (remap.get(&j.a), remap.get(&j.b)) {
                    let mut j = j.clone();
                    j.a = a;
                    j.b = b;
                    if a == GROUND {
                        j.anchor_a.0 += delta.0;
                        j.anchor_a.1 += delta.1;
                    }
                    if b == GROUND {
                        j.anchor_b.0 += delta.0;
                        j.anchor_b.1 += delta.1;
                    }
                    p.physics.add_joint(m, j);
                }
            }
        }
        front
    }
}
