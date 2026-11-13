//! Sparse scrolling-map regression checks: allocation, traversal and persistence.
use grave_engine::{
    Materials,
    overhead::{Aircraft, FlightInput, Point, map::WorldMap, space::Space},
};
fn map(m: &Materials) -> (WorldMap, Point) {
    WorldMap::load(std::path::Path::new("scenes/red-mesa.map"), m).unwrap()
}
#[test]
fn map_is_large_sparse_and_flight_crosses_many_patch_boundaries() {
    let m = Materials::load_dir("materials");
    let (mut world, spawn) = map(&m);
    assert!(world.width >= 1500 && world.depth >= 1100);
    assert!(world.allocated_cells() < world.width * world.depth * world.height / 10);
    let mut a = Aircraft::new(spawn);
    a.at.z = 74.;
    a.settings.acceleration = 145.;
    a.settings.speed = 65.;
    a.settings.damping = 2.2;
    for _ in 0..1400 {
        a.step(
            FlightInput {
                movement: [1., -0.65],
                ..Default::default()
            },
            &world,
            &m,
            1. / 60.,
        );
        world.focus = a.at;
        world.step(&m);
    }
    assert!(a.at.distance(spawn) > 1100.);
    assert!(!a.blocked);
    assert!(world.active_regions < 8);
}
#[test]
fn remote_material_damage_survives_leaving_and_returning_without_global_steps() {
    let m = Materials::load_dir("materials");
    let (mut world, spawn) = map(&m);
    let p = Point::new(19. * 64. + 15., 13. * 64. + 24., 18.);
    let original = world.cell(p).unwrap();
    assert_ne!(original.mat, 0);
    world.put_cell(p, None, &m);
    let key = (19, 13);
    let before = world.regions[&key].volume.frame;
    world.focus = spawn;
    for _ in 0..120 {
        world.step(&m);
    }
    assert_eq!(world.material_at(p), 0);
    let far_steps = world.regions[&key].volume.frame - before;
    assert!(far_steps < 20, "remote patch ran full rate: {far_steps}");
    world.focus = p;
    for _ in 0..20 {
        world.step(&m);
    }
    assert_eq!(world.material_at(p), 0);
    assert!(world.regions[&key].volume.frame >= before + 20);
}
#[test]
fn dropped_material_can_create_a_native_patch_in_open_terrain() {
    let m = Materials::load_dir("materials");
    let (mut world, _) = map(&m);
    let p = Point::new(960., 800., 45.);
    let oil = m.id("oil").unwrap();
    let mut c = world.cell(p).unwrap();
    c.mat = oil;
    c.temp = 240.;
    world.put_cell(p, Some(c), &m);
    world.focus = p;
    assert_eq!(world.material_at(p), oil);
    for _ in 0..3 {
        world.step(&m);
    }
    assert!(world.regions.contains_key(&(15, 12)));
}
#[test]
fn distant_simulation_has_a_deterministic_work_quota_and_fair_progress() {
    let m = Materials::load_dir("materials");
    let (mut world, _) = map(&m);
    world.active_radius = 0.;
    for r in world.regions.values_mut() {
        r.touched = true;
        r.urgent = 2;
    }
    for _ in 0..100 {
        world.step(&m);
        assert!(
            world.active_regions <= 1,
            "remote work synchronized into a spike"
        );
    }
    assert!(
        world.regions.values().all(|r| r.volume.frame >= 5),
        "a distant region starved"
    );
}
#[test]
fn projectiles_and_blasts_cross_native_patch_boundaries_in_global_coordinates() {
    use grave_engine::overhead::{
        Explosion,
        combat::{Bounds, Combat, Target, Weapon},
    };
    let m = Materials::load_dir("materials");
    let (mut world, _) = map(&m);
    let mut cell = world.cell(Point::new(66., 80., 50.)).unwrap();
    cell.mat = m.id("wood").unwrap();
    world.put_cell(Point::new(66., 80., 50.), Some(cell), &m);
    world.put_cell(Point::new(62., 80., 50.), Some(cell), &m);
    let mut combat = Combat::default();
    combat.targets.push(Target::new(
        99,
        2,
        Bounds {
            min: Point::new(65., 79., 49.),
            max: Point::new(68., 82., 52.),
        },
        100.,
        0.,
    ));
    let shot = Weapon {
        speed: 600.,
        range: 30.,
        damage: 30.,
        penetration: 1.,
        radius: 1.,
        force: 0.,
        spark_heat: 0.,
        guidance: 0.,
        explosive: false,
    };
    combat.shots.push(shot.launch(
        Point::new(63., 80., 50.),
        Point::new(67., 80., 50.),
        1,
        None,
    ));
    combat.step(&mut world, &m, 1. / 60.);
    assert_eq!(combat.target(99).unwrap().health, 70.);
    world.explode(Explosion {
        at: Point::new(63., 80., 50.),
        radius: 7.,
        power: 1000.,
        fire: None,
    });
    world.focus = Point::new(64., 80., 50.);
    world.step(&m);
    assert_eq!(
        world.explosions.len(),
        1,
        "one seam blast emitted duplicate game damage events"
    );
    for key in [(0, 1), (1, 1)] {
        assert_eq!(
            world.regions[&key].volume.explosions.len(),
            1,
            "blast stopped at a patch edge"
        );
    }
}
