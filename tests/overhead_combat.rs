//! Regressions for shared overhead weapons, sling loads and the open sky.
use grave_engine::{
    Materials, State,
    overhead::{
        Point, Volume,
        cargo::{Cargo, CargoState},
        combat::{Bounds, Combat, Target, Weapon},
    },
};
fn weapon() -> Weapon {
    Weapon {
        speed: 600.,
        range: 200.,
        damage: 30.,
        penetration: 1.,
        radius: 1.,
        force: 0.,
        spark_heat: 240.,
        guidance: 0.,
        explosive: false,
    }
}
fn room(m: &Materials) -> Volume {
    let mut v = Volume::new(40, 20, 32, m).unwrap();
    v.box_part(
        "floor",
        m.id("concrete").unwrap(),
        [0, 0, 0],
        [40, 20, 2],
        None,
        m,
    );
    v
}
#[test]
fn cannon_is_cold_on_wood_but_ignites_exposed_volatile_liquid() {
    let m = Materials::load_dir("materials");
    for name in ["wood", "oil"] {
        let mut v = room(&m);
        let id = m.id(name).unwrap();
        v.box_part("sample", id, [20, 8, 5], [3, 3, 4], None, &m);
        let mut c = Combat::default();
        c.shots
            .push(weapon().launch(Point::new(10., 9., 7.), Point::new(23., 9., 7.), 1, None));
        for _ in 0..3 {
            c.step(&mut v, &m, 1. / 60.);
        }
        let highest = v
            .sections
            .iter()
            .flat_map(|s| s.world.temp.iter())
            .copied()
            .fold(0., f32::max);
        if name == "wood" {
            assert!(highest < 100., "AP heated wood: {highest}");
        } else {
            assert!(highest >= 240., "AP failed to light oil: {highest}");
        }
    }
}
#[test]
fn swept_guided_hits_use_armour_and_decoys_leave_ballistic_rounds() {
    let m = Materials::load_dir("materials");
    let mut v = room(&m);
    let mut c = Combat::default();
    c.targets.push(Target::new(
        7,
        2,
        Bounds {
            min: Point::new(25., 8., 8.),
            max: Point::new(28., 12., 12.),
        },
        50.,
        12.,
    ));
    let mut w = weapon();
    w.guidance = 4.;
    c.shots.push(w.launch(
        Point::new(4., 10., 10.),
        Point::new(26., 10., 10.),
        1,
        Some(7),
    ));
    for _ in 0..4 {
        c.step(&mut v, &m, 1. / 60.);
    }
    assert_eq!(c.target(7).unwrap().health, 20.);
    c.shots.push(w.launch(
        Point::new(15., 10., 10.),
        Point::new(5., 10., 10.),
        2,
        Some(0),
    ));
    c.decoy(Point::new(10., 10., 10.), 20., 2);
    assert_eq!(c.shots.len(), 1);
    assert_eq!(c.shots[0].target, None);
    assert_eq!(c.shots[0].weapon.guidance, 0.);
}
#[test]
fn sky_gas_is_vented_and_continues_above_world() {
    let m = Materials::load_dir("materials");
    let mut v = room(&m);
    v.box_part(
        "smoke",
        m.id("smoke").unwrap(),
        [10, 8, 30],
        [4, 3, 2],
        None,
        &m,
    );
    v.step(&m);
    assert!(!v.plumes.is_empty());
    let first = v.plumes[0].at.z;
    for _ in 0..8 {
        v.step(&m);
    }
    assert!(v.plumes.iter().any(|p| p.at.z > 32. && p.at.z > first));
}
#[test]
fn barrel_carries_real_materials_and_spills_oil_on_a_hard_drop() {
    let m = Materials::load_dir("materials");
    let mut v = room(&m);
    let oil = m.id("oil").unwrap();
    v.box_part("oil", oil, [8, 8, 2], [3, 3, 4], Some([90, 40, 20]), &m);
    let mut c = Cargo::capture(1, &mut v, [8, 8, 2], [3, 3, 4]);
    assert_eq!(c.cells.len(), 36);
    assert_eq!(v.count(oil), 0);
    c.at = Point::new(20., 10., 25.);
    c.release(Point::default());
    for _ in 0..90 {
        c.step(&mut v, &m, None, 1. / 60.);
        if c.state == CargoState::Deployed {
            break;
        }
    }
    assert_eq!(c.state, CargoState::Deployed);
    assert_eq!(v.count(oil), 36);
    assert!(v.sections.iter().any(|s| {
        s.world
            .mat
            .iter()
            .enumerate()
            .any(|(i, id)| *id == oil && s.world.paint[i] != 0)
    }));
    assert_eq!(m.get(oil).state, State::Liquid);
}
#[test]
fn carried_materials_keep_burning_and_dont_freeze_their_timers() {
    use grave_engine::world::BURNING;
    let m = Materials::load_dir("materials");
    let mut v = room(&m);
    v.box_part(
        "timber",
        m.id("weathered_wood").unwrap(),
        [8, 8, 2],
        [3, 3, 4],
        None,
        &m,
    );
    let mut c = Cargo::capture(2, &mut v, [8, 8, 2], [3, 3, 4]);
    for (_, cell) in &mut c.cells {
        cell.temp = 750.;
        cell.flags |= BURNING;
        cell.burn = 100;
    }
    for _ in 0..20 {
        c.step(&mut v, &m, Some(Point::new(12., 10., 25.)), 1. / 60.);
    }
    assert!(
        c.cells
            .iter()
            .any(|(_, cell)| cell.burn > 0 && cell.burn < 100)
    );
    assert!(c.at.z >= 2., "the cable dragged the barrel through terrain");
}
#[test]
fn high_explosive_does_not_heat_the_entire_blast_to_ignition() {
    use grave_engine::overhead::Explosion;
    let m = Materials::load_dir("materials");
    let mut v = room(&m);
    v.box_part(
        "wood",
        m.id("weathered_wood").unwrap(),
        [4, 4, 2],
        [30, 12, 1],
        None,
        &m,
    );
    v.explode(Explosion {
        at: Point::new(18., 10., 4.),
        radius: 9.,
        power: 1000.,
        fire: None,
    });
    v.step(&m);
    let outer = Point::new(25., 10., 2.);
    let (y, i) = v.index(outer).unwrap();
    assert!(
        v.sections[y].world.temp[i] < 180.,
        "outer blast became a fire bomb"
    );
}
#[test]
fn empty_structure_bounds_do_not_stop_shots_and_burned_out_material_completes_target() {
    let m = Materials::load_dir("materials");
    let mut v = room(&m);
    let mut c = Combat::default();
    let mut t = Target::new(
        3,
        2,
        Bounds {
            min: Point::new(12., 8., 6.),
            max: Point::new(16., 12., 12.),
        },
        100.,
        0.,
    );
    t.material_backed = true;
    c.targets.push(t);
    v.box_part(
        "remote wall",
        m.id("wood").unwrap(),
        [12, 8, 6],
        [2, 4, 6],
        None,
        &m,
    );
    c.bind_materials(&v, &m);
    v.box_part("burnt away", 0, [12, 8, 6], [2, 4, 6], None, &m);
    v.box_part("oil", m.id("oil").unwrap(), [25, 8, 7], [2, 4, 4], None, &m);
    c.shots
        .push(weapon().launch(Point::new(6., 10., 9.), Point::new(29., 10., 9.), 1, None));
    for _ in 0..4 {
        c.step(&mut v, &m, 1. / 60.);
    }
    assert_eq!(
        c.target(3).unwrap().health,
        100.,
        "empty bounds took a shot"
    );
    assert!(
        v.sections
            .iter()
            .any(|s| s.world.temp.iter().any(|t| *t >= 240.)),
        "ghost structure intercepted oil shot"
    );
    c.environment(&mut v, &m);
    assert!(!c.alive(3), "material destruction did not complete target");
}
#[test]
fn projectile_range_clips_a_fast_sweep_before_distant_target() {
    let m = Materials::load_dir("materials");
    let mut v = room(&m);
    let mut c = Combat::default();
    c.targets.push(Target::new(
        9,
        2,
        Bounds {
            min: Point::new(20., 8., 8.),
            max: Point::new(23., 12., 12.),
        },
        50.,
        0.,
    ));
    let mut w = weapon();
    w.range = 5.;
    w.speed = 10000.;
    c.shots
        .push(w.launch(Point::new(4., 10., 10.), Point::new(25., 10., 10.), 1, None));
    for _ in 0..3 {
        c.step(&mut v, &m, 1. / 60.);
    }
    assert_eq!(c.target(9).unwrap().health, 50.);
    assert!(c.shots.is_empty());
}
#[test]
fn landing_preserves_burning_state_and_remaining_fuel() {
    use grave_engine::world::BURNING;
    let m = Materials::load_dir("materials");
    let mut v = room(&m);
    let id = m.id("weathered_wood").unwrap();
    v.box_part("load", id, [8, 8, 2], [3, 3, 4], None, &m);
    let mut c = Cargo::capture(4, &mut v, [8, 8, 2], [3, 3, 4]);
    for (_, cell) in &mut c.cells {
        cell.flags = BURNING;
        cell.burn = 150;
        cell.temp = 750.;
    }
    c.at = Point::new(20., 10., 7.);
    c.release(Point::default());
    for _ in 0..60 {
        c.step(&mut v, &m, None, 1. / 60.);
        if c.state == CargoState::Deployed {
            break;
        }
    }
    assert_eq!(c.state, CargoState::Deployed);
    assert!(
        v.sections
            .iter()
            .any(|s| s.world.mat.iter().enumerate().any(|(i, id2)| *id2 == id
                && s.world.flags[i] & BURNING != 0
                && s.world.burn[i] > 0
                && s.world.burn[i] < 150))
    );
}
