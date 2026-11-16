//! Soaked characters (src/physics/actor/soak.rs): water and oil soak into a
//! character, dry off, protect from heat (water) or set it alight (oil).
use grave_engine::physics::{Controls, Soak};
use grave_engine::{Materials, Sim, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn sim(m: &Materials, text: &str) -> Sim {
    let (sim, problems) = scene::load(text, m, None).into_sim(m);
    assert!(problems.is_empty(), "{problems:?}");
    sim
}

fn run(s: &mut Sim, m: &Materials, frames: usize, walk: f32) {
    s.physics.set_controls(Controls { walk, ..Default::default() });
    for _ in 0..frames {
        s.step(m);
    }
}

fn soak(s: &Sim) -> Option<Soak> {
    s.physics.actor_status()[0].soak
}

#[test]
fn standing_in_water_soaks_and_it_dries_off() {
    let m = mats();
    let mut s = sim(&m, "size 300 80
fill air
rect stone 0 70 300 10
rect stone 100 60 2 10
rect stone 140 60 2 10
rect water 102 62 38 8
player 120 70
");
    let water = m.id("water").unwrap();
    run(&mut s, &m, 120, 0.0);
    let k = soak(&s).expect("standing in the pool soaks");
    assert_eq!(k.liquid, water);
    assert!(k.amount > 0.5, "two seconds in the pool: {}", k.amount);
    // Drain the pool: water on a body dries in about 15 s.
    for y in 60..70 {
        for x in 102..140 {
            let i = s.world.idx(x, y);
            if s.world.mat[i] == water {
                s.world.make_air(i);
            }
        }
    }
    run(&mut s, &m, 60 * 20, 0.0);
    assert!(soak(&s).is_none(), "still wet after 20 s: {:?}", soak(&s));
}

#[test]
fn oil_soaked_catches_fire_and_water_puts_it_out() {
    let m = mats();
    let oil = m.id("oil").unwrap();
    // Standing in an oil puddle next to a fire.
    let mut s = sim(&m, "size 200 80\nfill air\nrect stone 0 70 200 10\nrect stone 70 64 2 6\nrect stone 110 64 2 6\nrect oil 72 66 38 4\nplayer 90 70\n");
    run(&mut s, &m, 90, 0.0);
    let k = soak(&s).expect("standing in oil soaks");
    assert_eq!(k.liquid, oil);
    assert!(!k.burning);
    // Light it.
    s.world.heat_circle(90, 60, 4, 900.0);
    let health = s.physics.actor_status()[0].health;
    let mut lit = false;
    for _ in 0..60 {
        run(&mut s, &m, 1, 0.0);
        lit |= soak(&s).is_some_and(|k| k.burning);
    }
    assert!(lit, "an oil-soaked character near flames should catch fire");
    assert!(s.physics.actor_status()[0].health < health, "being on fire hurts");
}

#[test]
fn a_dunk_in_water_washes_off_oil() {
    let m = mats();
    let water = m.id("water").unwrap();
    let mut s = sim(&m, "size 200 100\nfill air\nrect stone 0 90 200 10\nrect stone 60 60 2 30\nrect stone 140 60 2 30\nrect water 62 64 78 26\nplayer 100 60\n");
    s.physics.edit_actor(0, |a| a.soak = Some(Soak { liquid: m.id("oil").unwrap(), amount: 1.0, burning: true }));
    run(&mut s, &m, 120, 0.0);
    let k = soak(&s).expect("in a pool: soaked");
    assert_eq!(k.liquid, water, "the oil washed off");
    assert!(!k.burning, "the water put the fire out");
}

#[test]
fn a_wet_body_takes_less_heat_damage() {
    let m = mats();
    let hurt = |wet: bool| {
        let mut s = sim(&m, "size 120 60\nfill air\nrect stone 0 50 120 10\nplayer 60 50\n");
        run(&mut s, &m, 30, 0.0);
        if wet {
            s.physics.edit_actor(0, |a| a.soak = Some(Soak { liquid: m.id("water").unwrap(), amount: 1.0, burning: false }));
        }
        let before = s.physics.actor_status()[0].health;
        for _ in 0..40 {
            s.world.heat_circle(60, 42, 7, 3.0);
            run(&mut s, &m, 1, 0.0);
        }
        before - s.physics.actor_status()[0].health
    };
    let (dry, wet) = (hurt(false), hurt(true));
    assert!(dry > 1.0, "the heat should hurt ({dry})");
    assert!(wet < dry * 0.85, "wet {wet} should hurt less than dry {dry}");
}

#[test]
fn rain_wets_a_character() {
    let m = mats();
    let mut s = sim(&m, "size 200 80\nfill air\nrect stone 0 70 200 10\nplayer 100 70\nweather rain 1\n");
    run(&mut s, &m, 600, 0.0);
    assert!(soak(&s).is_some_and(|k| k.liquid == m.id("water").unwrap()), "standing in the rain gets you wet");
}


