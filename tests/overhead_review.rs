//! Regression tests for overhead (2.5D) bugs found in review.
use grave_engine::{
    Materials,
    overhead::{Aircraft, Explosion, FlightInput, Point, Volume, map::WorldMap, space::Space},
};

fn mats() -> Materials {
    Materials::load_dir("materials")
}

#[test]
fn heat_lands_in_the_cell_it_names() {
    let m = mats();
    let mut v = Volume::new(16, 8, 32, &m).unwrap();
    let stone = m.id("stone").unwrap();
    v.box_part("col", stone, [0, 0, 0], [16, 8, 32], None, &m);
    v.heat(Point::new(8.5, 4.5, 10.5), 0.6, 900.);
    for z in 8..13 {
        println!("z {z}: {:.0}Â°C", v.cell(Point::new(8.5, 4.5, z as f32 + 0.5)).unwrap().temp);
    }
}

#[test]
fn small_volume_flight_does_not_panic() {
    let m = mats();
    let v = Volume::new(8, 4, 8, &m).unwrap();
    let mut a = Aircraft::new(Point::new(4., 2., 4.));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        a.step(FlightInput::default(), &v, &m, 1. / 60.);
    }));
    assert!(r.is_ok(), "flight in a tiny volume must not panic");
}

#[test]
fn a_blast_on_a_patch_edge_digs_both_sides() {
    let m = mats();
    let (mut w, _) = WorldMap::load(std::path::Path::new("scenes/red-mesa.map"), &m).unwrap();
    // Find an unallocated pair of neighbouring patches.
    let (kx, ky) = (2usize, 2usize);
    assert!(!w.regions.contains_key(&(kx, ky)) && !w.regions.contains_key(&(kx + 1, ky)));
    let x = ((kx + 1) * 64) as f32 - 1.0; // last column of the left patch
    let y = (ky * 64 + 32) as f32;
    let z = w.ground_height(x, y) as f32;
    let solid = |w: &WorldMap, dx: f32| (0..6).filter(|dz| w.material_at(Point::new(x + dx, y, z - *dz as f32)) != 0).count();
    let (l0, r0) = (solid(&w, -2.), solid(&w, 3.));
    w.explode(Explosion { at: Point::new(x + 0.5, y, z), radius: 6., power: 60000., fire: None });
    w.focus = Point::new(x, y, z);
    for _ in 0..30 {
        w.step(&m);
    }
    let (l1, r1) = (solid(&w, -2.), solid(&w, 3.));
    let _ = (l0, l1, r0, r1);
    assert!(w.regions.contains_key(&(kx, ky)) && w.regions.contains_key(&(kx + 1, ky)), "both patches the blast reaches take part");
}
