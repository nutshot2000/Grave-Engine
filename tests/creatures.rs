//! Real cave actors: attacks, occlusion, chemical/heat reactions and persistent deaths.
use grave_engine::{
    Materials, Sim,
    creatures::Mood,
    edit::{History, Timeline},
    physics::Controls,
    scene,
    supplies::Catalog,
};
fn setup(text: &str) -> (Materials, Sim) {
    let m = Materials::load_dir("materials");
    assert!(m.problems.is_empty(), "{:?}", m.problems);
    let (s, p) = scene::load(text, &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    (m, s)
}
const ROOM: &str =
    "size 240 140\nfill air\nrect bedrock 0 100 240 40\nanchor 0 100 240 40\nplayer 50 100\n";
#[test]
fn guards_approach_telegraph_bites_and_hurt_the_player_at_a_limited_rate() {
    let (m, mut s) = setup(&(ROOM.to_string() + "creature skitter 100 100\n"));
    let mut warned = false;
    let mut hits = 0;
    let mut health = 100.;
    for _ in 0..300 {
        s.step(&m);
        let a = s.physics.actor_status();
        warned |= a[1].creature.as_ref().unwrap().mood == Mood::Windup;
        if a[0].health < health {
            assert!(warned, "a bite needs a readable wind-up");
            hits += 1;
            health = a[0].health;
        }
    }
    assert!(hits >= 1 && hits <= 4, "bite count {hits}");
    assert!(health > 50. && health < 100.);
    let mut r = grave_engine::render::Renderer::default();
    r.light(&s);
    grave_engine::headless::shot_with_actors(
        &s.world,
        &m,
        &mut r,
        &s.physics.actor_status(),
        Some((25, 65, 110, 45)),
        std::path::Path::new("shots/creature-bite.png"),
        6,
        false,
    )
    .unwrap();
}
#[test]
fn a_wall_blocks_sight_and_damage_even_when_a_guard_hears_mining() {
    let (m, mut s) = setup(
        &(ROOM.to_string()
            + "rect bedrock 78 0 8 100\nanchor 78 0 8 100\ncreature skitter 110 100\n"),
    );
    grave_engine::creatures::noise(&mut s.physics, (50., 92.));
    assert_eq!(
        s.physics.actor_status()[1].creature.as_ref().unwrap().mood,
        Mood::Investigate
    );
    for _ in 0..420 {
        s.step(&m);
    }
    let a = s.physics.actor_status();
    assert_eq!(a[0].health, 100.);
    assert!(a[1].x > 86.);
}
#[test]
fn moving_out_of_a_windup_makes_the_bite_miss() {
    let (m, mut s) = setup(&(ROOM.to_string() + "creature skitter 80 100\n"));
    let mut ready = false;
    for _ in 0..150 {
        s.step(&m);
        if s.physics.actor_status()[1].creature.as_ref().unwrap().mood == Mood::Windup {
            ready = true;
            break;
        }
    }
    assert!(ready);
    s.physics.set_controls(Controls {
        walk: -1.,
        ..Default::default()
    });
    for _ in 0..60 {
        s.step(&m);
    }
    assert_eq!(s.physics.player_status().unwrap().health, 100.);
}
#[test]
fn hot_and_corrosive_projectiles_interrupt_attacks_and_drive_a_retreat() {
    for material in ["flame_jet", "acid"] {
        let (m, mut s) = setup(&(ROOM.to_string() + "creature skitter 80 100\n"));
        for _ in 0..120 {
            s.step(&m);
            if s.physics.actor_status()[1].creature.as_ref().unwrap().mood == Mood::Windup {
                break;
            }
        }
        let a = s.physics.actor_status()[1].clone();
        s.world
            .spawn_flyer(&m, m.id(material).unwrap(), a.x - 14., a.y + 2., 1., 0.);
        let mut fled = false;
        for _ in 0..24 {
            s.step(&m);
            fled |= s.physics.actor_status()[1].creature.as_ref().unwrap().mood == Mood::Flee;
        }
        let after = s.physics.actor_status();
        assert!(fled, "must react to material properties of {material}");
        assert!(after[1].x > a.x + 1., "retreat must move away");
        assert_ne!(
            after[0].hurt_by,
            Some("were bitten"),
            "the hazard must interrupt the bite; splash damage is still real"
        );
    }
}
#[test]
fn crafted_material_weapons_can_kill_creatures_without_direct_health_cheats() {
    for (tool, ammo, kind) in [
        ("flamethrower", "oil", "skitter"),
        ("acid_shooter", "acid", "shellback"),
    ] {
        let text = format!(
            "{ROOM}rect bedrock 0 0 240 75\nanchor 0 0 240 75\nrect bedrock 107 0 8 100\nanchor 107 0 8 100\ncreature {kind} 90 100\nworkshop mechanic 50 92 24\nworkshop chemistry 50 92 24\n"
        );
        let text = text.replace("player 50 100", "player 65 100");
        let (m, mut s) = setup(&text);
        let cat = Catalog::load(&m);
        assert!(cat.problems.is_empty());
        for (&id, &n) in &cat.recipes.iter().find(|r| r.id == tool).unwrap().costs {
            s.supplies.stock.insert(id, n);
        }
        s.craft(&cat, tool).unwrap();
        let ammo = m.id(ammo).unwrap();
        s.supplies.stock.insert(ammo, 300);
        let mut damaged = false;
        for _ in 0..1200 {
            let a = s.physics.actor_status();
            if a[1].dead {
                break;
            }
            let aim = (a[1].x - a[0].x, a[1].y + 3.5 - a[0].y);
            s.use_tool(&m, &cat, (a[0].x, a[0].y), aim);
            s.step(&m);
            damaged |= s.physics.actor_status()[1].health < a[1].health;
        }
        let a = s.physics.actor_status();
        assert!(
            damaged && a[1].dead,
            "{tool} must actually kill {kind}: health {}, temp {}, ammo {}",
            a[1].health,
            a[1].temp,
            s.supplies.count(ammo)
        );
        assert!(!a[0].dead, "weapon test must finish with the player alive");
        assert!(s.supplies.count(ammo) < 300);
        for _ in 0..150 {
            s.step(&m);
        }
        assert!(
            s.physics.actor_status()[1].dead,
            "guards do not respawn like players"
        );
        let mut r = grave_engine::render::Renderer::default();
        r.light(&s);
        grave_engine::headless::shot_with_actors(
            &s.world,
            &m,
            &mut r,
            &s.physics.actor_status(),
            Some((30, 65, 95, 50)),
            std::path::Path::new(&format!("shots/creature-{tool}.png")),
            6,
            false,
        )
        .unwrap();
    }
}
#[test]
fn creature_state_deaths_and_home_survive_save_reset_undo_rewind_and_shift() {
    // A creature line before a player must not steal player controls or the HUD.
    let (m, mut s) = setup(&("creature shellback 120 100\n".to_string() + ROOM));
    assert!(s.physics.actor_status()[0].creature.is_none());
    let start = s.snapshot();
    let mut t = Timeline::new();
    t.every = 1;
    t.record(&mut s);
    let mut h = History::new();
    h.touch(&mut s, "defeat guard");
    s.physics.damage_actor(1, 200., "hurt");
    s.step(&m);
    h.commit(&mut s);
    t.record_now(&mut s);
    assert!(s.physics.actor_status()[1].dead);
    let text = scene::save(&s.snapshot(), &m);
    let (mut loaded, p) = scene::load(&text, &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    assert!(loaded.physics.actor_status()[1].dead);
    for _ in 0..130 {
        loaded.step(&m);
    }
    assert!(loaded.physics.actor_status()[1].dead);
    h.undo(&mut s, &m);
    assert!(!s.physics.actor_status()[1].dead);
    h.redo(&mut s, &m);
    assert!(s.physics.actor_status()[1].dead);
    assert!(t.restore(0, &mut s, &m));
    assert!(!s.physics.actor_status()[1].dead);
    assert!(t.restore(1, &mut s, &m));
    assert!(s.physics.actor_status()[1].dead);
    let home = s.physics.actor_status()[1].creature.as_ref().unwrap().home;
    s.shift(-20, 10);
    assert_eq!(
        s.physics.actor_status()[1].creature.as_ref().unwrap().home,
        (home.0 - 20., home.1 + 10.)
    );
    s = Sim::restore(&start, &m);
    assert!(!s.physics.actor_status()[1].dead);
}

#[test]
fn a_ceiling_guard_follows_the_surface_route_to_the_player_below() {
    let (m, mut s) = setup(
        "size 180 120\nfill bedrock\nrect air 20 20 120 60\nanchor 0 0 180 120\nplayer 80 80\ncreature skitter 80 30 normal 0 1 attached yes\n",
    );
    let mut used_wall = false;
    let mut on_ceiling = false;
    for _ in 0..750 {
        s.step(&m);
        let a = s.physics.actor_status();
        let n = a[1].surface.unwrap().normal;
        on_ceiling |= n.1 > 0.9;
        used_wall |= n.0.abs() > 0.9;
        if a[0].health < 100. {
            break;
        }
    }
    assert!(
        on_ceiling && used_wall,
        "must take a connected ceiling/wall route"
    );
    assert!(
        s.physics.player_status().unwrap().health < 100.,
        "must reach the player, rather than oscillate overhead"
    );
}
