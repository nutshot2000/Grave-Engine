//! Moving parts in assets: hinges (pivots), motors, wheels, ropes, pins to the ground.
use grave_engine::asset::{Asset, JointType};
use grave_engine::{Materials, Sim};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn ground(m: &Materials) -> Sim {
    let mut s = Sim::empty(220, 130, m);
    let stone = m.id("stone").unwrap();
    for y in 110..130 {
        for x in 0..220 {
            s.world.set(m, x, y, stone);
        }
    }
    s
}

fn angle(s: &Sim, id: u64) -> f32 {
    s.physics.spec_of(&s.world, id).unwrap().angle
}

#[test]
fn the_pumpjack_nods_by_itself() {
    let m = mats();
    let a = Asset::load_file(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/workshop/oil_pumpjack.asset")), &m).unwrap();
    assert!(a.validate(&m).is_empty(), "{:?}", a.validate(&m));
    assert!(a.joints.iter().any(|j| j.motor.is_some()));
    let mut s = ground(&m);
    let ids = a.instantiate(&mut s, &m, 110., 110.).unwrap();
    let id = |n: &str| ids.iter().find(|(name, _)| name == n).unwrap().1;
    let (beam, crank, base) = (id("beam"), id("crank"), id("base"));
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    let base_start = s.physics.spec_of(&s.world, base).unwrap();
    for f in 0..600 {
        s.step(&m);
        let b = angle(&s, beam);
        lo = lo.min(b);
        hi = hi.max(b);
        if f % 60 == 0 {
            println!("frame {f}: beam {:.1}°, crank {:.0}°", b.to_degrees(), angle(&s, crank).to_degrees());
        }
    }
    println!("beam swung between {:.1}° and {:.1}°", lo.to_degrees(), hi.to_degrees());
    assert!((hi - lo).to_degrees() > 8.0, "the beam rocks: {:.1}°..{:.1}°", lo.to_degrees(), hi.to_degrees());
    let base_now = s.physics.spec_of(&s.world, base).unwrap();
    assert!((base_now.x - base_start.x).abs() < 1.0 && base_now.angle.abs() < 0.02, "the base stays pinned to the ground");
    assert_eq!(s.physics.body_count(), 4, "nothing broke off");
}

#[test]
fn joints_save_load_and_reach_scenes() {
    let m = mats();
    let text = "format 1\nasset arm 20 10\npivot 10 9\npart post\nrect iron 9 2 2 8\npart lever\nrect wood 2 1 16 1\n\
                joint hinge lever post at 10 1.5 motor -0.5 strength 3 drive break 90\njoint hinge post ground at 10 9\n\
                joint rope lever post at 2 1.5 to 9.5 9 length 4\njoint wheel lever post at 17 1.5 spring 80 damping 5 travel 2\n";
    let a = Asset::parse(text, &m).unwrap();
    assert_eq!(a.joints.len(), 4);
    assert!(matches!(a.joints[2].kind, JointType::Rope { length } if length == 4.0));
    let m1 = a.joints[0].motor.as_ref().unwrap();
    assert!(m1.drive && m1.speed == -0.5 && m1.strength == 3.0 && a.joints[0].break_force == Some(90.0));
    let again = Asset::parse(&a.to_text(&m), &m).expect("saved joints load again");
    assert_eq!(a.joints, again.joints);
    // A scene placing the asset gets the same moving joints.
    let scene = a.scene_text(&m, 50., 40.);
    assert!(scene.contains("hinge lever post at 50 32.5 motor -0.5 strength 3 drive break 90"), "{scene}");
    assert!(scene.contains("hinge post ground"), "{scene}");
    // Renaming or removing a part keeps the joints consistent.
    let mut b = a.clone();
    b.rename_part(1, "boom").unwrap();
    assert!(b.joints.iter().all(|j| j.a != "lever" && j.b != "lever") && b.joints.iter().any(|j| j.a == "boom"));
    b.remove_part(1);
    assert_eq!(b.joints.len(), 1, "only the post's pin to the ground is left");
}

#[test]
fn joint_mistakes_are_explained() {
    let m = mats();
    let head = "format 1\nasset t 10 10\npart a\nrect iron 1 1 3 3\npart b\nrect iron 5 5 3 3\n";
    for (line, needle) in [
        ("joint hinge a c at 2 2", "missing part"),
        ("joint hinge a b", "joint hinge"),
        ("joint spin a b at 2 2", "joint hinge"),
        ("joint rope a b at 2 2 to 6 6 motor 1", "rope can't have a motor"),
        ("joint hinge ground a at 2 2", "only the second"),
    ] {
        let e = Asset::parse(&format!("{head}{line}\n"), &m).unwrap_err();
        assert!(e.contains(needle), "{line}: {e}");
    }
}
