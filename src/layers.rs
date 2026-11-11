//! Aligned physical depth planes. Heat passes only between adjacent planes,
//! using both materials' conductivity and heat capacity; material never changes
//! merely because it is on another plane. Each plane has its own rigid bodies.
use crate::{Materials, World, material::AIR, sim::Snapshot};

pub fn exchange_heat(a: &mut World, b: &mut World, mats: &Materials) {
    if (a.w, a.h) != (b.w, b.h) {
        return;
    }
    for i in 0..a.mat.len() {
        if a.mat[i] == AIR && b.mat[i] == AIR {
            continue;
        }
        let difference = a.temp[i] - b.temp[i];
        if difference.abs() < 0.05 {
            continue;
        }
        let (ma, mb) = (mats.get(a.mat[i]), mats.get(b.mat[i]));
        let (ca, cb) = (ma.heat_capacity.max(0.1), mb.heat_capacity.max(0.1));
        let conduct = ma.conducts_heat.min(mb.conducts_heat).clamp(0., 1.);
        let q = difference * (conduct * 0.12).min(1. / (1. / ca + 1. / cb));
        if q.abs() < 0.001 {
            continue;
        }
        a.temp[i] = (a.temp[i] - q / ca).clamp(-273., 5000.);
        b.temp[i] = (b.temp[i] + q / cb).clamp(-273., 5000.);
        a.wake_at(i);
        b.wake_at(i);
    }
}

/// Used by rewind's memory bound, including all retained interior cell arrays.
pub fn snapshot_bytes(s: &Snapshot) -> usize {
    s.world.mat.len() * 128
        + s.bodies
            .iter()
            .map(|b| b.cells.len() * std::mem::size_of::<Option<crate::physics::BodyCell>>())
            .sum::<usize>()
        + s.layers.iter().map(snapshot_bytes).sum::<usize>()
}
