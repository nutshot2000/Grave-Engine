//! Camera effects: slow motion, zoom, focus and shake, from triggers or code.

use grave_engine::camera::{Camera, Focus};
use grave_engine::{Materials, Sim, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

/// A ball rolling (falling) through a trigger box with these actions.
fn scene_with(m: &Materials, actions: &str) -> Sim {
    let text = format!("size 100 60\nfill air\nrect stone 0 50 100 10\nbody circle iron 20 20 3\nname ball\ntrigger 0 30 100 20 by ball : {actions}\n");
    let (s, p) = scene::load(&text, m, None).into_sim(m);
    assert!(p.is_empty(), "{p:?}");
    s
}

fn run_until_fired(s: &mut Sim, m: &Materials) -> u32 {
    for k in 0..200 {
        s.step(m);
        if s.triggers[0].fired {
            return k;
        }
    }
    panic!("the trigger never went off");
}

#[test]
fn slow_motion_from_a_trigger_lasts_as_long_as_it_says() {
    let m = mats();
    let mut s = scene_with(&m, "slowmo 0.25 2");
    assert_eq!(s.camera.time_scale(), 1.0);
    run_until_fired(&mut s, &m);
    assert_eq!(s.camera.time_scale(), 0.25, "slow motion starts");
    // 2 seconds as the player feels them, at a quarter speed = 30 frames.
    for _ in 0..28 {
        s.step(&m);
    }
    assert_eq!(s.camera.time_scale(), 0.25, "still slow");
    for _ in 0..3 {
        s.step(&m);
    }
    assert_eq!(s.camera.time_scale(), 1.0, "and back to normal");
}

#[test]
fn focus_it_looks_at_whatever_set_it_off_and_zoom_and_shake_work() {
    let m = mats();
    let mut s = scene_with(&m, "focus it 3 ; zoom 2.5 3 ; shake 5");
    let ball = s.physics.ids()[0];
    run_until_fired(&mut s, &m);
    assert_eq!(s.camera.focus.map(|f| f.value), Some(Focus::Object(ball)));
    assert_eq!(s.camera.zoom.map(|z| z.value), Some(2.5));
    assert!(s.camera.shake > 3.0, "shaking");
    for _ in 0..90 {
        s.step(&m);
    }
    assert_eq!(s.camera.shake, 0.0, "the shake dies away");
    // The camera eases over to the ball and zooms in.
    let mut cam = Camera::default();
    cam.snap(80.0, 10.0, 1.0);
    let mut v = cam.update(&s, (80.0, 10.0, 1.0), (0.0, 0.0), 1.0 / 60.0);
    for _ in 0..120 {
        v = cam.update(&s, (80.0, 10.0, 1.0), (0.0, 0.0), 1.0 / 60.0);
    }
    let (bx, by) = s.physics.centre(ball).unwrap();
    assert!((v.x - bx).abs() < 1.0 && (v.y - by).abs() < 1.0, "looking at the ball: {:?} vs {bx},{by}", (v.x, v.y));
    assert!((v.zoom - 2.5).abs() < 0.1, "zoomed in: {}", v.zoom);
    // `camera reset` puts everything back.
    s.camera.reset();
    assert!(s.camera.focus.is_none() && s.camera.zoom.is_none() && s.camera.slowmo.is_none());
}

#[test]
fn camera_actions_survive_saving_and_bad_ones_are_explained() {
    let m = mats();
    let mut s = scene_with(&m, "slowmo 0.3 1.5 ; zoom 2 ; focus 40 20 2 ; focus it ; focus 1 ; shake 4 ; camera reset");
    let text = scene::save(&s.snapshot(), &m);
    let line = text.lines().find(|l| l.starts_with("trigger")).unwrap();
    assert!(line.contains("slowmo 0.3 1.5") && line.contains("focus 40 20 2") && line.contains("focus it") && line.contains("camera reset"), "{line}");
    let (again, p) = scene::load(&text, &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    assert_eq!(again.triggers[0].actions, s.triggers[0].actions);
    let bad = scene::load("size 50 50\nfill air\ntrigger 0 0 10 10 : slowmo\n", &m, None);
    assert!(!bad.problems.is_empty(), "slowmo needs a number");
}
