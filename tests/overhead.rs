//! Reusable altitude, projection, material simulation and projectile integration.
use grave_engine::{
    Materials,
    material::AIR,
    overhead::{Aircraft, FlightInput, Point, Projectile, Volume, view::Projection},
    world::BURNING,
};
fn mats() -> Materials {
    Materials::load_dir("materials")
}
fn room(m: &Materials) -> Volume {
    let mut v = Volume::new(32, 12, 28, m).unwrap();
    v.box_part(
        "floor",
        m.id("concrete").unwrap(),
        [0, 0, 0],
        [32, 12, 2],
        None,
        m,
    );
    v
}
#[test]
fn altitude_distinguishes_roof_clearance_from_wall_hits_and_fast_projectiles() {
    let m = mats();
    let mut v = room(&m);
    v.box_part(
        "wall",
        m.id("wood").unwrap(),
        [16, 2, 2],
        [2, 8, 12],
        None,
        &m,
    );
    assert!(
        v.raycast(Point::new(4., 6., 22.), Point::new(27., 6., 22.), &m)
            .is_none()
    );
    assert!(
        v.raycast(Point::new(4., 6., 8.), Point::new(27., 6., 8.), &m)
            .is_some()
    );
    let mut shot = Projectile::aimed(
        Point::new(4., 6., 8.),
        Point::new(27., 6., 8.),
        10000.,
        3.,
        5000.,
        1050.,
        m.id("fire"),
    );
    assert!(shot.step(&mut v, &m, 1. / 60.));
    assert!(shot.at.x >= 16. && shot.at.x < 18.);
    assert!(
        v.sections
            .iter()
            .any(|s| s.world.temp.iter().any(|t| *t >= 1000.))
    );
    v.step(&m);
    assert!(!v.explosions.is_empty());
}
#[test]
fn aircraft_moves_stops_and_cannot_pass_through_low_obstacles() {
    let m = mats();
    let mut v = room(&m);
    v.box_part(
        "wall",
        m.id("brick").unwrap(),
        [18, 0, 2],
        [2, 12, 13],
        None,
        &m,
    );
    let mut a = Aircraft::new(Point::new(8., 6., 9.));
    for _ in 0..90 {
        a.step(
            FlightInput {
                movement: [1., 0.],
                ..Default::default()
            },
            &v,
            &m,
            1. / 60.,
        );
    }
    assert!(a.at.x < 16.);
    a.at.z = 22.;
    for _ in 0..100 {
        a.step(
            FlightInput {
                movement: [1., 0.],
                ..Default::default()
            },
            &v,
            &m,
            1. / 60.,
        );
    }
    assert!(a.at.x > 21.);
    for _ in 0..180 {
        a.step(FlightInput::default(), &v, &m, 1. / 60.);
    }
    assert!(a.velocity.x.abs() < 0.01);
}
#[test]
fn projection_roundtrips_height_and_picks_roofs_instead_of_ground() {
    let m = mats();
    let mut v = room(&m);
    v.box_part(
        "roof",
        m.id("wood").unwrap(),
        [10, 3, 12],
        [9, 6, 2],
        None,
        &m,
    );
    let p = Projection {
        origin: [300., 200.],
        scale: 3.,
    };
    let q = Point::new(13.5, 5.5, 13.5);
    let screen = p.project(q);
    let restored = p.unproject(screen, q.z);
    assert!(q.distance(restored) < 0.001);
    let hit = p.pick(screen, &v, &m);
    assert!(hit.z >= 12. && hit.z < 14.);
}
#[test]
fn fire_uses_existing_materials_and_spreads_between_ground_depth_sections() {
    let m = mats();
    let mut v = room(&m);
    let wood = m.id("wood").unwrap();
    v.box_part("fuel", wood, [7, 4, 2], [8, 2, 4], None, &m);
    let row = v.height - 1 - 4;
    for x in 7..15 {
        let i = row * v.width + x;
        v.sections[4].world.temp[i] = 1100.;
    }
    let before = v.count(wood);
    let mut adjacent_burned = false;
    for _ in 0..450 {
        v.step(&m);
        adjacent_burned |= v.sections[5].world.flags.iter().any(|f| f & BURNING != 0);
    }
    assert!(adjacent_burned);
    assert!(v.count(wood) < before);
}
#[test]
fn unsupported_roof_fragments_fall_along_altitude_not_ground_y() {
    let m = mats();
    let mut v = room(&m);
    let wood = m.id("wood").unwrap();
    v.box_part("left", wood, [5, 4, 2], [2, 2, 10], None, &m);
    v.box_part("right", wood, [23, 4, 2], [2, 2, 10], None, &m);
    v.box_part("roof", wood, [5, 4, 12], [20, 2, 2], None, &m);
    for _ in 0..30 {
        v.step(&m);
    }
    assert_eq!(v.material_at(Point::new(14., 4., 12.)), wood);
    v.box_part("remove", AIR, [5, 4, 2], [2, 2, 10], None, &m);
    v.box_part("remove", AIR, [23, 4, 2], [2, 2, 10], None, &m);
    for _ in 0..150 {
        v.step(&m);
    }
    assert_ne!(v.material_at(Point::new(14., 4., 12.)), wood);
    assert!(v.sections[4].physics.body_count() > 0);
}
#[test]
fn named_prefabs_load_and_bad_worlds_are_rejected() {
    let m = mats();
    let (v, spawn) = Volume::load(std::path::Path::new("scenes/flight-test.flight"), &m).unwrap();
    assert!(v.index(spawn).is_some());
    assert!(v.parts.iter().any(|p| p.name == "roof"));
    assert!(v.parts.iter().any(|p| p.name == "oil_inside"));
    assert!(v.count(m.id("gunpowder").unwrap()) > 0);
    assert!(Volume::new(usize::MAX, 12, 24, &m).is_err());
    assert!(v.index(Point::new(f32::NAN, 0., 0.)).is_none());
}

#[test]
fn overhead_instances_are_hidden_by_roofs_and_visible_above_them() {
    use grave_engine::overhead::view::{Instance, Viewport, VisualCell};
    let m = mats();
    let mut v = Volume::new(40, 32, 32, &m).unwrap();
    v.box_part(
        "roof",
        m.id("wood").unwrap(),
        [4, 2, 15],
        [30, 26, 2],
        None,
        &m,
    );
    let cells = [VisualCell {
        at: Point::default(),
        color: [0, 255, 255],
    }];
    let mut view = Viewport::new(240, 200);
    let camera = Point::new(18., 15., 0.);
    let empty = view.draw(&v, &m, camera, &[], &[]).to_vec();
    let hidden = [Instance {
        at: Point::new(13., 8., 8.),
        angle: 0.,
        cells: &cells,
        shadow_radius: 0.,
    }];
    assert_eq!(empty, view.draw(&v, &m, camera, &hidden, &[]));
    let above = [Instance {
        at: Point::new(13., 8., 22.),
        angle: 0.,
        cells: &cells,
        shadow_radius: 0.,
    }];
    assert_ne!(empty, view.draw(&v, &m, camera, &above, &[]));
}
