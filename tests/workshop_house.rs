//! The reusable house must stand intact, roundtrip and suffer real fire collapse.
use grave_engine::{Materials, Sim, asset::Asset, scene};
use std::path::Path;

#[test]
fn reusable_house_stands_then_burns_and_collapses() {
    let m = Materials::load_dir("materials");
    let a = Asset::load_file(Path::new("assets/workshop/two_storey_house.asset"), &m).unwrap();
    assert!(a.validate(&m).is_empty(), "{:?}", a.validate(&m));
    assert_eq!(Asset::parse(&a.to_text(&m), &m).unwrap(), a);
    assert_eq!(a.parts.len(), 24);
    for material in ["wood", "brick", "glass", "stone", "cloth"] {
        assert!(
            a.parts
                .iter()
                .any(|p| p.cells.contains(&m.id(material).unwrap()))
        );
    }
    let mut s = Sim::empty(240, 180, &m);
    for y in 160..180 {
        for x in 0..240 {
            s.world.set(&m, x, y, m.id("stone").unwrap());
        }
    }
    let names = a.instantiate(&mut s, &m, 120., 160.).unwrap();
    let attic = names.iter().find(|(n, _)| n == "attic_floor").unwrap().1;
    let initial_y = s.physics.spec_of(&s.world, attic).unwrap().y;
    let original_joints = s.physics.joints().len();
    for _ in 0..300 {
        s.step(&m);
    }
    grave_engine::headless::shot(
        &s.world,
        &m,
        &mut Default::default(),
        Path::new("shots/workshop-house-standing.png"),
        5,
        false,
    )
    .unwrap();
    assert_eq!(
        s.physics.body_count(),
        24,
        "house should not shed parts without damage"
    );
    assert_eq!(
        s.physics.joints().len(),
        original_joints,
        "house should hold together under gravity"
    );
    assert!((s.physics.spec_of(&s.world, attic).unwrap().y - initial_y).abs() < 3.);
    let saved = scene::save(&s.snapshot(), &m);
    let (_, problems) = scene::load(&saved, &m, None).into_sim(&m);
    assert!(problems.is_empty(), "{problems:?}");
    let wood = m.id("wood").unwrap();
    let before = s.world.mat.iter().filter(|v| **v == wood).count();
    let mut ignition = false;
    for frame in 0..720 {
        if frame < 180 && frame % 15 == 0 {
            for x in [68, 120, 172] {
                s.world.heat_circle(x, 141, 15, 950.);
            }
        }
        s.step(&m);
        ignition |= s
            .world
            .flags
            .iter()
            .any(|f| f & grave_engine::world::BURNING != 0);
        if frame == 90 {
            grave_engine::headless::shot(
                &s.world,
                &m,
                &mut Default::default(),
                Path::new("shots/workshop-house-burning.png"),
                5,
                false,
            )
            .unwrap();
        }
    }
    grave_engine::headless::shot(
        &s.world,
        &m,
        &mut Default::default(),
        Path::new("shots/workshop-house-collapsed.png"),
        5,
        false,
    )
    .unwrap();
    let after = s.world.mat.iter().filter(|v| **v == wood).count();
    assert!(ignition, "real timber must ignite");
    assert!(
        after < before * 3 / 4,
        "substantial timber must burn away: {before} -> {after}"
    );
    assert!(
        s.physics.joints().len() < original_joints,
        "burning supports must break attachments"
    );
    assert!(
        s.physics
            .spec_of(&s.world, attic)
            .is_none_or(|b| b.y > initial_y + 8. || b.angle.abs() > 0.15),
        "upper structure must fall after support fire"
    );
}
