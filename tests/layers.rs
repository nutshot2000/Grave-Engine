//! Physical depth planes: overlap, occlusion, heat, persistence and real shop damage.
use grave_engine::{
    Materials, Sim, World,
    asset::{Asset, Brush, pack_paint},
    edit::{History, Timeline},
    material::AIR,
    render::{Renderer, View},
    scene,
};
use std::path::Path;

#[test]
fn night_shop_retains_per_plane_lighting_and_projected_sign_lamps() {
    let m=mats();
    let text=std::fs::read_to_string("scenes/layered-shop-night.txt").unwrap();
    let (mut s,p)=scene::load(&text,&m,Some(Path::new("scenes"))).into_sim(&m);
    assert!(p.is_empty(),"{p:?}");assert_eq!(s.layers.len(),2);
    assert!(s.lighting.ambient<0.5);
    assert!(s.layers[0].lighting.ambient>s.lighting.ambient);
    assert_eq!(s.lighting.lights.iter().filter(|l|!l.shadows).count(),2);
    let lamps=s.lamps();assert!(lamps.iter().any(|l|(l.x-132.).abs()<1. && (l.y-110.).abs()<1.));
    for _ in 0..360 {s.step(&m);}
    assert_eq!(s.physics.body_count(),16,"night frontage must remain intact before ignition");
    let (copy,p)=scene::load(&scene::save(&s.snapshot(),&m),&m,None).into_sim(&m);
    assert!(p.is_empty(),"{p:?}");
    assert_eq!(copy.lighting.lights.iter().filter(|l|!l.shadows).count(),2);
    assert_eq!(copy.layers[0].lighting,s.layers[0].lighting);
    let mut r=Renderer::default();r.light(&s);let pixels=r.draw(&s.world,&m,View::default());
    assert!(pixels.chunks_exact(4).any(|p|p[0]>100 && p[0]>p[2]));
}
fn mats() -> Materials {
    Materials::load_dir("materials")
}

#[test]
fn depth_faces_supply_air_but_sealed_interiors_still_need_an_opening() {
    let m=mats();let iron=m.id("iron").unwrap();let wood=m.id("wood").unwrap();
    let mut s=Sim::empty(9,9,&m);s.world.fill(&m,iron);
    s.ensure_layer(1,"inside",&m);s.ensure_layer(2,"rear",&m);
    let i=s.world.idx(4,4);
    s.layers[0].world.set(&m,4,4,wood);s.layers[0].world.temp[i]=900.;s.step(&m);
    assert_eq!(s.layers[0].world.flags[i]&grave_engine::world::BURNING,0,"solid neighbouring planes must still smother enclosed wood");
    s.world.make_air(i);s.layers[0].world.temp[i]=900.;s.step(&m);
    assert_ne!(s.layers[0].world.flags[i]&grave_engine::world::BURNING,0,"an opening in the front must admit air to hot interior wood");
    s.world.set(&m,4,4,wood);s.world.temp[i]=900.;s.step(&m);
    assert_ne!(s.world.flags[i]&grave_engine::world::BURNING,0,"the exterior face touches outside air even when its flat neighbours are solid");
}

#[test]
fn overlapping_parts_are_independent_and_legacy_assets_still_work() {
    let m = mats();
    let mut a = Asset::blank("layers", 16, 16).unwrap();
    a.draw(0, Brush::Rectangle, (2, 2), (12, 12), m.id("wood").unwrap());
    a.add_layer("inside").unwrap();
    let p = a.add_part("counter").unwrap();
    a.parts[p].layer = 1;
    a.draw(p, Brush::Rectangle, (2, 2), (12, 12), m.id("iron").unwrap());
    assert_eq!(a.parts[0].cells[3 * 16 + 3], m.id("wood").unwrap());
    assert_eq!(a, Asset::parse(&a.to_text(&m), &m).unwrap());
    let mut s = Sim::empty(60, 60, &m);
    a.instantiate(&mut s, &m, 25., 25.).unwrap();
    assert_eq!(s.physics.body_count(), 1);
    assert_eq!(s.layers[0].physics.body_count(), 1);
    assert_eq!(
        Asset::load_file(Path::new("assets/workshop/tnt_box.asset"), &m)
            .unwrap()
            .layers,
        vec!["front"]
    );
    assert!(Asset::parse("asset bad 4 4\npart a\nrect wood 0 0 2 2\nlayer inside\npart b\nrect iron 0 0 2 2\nweld a b 1 1 20",&m).is_err());
}
#[test]
fn heat_exchange_is_bidirectional_conservative_and_respects_insulation() {
    let m = mats();
    let mut a = World::new(8, 8);
    let mut b = World::new(8, 8);
    a.set(&m, 3, 3, m.id("wood").unwrap());
    b.set(&m, 3, 3, m.id("iron").unwrap());
    let i = a.idx(3, 3);
    a.temp[i] = 1000.;
    b.temp[i] = 20.;
    let (ca, cb) = (m.get(a.mat[i]).heat_capacity, m.get(b.mat[i]).heat_capacity);
    let energy = ca * a.temp[i] + cb * b.temp[i];
    grave_engine::layers::exchange_heat(&mut a, &mut b, &m);
    assert!(a.temp[i] < 1000. && b.temp[i] > 20.);
    assert!((ca * a.temp[i] + cb * b.temp[i] - energy).abs() < 0.01);
    a.temp[i] = 20.;
    b.temp[i] = 1000.;
    grave_engine::layers::exchange_heat(&mut a, &mut b, &m);
    assert!(a.temp[i] > 20.);
    let mut insulated = m.clone();
    insulated.list[a.mat[i] as usize].conducts_heat = 0.;
    let temp = a.temp[i];
    grave_engine::layers::exchange_heat(&mut a, &mut b, &insulated);
    assert_eq!(a.temp[i], temp);
}
#[test]
fn flames_ignite_the_other_plane_without_painting_fire_into_it() {
    let m = mats();
    let mut s = Sim::empty(40, 40, &m);
    s.ensure_layer(1, "inside", &m);
    let wood = m.id("wood").unwrap();
    for y in 12..25 {
        for x in 12..25 {
            s.world.set(&m, x, y, wood);
            s.layers[0].world.set(&m, x, y, wood);
        }
    }
    let mut ignition = false;
    for f in 0..240 {
        if f < 90 && f % 10 == 0 {
            s.world.heat_circle(18, 18, 10, 500.);
        }
        s.step(&m);
        ignition |= s.layers[0]
            .world
            .flags
            .iter()
            .any(|f| f & grave_engine::world::BURNING != 0);
    }
    assert!(
        ignition,
        "front-only heating must ignite interior timber through layer heat transfer"
    );
}
#[test]
fn front_holes_reveal_rear_material_and_cropped_rendering_agrees() {
    let m = mats();
    let mut s = Sim::empty(16, 16, &m);
    s.ensure_layer(1, "inside", &m);
    let i = s.world.idx(8, 8);
    s.world.set(&m, 8, 8, m.id("wood").unwrap());
    s.world.paint[i] = pack_paint([220, 10, 10]);
    s.layers[0].world.set(&m, 8, 8, m.id("iron").unwrap());
    s.layers[0].world.paint[i] = pack_paint([10, 10, 220]);
    let mut r = Renderer::default();
    r.light(&s);
    let front = r.draw(&s.world, &m, View::default())[i * 4..i * 4 + 3].to_vec();
    assert!(front[0] > front[2]);
    s.world.make_air(i);
    r.light(&s);
    let revealed = r.draw(&s.world, &m, View::default())[i * 4..i * 4 + 3].to_vec();
    assert!(revealed[2] > revealed[0]);
    assert_eq!(
        r.draw_window(&s.world, &m, View::default(), (8, 8, 1, 1))[..3],
        revealed
    );
}
#[test]
fn layers_survive_scene_save_reset_history_and_rewind() {
    let m = mats();
    let mut s = Sim::empty(30, 30, &m);
    let mut h = History::new();
    h.touch(&mut s, "Add interior");
    s.ensure_layer(1, "inside", &m)
        .world
        .set(&m, 10, 10, m.id("wood").unwrap());
    h.commit(&mut s);
    h.undo(&mut s, &m);
    assert!(s.layers.is_empty());
    h.redo(&mut s, &m);
    assert_eq!(s.layers.len(), 1);
    let mut t = Timeline::new();
    t.every = 1;
    t.record(&mut s);
    let i = s.world.idx(10, 10);
    s.layers[0].world.temp[i] = 700.;
    t.record(&mut s);
    assert!(t.restore(0, &mut s, &m));
    assert!(s.layers[0].world.temp[i] < 100.);
    let snap = s.snapshot();
    let mut restored = Sim::restore(&snap, &m);
    assert_eq!(restored.layer_names, vec!["inside"]);
    let (loaded, p) = scene::load(&scene::save(&restored.snapshot(), &m), &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    assert_eq!(loaded.layers.len(), 1);
    assert_eq!(loaded.layers[0].world.mat[i], m.id("wood").unwrap());
    let bad = scene::load(
        "size 20 20\nplane 1 inside\nsize 20 20\nplane 1 nested\nsize 20 20\nendplane\nendplane",
        &m,
        None,
    );
    let (_, p) = bad.into_sim(&m);
    assert!(!p.is_empty());
}
#[test]
fn layered_shop_stands_then_front_fire_spreads_to_stock_and_reveals_interior() {
    let m = mats();
    let a = Asset::load_file(Path::new("assets/workshop/layered_shop.asset"), &m).unwrap();
    assert!(a.validate(&m).is_empty(), "{:?}", a.validate(&m));
    assert_eq!(Asset::parse(&a.to_text(&m), &m).unwrap(), a);
    let text = std::fs::read_to_string("scenes/layered-shop.txt").unwrap();
    let (mut s, p) = scene::load(&text, &m, Some(Path::new("scenes"))).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    assert_eq!(s.layers.len(), 2);
    let count: Vec<_> = s.layers.iter().map(|l| l.physics.body_count()).collect();
    for _ in 0..240 {
        s.step(&m);
    }
    assert_eq!(s.physics.body_count(), 16);
    assert_eq!(
        s.layers
            .iter()
            .map(|l| l.physics.body_count())
            .collect::<Vec<_>>(),
        count
    );
    let mut r = Renderer::default();
    r.light(&s);
    grave_engine::headless::shot(
        &s.world,
        &m,
        &mut r,
        Path::new("shots/layered-shop-before.png"),
        5,
        false,
    )
    .unwrap();
    let snap = s.snapshot();
    let (copy, p) = scene::load(&scene::save(&snap, &m), &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    assert_eq!(copy.layers.len(), 2);
    let mut caught = false;
    let mut exposed = false;
    for f in 0..1500 {
        if f < 120 && f % 20 == 0 {
            s.world.heat_circle(78, 116, 14, 450.);
        }
        s.step(&m);
        caught |= s.layers[0]
            .world
            .flags
            .iter()
            .any(|v| v & grave_engine::world::BURNING != 0);
        for y in 100..135 {
            for x in 62..100 {
                let i = s.world.idx(x, y);
                if s.world.mat[i] == AIR && s.layers[0].world.mat[i] != AIR {
                    exposed = true;
                }
            }
        }
        if f == 180 {
            r.light(&s);
            grave_engine::headless::shot(
                &s.world,
                &m,
                &mut r,
                Path::new("shots/layered-shop-burning.png"),
                5,
                false,
            )
            .unwrap();
        }
    }
    r.light(&s);
    grave_engine::headless::shot(
        &s.world,
        &m,
        &mut r,
        Path::new("shots/layered-shop-after.png"),
        5,
        false,
    )
    .unwrap();
    assert!(caught, "interior must catch from outside fire");
    assert!(exposed, "damage must expose physical interior cells");
}
