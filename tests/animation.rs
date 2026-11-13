//! Graph behavior, actor integration, persistence and invalid-input regression tests.
use grave_engine::animation::{Animator, DEFAULT, Graph};
use grave_engine::{Materials, Sim, scene};
use std::sync::Arc;

fn player(text: &str) -> Animator {
    Animator::new(Arc::new(Graph::parse(text).unwrap())).unwrap()
}
const ACTION: &str = "initial idle
param speed 0
param dead 0
trigger attack
state idle clip idle loop speed 1
state attack clip walk once speed 1
event attack 0.1 hit
state dead clip die once speed 1
transition * dead priority 100 after 0 when dead > 0
transition idle attack priority 10 after 0 when attack > 0
transition attack idle priority 0 after 0.2 when finished > 0
";

#[test]
fn actions_finish_markers_fire_once_and_death_interrupts() {
    let mut a = player(ACTION);
    a.trigger("attack");
    assert_eq!(a.advance(0.01, |_| 0.3)[0].name, "enter");
    assert_eq!(a.state().name, "attack");
    assert!(a.advance(0.05, |_| 0.3).is_empty());
    assert_eq!(a.advance(0.06, |_| 0.3)[0].name, "hit");
    assert!(a.advance(0.1, |_| 0.3).is_empty());
    assert_eq!(
        a.state().name,
        "attack",
        "one-shot cannot exit before its end"
    );
    a.advance(0.1, |_| 0.3);
    assert_eq!(a.state().name, "idle");
    a.trigger("attack");
    a.set("dead", 1.0);
    a.advance(0.01, |_| 0.3);
    assert_eq!(a.state().name, "dead", "priority beats file position");
    a.advance(10.0, |_| 0.3);
    assert!(a.sample_time(|_| 0.3) < 0.3);
    assert!(
        a.sample_time(|_| 0.3) > 0.29,
        "one-shot holds its last frame"
    );
}

#[test]
fn cloned_entities_are_independent_and_trigger_requests_expire() {
    let mut a = player(ACTION);
    let mut b = a.clone();
    a.trigger("attack");
    a.advance(0.01, |_| 0.3);
    b.advance(0.01, |_| 0.3);
    assert_eq!(a.state().name, "attack");
    assert_eq!(b.state().name, "idle");
    a.trigger("attack");
    a.advance(0.5, |_| 0.3);
    a.advance(0.01, |_| 0.3);
    assert_eq!(
        a.state().name,
        "idle",
        "unhandled trigger must not remain latched"
    );
    assert!(!a.trigger("typo"));
    assert!(!a.set("speed", f32::NAN));
    assert!(!a.set("typo", 1.0));
}

#[test]
fn looping_markers_rate_and_real_time_exit_gates() {
    let mut a = player(
        "initial walk\nparam speed 2\nstate walk clip walk loop speed 1 rate speed 0.5 0 2\nevent walk 0 start\nevent walk 0.25 step\nstate end clip idle once speed 1\ntransition walk end priority 0 after 1 when speed == 0\n",
    );
    assert_eq!(a.advance(0.3, |_| 0.5).len(), 2);
    let e = a.advance(0.5, |_| 0.5);
    assert!(e.iter().any(|e| e.name == "step"));
    assert!((a.sample_time(|_| 0.5) - 0.3).abs() < 0.0001);
    a.set("speed", 0.0);
    a.advance(0.1, |_| 0.5);
    assert_eq!(a.state().name, "walk");
    a.advance(0.2, |_| 0.5);
    assert_eq!(
        a.state().name,
        "end",
        "after uses real state time, even with frozen clip"
    );
}

#[test]
fn ties_keep_file_order_and_only_one_transition_runs_per_tick() {
    let mut a = player(
        "initial a\nstate a clip idle loop speed 1\nstate b clip idle loop speed 1\nstate c clip idle loop speed 1\ntransition a b priority 1 after 0 when\ntransition a c priority 1 after 0 when\ntransition b c priority 9 after 0 when\n",
    );
    a.advance(0.01, |_| 1.0);
    assert_eq!(a.state().name, "b");
    a.advance(0.01, |_| 1.0);
    assert_eq!(a.state().name, "c");
    a.advance(f32::NAN, |_| 1.0);
    assert!(a.time.is_finite());
}

#[test]
fn roundtrip_and_validation_reject_broken_graphs() {
    let g = Graph::parse(DEFAULT).unwrap();
    assert_eq!(Graph::parse(&g.to_text()).unwrap(), g);
    for text in [
        DEFAULT.replace("initial idle", "initial missing"),
        DEFAULT.replace("speed > 0.4", "typo > 0.4"),
        DEFAULT.replace(
            "state idle clip idle loop speed 1",
            "state idle clip idle loop speed NaN",
        ),
        DEFAULT.replace("transition idle walk", "transition idle missing"),
        format!("{DEFAULT}\nstate idle clip idle loop speed 1"),
        format!("{DEFAULT}\nparam finished 0"),
        format!("{DEFAULT}\ntransition idle walk priority 1 after -1 when"),
    ] {
        assert!(Graph::parse(&text).is_err(), "{text}");
    }
}

#[test]
fn markers_outside_a_clip_do_not_fire_after_it_finishes() {
    for mode in ["once", "loop"] {
        let mut a = player(&format!("initial a\nstate a clip idle {mode} speed 1\nevent a 2 outside\n"));
        assert!(a.advance(3.0, |_| 1.0).is_empty());
    }
}

#[test]
fn imported_pack_switches_water_states_and_keeps_traversal_actions_stable() {
    let mut a = player(include_str!("../assets/animations/mixamo-pack.graph"));
    a.set("swimming", 1.0);
    a.advance(0.01, |_| 1.0);
    assert_eq!(a.state().name, "tread");
    a.set("swim_moving", 1.0);
    a.advance(0.01, |_| 1.0);
    assert_eq!(a.state().name, "swim");
    a.set("swim_moving", 0.0);
    a.advance(0.01, |_| 1.0);
    assert_eq!(a.state().name, "tread");
    a.set("swimming", 0.0);
    a.advance(0.01, |_| 1.0);
    a.trigger("grab"); a.advance(0.01, |_| 1.0);
    assert_eq!(a.state().name, "hang");
    a.set("airborne", 1.0); a.set("vy", 2.0);
    a.advance(2.0, |_| 1.0);
    assert_eq!(a.state().name, "hang", "movement rules must not interrupt hanging");
    a.trigger("climb"); a.advance(0.01, |_| 1.0);
    assert_eq!(a.state().name, "climb");
    a.advance(0.5, |_| 1.0);
    assert_eq!(a.state().name, "climb");
    a.set("dead", 1.0); a.advance(0.01, |_| 1.0);
    assert_eq!(a.state().name, "die");
    a.reset(); a.trigger("climb"); a.advance(0.01, |_| 1.0);
    a.advance(1.1, |_| 1.0);
    assert_eq!(a.state().name, "idle");
    a.trigger("grab"); a.advance(0.01, |_| 1.0);
    a.trigger("release"); a.advance(0.01, |_| 1.0);
    assert_eq!(a.state().name, "idle");
    a.set("speed", 4.0); a.advance(0.01, |_| 1.0);
    assert_eq!(a.state().name, "run");
    a.advance(0.01, |_| 1.0);
    assert_eq!(a.state().name, "run", "gaits must not oscillate");
    a.set("speed", 1.0); a.advance(0.01, |_| 1.0);
    assert_eq!(a.state().name, "walk");
    a.trigger("ual_sitting_enter"); a.advance(0.01, |_| 1.0);
    a.advance(1.1, |_| 1.0);
    assert_eq!(a.state().name, "ual_sitting_idle_loop");
    a.trigger("release"); a.advance(0.01, |_| 1.0);
    assert_eq!(a.state().name, "idle");
}

fn setup() -> (Materials, Sim) {
    let m = Materials::load_dir("materials");
    let (s, errors) = scene::load(
        "size 300 140\nfill air\nrect stone 0 130 300 10\nplayer 40 130 look hero\n",
        &m,
        None,
    )
    .into_sim(&m);
    assert!(errors.is_empty(), "{errors:?}");
    (m, s)
}

#[test]
fn actor_graph_replaces_rules_and_survives_rewind_and_scene_save() {
    let (m, mut s) = setup();
    s.physics
        .set_actor_graph(0, Arc::new(Graph::parse(ACTION).unwrap()))
        .unwrap();
    assert!(s.physics.trigger_actor_animation(0, "attack"));
    s.step(&m);
    assert_eq!(s.physics.actor_status()[0].animation_state, "attack");
    for _ in 0..8 {
        s.step(&m);
    }
    let markers = s.physics.take_actor_animation_events(0);
    assert!(markers.iter().any(|e| e.name == "hit"));
    assert!(s.physics.take_actor_animation_events(0).is_empty());
    let snapshot = s.snapshot();
    let restored = Sim::restore(&snapshot, &m);
    let a = s.physics.actor_status()[0].clone();
    let b = restored.physics.actor_status()[0].clone();
    assert_eq!(a.animation_state, b.animation_state);
    assert_eq!(a.anim_time, b.anim_time);
    let text = scene::save(&snapshot, &m);
    assert!(text.contains("endanimation"));
    let (mut loaded, errors) = scene::load(&text, &m, None).into_sim(&m);
    assert!(errors.is_empty(), "{errors:?}");
    loaded.physics.trigger_actor_animation(0, "attack");
    loaded.step(&m);
    assert_eq!(loaded.physics.actor_status()[0].animation_state, "attack");
}

#[test]
fn default_actor_runs_takeoff_fall_landing_and_walk() {
    let (m, mut s) = setup();
    for _ in 0..20 {
        s.step(&m);
    }
    s.physics.set_controls(grave_engine::physics::Controls {
        walk: 1.0,
        jump: true,
    });
    let mut states = std::collections::BTreeSet::new();
    for i in 0..120 {
        if i == 1 {
            s.physics.set_controls(grave_engine::physics::Controls {
                walk: 1.0,
                jump: false,
            });
        }
        s.step(&m);
        states.insert(s.physics.actor_status()[0].animation_state.clone());
    }
    for expected in ["jump", "fall", "landing", "walk"] {
        assert!(states.contains(expected), "missing {expected}: {states:?}");
    }
}

#[test]
fn invalid_actor_or_scene_graph_does_not_replace_good_state() {
    let (m, mut s) = setup();
    let bad = DEFAULT.replace("clip idle", "clip unavailable");
    assert!(
        s.physics
            .set_actor_graph(0, Arc::new(Graph::parse(&bad).unwrap()))
            .is_err()
    );
    assert_eq!(s.physics.actor_status()[0].animation_state, "idle");
    let result = scene::load(
        "size 30 30\nplayer 10 20\nanimation\ninitial missing\nendanimation\n",
        &m,
        None,
    );
    assert!(!result.problems.is_empty());
}
