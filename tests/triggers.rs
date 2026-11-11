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
