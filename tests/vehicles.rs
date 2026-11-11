//! Vehicle controls in the engine: brakes, rolling resistance, balance.
//! (From Wreck Run's wish list: one vehicle can brake on stone, slide on
//! ice, slow in sand and mud, and isn't braked by the ground in the air.)

use grave_engine::sim::Sim;
use grave_engine::{Materials, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn sim(m: &Materials, text: &str) -> Sim {
    let (s, p) = scene::load(text, m, None).into_sim(m);
    assert!(p.is_empty(), "{p:?}");
    s
}

/// A little cart on 500 cells of `ground`, on a freezing day (so ice stays ice).
fn cart(ground: &str) -> String {
    format!(
        "size 500 100\nambient -10\nfill air\nheat rect 0 0 500 100 -10\nrect stone 0 90 500 10\nrect {ground} 0 86 500 4\nheat rect 0 86 500 14 -10\n\
         body rect iron 20 66 40 4\nname cart\n\
         body circle rubber 26 76 5\nname back\n\
         body circle rubber 54 76 5\nname front\n\
         wheel cart back spring 40 damping 8 travel 2 motor 3 strength 8 drive\n\
         wheel cart front spring 40 damping 8 travel 2 motor 3 strength 8 drive\n"
    )
}

fn cart_body(s: &mut Sim) -> grave_engine::physics::BodySpec {
    let specs = s.physics.export(&mut s.world);
    specs.into_iter().max_by_key(|b| b.cell_count()).unwrap()
}

/// Drive at full throttle for `go` frames, then hold `brake` for `stop` frames.
/// Returns (speed when the brakes went on, distance travelled while braking).
fn run(ground: &str, go: usize, stop: usize, brake: f32) -> (f32, f32) {
    let m = mats();
    let mut s = sim(&m, &cart(ground));
    s.physics.set_throttle(1.0);
    for _ in 0..go {
        s.step(&m);
    }
    let b = cart_body(&mut s);
    let (x0, v0) = (b.x, b.vx);
    s.physics.set_throttle(0.0);
    s.physics.set_brake(brake);
    for _ in 0..stop {
        s.step(&m);
    }
    (v0, cart_body(&mut s).x - x0)
}

/// The cart already rolling at `speed` (cells/s) on `ground`, brakes held for
/// `frames`: how far it goes.
fn braking_distance(ground: &str, speed: f32, brake: f32, frames: usize) -> f32 {
    let m = mats();
    let mut s = sim(&m, &cart(ground));
    for _ in 0..20 {
        s.step(&m); // settle onto the ground
    }
    for id in s.physics.ids() {
        s.physics.edit(&mut s.world, &m, id, |b| b.vx = speed);
    }
    let x0 = cart_body(&mut s).x;
    s.physics.set_brake(brake);
    for _ in 0..frames {
        s.step(&m);
    }
    cart_body(&mut s).x - x0
}

#[test]
fn brakes_grip_on_stone_and_slide_on_ice() {
    let coast = braking_distance("stone", 60.0, 0.0, 90);
    let stone = braking_distance("stone", 60.0, 1.0, 90);
    let ice = braking_distance("ice", 60.0, 1.0, 90);
    println!("from 6 m/s: coasting {coast:.0}, braking on stone {stone:.0}, on ice {ice:.0} cells");
    assert!(stone < coast * 0.5, "brakes stop it on stone: {stone} vs coasting {coast}");
    // (Locked tyres on ice slow a little more than its grip alone explains:
    // some loss in the wheel suspension. Still clearly a slide.)
    assert!(ice > stone * 1.5, "on ice the brakes barely bite: ice {ice} vs stone {stone}");
}

#[test]
fn sand_and_mud_slow_rolling_wheels() {
    let (_, stone) = run("stone", 60, 120, 0.0);
    let (_, sand) = run("sand", 60, 120, 0.0);
    let (_, mud) = run("wet_sand", 60, 120, 0.0);
    println!("coasting: stone {stone:.0}, sand {sand:.0}, mud {mud:.0} cells");
    assert!(sand < stone * 0.8, "sand slows it: {sand} vs {stone}");
    assert!(mud < sand, "mud slows it more: {mud} vs {sand}");
    let m = mats();
    let mut s = sim(&m, &cart("wet_sand"));
    for _ in 0..30 {
        s.step(&m);
    }
    let id = s.physics.ids()[0];
    let c = s.physics.wheel_contact(&s.world, &m, id);
    assert!(c.grounded && c.surface == m.id("wet_sand") && c.resistance > 1.0, "{c:?}");
}

#[test]
fn brakes_do_nothing_in_the_air() {
    let m = mats();
    // Launch the cart off a ledge, then brake mid-air.
    let mut s = sim(&m, "size 400 200\nfill air\nrect stone 0 190 400 10\nrect stone 0 70 80 120\n\
        body rect iron 20 50 40 4\nname cart\nbody circle rubber 26 60 5\nname back\nbody circle rubber 54 60 5\nname front\n\
        wheel cart back spring 40 damping 8 travel 2 motor 3 strength 8 drive\nwheel cart front spring 40 damping 8 travel 2 motor 3 strength 8 drive\n");
    s.physics.set_throttle(1.0);
    let mut airborne_v = None;
    for _ in 0..200 {
        s.step(&m);
        let id = s.physics.ids()[0];
        if !s.physics.wheel_contact(&s.world, &m, id).grounded && cart_body(&mut s).x > 85.0 {
            airborne_v = Some(cart_body(&mut s).vx);
            break;
        }
    }
    let v0 = airborne_v.expect("the cart flew off the ledge");
    s.physics.set_throttle(0.0);
    s.physics.set_brake(1.0);
    for _ in 0..8 {
        s.step(&m);
    }
    let v1 = cart_body(&mut s).vx;
    assert!((v1 - v0).abs() < v0.abs() * 0.1, "no ground, no braking: {v0} -> {v1}");
}

#[test]
fn a_balance_assist_keeps_a_tall_thing_upright_and_lean_tips_it() {
    let m = mats();
    let pole = "size 100 100\nfill air\nrect stone 0 90 100 10\nbody rect wood 48 40 4 50\nname pole\n";
    let angle_after = |extra: &str, lean: f32| {
        let mut s = sim(&m, &format!("{pole}{extra}"));
        // A shove to start it falling.
        let id = s.physics.ids()[0];
        s.physics.edit(&mut s.world, &m, id, |b| b.spin = 0.6);
        s.physics.set_lean(lean);
        for _ in 0..180 {
            s.step(&m);
        }
        s.physics.export(&mut s.world)[0].angle
    };
    let free = angle_after("", 0.0);
    let held = angle_after("upright pole strength 60\n", 0.0);
    let leaning = angle_after("upright pole strength 60\n", 1.0);
    println!("angle after 3 s: no assist {free:.2}, assist {held:.2}, assist + lean {leaning:.2}");
    assert!(free.abs() > 0.8, "without help it falls: {free}");
    assert!(held.abs() < 0.15, "the assist holds it up: {held}");
    assert!(leaning > held + 0.1, "leaning tips it forward: {leaning} vs {held}");

    // The assist is part of the scene: it survives save and load.
    let mut s = sim(&m, &format!("{pole}upright pole strength 60\n"));
    let text = scene::save(&s.snapshot(), &m);
    assert!(text.contains("upright obj1 strength 60"), "{text}");
    let (back, problems) = scene::load(&text, &m, None).into_sim(&m);
    assert!(problems.is_empty());
    assert_eq!(back.physics.upright().len(), 1);
}

#[test]
fn driven_tyres_throw_sand_behind_them_but_not_stone() {
    let m = mats();
    let count = |ground: &str| {
        let mut s = sim(&m, &cart(ground));
        let sand = m.id("sand").unwrap();
        let before = s.world.census(&m).into_iter().find(|(n, _, _)| n == "sand").map_or(0, |c| c.1);
        s.physics.set_throttle(1.0);
        let mut most = 0;
        for _ in 0..240 {
            s.step(&m);
            most = most.max(s.world.flying.iter().filter(|f| f.mat == sand).count());
        }
        let after = s.world.census(&m).into_iter().find(|(n, _, _)| n == "sand").map_or(0, |c| c.1);
        (most, before, after, cart_body(&mut s).x)
    };
    let (sand_spray, before, after, x) = count("sand");
    let (stone_spray, _, _, _) = count("stone");
    assert!(sand_spray > 3, "sand flies off the back tyre: {sand_spray}");
    assert_eq!(stone_spray, 0, "stone isn't loose: nothing flies");
    assert_eq!(before, after, "every grain lands somewhere");
    assert!(x > 60.0, "and the cart still drives on: {x}");
}

#[test]
fn dirt_and_wet_sand_come_off_in_clumps() {
    let m = mats();
    for ground in ["dirt", "wet_sand"] {
        let mut s = sim(&m, &cart(ground));
        let id = m.id(ground).unwrap();
        // A running start (on mud the cart is too slow to throw much from
        // a standstill, like a real bike bogging down).
        for _ in 0..20 {
            s.step(&m);
        }
        for b in s.physics.ids() {
            s.physics.edit(&mut s.world, &m, b, |b| b.vx = 70.0);
        }
        s.physics.set_throttle(1.0);
        let mut clumped = false;
        let mut most = 0;
        for _ in 0..90 {
            s.step(&m);
            let fl: Vec<_> = s.world.flying.iter().filter(|f| f.mat == id).collect();
            most = most.max(fl.len());
            // Two cells flying side by side with the same speed: a clump.
            clumped |= fl.iter().any(|a| fl.iter().any(|b| (a.x - b.x).abs() > 0.5 && (a.x - b.x).abs() < 1.5 && (a.y - b.y).abs() < 1.5 && a.vx == b.vx && a.vy == b.vy));
        }
        assert!(most > 2, "{ground}: thrown by the tyre ({most})");
        assert!(clumped, "{ground}: thrown in clumps");
    }
}
