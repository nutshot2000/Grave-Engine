//! The exterior shop is reusable, stable and uses real burnable materials.
use grave_engine::{Materials, Sim, asset::Asset, scene};
use std::path::Path;

#[test]
fn shop_front_is_reusable_stands_and_burns() {
    let m = Materials::load_dir("materials");
    let a = Asset::load_file(Path::new("assets/workshop/shop_front.asset"), &m).unwrap();
    assert!(a.validate(&m).is_empty(), "{:?}", a.validate(&m));
    assert_eq!(Asset::parse(&a.to_text(&m), &m).unwrap(), a);
    let mut s = Sim::empty(220, 155, &m);
    for y in 140..155 {
        for x in 0..220 {
            s.world.set(&m, x, y, m.id("stone").unwrap());
        }
    }
    a.instantiate(&mut s, &m, 110., 140.).unwrap();
    let joints = s.physics.joints().len();
    for _ in 0..240 {
        s.step(&m);
    }
    assert_eq!(
        s.physics.body_count(),
        a.parts.len(),
        "facade should stand without damage"
    );
    assert_eq!(
        s.physics.joints().len(),
        joints,
        "facade must hold together"
    );
    let saved = scene::save(&s.snapshot(), &m);
    let (_, problems) = scene::load(&saved, &m, None).into_sim(&m);
    assert!(problems.is_empty(), "{problems:?}");
    let cloth = m.id("cloth").unwrap();
    let count = s.world.mat.iter().filter(|v| **v == cloth).count();
    s.world.heat_circle(110, 94, 45, 1000.);
    let mut burning = false;
    for frame in 0..480 {
        if frame < 180 && frame % 30 == 0 {
            s.world.heat_circle(110, 94, 60, 1000.);
        }
        s.step(&m);
        burning |= s
            .world
            .flags
            .iter()
            .any(|f| f & grave_engine::world::BURNING != 0);
    }
    assert!(burning);
    grave_engine::headless::shot(
        &s.world,
        &m,
        &mut Default::default(),
        Path::new("shots/shop-burn-check.png"),
        4,
        false,
    )
    .unwrap();
    assert!(
        s.world.mat.iter().filter(|v| **v == cloth).count() < count * 9 / 10,
        "exposed fabric must be consumed by fire"
    );
}
