//! Water has weight on objects: the Millbrook waterwheel turns on the water
//! in its buckets, and a seesaw tips towards the side holding water.
use grave_engine::{Materials, Sim, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

/// Total turns made (unwrapping the angle), stepping for `frames`.
fn turns(s: &mut Sim, m: &Materials, id: u64, frames: usize) -> f32 {
    let (mut last, mut total) = (s.physics.spec_of(&s.world, id).unwrap().angle, 0.0f32);
    for _ in 0..frames {
        s.step(m);
        let a = s.physics.spec_of(&s.world, id).unwrap().angle;
        let d = (a - last + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        total += d;
        last = a;
    }
    total / std::f32::consts::TAU
}

#[test]
fn the_millbrook_wheel_turns_on_the_water_in_its_buckets() {
    let m = mats();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/scenes/millbrook-waterwheel.txt");
    let (mut s, problems) = scene::load(&std::fs::read_to_string(path).unwrap(), &m, std::path::Path::new(path).parent()).into_sim(&m);
    assert!(problems.is_empty(), "{problems:?}");
    let wheel = s.physics.ids()[0];
    let t = turns(&mut s, &m, wheel, 20 * 60);
    println!("{t:.2} turns in 20 s");
    assert!(t.abs() > 1.5, "the water turns the wheel: {t:.2} turns in 20 s");
    assert_eq!(s.physics.body_count(), 1, "nothing broke off");
}

#[test]
fn a_seesaw_tips_towards_the_water() {
    let m = mats();
    // A level plank on a pin, with a little walled tray at each end; water in the right one.
    let text = "size 120 80\nfill air\nrect stone 0 70 120 10\n\
        body rect wood 20 40 80 2\nname plank\nhinge plank ground at 60 41\n\
        body rect wood 20 34 2 6\nname wl\nweld wl plank\nbody rect wood 98 34 2 6\nname wr\nweld wr plank\n\
        body rect wood 86 34 2 6\nname wm\nweld wm plank\nbody rect wood 32 34 2 6\nname wn\nweld wn plank\nrect water 88 34 10 6\n";
    let (mut s, problems) = scene::load(text, &m, None).into_sim(&m);
    assert!(problems.is_empty(), "{problems:?}");
    let plank = s.physics.ids()[0];
    for _ in 0..90 {
        s.step(&m);
    }
    let a = s.physics.spec_of(&s.world, plank).unwrap().angle;
    println!("plank angle {:.1}°", a.to_degrees());
    assert!(a > 0.05, "the water's side goes down (clockwise): {:.1}°", a.to_degrees());
}

#[test]
fn a_lone_crumb_falls_but_anchored_scenery_stays() {
    let m = mats();
    // A single brass pixel in mid-air next to wood that burns away, and an
    // anchored lone pixel (decoration) that must stay put.
    let text = "size 80 60\nfill air\nrect stone 0 50 80 10\nrect brass 20 20 1 1\nrect wood 21 20 3 1\nheat rect 21 20 3 1 900\n\
                rect iron 60 20 1 1\nanchor 60 20 1 1\n";
    let (mut s, problems) = scene::load(text, &m, None).into_sim(&m);
    assert!(problems.is_empty(), "{problems:?}");
    let (brass, iron) = (m.id("brass").unwrap(), m.id("iron").unwrap());
    for _ in 0..600 {
        s.step(&m);
    }
    let at = |id| (0..s.world.mat.len()).find(|&i| s.world.mat[i] == id).map(|i| i / s.world.w);
    assert_eq!(at(brass), Some(49), "the bolt fell to the ground");
    assert_eq!(at(iron), Some(20), "anchored decoration stays");
}
