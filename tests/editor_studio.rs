//! Complete multi-plane asset operations and reusable authoring tools.
use grave_engine::{
    Materials, Sim, assemblies,
    asset::Asset,
    building::Building,
    edit::History,
    scene,
    test_bay::{Recipe, TestKind},
};
use std::path::Path;
fn mats() -> Materials {
    Materials::load_dir("materials")
}
fn sample(m: &Materials) -> Asset {
    Asset::load_file(Path::new("assets/workshop/layered_shop.asset"), m).unwrap()
}
#[test]
fn group_transform_copy_delete_and_undo_include_all_layers_and_joints() {
    let m = mats();
    let a = sample(&m);
    let mut s = Sim::empty(600, 300, &m);
    let front = a.instantiate(&mut s, &m, 150., 200.).unwrap();
    let id = front[0].1;
    let mut lamp = grave_engine::light::Light::at(0., 0., 35.);
    lamp.on = Some(id);
    s.lighting.lights.push(lamp);
    let group = s.physics.assembly(id).unwrap().to_string();
    let members = assemblies::members(&s, &[id], true);
    assert_eq!(members.iter().map(Vec::len).sum::<usize>(), 23);
    let initial: Vec<_> = members
        .iter()
        .enumerate()
        .flat_map(|(z, ids)| ids.iter().map(move |id| (z, *id)))
        .map(|(z, id)| {
            let b = assemblies::plane(&s, z)
                .physics
                .spec_of(&assemblies::plane(&s, z).world, id)
                .unwrap();
            (z, id, b.x, b.y)
        })
        .collect();
    assemblies::transform(&mut s, &m, &[id], true, (40., -10.), 0., (0., 0.));
    for (z, id, x, y) in initial {
        let p = assemblies::plane(&s, z);
        let b = p.physics.spec_of(&p.world, id).unwrap();
        assert!((b.x - x - 40.).abs() < 0.02);
        assert!((b.y - y + 10.).abs() < 0.02);
    }
    let joints: usize = (0..=s.layers.len())
        .map(|z| assemblies::plane(&s, z).physics.joints().len())
        .sum();
    let clip = assemblies::Clip::capture(&s, &[id], true);
    let new = clip.paste(&mut s, &m, 400., 150.);
    assert_ne!(s.physics.assembly(new[0]).unwrap(), group);
    assert_eq!(assemblies::names(&s).len(), 2);
    assert_eq!(s.lighting.lights.len(), 2);
    assert!(new.contains(&s.lighting.lights[1].on.unwrap()));
    assert_eq!(
        (0..=s.layers.len())
            .map(|z| assemblies::plane(&s, z).physics.joints().len())
            .sum::<usize>(),
        joints * 2
    );
    let mut h = History::new();
    h.touch(&mut s, "delete complete asset");
    assemblies::remove(&mut s, &new, true);
    h.commit(&mut s);
    assert_eq!(assemblies::names(&s).len(), 1);
    h.undo(&mut s, &m);
    assert_eq!(assemblies::names(&s).len(), 2);
    let (loaded, p) = scene::load(&scene::save(&s.snapshot(), &m), &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    assert_eq!(assemblies::names(&loaded).len(), 2);
}
#[test]
fn scene_loaded_groups_rotate_and_ground_anchors_follow_the_building() {
    let m = mats();
    let text = std::fs::read_to_string("scenes/layered-shop-night.txt").unwrap();
    let (mut s, p) = scene::load(&text, &m, Some(Path::new("scenes"))).into_sim(&m);
    assert!(p.is_empty());
    let id = s.physics.ids()[0];
    assert_eq!(s.physics.assembly(id), Some("shop"));
    assert_eq!(
        s.layers[0].physics.assembly(s.layers[0].physics.ids()[0]),
        Some("shop")
    );
    let before = s
        .physics
        .joints()
        .into_iter()
        .find(|j| j.a == 0)
        .unwrap()
        .anchor_a;
    assemblies::transform(&mut s, &m, &[id], true, (10., 0.), 0., (0., 0.));
    let after = s
        .physics
        .joints()
        .into_iter()
        .find(|j| j.a == 0)
        .unwrap()
        .anchor_a;
    assert!((after.0 - before.0 - 10.).abs() < 0.01);
    let pivot = (170., 128.);
    assemblies::transform(&mut s, &m, &[id], true, (0., 0.), 0.2, pivot);
    for z in 0..=s.layers.len() {
        let p = assemblies::plane(&s, z);
        for id in p.physics.ids() {
            if p.physics.assembly(id) == Some("shop") {
                let b = p.physics.spec_of(&p.world, id).unwrap();
                assert!((b.angle - 0.2).abs() < 0.01);
            }
        }
    }
}
#[test]
fn building_designer_produces_valid_named_material_parts() {
    let m = mats();
    for stories in 1..=3 {
        for facade in [false, true] {
            let b = Building {
                storeys: stories,
                facade,
                ..Default::default()
            };
            let a = b.build(&m).unwrap();
            assert!(
                a.validate(&m).is_empty(),
                "{stories} {facade}: {:?}",
                a.validate(&m)
            );
            assert_eq!(a.layers.len(), if facade { 2 } else { 1 });
            for z in 0..a.layers.len() {
                for i in 0..a.w * a.h {
                    assert!(
                        a.parts
                            .iter()
                            .filter(|p| p.layer == z && p.cells[i] != 0)
                            .count()
                            <= 1,
                        "overlapping parts"
                    );
                }
            }
            assert_eq!(a, Asset::parse(&a.to_text(&m), &m).unwrap());
            assert!(
                a.parts
                    .iter()
                    .any(|p| p.cells.contains(&m.id("glass").unwrap()))
            );
        }
    }
}
#[test]
fn saved_test_recipes_are_repeatable_and_never_change_the_source() {
    let m = mats();
    let a = Asset::load_file(Path::new("assets/workshop/fuel_crate.asset"), &m).unwrap();
    let original = a.clone();
    let r = Recipe {
        kind: TestKind::Burn,
        frames: 120,
        ..Default::default()
    };
    let parsed = Recipe::parse(&r.text()).unwrap();
    assert_eq!(parsed.kind, r.kind);
    assert_eq!(parsed.asset, r.asset);
    let mut one = r.start(&a, &m).unwrap();
    let mut two = parsed.start(&a, &m).unwrap();
    for _ in 0..120 {
        one.step(&m);
        two.step(&m);
    }
    assert_eq!(one.world.mat, two.world.mat);
    assert_eq!(one.world.temp, two.world.temp);
    assert_eq!(original, a);
    for kind in TestKind::ALL {
        assert!(
            Recipe {
                kind,
                ..Default::default()
            }
            .start(&a, &m)
            .is_ok()
        );
    }
    assert!(Recipe::parse("test 1\nname ../unsafe\nkind Burn").is_err());
}
