//! Frontier kit roundtrips, usable layer diagnostics, and real fire/water behavior.
use grave_engine::{
    Materials, Sim,
    asset::{Asset, Brush},
    world::BURNING,
};

#[test]
fn western_assets_are_valid_reusable_named_material_assemblies() {
    let m = Materials::load_dir("materials");
    assert!(m.problems.is_empty(), "{:?}", m.problems);
    for name in [
        "ranch",
        "store",
        "saloon",
        "sheriff",
        "barrel",
        "powder_keg",
        "hay_bale",
        "wagon",
        "hitching_rail",
        "water_trough",
        "lantern",
        "water_tower",
        "dry_tree",
    ] {
        let path = std::path::PathBuf::from(format!("assets/workshop/west_{name}.asset"));
        let a = Asset::load_file(&path, &m).unwrap();
        assert!(a.validate(&m).is_empty(), "{name}: {:?}", a.validate(&m));
        assert_eq!(a, Asset::parse(&a.to_text(&m), &m).unwrap());
        assert!(a.parts.len() <= grave_engine::asset::MAX_PARTS);
        assert!(
            a.parts
                .iter()
                .any(|p| p.cells.contains(&m.id("weathered_wood").unwrap()))
                || name == "lantern"
                || name == "hay_bale"
        );
    }
    let a = Asset::load_file(
        std::path::Path::new("assets/workshop/west_saloon.asset"),
        &m,
    )
    .unwrap();
    assert_eq!(a.layers.len(), 4);
    for name in [
        "balcony_rail",
        "bar_counter",
        "upright_piano",
        "upper_timber_front",
    ] {
        assert!(a.parts.iter().any(|p| p.name == name));
    }
}
#[test]
fn separate_depth_planes_do_not_require_impossible_cross_plane_welds() {
    let m = Materials::load_dir("materials");
    let wood = m.id("wood").unwrap();
    let mut a = Asset::blank("layered_test", 8, 8).unwrap();
    a.draw(0, Brush::Rectangle, (1, 1), (5, 5), wood);
    a.layers.push("rear".into());
    let p = a.add_part("rear_part").unwrap();
    a.parts[p].layer = 1;
    a.draw(p, Brush::Rectangle, (1, 1), (5, 5), wood);
    assert!(a.validate(&m).is_empty());
    a.parts[p].layer = 0;
    assert!(a.validate(&m).iter().any(|s| s.contains("unjoined")));
}
#[test]
fn weathered_timber_keeps_wood_physics_and_dry_straw_burns() {
    let m = Materials::load_dir("materials");
    let (a, b) = (
        m.get(m.id("wood").unwrap()),
        m.get(m.id("weathered_wood").unwrap()),
    );
    assert_eq!(a.density, b.density);
    assert!(b.catches_fire_at >= a.catches_fire_at);
    assert!(b.burns_for >= a.burns_for * 3);
    assert_eq!(a.toughness, b.toughness);
    assert_eq!(a.conducts_heat, b.conducts_heat);
    let text = "size 60 40\nfill air\nrect stone 0 35 60 5\nrect straw 20 25 14 10\nheat rect 0 0 60 35 450\n";
    let (mut s, p) = grave_engine::scene::load(text, &m, None).into_sim(&m);
    assert!(p.is_empty());
    let straw = m.id("straw").unwrap();
    let before = s.world.mat.iter().filter(|v| **v == straw).count();
    let mut burning = false;
    for _ in 0..400 {
        s.step(&m);
        burning |= s.world.flags.iter().any(|v| v & BURNING != 0);
    }
    assert!(burning);
    assert!(s.world.mat.iter().filter(|v| **v == straw).count() < before);
}
#[test]
fn burning_the_water_tower_legs_releases_its_water() {
    let m = Materials::load_dir("materials");
    let a = Asset::load_file(
        std::path::Path::new("assets/workshop/west_water_tower.asset"),
        &m,
    )
    .unwrap();
    let mut s = Sim::empty(100, 145, &m);
    let stone = m.id("stone").unwrap();
    for y in 130..145 {
        for x in 0..100 {
            s.world.set(&m, x, y, stone);
        }
    }
    a.instantiate(&mut s, &m, 50., 130.).unwrap();
    let water = m.id("water").unwrap();
    for y in 49..66 {
        for x in 33..67 {
            s.layers[0].world.set(&m, x, y, water);
        }
    }
    for _ in 0..90 {
        s.step(&m);
    }
    assert!(
        (80..130).all(|y| (0..100).all(|x| s.layers[0].world.mat[y * 100 + x] != water)),
        "water stays in the intact tank"
    );
    let mut spilled = false;
    for frame in 0..720 {
        if frame < 180 && frame % 15 == 0 {
            for x in [33, 67] {
                s.world.heat_circle(x, 105, 9, 950.);
            }
        }
        s.step(&m);
        spilled |= (80..130).any(|y| (0..100).any(|x| s.layers[0].world.mat[y * 100 + x] == water));
    }
    assert!(
        spilled,
        "water should fall out when the heated supports give way"
    );
}

#[test]
fn building_cladding_uses_small_individually_joined_boards() {
    let m = Materials::load_dir("materials");
    for name in ["ranch", "store", "saloon", "sheriff"] {
        let a = Asset::load_file(
            std::path::Path::new(&format!("assets/workshop/west_{name}.asset")),
            &m,
        )
        .unwrap();
        assert!(a.parts.len() > 100);
        for p in a
            .parts
            .iter()
            .filter(|p| p.name.contains("timber_front") || p.layer == 3)
        {
            let cells: Vec<_> = p
                .cells
                .iter()
                .enumerate()
                .filter(|(_, c)| **c != 0)
                .map(|(i, _)| i)
                .collect();
            assert!(
                cells.iter().map(|i| i % a.w).max().unwrap()
                    - cells.iter().map(|i| i % a.w).min().unwrap()
                    < 40
            );
            assert!(
                cells.iter().map(|i| i / a.w).max().unwrap()
                    - cells.iter().map(|i| i / a.w).min().unwrap()
                    < 6
            );
            assert!(a.welds.iter().any(|j| j.a == p.name || j.b == p.name));
        }
        let bodies = a.bodies(&m, 100., 100.);
        let allocated: usize = bodies.iter().map(|(_, b)| b.w * b.h).sum();
        assert!(
            allocated < a.parts.iter().filter(|p| p.layer == 0).count() * a.w * a.h / 3,
            "runtime boards must be cropped"
        );
    }
}
