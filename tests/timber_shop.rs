//! Burnable cladding must remain timber, while the masonry behind stays brick.
use grave_engine::{Materials, asset::Asset, render::Renderer, scene, world::BURNING};
use std::path::Path;

#[test]
fn timber_shop_is_stable_and_fire_reveals_its_interior() {
    let m = Materials::load_dir("materials");
    let a = Asset::load_file(Path::new("assets/workshop/timber_shop.asset"), &m).unwrap();
    assert!(a.validate(&m).is_empty(), "{:?}", a.validate(&m));
    let wood = m.id("wood").unwrap();
    let brick = m.id("brick").unwrap();
    let facade = a.parts.iter().find(|p| p.name == "timber_facade").unwrap();
    assert!(facade.cells.iter().any(|c| *c == wood));
    assert!(!facade.cells.contains(&brick));
    assert!(
        a.parts
            .iter()
            .any(|p| p.layer == 2 && p.cells.contains(&brick))
    );
    let text = std::fs::read_to_string("scenes/timber-shop-night.txt").unwrap();
    let (mut s, p) = scene::load(&text, &m, Some(Path::new("scenes"))).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    for _ in 0..360 {
        s.step(&m);
    }
    assert_eq!(s.physics.body_count(), 16);
    let before = s.world.mat.iter().filter(|c| **c == wood).count();
    let mut caught = false;
    for frame in 0..1200 {
        if frame < 120 && frame % 20 == 0 {
            s.world.heat_circle(106, 131, 16, 450.);
        }
        s.step(&m);
        caught |= s.layers[0].world.flags.iter().any(|f| f & BURNING != 0);
    }
    let after = s.world.mat.iter().filter(|c| **c == wood).count();
    assert!(
        after < before * 3 / 4,
        "timber facade must burn away: {before}->{after}"
    );
    assert!(caught, "interior must catch from the outside fire");
    let mut r = Renderer::default();
    r.light(&s);
    grave_engine::headless::shot(
        &s.world,
        &m,
        &mut r,
        Path::new("shots/timber-shop-burn.png"),
        4,
        false,
    )
    .unwrap();
}
