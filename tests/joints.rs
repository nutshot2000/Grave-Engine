//! Joints, motors and driving.

use grave_engine::edit::History;
use grave_engine::physics::{GROUND, JointKind, Motor};
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

const CAR: &str = "size 300 100\nfill air\nrect stone 0 90 300 10\n\
body rect wood 20 70 40 4\nname car\n\
body circle rubber 26 80 5\nname back\n\
body circle rubber 54 80 5\nname front\n\
wheel car back spring 40 damping 8 travel 2 motor 2 strength 8 drive\n\
wheel car front spring 40 damping 8 travel 2 motor 2 strength 8 drive\n";

fn car_x(s: &mut Sim) -> f32 {
    let specs = s.physics.export(&mut s.world);
    specs.iter().max_by_key(|b| b.cell_count()).unwrap().x
}

#[test]
fn a_hinged_plank_swings_like_a_pendulum() {
    let m = mats();
    let mut s = sim(&m, "size 100 100\nfill air\nbody rect wood 50 20 30 3\nname plank\nhinge ground plank at 50 21\n");
    assert_eq!(s.physics.joints().len(), 1);
    let mut lowest = 0.0f32;
    for _ in 0..120 {
        s.step(&m);
        let b = &s.physics.export(&mut s.world)[0];
        lowest = lowest.max(b.y);
        // The pinned end stays at the pin.
        let (c, sn) = (b.angle.cos(), b.angle.sin());
        let (px, py) = (b.x + c * -14.5 - sn * 0.0, b.y + sn * -14.5 + c * 0.0);
        assert!((px - 50.0).abs() < 1.5 && (py - 21.0).abs() < 1.5, "pin end wandered to ({px}, {py})");
    }
    assert!(lowest > 30.0, "it should swing down, lowest centre y={lowest}");
}

#[test]
fn a_car_drives_when_you_press_go_and_coasts_when_you_dont() {
    let m = mats();
    let mut parked = sim(&m, CAR);
    let start = car_x(&mut parked);
    for _ in 0..180 {
        parked.step(&m);
    }
    assert!((car_x(&mut parked) - start).abs() < 3.0, "no throttle: it stays put");

    let mut driven = sim(&m, CAR);
    driven.physics.set_throttle(1.0);
    for _ in 0..180 {
        driven.step(&m);
    }
    let moved = car_x(&mut driven) - start;
    assert!(moved > 40.0, "full throttle for 3 s should go a long way, went {moved}");

    let mut back = sim(&m, CAR);
    back.physics.set_throttle(-1.0);
    for _ in 0..120 {
        back.step(&m);
    }
    assert!(car_x(&mut back) < start - 10.0, "reverse goes backwards");
}

#[test]
fn welded_things_stay_together() {
    let m = mats();
    let mut s = sim(&m, "size 100 100\nfill air\nrect stone 0 90 100 10\nbody rect wood 30 20 20 4\nname a\nbody rect iron 50 20 6 6\nname b\nweld a b at 50 22\n");
    let gap = |s: &mut Sim| {
        let b = s.physics.export(&mut s.world);
        ((b[0].x - b[1].x).powi(2) + (b[0].y - b[1].y).powi(2)).sqrt()
    };
    let before = gap(&mut s);
    for _ in 0..180 {
        s.step(&m);
    }
    assert!((gap(&mut s) - before).abs() < 1.0, "the two should keep their distance");
}

#[test]
fn a_rope_holds_things_up() {
    let m = mats();
    let mut s = sim(&m, "size 100 100\nfill air\nbody rect iron 50 40 6 6\nname lamp\nrope ground lamp at 53 10 to 53 40 length 30\n");
    for _ in 0..180 {
        s.step(&m);
    }
    let b = &s.physics.export(&mut s.world)[0];
    assert!(b.y < 50.0, "the rope should stop it falling, y={}", b.y);
}

#[test]
fn joints_survive_save_load_undo_and_reset() {
    let m = mats();
    let mut s = sim(&m, CAR);
    // Save and load.
    let text = scene::save(&s.snapshot(), &m);
    assert!(text.contains("wheel obj1 obj2"), "{text}");
    let mut s2 = sim(&m, &text);
    assert_eq!(s2.physics.joints().len(), 2);
    s2.physics.set_throttle(1.0);
    let x0 = car_x(&mut s2);
    for _ in 0..120 {
        s2.step(&m);
    }
    assert!(car_x(&mut s2) > x0 + 15.0, "the loaded car still drives");

    // Undo removing a joint.
    let mut h = History::new();
    let car = s.physics.ids()[0];
    h.touch(&mut s, "unjoin");
    s.physics.unjoin(car);
    h.commit(&mut s);
    assert_eq!(s.physics.joints().len(), 0);
    h.undo(&mut s, &m);
    assert_eq!(s.physics.joints().len(), 2, "undo brings the joints back");

    // Snapshot/restore (Reset and Rewind use this).
    let snap = s.snapshot();
    let back = Sim::restore(&snap, &m);
    assert_eq!(back.physics.joints().len(), 2);
}

#[test]
fn moving_a_jointed_object_keeps_its_joints() {
    let m = mats();
    let mut s = sim(&m, CAR);
    let car = s.physics.ids()[0];
    s.physics.edit(&mut s.world, &m, car, |b| b.x += 5.0);
    assert_eq!(s.physics.joints().len(), 2);
}

#[test]
fn joining_in_code() {
    let m = mats();
    let mut s = sim(&m, "size 100 100\nfill air\nbody rect wood 50 20 30 3\n");
    let id = s.physics.ids()[0];
    let motor = Some(Motor { speed: 0.5, strength: 5.0, drive: false });
    assert!(s.physics.join(&m, GROUND, id, JointKind::Hinge, (65.0, 21.0), None, motor));
    for _ in 0..60 {
        s.step(&m);
    }
    let b = &s.physics.export(&mut s.world)[0];
    assert!(b.spin.abs() > 1.0, "a motorised hinge keeps it turning, spin {}", b.spin);
}
