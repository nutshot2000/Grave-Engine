//! Shared 3D rubble: real contacts and preserved material state, not visual particles.
use grave_engine::{
    Materials,
    overhead::{Explosion, Point, Volume},
    physics::BodyCell,
    world::BURNING,
};
fn mats() -> Materials {
    Materials::load_dir("materials")
}
fn room(m: &Materials) -> Volume {
    let mut v = Volume::new(40, 32, 32, m).unwrap();
    v.box_part(
        "floor",
        m.id("concrete").unwrap(),
        [0, 0, 0],
        [40, 32, 2],
        None,
        m,
    );
    v
}
fn cell(m: &Materials, name: &str) -> BodyCell {
    let id = m.id(name).unwrap();
    BodyCell {
        mat: id,
        temp: 20.,
        burn: 0,
        life: 0,
        flags: 0,
        shade: 128,
        paint: grave_engine::asset::pack_paint([130, 75, 39]),
    }
}
#[test]
fn rubble_crosses_ground_depth_and_preserves_solid_cells() {
    let m = mats();
    let mut v = room(&m);
    let c = cell(&m, "wood");
    v.debris
        .spawn(
            vec![
                (Point::new(12.5, 4.5, 12.5), c),
                (Point::new(13.5, 4.5, 12.5), c),
            ],
            Point::new(0., 40., 0.),
            Point::new(1., 2., 3.),
            &m,
        )
        .unwrap();
    for _ in 0..35 {
        v.step(&m);
    }
    let s = v.debris.states();
    assert_eq!(s.len(), 1);
    assert!(
        s[0].at.y > 12.,
        "rubble must travel through multiple former sections: {:?}",
        s[0]
    );
    assert_eq!(
        v.count(c.mat),
        2,
        "stamping must neither duplicate nor lose solid material"
    );
    assert!(v.sections.iter().all(|s| s.physics.body_count() == 0));
}
#[test]
fn fast_fragment_cannot_tunnel_through_a_wall_in_another_section() {
    let m = mats();
    let mut v = room(&m);
    v.box_part(
        "wall",
        m.id("brick").unwrap(),
        [0, 14, 2],
        [40, 1, 25],
        None,
        &m,
    );
    v.debris
        .spawn(
            vec![(Point::new(12.5, 5.5, 14.5), cell(&m, "iron"))],
            Point::new(0., 180., 0.),
            Point::default(),
            &m,
        )
        .unwrap();
    let mut furthest = 0f32;
    for _ in 0..25 {
        v.step(&m);
        furthest = furthest.max(v.debris.states()[0].at.y);
    }
    assert!(
        furthest < 14.,
        "continuous 3D collision must catch a one-cell wall: {furthest}"
    );
}
#[test]
fn fragments_collide_stack_and_settle_on_the_ground() {
    let m = mats();
    let mut v = room(&m);
    for z in [8.5, 13.5] {
        v.debris
            .spawn(
                vec![(Point::new(12.5, 12.5, z), cell(&m, "stone"))],
                Point::default(),
                Point::default(),
                &m,
            )
            .unwrap();
    }
    for _ in 0..480 {
        v.step(&m);
    }
    let mut states = v.debris.states();
    states.sort_by(|a, b| a.at.z.total_cmp(&b.at.z));
    assert_eq!(states.len(), 2);
    assert!(
        (states[0].at.z - 2.5).abs() < 0.2,
        "ground contact {:?}",
        states[0]
    );
    assert!(
        (states[1].at.z - 3.5).abs() < 0.25,
        "rubble-rubble contact {:?}",
        states[1]
    );
    assert!(states.iter().all(|s| s.velocity.z.abs() < 0.1));
    assert_eq!(v.count(m.id("stone").unwrap()), 2);
}
#[test]
fn flying_burning_material_keeps_paint_and_burns_even_above_the_world() {
    let m = mats();
    let mut v = room(&m);
    let mut c = cell(&m, "wood");
    c.flags = BURNING;
    c.burn = 160;
    c.temp = 900.;
    v.debris
        .spawn(
            vec![(Point::new(12.5, 6.5, 90.), c)],
            Point::new(0., 20., 0.),
            Point::default(),
            &m,
        )
        .unwrap();
    for _ in 0..35 {
        v.step(&m);
    }
    let cells: Vec<_> = v.debris.voxels().collect();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].1.paint, c.paint);
    assert!(cells[0].1.burn < 160 && cells[0].1.burn > 0);
    assert!(cells[0].0.z > v.height as f32);
    assert_eq!(v.count(c.mat), 1);
    for _ in 0..200 {
        v.step(&m);
    }
    assert_eq!(v.count(c.mat), 0, "airborne fuel must finish burning");
    assert!(v.debris.is_empty());
}
#[test]
fn native_blast_promotes_real_building_material_and_pushes_both_depth_directions() {
    let m = mats();
    let mut v = room(&m);
    v.box_part(
        "wall",
        m.id("wood").unwrap(),
        [13, 7, 2],
        [3, 18, 15],
        None,
        &m,
    );
    for _ in 0..5 {
        v.step(&m);
    }
    v.explode(Explosion {
        at: Point::new(14.5, 16., 10.),
        radius: 10.,
        power: 22000.,
        fire: None,
    });
    let mut pos = false;
    let mut neg = false;
    for _ in 0..10 {
        v.step(&m);
        for s in v.debris.states() {
            pos |= s.velocity.y > 2.;
            neg |= s.velocity.y < -2.;
        }
    }
    assert!(v.debris.len() > 1);
    assert!(
        pos && neg,
        "an explosion must scatter rubble across both ground-depth directions"
    );
    assert!(v.sections.iter().all(|s| s.physics.body_count() == 0));
}
#[test]
fn secondary_blast_relaunches_existing_rubble() {
    let m = mats();
    let mut v = room(&m);
    v.debris
        .spawn(
            vec![(Point::new(15.5, 18.5, 4.5), cell(&m, "wood"))],
            Point::default(),
            Point::default(),
            &m,
        )
        .unwrap();
    v.step(&m);
    v.explode(Explosion {
        at: Point::new(15.5, 15.5, 4.),
        radius: 8.,
        power: 12000.,
        fire: None,
    });
    v.step(&m);
    let s = v.debris.states();
    assert!(s[0].velocity.y > 10.);
    assert!(s[0].spin.x.abs() + s[0].spin.y.abs() + s[0].spin.z.abs() > 0.1);
}

#[test]
fn dense_metal_has_more_mass_and_receives_less_blast_speed_than_wood() {
    let m = mats();
    let mut wood = room(&m);
    let mut iron = room(&m);
    let at = Point::new(15.5, 18.5, 8.5);
    wood.debris
        .spawn(
            vec![(at, cell(&m, "wood"))],
            Point::default(),
            Point::default(),
            &m,
        )
        .unwrap();
    iron.debris
        .spawn(
            vec![(at, cell(&m, "iron"))],
            Point::default(),
            Point::default(),
            &m,
        )
        .unwrap();
    let blast = Explosion {
        at: Point::new(15.5, 15.5, 8.5),
        radius: 8.,
        power: 12000.,
        fire: None,
    };
    wood.debris.blast(blast);
    iron.debris.blast(blast);
    assert!(iron.debris.states()[0].mass > wood.debris.states()[0].mass * 5.);
    assert!(wood.debris.states()[0].velocity.y > iron.debris.states()[0].velocity.y * 2.);
}

#[test]
fn roof_panels_lift_clear_instead_of_wedging_in_the_blast_hole() {
    let m = mats();
    let mut v = room(&m);
    let wood = m.id("wood").unwrap();
    v.box_part("left", wood, [8, 4, 2], [2, 24, 14], None, &m);
    v.box_part("right", wood, [30, 4, 2], [2, 24, 14], None, &m);
    v.box_part("roof", wood, [8, 4, 16], [24, 24, 2], None, &m);
    for _ in 0..10 {
        v.step(&m);
    }
    v.explode(Explosion {
        at: Point::new(20., 16., 17.),
        radius: 9.,
        power: 18000.,
        fire: None,
    });
    let mut peak = 17.;
    for _ in 0..25 {
        v.step(&m);
        for p in v.debris.states() {
            peak = f32::max(peak, p.at.z);
        }
    }
    assert!(
        peak > 22.,
        "a roof blast must eject real panels: max height {peak}"
    );
}

#[test]
fn fracture_produces_a_mix_of_shapes_sizes_and_long_pieces() {
    use std::collections::BTreeSet;
    let m = mats();
    let mut v = room(&m);
    v.box_part(
        "timber",
        m.id("wood").unwrap(),
        [6, 4, 2],
        [28, 24, 18],
        None,
        &m,
    );
    for _ in 0..5 {
        v.step(&m);
    }
    v.explode(Explosion {
        at: Point::new(20., 16., 12.),
        radius: 15.,
        power: 30000.,
        fire: None,
    });
    v.step(&m);
    let states = v.debris.states();
    let profiles: BTreeSet<_> = states
        .iter()
        .map(|s| {
            (
                (s.extent.x * 10.).round() as i32,
                (s.extent.y * 10.).round() as i32,
                (s.extent.z * 10.).round() as i32,
                s.cells,
            )
        })
        .collect();
    let small = states.iter().map(|s| s.cells).min().unwrap();
    let large = states.iter().map(|s| s.cells).max().unwrap();
    assert!(
        profiles.len() > 8,
        "fracture needs varied physical silhouettes: {profiles:?}"
    );
    assert!(
        large > small * 4,
        "mix small splinters with larger chunks: {small}..{large}"
    );
    assert!(
        states
            .iter()
            .any(|s| s.extent.x > s.extent.y * 2. && s.extent.x > 4.),
        "include longer, narrow pieces"
    );
    assert!(
        states
            .iter()
            .any(|s| (s.cells as f32) < s.extent.x * s.extent.y * s.extent.z * 0.7),
        "include irregular shapes, not only cuboids"
    );
}
