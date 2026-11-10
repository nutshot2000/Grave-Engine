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

#[test]
fn breakable_weld_releases_under_load_and_roundtrips() {
    let m=mats();
    let text="size 80 120\nfill air\nbody rect iron 30 20 6 6\ndepth 0.02\nname weight\nweld ground weight break 0.01\n";
    let mut weak=sim(&m,text);
    let snap=weak.snapshot();
    assert_eq!(snap.bodies[0].depth,0.02);
    assert_eq!(snap.joints[0].break_force,Some(0.01));
    let encoded=scene::save(&snap,&m);
    let mut restored=sim(&m,&encoded);
    assert_eq!(restored.snapshot().bodies[0].depth,0.02);
    assert_eq!(restored.physics.joints()[0].break_force,Some(0.01));
    let mut strong=sim(&m,&text.replace("break 0.01","break 1000"));
    for _ in 0..45 {weak.step(&m);restored.step(&m);strong.step(&m);}
    assert!(weak.physics.joints().is_empty());
    assert!(restored.physics.joints().is_empty());
    assert_eq!(strong.physics.joints().len(),1);
    let b=weak.physics.spec_of(&weak.world,1).unwrap();
    assert!(b.y>30. && b.vy>20.,"released body must fall with momentum: {b:?}");
}

#[test]
fn depth_changes_mass_without_changing_material_and_survives_fragments() {
    let m=mats();
    let mut thin=sim(&m,"size 100 100\nfill air\nbody rect iron 20 10 20 6\ndepth 0.02\n");
    let thick=sim(&m,"size 100 100\nfill air\nbody rect iron 20 10 20 6\n");
    assert!((thin.physics.mass(1).unwrap()/thick.physics.mass(1).unwrap()-0.02).abs()<0.0001);
    assert!(thin.physics.spec_of(&thin.world,1).unwrap().cells.iter().flatten().all(|c|c.mat==m.id("iron").unwrap()));
    for y in 10..16 {for x in 29..31 {thin.world.set(&m,x,y,0);}}
    thin.step(&m);
    assert_eq!(thin.physics.body_count(),2);
    for id in thin.physics.ids(){assert_eq!(thin.physics.spec_of(&thin.world,id).unwrap().depth,0.02);}
}

#[test]
fn horizontal_drag_preserves_vertical_motion_and_attachments() {
    let m=mats();let mut s=sim(&m,CAR);let ids=s.physics.ids();
    for id in &ids {s.physics.edit(&mut s.world,&m,*id,|b|{b.vx=80.;b.vy=-12.;});}
    let joints=s.physics.joints().len();
    s.physics.damp_horizontal(&ids,0.5);
    for id in &ids {let b=s.physics.spec_of(&s.world,*id).unwrap();assert!((b.vx-40.).abs()<0.01);assert!((b.vy+12.).abs()<0.01);}
    s.physics.damp_horizontal(&ids,f32::NAN);
    assert_eq!(s.physics.joints().len(),joints);
    assert!((s.physics.spec_of(&s.world,ids[0]).unwrap().vx-40.).abs()<0.01);
}
