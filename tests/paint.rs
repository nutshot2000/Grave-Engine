//! Cosmetic paint must survive editing/physics/saving without changing the material.
use grave_engine::{
    Materials, Sim,
    asset::{Asset, Brush, pack_paint},
    material::AIR,
    scene,
};
fn materials() -> Materials {
    Materials::load_dir("materials")
}
fn painted(m: &Materials) -> Asset {
    let mut a = Asset::parse("asset painted 32 24\npart box\nrect wood 2 2 28 20", m).unwrap();
    a.paint_surface(0, Brush::Rectangle, (2, 8), (29, 16), Some([195, 35, 30]));
    a.paint_text(0, (7, 9), "TNT", 1, [8, 8, 8]);
    a
}
#[test]
fn paint_and_letters_do_not_change_materials_and_roundtrip() {
    let m = materials();
    let plain = Asset::parse("asset painted 32 24\npart box\nrect wood 2 2 28 20", &m).unwrap();
    let mut a = painted(&m);
    assert_eq!(plain.composite(), a.composite());
    assert!(a.parts[0].paint.contains(&pack_paint([8, 8, 8])));
    assert_eq!(Asset::parse(&a.to_text(&m), &m).unwrap(), a);
    let before = a.parts[0].paint.clone();
    a.move_part(0, 1, 1);
    assert_eq!(a.parts[0].paint[10 * 32 + 8], before[9 * 32 + 7]);
    a.resize(40, 30).unwrap();
    assert_eq!(a.parts[0].paint[10 * 40 + 8], before[9 * 32 + 7]);
    a.paint_surface(0, Brush::Erase, (8, 10), (8, 10), None);
    assert_eq!(a.parts[0].paint[10 * 40 + 8], 0);
    assert_eq!(a.parts[0].cells[10 * 40 + 8], m.id("wood").unwrap());
}
#[test]
fn paint_fill_respects_color_boundaries_empty_cells_and_other_parts() {
    let m = materials();
    let mut a = painted(&m);
    a.paint_surface(0, Brush::Fill, (2, 8), (2, 8), Some([40, 60, 200]));
    assert_eq!(a.parts[0].paint[8 * 32 + 2], pack_paint([40, 60, 200]));
    assert_eq!(a.parts[0].paint[2 * 32 + 2], 0);
    assert_eq!(a.parts[0].paint[9 * 32 + 7], pack_paint([8, 8, 8]));
    a.paint_surface(0, Brush::Rectangle, (0, 0), (31, 23), Some([1, 2, 3]));
    assert_eq!(a.parts[0].paint[0], 0);
    let p = a.add_part("powder").unwrap();
    a.put(p, 2, 8, m.id("gunpowder").unwrap());
    assert_eq!(a.parts[0].paint[8 * 32 + 2], 0);
    a.paint_surface(0, Brush::Pencil, (2, 8), (2, 8), Some([1, 2, 3]));
    assert_eq!(a.parts[p].paint[8 * 32 + 2], 0);
}
#[test]
fn paint_follows_moving_bodies_snapshot_and_saved_scene() {
    let m = materials();
    let a = painted(&m);
    let mut s = Sim::empty(200, 160, &m);
    let ids = a.instantiate(&mut s, &m, 80., 50.).unwrap();
    let original = s.physics.spec_of(&s.world, ids[0].1).unwrap();
    s.physics.set_velocity(ids[0].1, 10., 0., 0.3);
    for _ in 0..20 {
        s.step(&m);
    }
    let moved = s.physics.spec_of(&s.world, ids[0].1).unwrap();
    assert_eq!(
        original
            .cells
            .iter()
            .flatten()
            .map(|c| c.paint)
            .collect::<Vec<_>>(),
        moved
            .cells
            .iter()
            .flatten()
            .map(|c| c.paint)
            .collect::<Vec<_>>()
    );
    assert_ne!(original.x, moved.x);
    s.world.set(&m, 4, 4, m.id("wood").unwrap());
    let i = s.world.idx(4, 4);
    s.world.paint[i] = pack_paint([3, 4, 5]);
    let snap = s.snapshot();
    let saved = scene::save(&snap, &m);
    let (mut loaded, problems) = scene::load(&saved, &m, None).into_sim(&m);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(loaded.world.paint[i], pack_paint([3, 4, 5]));
    let b = loaded.snapshot().bodies.remove(0);
    assert_eq!(
        b.cells
            .iter()
            .flatten()
            .map(|c| c.paint)
            .collect::<Vec<_>>(),
        moved
            .cells
            .iter()
            .flatten()
            .map(|c| c.paint)
            .collect::<Vec<_>>()
    );
    let (expanded, p) = scene::load(
        &format!("size 200 160\n{}", a.scene_text(&m, 80., 50.)),
        &m,
        None,
    )
    .into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    assert!(expanded.world.paint.contains(&pack_paint([8, 8, 8])));
}
#[test]
fn paint_does_not_stop_wood_burning_and_material_changes_clear_it() {
    let m = materials();
    let a = painted(&m);
    let mut s = Sim::empty(160, 120, &m);
    a.instantiate(&mut s, &m, 70., 45.).unwrap();
    s.world.heat_circle(70, 45, 24, 900.);
    for _ in 0..12 {
        s.step(&m);
    }
    assert!(
        s.world
            .flags
            .iter()
            .any(|v| v & grave_engine::world::BURNING != 0)
    );
    s.world.set(&m, 5, 5, m.id("wood").unwrap());
    let i = s.world.idx(5, 5);
    s.world.paint[i] = pack_paint([0, 0, 0]);
    s.world.transform(&m, i, m.id("ash").unwrap());
    assert_eq!(s.world.paint[i], 0);
    s.world.paint[i] = pack_paint([0, 0, 0]);
    s.world.make_air(i);
    assert_eq!(s.world.paint[i], 0);
    assert_eq!(s.world.mat[i], AIR);
}
#[test]
fn painted_tnt_box_contains_real_gunpowder_and_explodes() {
    let m = materials();
    let a = Asset::load_file(std::path::Path::new("assets/workshop/tnt_box.asset"), &m).unwrap();
    assert!(
        a.parts
            .iter()
            .any(|p| p.cells.contains(&m.id("gunpowder").unwrap()))
    );
    let mut s = Sim::empty(200, 160, &m);
    a.instantiate(&mut s, &m, 100., 60.).unwrap();
    grave_engine::headless::shot(
        &s.world,
        &m,
        &mut Default::default(),
        std::path::Path::new("shots/painted-tnt.png"),
        4,
        false,
    )
    .unwrap();
    s.world.heat_circle(100, 60, 24, 1000.);
    let mut blasts = 0;
    for _ in 0..60 {
        s.world.step(&m);
        blasts += s.world.blasts.len();
        s.physics.update(&mut s.world, &m);
    }
    assert!(blasts > 0);
}

#[test]
fn shattered_parts_and_thrown_cells_keep_their_paint() {
    let m = materials();
    let mut a = painted(&m);
    a.paint_surface(0, Brush::Rectangle, (0, 0), (31, 23), Some([30, 160, 70]));
    let color = pack_paint([30, 160, 70]);
    let mut s = Sim::empty(200, 160, &m);
    a.instantiate(&mut s, &m, 80., 50.).unwrap();
    for y in 0..160 {
        for x in 79..=80 {
            let i = s.world.idx(x, y);
            s.world.make_air(i);
        }
    }
    s.physics.update(&mut s.world, &m);
    assert!(s.physics.body_count() >= 2, "cut must separate the body");
    for id in s.physics.ids() {
        let b = s.physics.spec_of(&s.world, id).unwrap();
        assert!(b.cells.iter().flatten().all(|c| c.paint == color));
    }
    s.world.set(&m, 5, 5, m.id("wood").unwrap());
    let i = s.world.idx(5, 5);
    s.world.paint[i] = color;
    assert!(s.world.throw(&m, 5, 5, 0., -1.));
    assert_eq!(s.world.flying.last().unwrap().paint, color);
    assert_eq!(s.world.paint[i], 0);
}

#[test]
fn editor_clipboard_and_history_preserve_surface_paint() {
    use grave_engine::edit::{self, History, Region};
    let m = materials();
    let mut s = Sim::empty(60, 60, &m);
    s.world.set(&m, 5, 5, m.id("wood").unwrap());
    let i = s.world.idx(5, 5);
    let color = pack_paint([200, 20, 10]);
    let mut h = History::new();
    h.touch(&mut s, "Paint");
    s.world.paint[i] = color;
    h.commit(&mut s);
    h.undo(&mut s, &m);
    assert_eq!(s.world.paint[i], 0);
    h.redo(&mut s, &m);
    assert_eq!(s.world.paint[i], color);
    let r = Region::new(&s.world, 5, 5, 5, 5);
    let clip = edit::copy_region(&s.world, r);
    edit::paste_clip(&mut s.world, &clip, 10, 10);
    assert_eq!(s.world.paint[s.world.idx(10, 10)], color);
}
