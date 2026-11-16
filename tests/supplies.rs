//! Exact inventories, atomic workshop recipes, persistence and real material tools.
use grave_engine::{
    Materials, Sim,
    edit::{History, Timeline},
    scene,
    supplies::{Catalog, Product},
};
fn setup(text: &str) -> (Materials, Sim, Catalog) {
    let m = Materials::load_dir("materials");
    assert!(m.problems.is_empty(), "{:?}", m.problems);
    let (s, p) = scene::load(text, &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    let c = Catalog::load(&m);
    assert!(c.problems.is_empty(), "{:?}", c.problems);
    (m, s, c)
}
const ROOM: &str = "size 160 100\nfill air\nrect rock 0 90 160 10\nanchor 0 90 160 10\nplayer 40 90\nworkshop mechanic 40 82 24\nworkshop chemistry 40 82 24\n";
fn fund(s: &mut Sim, c: &Catalog, id: &str) {
    for (&mat, &n) in &c.recipes.iter().find(|r| r.id == id).unwrap().costs {
        s.supplies.stock.insert(mat, n + 3);
    }
    s.supplies.tracking = true;
}
#[test]
fn each_removed_cell_is_stored_once_under_its_actual_material() {
    let (m, mut s, c) = setup(
        "size 120 100\nfill air\nrect rock 60 20 2 12\nrect sandstone 60 40 2 12\nrect crystal_cyan 60 60 2 12\nrect sand 60 80 2 12\nanchor 0 0 120 100\n",
    );
    for (name, y) in [
        ("rock", 25.),
        ("sandstone", 45.),
        ("crystal_cyan", 65.),
        ("sand", 85.),
    ] {
        let id = m.id(name).unwrap();
        let before = s.world.mat.iter().filter(|m| **m == id).count();
        let mut removed = 0;
        for _ in 0..150 {
            let mined = s.use_tool(&m, &c, (48., y), (1., 0.));
            removed += mined.get(&id).copied().unwrap_or(0);
            s.world.frame += 1;
        }
        let after = s.world.mat.iter().filter(|m| **m == id).count();
        assert!(removed > 0);
        assert_eq!(removed, before - after);
        assert_eq!(s.supplies.count(id), removed as u64);
    }
    assert_eq!(s.supplies.count(0), 0);
    let stored = s.supplies.stock.clone();
    for _ in 0..60 {
        s.use_tool(&m, &c, (10., 10.), (-1., 0.));
        s.world.frame += 1;
    }
    assert_eq!(s.supplies.stock, stored);
}
#[test]
fn recipes_require_the_right_workshop_spend_exactly_and_cannot_charge_twice() {
    let (_m, mut s, c) = setup(ROOM);
    fund(&mut s, &c, "reinforced_drill");
    let mut distant = s.supplies.clone();
    distant.stations[0].x = 140.;
    let r = c
        .recipes
        .iter()
        .find(|r| r.id == "reinforced_drill")
        .unwrap();
    let before = distant.clone();
    assert!(r.craft(&mut distant, (40., 82.)).is_err());
    assert_eq!(before, distant);
    s.craft(&c, "reinforced_drill").unwrap();
    assert_eq!(s.supplies.equipped, "reinforced_drill");
    for &id in r.costs.keys() {
        assert_eq!(s.supplies.count(id), 3);
    }
    let before = s.supplies.clone();
    assert!(s.craft(&c, "reinforced_drill").is_err());
    assert_eq!(before, s.supplies);
    assert!(s.craft(&c, "acid_charge").is_err());
    assert_eq!(before, s.supplies);
    assert!(!s.supplies.equip("not_owned"));
    s.supplies.cycle(&c);
    assert_eq!(s.supplies.equipped, "drill");
}
#[test]
fn spending_crystals_changes_the_carried_mission_quota_and_persists_with_the_world() {
    let (m, mut s, c) = setup(
        &(ROOM.to_string()
            + "mission 140 82 10\nsample crystal_cyan 6 0\nsample crystal_violet 6 0\n"),
    );
    fund(&mut s, &c, "reinforced_drill");
    s.sync_carried_samples();
    let start = s.snapshot();
    let mut history = History::new();
    let mut t = Timeline::new();
    t.every = 1;
    t.record(&mut s);
    history.touch(&mut s, "craft");
    s.craft(&c, "reinforced_drill").unwrap();
    history.commit(&mut s);
    t.record_now(&mut s);
    assert_eq!(s.mission.as_ref().unwrap().goals[0].collected, 3);
    assert!(!s.mission.as_ref().unwrap().ready());
    let crafted = s.supplies.clone();
    let text = scene::save(&s.snapshot(), &m);
    let (loaded, p) = scene::load(&text, &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    assert_eq!(loaded.supplies, crafted);
    history.undo(&mut s, &m);
    assert_eq!(s.supplies, start.supplies);
    history.redo(&mut s, &m);
    assert_eq!(s.supplies, crafted);
    assert!(t.restore(0, &mut s, &m));
    assert_eq!(s.supplies, start.supplies);
    assert!(t.restore(1, &mut s, &m));
    assert_eq!(s.supplies, crafted);
    s.shift(10, -5);
    assert_eq!(
        (s.supplies.stations[0].x, s.supplies.stations[0].y),
        (50., 77.)
    );
    s = Sim::restore(&start, &m);
    assert_eq!(s.supplies, start.supplies);
}
#[test]
fn spray_spends_one_ammo_per_real_cell_and_stops_when_dry_or_blocked() {
    let (m, mut s, c) = setup(ROOM);
    fund(&mut s, &c, "acid_shooter");
    s.craft(&c, "acid_shooter").unwrap();
    let acid = m.id("acid").unwrap();
    s.supplies.stock.insert(acid, 3);
    for _ in 0..40 {
        s.use_tool(&m, &c, (40., 82.), (1., 0.));
        s.world.frame += 1;
    }
    assert_eq!(s.supplies.count(acid), 0);
    assert_eq!(s.world.flying.iter().filter(|f| f.mat == acid).count(), 3);
    s.world.flying.clear();
    s.supplies.stock.insert(acid, 3);
    let i = s.world.idx(44, 82);
    s.world.mat[i] = m.id("steel").unwrap();
    for _ in 0..20 {
        s.use_tool(&m, &c, (40., 82.), (1., 0.));
        s.world.frame += 1;
    }
    // It may splash onto a close face, but cannot originate behind it.
    assert!(s.world.flying.iter().all(|f| f.x < 44.));
    s.supplies.equipped = "flamethrower".into();
    let n = s.world.flying.len();
    s.use_tool(&m, &c, (40., 82.), (1., 0.));
    assert_eq!(s.world.flying.len(), n, "unowned tool cannot fire");
}
#[test]
fn fire_ignites_timber_and_acid_cuts_steel_using_ordinary_world_reactions() {
    for (tool, ammo, target) in [
        ("flamethrower", "oil", "wood"),
        ("acid_shooter", "acid", "steel"),
    ] {
        let text = format!("{ROOM}rect {target} 70 55 3 35\nanchor 70 55 3 35\n");
        let (m, mut s, c) = setup(&text);
        fund(&mut s, &c, tool);
        s.craft(&c, tool).unwrap();
        s.supplies.stock.insert(m.id(ammo).unwrap(), 160);
        let id = m.id(target).unwrap();
        let before = s.world.mat.iter().filter(|x| **x == id).count();
        let mut burned = false;
        for step in 0..420 {
            let p = s.physics.actor_status()[0].clone();
            s.use_tool(&m, &c, (p.x, p.y), (1., -0.35));
            s.step(&m);
            if step == 120 {
                let mut renderer = grave_engine::render::Renderer::default();
                renderer.light(&s);
                grave_engine::headless::shot_with_actors(
                    &s.world,
                    &m,
                    &mut renderer,
                    &s.physics.actor_status(),
                    None,
                    std::path::Path::new(&format!("shots/workshop-{tool}-active.png")),
                    5,
                    false,
                )
                .unwrap();
            }
            burned |= s
                .world
                .mat
                .iter()
                .enumerate()
                .any(|(i, x)| *x == id && s.world.is_burning(i));
        }
        let after = s.world.mat.iter().filter(|x| **x == id).count();
        let temp = s
            .world
            .mat
            .iter()
            .enumerate()
            .filter(|(_, v)| **v == id)
            .map(|(i, _)| s.world.temp[i])
            .fold(0f32, f32::max);
        assert!(
            after < before || burned,
            "{tool} must change {target}: {before}->{after}, temp={temp}, ammo={}, flyers={}",
            s.supplies.count(m.id(ammo).unwrap()),
            s.world.flying.len()
        );
        let mut r = grave_engine::render::Renderer::default();
        r.light(&s);
        grave_engine::headless::shot_with_actors(
            &s.world,
            &m,
            &mut r,
            &s.physics.actor_status(),
            None,
            std::path::Path::new(&format!("shots/workshop-{tool}.png")),
            4,
            false,
        )
        .unwrap();
    }
}
#[test]
fn malformed_recipes_do_not_create_free_or_partial_transactions() {
    let m = Materials::load_dir("materials");
    let c = Catalog::parse(
        "drill | Drill | drill 6 19 1 3 | none",
        "free | Bad | mechanic | stock rock 10 | rock -2\nwrong | Bad | mechanic | tool nonexistent | rock 5",
        &m,
    );
    assert_eq!(c.problems.len(), 2);
    assert!(c.recipes.is_empty());
    let c = Catalog::load(&m);
    assert!(c.recipes.iter().all(|r| match r.product {
        Product::Stock(_, n) => n > 0,
        Product::Tool(_) => true,
    }));
}

#[test]
fn chemical_contact_harms_the_player_but_water_does_not() {
    for (liquid, harmful) in [("acid", true), ("water", false)] {
        let text = format!(
            "size 120 100\nfill air\nrect bedrock 0 90 120 10\nanchor 0 90 120 10\nrect {liquid} 30 75 20 15\nplayer 40 90\n"
        );
        let (m, mut s, _) = setup(&text);
        for _ in 0..60 {
            s.step(&m);
        }
        let health = s.physics.actor_status()[0].health;
        if harmful {
            assert!(health < 95., "contact hazard must hurt: {health}");
        } else {
            assert!(health > 99.);
        }
    }
}
