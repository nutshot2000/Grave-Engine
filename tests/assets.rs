//! Material assets: text roundtrips, named assemblies and real destructive tests.
use grave_engine::{
    Materials, Sim,
    asset::{Asset, Brush},
    material::AIR,
    scene,
};
fn mats() -> Materials {
    Materials::load_dir("materials")
}
fn sample(m: &Materials) -> Asset {
    Asset::load_file(std::path::Path::new("assets/workshop/fuel_crate.asset"), m).unwrap()
}
#[test]
fn roundtrip_preserves_named_parts_materials_pivot_depth_and_welds() {
    let m = mats();
    let mut a = sample(&m);
    a.pivot = (12.5, 9.);
    let text = a.to_text(&m);
    assert_eq!(a, Asset::parse(&text, &m).unwrap());
    assert_eq!(a.parts.len(), 3);
    assert_eq!(a.welds.len(), 2);
    assert_eq!(a.parts[2].name, "tank");
}
#[test]
fn shapes_clip_and_each_cell_has_one_owner() {
    let m = mats();
    let mut a = Asset::blank("prop", 16, 12).unwrap();
    let wood = m.id("wood").unwrap();
    let glass = m.id("glass").unwrap();
    a.draw(0, Brush::Rectangle, (2, 2), (13, 9), wood);
    let window = a.add_part("window").unwrap();
    a.draw(window, Brush::Circle, (4, 3), (10, 8), glass);
    for i in 0..a.w * a.h {
        assert!(!(a.parts[0].cells[i] != AIR && a.parts[1].cells[i] != AIR));
    }
    let before = a.composite();
    a.draw(window, Brush::Rectangle, (-20, -20), (-2, -2), glass);
    assert_eq!(before, a.composite());
    assert!(a.parts[window].cells.iter().any(|c| *c == glass));
}
#[test]
fn malformed_assets_fail_without_allocating_unbounded_grids() {
    let m = mats();
    for text in [
        "asset x 999999 2",
        "asset x -1 3",
        "asset ../x 3 3",
        "asset x 3 3\npart f\nrect missing 0 0 2 2",
        "asset x 3 3\npart f\ngrid\n...\nend",
        "asset x 3 3\nweld f missing 1 1 200",
        "asset x 3 3\ndepth NaN",
        "asset x 3 3\nrect wood -2147483648 1 1 1",
    ] {
        assert!(Asset::parse(text, &m).is_err(), "must reject {text}");
    }
}
#[test]
fn renames_keep_weld_references_and_removal_cleans_them() {
    let m = mats();
    let mut a = sample(&m);
    a.rename_part(0, "chassis").unwrap();
    assert!(a.welds.iter().all(|j| j.a == "chassis"));
    assert!(a.rename_part(1, "chassis").is_err());
    a.remove_part(2);
    assert_eq!(a.welds.len(), 1);
    assert_eq!(Asset::parse(&a.to_text(&m), &m).unwrap(), a);
}
#[test]
fn parts_instantiate_as_real_bodies_and_tests_cannot_mutate_the_asset() {
    let m = mats();
    let a = sample(&m);
    let original = a.clone();
    let mut s = Sim::empty(200, 160, &m);
    let ids = a.instantiate(&mut s, &m, 100., 60.).unwrap();
    assert_eq!(ids.len(), 3);
    assert_eq!(s.physics.joints().len(), 2);
    let fuel = m.id("gunpowder").unwrap();
    assert!(
        s.physics
            .spec_of(&s.world, ids[2].1)
            .unwrap()
            .cells
            .iter()
            .flatten()
            .any(|c| c.mat == fuel)
    );
    s.world.heat_circle(100, 60, 40, 1000.);
    let mut blasts = 0;
    for _ in 0..60 {
        s.world.step(&m);
        blasts += s.world.blasts.len();
        s.physics.update(&mut s.world, &m);
    }
    assert!(blasts > 0);
    assert_eq!(a, original);
    assert!(s.physics.joints().len() < 2 || s.physics.body_count() != 3);
    grave_engine::headless::shot(
        &s.world,
        &m,
        &mut Default::default(),
        std::path::Path::new("shots/workshop-burn.png"),
        3,
        false,
    )
    .unwrap();
}
#[test]
fn scene_asset_command_offsets_multiple_assemblies_and_resolves_part_names() {
    let m = mats();
    let text = "size 300 180\nfill air\nbody rect iron 20 20 5 5\nname support\nasset assets/workshop/fuel_crate.asset 100 60 as a\nasset assets/workshop/window_frame.asset 200 60 as b\nweld support a.base at 50 60\n";
    let (s, p) = scene::load(text, &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    assert_eq!(s.physics.body_count(), 7);
    assert_eq!(s.physics.joints().len(), 5);
}
#[test]
fn plain_wood_burns_and_brittle_glass_breaks_under_impact() {
    let m = mats();
    let a = Asset::parse(
        "asset plank 32 18\npart frame\nrect wood 2 4 28 10\ndepth 0.03",
        &m,
    )
    .unwrap();
    let mut burn = Sim::empty(160, 120, &m);
    a.instantiate(&mut burn, &m, 60., 45.).unwrap();
    burn.world.heat_circle(60, 45, 25, 900.);
    for _ in 0..12 {
        burn.step(&m);
    }
    assert!(
        burn.world
            .flags
            .iter()
            .any(|f| f & grave_engine::world::BURNING != 0)
    );
    let mut hit = Sim::empty(180, 120, &m);
    let stone = m.id("stone").unwrap();
    for y in 0..120 {
        for x in 110..116 {
            hit.world.set(&m, x, y, stone);
        }
    }
    let mut glass = a.clone();
    for part in &mut glass.parts {
        for mat in &mut part.cells {
            if *mat != AIR {
                *mat = m.id("glass").unwrap();
            }
        }
    }
    let ids = glass.instantiate(&mut hit, &m, 55., 50.).unwrap();
    for _ in 0..10 {
        hit.step(&m);
    }
    hit.physics.set_velocity(ids[0].1, 1000., 0., 0.);
    for _ in 0..35 {
        hit.step(&m);
    }
    let remaining = hit
        .physics
        .spec_of(&hit.world, ids[0].1)
        .map_or(0, |b| b.cell_count());
    assert!(
        remaining < 280 || hit.physics.body_count() > 1,
        "wall collision must damage brittle glass"
    );
}

#[test]
fn file_save_reload_works_for_ai_edits() {
    let m = mats();
    let a = sample(&m);
    let path = std::env::temp_dir().join(format!(
        "grave-asset-roundtrip-{}.asset",
        std::process::id()
    ));
    a.save(&path, &m).unwrap();
    let b = Asset::load_file(&path, &m).unwrap();
    assert_eq!(a, b);
    std::fs::remove_file(path).unwrap();
}
