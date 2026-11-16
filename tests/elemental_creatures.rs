//! Finite, once-only elemental deaths and normal material reactions afterwards.
use grave_engine::{Materials, Sim, edit::Timeline, scene};
const ROOM: &str = "size 200 140\nfill air\nrect bedrock 0 100 200 40\nanchor 0 100 200 40\n";
fn setup(text: &str) -> (Materials, Sim) {
    let m = Materials::load_dir("materials");
    let (s, p) = scene::load(text, &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    (m, s)
}
fn count(s: &Sim, id: u16) -> usize {
    s.world.mat.iter().filter(|m| **m == id).count()
        + s.world.flying.iter().filter(|f| f.mat == id).count()
}
#[test]
fn both_species_accept_every_element_and_release_only_the_authored_real_cells() {
    for kind in ["skitter", "shellback"] {
        for element in ["fire", "acid", "oil", "lava", "water"] {
            let (m, mut s) = setup(&format!(
                "{ROOM}creature {kind} 90 100 element {element} burst 80\n"
            ));
            let id = m.id(element).unwrap();
            s.physics.damage_actor(0, 2000., "hurt");
            s.step(&m);
            assert!(s.physics.actor_status()[0].dead);
            assert!(
                s.physics.actor_status()[0]
                    .creature
                    .as_ref()
                    .unwrap()
                    .erupted
            );
            assert_eq!(count(&s, id), 80, "{kind} / {element}");
            grave_engine::creatures::death_bursts(&mut s.physics, &mut s.world, &m);
            assert_eq!(
                count(&s, id),
                80,
                "a second callback cannot duplicate a payload"
            );
        }
    }
}
#[test]
fn water_payload_survives_save_load_and_rewind_without_respawning_the_burst() {
    let (m, mut s) = setup(&format!(
        "{ROOM}creature skitter 90 100 element water burst 120\n"
    ));
    let id = m.id("water").unwrap();
    let start = s.snapshot();
    let mut t = Timeline::new();
    t.every = 1;
    t.record(&mut s);
    s.physics.damage_actor(0, 1000., "hurt");
    s.step(&m);
    t.record_now(&mut s);
    assert_eq!(count(&s, id), 120);
    let text = scene::save(&s.snapshot(), &m);
    let (mut loaded, p) = scene::load(&text, &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    for _ in 0..60 {
        loaded.step(&m);
    }
    assert_eq!(count(&loaded, id), 120);
    assert!(t.restore(0, &mut s, &m));
    assert_eq!(count(&s, id), 0);
    assert!(
        !s.physics.actor_status()[0]
            .creature
            .as_ref()
            .unwrap()
            .erupted
    );
    assert!(t.restore(1, &mut s, &m));
    assert_eq!(count(&s, id), 120);
    grave_engine::creatures::death_bursts(&mut s.physics, &mut s.world, &m);
    assert_eq!(count(&s, id), 120);
    s = Sim::restore(&start, &m);
    assert_eq!(count(&s, id), 0);
    assert!(!s.physics.actor_status()[0].dead);
}
#[test]
fn acid_death_releases_a_pool_that_dissolves_real_scenery() {
    let (m, mut s) = setup(&format!(
        "{ROOM}rect steel 105 80 3 20\nanchor 105 80 3 20\ncreature skitter 90 100 element acid burst 112\n"
    ));
    let steel = m.id("steel").unwrap();
    let before = count(&s, steel);
    s.physics.damage_actor(0, 1000., "hurt");
    s.step(&m);
    for _ in 0..240 {
        s.step(&m);
    }
    assert!(
        count(&s, steel) < before,
        "acid must react with the scenery, not just draw green particles"
    );
    let mut r = grave_engine::render::Renderer::default();
    r.light(&s);
    grave_engine::headless::shot_with_actors(
        &s.world,
        &m,
        &mut r,
        &s.physics.actor_status(),
        Some((55, 65, 85, 45)),
        std::path::Path::new("shots/elemental-acid-pool.png"),
        6,
        false,
    )
    .unwrap();
}
#[test]
fn affinity_uses_the_elements_temperature_and_contact_damage_properties() {
    for element in ["none", "fire", "lava"] {
        let (m, mut s) = setup(&format!(
            "{ROOM}creature skitter 90 100 element {element} temp 120\n"
        ));
        for _ in 0..25 {
            s.step(&m);
        }
        let a = s.physics.actor_status().remove(0);
        if element == "none" {
            assert!(a.health < 36.);
        } else {
            assert_eq!(
                a.health, 36.,
                "hot element should tolerate its own temperature range"
            );
        }
    }
    for element in ["none", "acid"] {
        let (m, mut s) = setup(&format!(
            "{ROOM}rect acid 60 85 60 15\ncreature skitter 90 100 element {element}\n"
        ));
        for _ in 0..30 {
            s.step(&m);
        }
        let a = s.physics.actor_status().remove(0);
        if element == "none" {
            assert!(a.health < 36.);
        } else {
            assert_eq!(
                a.health, 36.,
                "acid creatures resist their own corrosive material"
            );
        }
    }
}
#[test]
fn a_fire_death_can_ignite_an_oil_death_pool_through_normal_world_rules() {
    let (m, mut s) = setup(&format!(
        "{ROOM}creature skitter 90 100 element oil burst 160\ncreature skitter 110 100 element fire burst 160\n"
    ));
    let oil = m.id("oil").unwrap();
    s.physics.damage_actor(0, 1000., "hurt");
    s.step(&m);
    for _ in 0..80 {
        s.step(&m);
    }
    s.physics.damage_actor(1, 1000., "hurt");
    s.step(&m);
    let mut ignited = false;
    for _ in 0..180 {
        s.step(&m);
        ignited |= s
            .world
            .mat
            .iter()
            .enumerate()
            .any(|(i, id)| *id == oil && s.world.is_burning(i));
    }
    assert!(
        ignited,
        "a real fire payload should ignite a real oil spill without a scripted oil/fire special case"
    );
}
#[test]
fn invalid_elements_are_reported_and_air_cannot_be_a_payload() {
    let m = Materials::load_dir("materials");
    for element in ["not_a_material", "AIR"] {
        let (_, p) = scene::load(
            &format!("{ROOM}creature skitter 90 100 element {element}\n"),
            &m,
            None,
        )
        .into_sim(&m);
        assert!(!p.is_empty());
    }
}

/// `creatures AMOUNT` fills a generated level with elemental creatures on
/// real floors, never inside the safe zone, the same every time.
#[test]
fn creatures_line_scatters_elemental_creatures_over_the_level() {
    let m = grave_engine::Materials::load_dir("materials");
    let text = "size 1024 640\nfill air\ngenerate 0 0 1024 640 seed 5 tile 140 caverns 1 rooms 0 bend 1\ncreatures 0.6 seed 5 avoid 200 300 150\n";
    let load = || grave_engine::scene::load(text, &m, None).into_sim(&m);
    let (s, problems) = load();
    assert!(problems.is_empty(), "{problems:?}");
    let bugs: Vec<_> = s.physics.actor_status().into_iter().filter(|a| a.creature.is_some()).collect();
    assert!(bugs.len() >= 15, "only {} creatures", bugs.len());
    let elements: std::collections::BTreeSet<_> = bugs.iter().filter_map(|a| a.creature.as_ref()?.element.clone()).collect();
    assert!(elements.len() >= 3, "a mix of elements: {elements:?}");
    for a in &bugs {
        assert!((a.x - 200.0).hypot(a.y - 300.0) > 140.0, "creature inside the safe zone at ({}, {})", a.x, a.y);
    }
    let (again, _) = load();
    let place = |s: &grave_engine::Sim| s.physics.actor_status().iter().map(|a| (a.x as i32, a.y as i32)).collect::<Vec<_>>();
    assert_eq!(place(&s), place(&again), "same scene, same creatures");
}

/// Creatures seek out their element, chew through soft rock to reach it,
/// and feed on it (their death burst grows). Hard stone stops them.
#[test]
fn creatures_seek_burrow_to_and_feed_on_their_element() {
    let m = grave_engine::Materials::load_dir("materials");
    let run = |wall: &str| {
        let text = format!("size 320 140\nfill air\nrect stone 0 110 320 30\nrect stone 0 0 320 20\nrect stone 190 80 4 30\nrect stone 250 80 4 30\nrect water 194 92 56 18\n{wall}creature skitter 100 110 element water\n");
        let (mut s, problems) = grave_engine::scene::load(&text, &m, None).into_sim(&m);
        assert!(problems.is_empty(), "{problems:?}");
        let water = m.id("water").unwrap();
        let before = s.world.mat.iter().filter(|&&c| c == water).count();
        for _ in 0..1800 {
            s.step(&m);
        }
        let after = s.world.mat.iter().filter(|&&c| c == water).count();
        let bug = s.physics.actor_status().into_iter().find(|a| a.creature.is_some()).unwrap();
        (before as i64, after as i64, bug.creature.unwrap().burst, bug.x)
    };
    let (before, after, burst, _) = run("");
    assert!(after < before - 40 && burst > 120, "open route: drank {} cells, burst {burst}", before - after);
    let shale = m.id("shale").unwrap();
    let (before, after, burst, x) = run("rect shale 140 20 24 90\n");
    assert!(after < before - 20 && burst > 100 && x > 150.0, "chewed through shale: drank {}, burst {burst}, at x {x}", before - after);
    let _ = shale;
    let (before, after, burst, x) = run("rect stone 140 20 24 90\n");
    assert!(after >= before - 2 && burst == 80 && x < 140.0, "stone stops it: drank {}, burst {burst}, at x {x}", before - after);
}

/// The newer creatures: a crystal mite eats crystals and bursts them back
/// out when killed; a gravel worm tunnels through shale leaving gravel; an
/// ember beetle drops sparks as it runs to gas.
#[test]
fn mites_worms_and_beetles_do_their_thing() {
    let m = grave_engine::Materials::load_dir("materials");
    let count = |s: &grave_engine::Sim, name: &str| { let id = m.id(name).unwrap(); s.world.mat.iter().filter(|&&c| c == id).count() };
    let load = |extra: &str| {
        let text = format!("size 320 140\nfill air\nrect stone 0 110 320 30\nrect stone 0 0 320 20\n{extra}");
        let (s, problems) = grave_engine::scene::load(&text, &m, None).into_sim(&m);
        assert!(problems.is_empty(), "{problems:?}");
        s
    };
    // Mite.
    let mut s = load("rect crystal_cyan 170 96 8 14\nanchor 170 96 8 14\ncreature mite 110 110 element crystal_cyan burst 20\n");
    let crystals = count(&s, "crystal_cyan");
    for _ in 0..1200 { s.step(&m); }
    let left = count(&s, "crystal_cyan");
    let i = s.physics.actor_status().iter().position(|a| a.creature.is_some()).unwrap();
    let burst = s.physics.actor_status()[i].creature.as_ref().unwrap().burst;
    assert!(left < crystals && burst as usize > 20, "the mite ate crystals: {crystals} -> {left}, burst {burst}");
    s.physics.damage_actor(i, 999.0, "test");
    for _ in 0..120 { s.step(&m); }
    let back = count(&s, "crystal_cyan");
    assert!(back > left + 10, "killing it bursts the crystals back out: {left} -> {back}");
    // Worm.
    let mut s = load("rect shale 140 20 30 90\nrect sandstone 200 60 30 50\ncreature worm 100 110 element gravel\n");
    for _ in 0..1500 { s.step(&m); }
    assert!(count(&s, "gravel") > 10, "the worm left gravel behind: {}", count(&s, "gravel"));
    // Beetle.
    let mut s = load("rect volatile_gas 220 60 30 20\ncreature beetle 100 110 element ember\n");
    let mut sparks = 0;
    for _ in 0..600 { s.step(&m); sparks = sparks.max(count(&s, "ember") + count(&s, "fire")); }
    assert!(sparks > 0, "the beetle dropped sparks");
}
