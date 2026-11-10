//! The player: walking, jumping, swimming, getting hurt, dying and coming back.

use grave_engine::physics::Controls;
use grave_engine::{Materials, Sim, scene};

fn mats() -> Materials {
    let m = Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"));
    assert!(m.problems.is_empty(), "{:?}", m.problems);
    m
}

fn sim(m: &Materials, text: &str) -> Sim {
    let (sim, problems) = scene::load(text, m, None).into_sim(m);
    assert!(problems.is_empty(), "{problems:?}");
    sim
}

fn run(s: &mut Sim, m: &Materials, frames: usize, walk: f32, jump: bool) {
    s.physics.set_controls(Controls { walk, jump });
    for _ in 0..frames {
        s.step(m);
    }
}

const FLAT: &str = "size 300 120\nfill air\nrect stone 0 110 300 10\n";

#[test]
fn the_player_stands_walks_and_jumps() {
    let m = mats();
    let mut s = sim(&m, &format!("{FLAT}player 50 110\n"));
    run(&mut s, &m, 60, 0.0, false);
    let p = &s.physics.actor_status()[0];
    assert!(p.grounded, "standing on the ground");
    assert!((p.y - 102.0).abs() < 2.0, "feet on the ground: centre y {}", p.y);
    assert_eq!(p.health, 100.0);

    run(&mut s, &m, 60, 1.0, false);
    let x = s.physics.actor_status()[0].x;
    assert!(x > 80.0, "walked right about 4 m/s: x {x}");

    let y0 = s.physics.actor_status()[0].y;
    s.physics.set_controls(Controls { walk: 0.0, jump: true });
    let mut top = y0;
    for _ in 0..50 {
        s.step(&m);
        top = top.min(s.physics.actor_status()[0].y);
    }
    let rise = y0 - top;
    assert!((10.0..20.0).contains(&rise), "a jump goes about 1.5 m: {rise} cells");
}

#[test]
fn sand_piles_around_the_player_but_never_through_it() {
    let m = mats();
    let mut s = sim(&m, &format!("{FLAT}player 50 110
"));
    run(&mut s, &m, 10, 0.0, false);
    let flesh = m.id("flesh").unwrap();
    let count = |s: &Sim, id| s.world.mat.iter().filter(|&&x| x == id).count();
    let body = count(&s, flesh);
    assert!(body > 60, "the figure is drawn into the world: {body} cells");
    // Dump a load of sand right on top.
    let sand = m.id("sand").unwrap();
    for y in 30..50 {
        for x in 30..70 {
            s.world.set(&m, x, y, sand);
        }
    }
    let sand0 = count(&s, sand);
    run(&mut s, &m, 240, 0.0, false);
    assert_eq!(count(&s, flesh), body, "sand never pushes into the player");
    assert_eq!(count(&s, sand), sand0, "and none is lost");
    // It isn't buried: it ends up standing on the heap with its head clear.
    let p = &s.physics.actor_status()[0];
    assert!(!p.dead && p.breath > 9.0, "not buried: breath {}", p.breath);
}

#[test]
fn long_falls_hurt_short_ones_dont() {
    let m = mats();
    let mut low = sim(&m, "size 100 300\nfill air\nrect stone 0 290 100 10\nplayer 50 270\n");
    run(&mut low, &m, 120, 0.0, false);
    assert_eq!(low.physics.actor_status()[0].health, 100.0, "a 2 m drop is fine");

    let mut high = sim(&m, "size 100 300\nfill air\nrect stone 0 290 100 10\nplayer 50 30\n");
    run(&mut high, &m, 200, 0.0, false);
    let p = &high.physics.actor_status()[0];
    assert!(p.health < 60.0 || p.dead, "a 26 m drop hurts a lot: health {}", p.health);
}

#[test]
fn lava_kills_and_the_player_comes_back_at_the_start() {
    let m = mats();
    let mut s = sim(&m, "size 200 120\nfill air\nrect stone 0 110 200 10\nrect lava 120 100 80 10\nheat rect 120 100 80 10 1100\nplayer 40 110\n");
    // Walk into the lava.
    s.physics.set_controls(Controls { walk: 1.0, jump: false });
    let mut died = false;
    for _ in 0..600 {
        s.step(&m);
        if s.physics.actor_status()[0].dead {
            died = true;
            break;
        }
    }
    assert!(died, "walking into lava is deadly");
    assert_eq!(s.physics.actor_status()[0].hurt_by, Some("burned"));
    s.physics.set_controls(Controls::default());
    for _ in 0..130 {
        s.step(&m);
    }
    let p = &s.physics.actor_status()[0];
    assert!(!p.dead && p.health > 95.0, "back to life (hot air drifting about may sting a little): {p:?}");
    assert!((p.x - 40.0).abs() < 4.0, "back at the start: x {}", p.x);
}

#[test]
fn under_water_breath_runs_out_but_swimming_up_keeps_it() {
    let m = mats();
    let pool = "size 100 200\nfill air\nrect stone 0 190 100 10\nrect stone 0 0 2 190\nrect stone 98 0 2 190\nrect water 2 100 96 90\nrect stone 2 100 96 2\nplayer 50 190\n";
    // A lid over the water: no way up for air.
    let mut trapped = sim(&m, pool);
    run(&mut trapped, &m, 60 * 12, 0.0, false);
    let p = &trapped.physics.actor_status()[0];
    assert_eq!(p.breath, 0.0, "out of breath after 10 s under water");
    assert!(p.health < 100.0, "then drowning hurts");

    let mut free = sim(&m, &pool.replace("rect stone 2 100 96 2\n", ""));
    run(&mut free, &m, 60 * 12, 0.0, true);
    let p = &free.physics.actor_status()[0];
    assert!(p.y < 115.0, "swimming at the surface (water starts at 100): y {}", p.y);
    assert!(p.breath > 5.0, "head above water: breath {}", p.breath);
    assert_eq!(p.health, 100.0);
}

#[test]
fn blasts_hurt_and_throw_the_player() {
    let m = mats();
    let mut s = sim(&m, &format!("{FLAT}player 60 110\nrect gunpowder 40 102 8 8\nheat rect 40 102 8 8 600\n"));
    let mut fastest = 0.0f32;
    let mut lowest = 100.0f32;
    for _ in 0..30 {
        s.step(&m);
        let a = &s.physics.export_actors()[0];
        fastest = fastest.max(a.vx);
        lowest = lowest.min(a.health);
    }
    assert!(fastest > 40.0, "thrown away from the blast: vx {fastest} cells/s");
    assert!(lowest < 80.0, "and hurt: health {lowest}");
}

#[test]
fn the_player_is_saved_and_rewound() {
    let m = mats();
    let mut s = sim(&m, &format!("{FLAT}player 50 110 zoom 3\n"));
    run(&mut s, &m, 30, 1.0, false);
    let snap = s.snapshot();
    assert_eq!(snap.actors.len(), 1);
    let text = scene::save(&snap, &m);
    assert!(text.contains("player "), "{text}");
    let (back, problems) = scene::load(&text, &m, None).into_sim(&m);
    assert!(problems.is_empty());
    assert_eq!(back.physics.actor_count(), 1);
    assert_eq!(back.zoom, 3.0);

    let restored = Sim::restore(&snap, &m);
    let (a, b) = (&snap.actors[0], &restored.physics.export_actors()[0]);
    assert!((a.x - b.x).abs() < 0.01 && (a.vx - b.vx).abs() < 0.01);
}

#[test]
fn the_blender_hero_look_loads_and_its_outline_is_its_body() {
    let m = mats();
    let mut s = sim(&m, &format!("{FLAT}player 50 110 look hero\n"));
    let look = s.physics.actor_status()[0].look.clone().expect("the hero look loaded");
    for anim in ["idle", "walk", "jump", "fall", "swim", "die"] {
        assert!(look.anims.contains_key(anim), "{anim}");
    }
    // Standing: a tall, narrow outline. Lying dead: a long, flat one.
    let extent = |mask: &[bool]| {
        let w = grave_engine::look::MASK_W;
        let cols = (0..w).filter(|x| mask.chunks(w).any(|r| r[*x])).count();
        let rows = mask.chunks(w).filter(|r| r.iter().any(|c| *c)).count();
        (cols, rows)
    };
    let (sw, sh) = extent(&look.frame("idle", 0.0).0.mask);
    let (dw, dh) = extent(&look.frame("die", 10.0).0.mask);
    assert!(sh >= 15 && sw <= 6, "standing outline {sw}x{sh}");
    assert!(dw >= 14 && dh <= 6, "lying outline {dw}x{dh}");
    // With a look, the body's cells are hidden (the pictures are drawn
    // instead) but still there for sand and heat.
    run(&mut s, &m, 10, 0.0, false);
    let flesh = m.id("flesh").unwrap();
    let body: Vec<usize> = (0..s.world.mat.len()).filter(|&i| s.world.mat[i] == flesh).collect();
    assert!(body.len() > 40);
    assert!(body.iter().all(|&i| s.world.flags[i] & grave_engine::world::HIDDEN != 0));
    // Walking plays the walk animation; saving keeps the look.
    run(&mut s, &m, 30, 1.0, false);
    assert_eq!(s.physics.actor_status()[0].anim, "walk");
    let text = scene::save(&s.snapshot(), &m);
    assert!(text.contains("look hero"), "{text}");
}
