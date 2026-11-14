//! Regression tests for bugs found in review: animation triggers, asset part moves, round trips.
use grave_engine::animation::{Animator, Graph};
use grave_engine::asset::{Asset, Brush};
use grave_engine::Materials;
use std::sync::Arc;

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

#[test]
fn graphs_round_trip() {
    for f in ["locomotion", "mixamo-pack", "action-demo"] {
        let text = std::fs::read_to_string(format!("assets/animations/{f}.graph")).unwrap();
        let g = Graph::parse(&text).unwrap_or_else(|e| panic!("{f}: {e}"));
        let again = Graph::parse(&g.to_text()).unwrap_or_else(|e| panic!("{f} round trip: {e}"));
        assert_eq!(g, again, "{f} changes when saved and reloaded");
    }
}

#[test]
fn a_request_during_a_blend_waits_instead_of_being_lost() {
    // idle -> walk blend, then an attack trigger arrives mid-blend (priority 90 should still win).
    let g = Graph::parse(
        "initial idle\nparam speed 0\ntrigger attack\nstate idle clip idle loop speed 1\nstate walk clip walk loop speed 1\nstate hit clip hit once speed 1\n\
         transition idle walk priority 0 after 0 when speed > 0.4\ntransition * hit priority 40 after 0 when attack > 0\nblend idle walk 0.3\n",
    )
    .unwrap();
    let mut a = Animator::new(Arc::new(g)).unwrap();
    a.set("speed", 1.0);
    a.advance(0.016, |_| 1.0);
    assert_eq!(a.state().name, "walk");
    assert!(a.blend.is_some());
    a.trigger("attack");
    a.advance(0.016, |_| 1.0);
    // The trigger is consumed this tick even though the blend blocked the transition.
    for _ in 0..40 {
        a.advance(0.016, |_| 1.0);
    }
    assert_eq!(a.state().name, "hit", "the attack waited for the blend to end");
}

#[test]
fn moving_a_part_over_another_is_refused_not_destructive() {
    let m = mats();
    let wood = m.id("wood").unwrap();
    let iron = m.id("iron").unwrap();
    let mut a = Asset::blank("t", 8, 8).unwrap();
    let b = a.add_part("beam").unwrap();
    a.draw(0, Brush::Rectangle, (0, 0), (3, 3), wood);
    a.draw(b, Brush::Rectangle, (4, 0), (7, 3), iron);
    let before: usize = a.parts[b].cells.iter().filter(|c| **c != 0).count();
    a.move_part(0, 4, 0);
    let after: usize = a.parts[b].cells.iter().filter(|c| **c != 0).count();
    assert_eq!((before, after), (16, 16), "moving onto another part is refused, nothing erased");
}

#[test]
fn asset_text_round_trips_with_many_materials() {
    let m = mats();
    let mut a = Asset::blank("many", 100, 2).unwrap();
    let n = m.list.len();
    println!("{n} materials");
    for x in 0..100.min(n.saturating_sub(1)) {
        a.put(0, x as i32, 0, (x + 1) as u16 as _);
    }
    let text = a.to_text(&m);
    let b = Asset::parse(&text, &m).expect("reload");
    assert_eq!(a.parts[0].cells, b.parts[0].cells);
}

#[test]
fn asset_files_can_use_more_than_88_materials() {
    let names: Vec<String> = (0..120).map(|i| format!("m{i}")).collect();
    let texts: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "state = solid\n")).collect();
    let m = Materials::from_texts(&texts);
    let mut a = Asset::blank("many", 120, 1).unwrap();
    for x in 0..120 {
        a.put(0, x, 0, m.id(&format!("m{x}")).unwrap());
    }
    let b = Asset::parse(&a.to_text(&m), &m).expect("an asset with 120 materials saves and loads");
    assert_eq!(a.parts[0].cells, b.parts[0].cells);
}
