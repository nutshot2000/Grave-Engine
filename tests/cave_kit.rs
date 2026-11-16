//! Starter rope, hand-mounted tools and finite gas collection across real engine state.
use grave_engine::{Materials, Sim, physics::Controls, scene, supplies::Catalog};
#[test]
fn invalid_world_dimensions_are_reported_before_creatures_can_step() {
    let m=Materials::load_dir("materials");
    for bad in ["NaN","inf","-inf"] {
        let (mut s,p)=scene::load(&format!("size {bad} 140\nfill air\ncreature skitter 50 90\n"),&m,None).into_sim(&m);
        assert!(p.iter().any(|p|p.contains("finite")));assert!(s.world.w>=8&&s.world.h>=8);
        for _ in 0..3{s.step(&m);}
    }
}
fn setup(text: &str) -> (Materials, Sim, Catalog) {
    let m = Materials::load_dir("materials");
    assert!(m.problems.is_empty(), "{:?}", m.problems);
    let (s, p) = scene::load(text, &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    let c = Catalog::load(&m);
    assert!(c.problems.is_empty(), "{:?}", c.problems);
    (m, s, c)
}
const ROOM: &str = "size 260 260\nfill air\nrect bedrock 0 0 260 10\nrect bedrock 0 245 260 15\nanchor 0 0 260 260\nplayer 90 170 rope rope_range 220\n";
#[test]
fn rope_wraps_around_a_rock_corner_and_unwraps_when_it_has_a_clear_path() {
    let (m, mut s, _) = setup(
        "size 220 240\nfill air\nrect bedrock 0 0 220 10\nrect rock 112 70 8 8\nanchor 0 0 220 240\nplayer 170 174 rope rope_range 220 rope_length 210 rope_path 80,9.5\n",
    );
    s.step(&m);
    let pins = s.physics.player_status().unwrap().rope.unwrap().pins;
    assert!(
        pins.len() > 1,
        "obstructed line needs a corner pin: {pins:?}"
    );
    for y in 70..78 {
        for x in 112..120 {
            let i = s.world.idx(x, y);
            s.world.make_air(i);
        }
    }
    s.step(&m);
    assert_eq!(
        s.physics.player_status().unwrap().rope.unwrap().pins.len(),
        1,
        "removing the corner frees the line"
    );
}
#[test]
fn rope_catches_falls_reels_and_releases_with_momentum() {
    let (m, mut s, _) = setup(ROOM);
    assert!(s.physics.grapple(&s.world, &m, (0., -1.)));
    let initial = s.physics.player_status().unwrap().rope.unwrap().length;
    s.physics.reel_rope(1.);
    s.physics.set_controls(Controls {
        walk: 0.8,
        ..Default::default()
    });
    let mut max_x = 90f32;
    for _ in 0..150 {
        s.step(&m);
        let p = s.physics.player_status().unwrap();
        max_x = max_x.max(p.x);
        assert!(
            p.health > 95.,
            "rope catch should not cause impact damage: {}",
            p.health
        );
    }
    let p = s.physics.player_status().unwrap();
    let r = p.rope.unwrap();
    assert!(r.length < initial - 40.);
    assert!(!r.pins.is_empty());
    assert!(max_x > 110., "rope must allow a swing");
    let before = s.snapshot().actors[0].vx;
    assert!(s.physics.grapple(&s.world, &m, (0., -1.)));
    let after = s.snapshot().actors[0].vx;
    assert_eq!(before, after, "detaching must retain velocity");
    assert!(
        s.physics
            .player_status()
            .unwrap()
            .rope
            .unwrap()
            .pins
            .is_empty()
    );
}
#[test]
fn rope_loses_a_mined_anchor_and_survives_save_reset_and_shift() {
    let (m, mut s, _) = setup(ROOM);
    assert!(s.physics.grapple(&s.world, &m, (0., -1.)));
    s.physics.reel_rope(0.5);
    for _ in 0..12 {
        s.step(&m);
    }
    s.physics.reel_rope(0.);
    let snap = s.snapshot();
    let text = scene::save(&snap, &m);
    let (mut again, p) = scene::load(&text, &m, None).into_sim(&m);
    assert!(p.is_empty());
    assert_eq!(
        again.physics.player_status().unwrap().rope,
        s.physics.player_status().unwrap().rope
    );
    let old = again.physics.player_status().unwrap().rope.unwrap().pins[0];
    again.shift(10, 10);
    let shifted = again.physics.player_status().unwrap().rope.unwrap().pins[0];
    assert_eq!(shifted, (old.0 + 10., old.1 + 10.));
    let pin = s.physics.player_status().unwrap().rope.unwrap().pins[0];
    for y in pin.1 as usize - 1..=pin.1 as usize + 1 {
        for x in pin.0 as usize - 1..=pin.0 as usize + 1 {
            let i = s.world.idx(x, y);
            s.world.make_air(i);
        }
    }
    s.step(&m);
    assert!(
        s.physics
            .player_status()
            .unwrap()
            .rope
            .unwrap()
            .pins
            .is_empty()
    );
    s = Sim::restore(&snap, &m);
    assert!(
        !s.physics
            .player_status()
            .unwrap()
            .rope
            .unwrap()
            .pins
            .is_empty()
    );
}
#[test]
fn suction_is_finite_does_not_collect_air_or_pass_rock_and_can_refine_fuel() {
    let (m, mut s, c) = setup(
        "size 160 120\nfill air\nrect bedrock 0 100 160 20\nrect volatile_gas 50 60 12 26\nrect bedrock 66 30 4 70\nrect volatile_gas 74 60 12 26\nanchor 0 0 160 120\nplayer 40 100\nowned_tool vacuum\nequipped_tool vacuum\nworkshop chemistry 40 92 24\n",
    );
    let gas = m.id("volatile_gas").unwrap();
    let original = s.world.mat.iter().filter(|&&id| id == gas).count() as u64;
    for step in 0..100 {
        s.world.frame = step * 3;
        s.use_tool(&m, &c, (42., 75.), (1., 0.));
    }
    let stored = s.supplies.gas_total();
    assert!(stored > 0 && stored <= 512);
    let left = s.world.mat.iter().filter(|&&id| id == gas).count() as u64;
    assert_eq!(original, stored + left);
    assert_eq!(
        s.world.mat[s.world.idx(78, 75)],
        gas,
        "sealed gas must remain"
    );
    assert_eq!(s.supplies.count(0), 0);
    let snap = s.snapshot();
    let text = scene::save(&snap, &m);
    let (again, p) = scene::load(&text, &m, None).into_sim(&m);
    assert!(p.is_empty());
    assert_eq!(again.supplies.gas, s.supplies.gas);
    let before = s.supplies.count(gas);
    s.craft(&c, "gas_fuel").unwrap();
    assert_eq!(s.supplies.count(gas), before - 48);
    assert_eq!(s.supplies.count(m.id("oil").unwrap()), 96);
    s.supplies.gas.insert(gas, 512);
    for step in 0..20 {
        s.world.frame = step * 3;
        s.use_tool(&m, &c, (42., 75.), (1., 0.));
    }
    assert_eq!(s.supplies.gas_total(), 512);
    let count = s.supplies.gas_total();
    assert!(s.vent_gas(&m, &c, (1., 0.)));
    assert_eq!(s.supplies.gas_total(), count - 1);
    assert!(s.world.flying.iter().any(|f| f.mat == gas));
}
#[test]
fn mining_preview_is_exact_and_the_local_rig_uses_its_weapon_poses() {
    let (m, mut s, c) = setup(
        "size 140 120\nfill air\nrect rock 65 30 20 70\nanchor 0 0 140 120\nplayer 50 100 look cave-diver moves climb 0.65 reach 8 rope\n",
    );
    let d = grave_engine::mining::Drill::default();
    let from = (51., 75.);
    let hit = grave_engine::mining::surface(&s.world, &m, from, (1., 0.), d.reach).unwrap();
    let candidates = grave_engine::mining::candidates(&s.world, &m, from, hit, d);
    s.mining.strike(&mut s.world, &m, from, (1., 0.), d);
    assert_eq!(candidates.len(), s.mining.fractures.len());
    for (i, _) in candidates {
        assert!(s.mining.fractures.contains_key(&i));
    }
    if std::path::Path::new("assets/characters/mixamo-fbx-local/character.txt").is_file() {
        s.aim_tool((1., -0.8), true, true);
        for _ in 0..40 {
            s.step(&m);
        }
        let a = s.physics.player_status().unwrap();
        assert_eq!(a.anim, "ual_pistol_aim_up");
        let pose = grave_engine::tool_pose::pose(&a, (1., -0.8));
        assert!(pose.hand.1 < a.y, "weapon hand must be above waist");
        assert!(!a.look.unwrap().hands.is_empty());
    }
    assert!(c.tool("vacuum").is_some());
}
