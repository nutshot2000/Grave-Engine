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
