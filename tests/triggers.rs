//! Triggers: level events as scene data.

use grave_engine::trigger::Event;
use grave_engine::{Materials, Sim, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn sim(m: &Materials, text: &str) -> Sim {
    let (s, p) = scene::load(text, m, None).into_sim(m);
    assert!(p.is_empty(), "{p:?}");
    s
}

/// A crate falling past a trigger box, a fuse and a checkpoint.
const DROP: &str = "size 100 120\nfill air\nrect stone 0 110 100 10\nrect gunpowder 70 104 6 6\n\
    body rect wood 20 10 8 8\nname crate\n\
    trigger 10 50 30 20 by crate : heat rect 70 104 6 6 600 ; message FUSE LIT ; score 100 ; checkpoint 24 40\n\
    trigger 0 0 100 120 by player : message never\n";

#[test]
fn a_trigger_fires_once_when_its_object_enters() {
    let m = mats();
    let mut s = sim(&m, DROP);
    let mut events = Vec::new();
    let powder = m.id("gunpowder").unwrap();
    let before = s.world.mat.iter().filter(|&&c| c == powder).count();
    for _ in 0..240 {
        s.step(&m);
        events.extend(s.events.drain(..));
    }
    let after = s.world.mat.iter().filter(|&&c| c == powder).count();
    assert_eq!(events, vec![Event::Message("FUSE LIT".into()), Event::Score(100), Event::Checkpoint(24.0, 40.0)], "fired once, in order");
    assert!(after < before / 2, "the heat set off real gunpowder: {before} -> {after}");
    assert_eq!(s.checkpoint, Some((24.0, 40.0)));
    assert!(!s.triggers[1].fired, "no player in this scene");
}

#[test]
fn triggers_survive_save_load_and_restore() {
    let m = mats();
    let mut s = sim(&m, DROP);
    for _ in 0..120 {
        s.step(&m);
    }
    assert!(s.triggers[0].fired);
    // Restored (reset, rewind, undo): still fired, still tied to the crate.
    let snap = s.snapshot();
    let back = Sim::restore(&snap, &m);
    assert!(back.triggers[0].fired && back.checkpoint == s.checkpoint);
    // Saved: written as trigger lines, and loads again.
    let text = scene::save(&snap, &m);
    assert!(text.contains("trigger 10 50 30 20 by obj1 fired : heat rect 70 104 6 6 600 ; message FUSE LIT ; score 100 ; checkpoint 24 40"), "{text}");
    let loaded = sim(&m, &text);
    assert!(loaded.triggers[0].fired);
}

#[test]
fn repeat_triggers_fire_every_time_something_enters() {
    let m = mats();
    // A ball bouncing on rubber through a low box: it fires on each entry.
    let mut s = sim(&m, "size 60 100\nfill air\nrect rubber 0 90 60 10\nbody circle rubber 30 40 3\nname ball\ntrigger 0 75 60 15 by ball repeat : score 1\n");
    let mut score = 0;
    for _ in 0..600 {
        s.step(&m);
        score += s.events.drain(..).filter(|e| matches!(e, Event::Score(_))).count();
    }
    assert!(score >= 2, "fired on each bounce: {score}");
}

#[test]
fn bad_trigger_lines_say_what_is_wrong() {
    let m = mats();
    let l = scene::load("size 50 50\nfill air\ntrigger 1 2 3 4 by nobody : score 1\ntrigger 1 2 3 4 : explode\ntrigger 1 2 3 4\n", &m, None);
    assert_eq!(l.problems.len(), 3, "{:?}", l.problems);
}

#[test]
fn a_fast_thing_cannot_leap_over_a_thin_trigger() {
    use grave_engine::trigger::{Action, Trigger, By};
    // The box is 2 cells wide; a thing moving 20 cells a step is never inside it.
    let t = Trigger { x: 45.0, y: 30.0, w: 2.0, h: 20.0, by: By::Any, repeat: false, actions: vec![Action::Score(1)], fired: false, occupied: false };
    assert!(!t.contains(40.0, 41.0) && !t.contains(60.0, 41.0));
    assert!(t.crossed((40.0, 41.0), (60.0, 41.0)), "the path goes through it");
    assert!(!t.crossed((40.0, 10.0), (60.0, 10.0)), "above it, no");
    assert!(!t.crossed((50.0, 41.0), (60.0, 41.0)), "past it, no");
    assert!(!t.crossed((-500.0, 41.0), (60.0, 41.0)), "a teleport doesn't count");
    // And in a running scene: a heavy iron ball thrown fast through a thin box.
    let m = mats();
    let text = "size 200 100\nfill air\nbody circle iron 10 40 3\nname ball\ntrigger 100 20 2 40 by ball : score 5\n";
    let mut s = sim(&m, text);
    let id = s.physics.ids()[0];
    s.physics.set_velocity(id, 1200.0, 0.0, 0.0);
    let mut score = 0;
    for _ in 0..60 {
        s.step(&m);
        for e in s.events.drain(..) {
            if let Event::Score(n) = e {
                score += n;
            }
        }
    }
    assert_eq!(score, 5, "the thin trigger saw the ball go through");
}

#[test]
fn scrolling_and_loading_preserve_trigger_targets_and_entry_state() {
    let m = mats();
    let mut s = sim(&m, "size 100 100\nplayer 40 60\ntrigger 20 20 45 50 by player repeat : checkpoint 30 60 ; heat rect 20 70 2 2 420 ; score 1\n");
    s.shift(8, -5);
    s.step(&m);
    assert_eq!(s.checkpoint, Some((38., 55.)));
    assert_eq!(s.world.temp[s.world.idx(28, 65)], 420.);
    assert!(s.events.contains(&Event::Score(1)));
    s.events.clear();
    s.shift(-2, 3);
    s.step(&m);
    assert!(!s.events.contains(&Event::Score(1)));
    let mut loaded = sim(&m, &scene::save(&s.snapshot(), &m));
    loaded.step(&m);
    assert!(!loaded.events.contains(&Event::Score(1)));
}
