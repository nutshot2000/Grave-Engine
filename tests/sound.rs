//! Sound from materials: what happens in the world is heard, with the
//! right voice (see src/sound.rs). These look at the cues, not the speakers.
use grave_engine::sound::{Kind, Sound, Voice, voice};
use grave_engine::{Materials, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

/// Run a scene for some frames, keeping every cue (as a player would drain them).
fn run(text: &str, m: &Materials, frames: usize) -> Vec<(usize, Sound)> {
    let (mut s, problems) = scene::load(text, m, None).into_sim(m);
    assert!(problems.is_empty(), "{problems:?}");
    let mut heard = Vec::new();
    for f in 0..frames {
        s.step(m);
        heard.extend(std::mem::take(&mut s.world.sounds).into_iter().map(|c| (f, c)));
    }
    heard
}

#[test]
fn dropped_things_are_heard_in_their_own_voice_then_go_quiet() {
    let m = mats();
    for (mat, want) in [("wood", Voice::Wood), ("iron", Voice::Metal), ("stone", Voice::Stone), ("rubber", Voice::Soft)] {
        let text = format!("size 120 80\nfill air\nrect concrete 0 70 120 10\nbody rect {mat} 55 30 8 6\n");
        let heard = run(&text, &m, 240);
        let hits: Vec<&(usize, Sound)> = heard.iter().filter(|(_, c)| c.kind == Kind::Hit).collect();
        assert!(!hits.is_empty(), "{mat}: dropping it made no sound");
        let first = hits[0].1;
        assert_eq!(voice(m.get(first.mat)), want, "{mat}: heard {}", m.get(first.mat).name);
        assert!(first.loud > 0.1, "{mat}: the landing is too quiet ({})", first.loud);
        // Resting on the floor is silent.
        let late = hits.iter().filter(|(f, _)| *f > 180).count();
        assert_eq!(late, 0, "{mat}: still making noise while resting: {hits:?}");
    }
}

#[test]
fn a_harder_landing_is_louder() {
    let m = mats();
    let loudest = |drop: usize| {
        let text = format!("size 120 160\nfill air\nrect concrete 0 150 120 10\nbody rect wood 55 {} 8 6\n", 150 - 6 - drop);
        run(&text, &m, 300).iter().filter(|(_, c)| c.kind == Kind::Hit).map(|(_, c)| c.loud).fold(0.0, f32::max)
    };
    let (low, high) = (loudest(4), loudest(100));
    assert!(high > low + 0.05, "a 10 m drop ({high}) should be louder than a 40 cm one ({low})");
}

#[test]
fn explosions_and_lightning_are_heard() {
    let m = mats();
    let heard = run("size 120 80\nfill air\nrect stone 0 70 120 10\nrect gunpowder 55 62 8 8\nheat circle 59 66 4 900\n", &m, 120);
    assert!(heard.iter().any(|(_, c)| c.kind == Kind::Blast), "the gunpowder went off silently");

    let (mut s, _) = scene::load("size 200 120\nfill air\nrect stone 0 100 200 20\n", &m, None).into_sim(&m);
    s.weather_run.strike_now = true;
    let mut thunder = false;
    for _ in 0..60 {
        s.step(&m);
        thunder |= s.world.sounds.drain(..).any(|c| c.kind == Kind::Thunder);
    }
    assert!(thunder, "a lightning strike made no thunder");
}

#[test]
fn spray_landing_is_heard_as_liquid() {
    let m = mats();
    let (mut s, _) = scene::load("size 120 80\nfill air\nrect stone 0 70 120 10\n", &m, None).into_sim(&m);
    let water = m.id("water").unwrap();
    for k in 0..20 {
        s.world.spawn_flyer(&m, water, 40.0 + k as f32, 20.0, 0.5, 1.5);
    }
    let mut landed = Vec::new();
    for _ in 0..90 {
        s.step(&m);
        landed.extend(s.world.sounds.drain(..).filter(|c| c.kind == Kind::Land));
    }
    assert!(!landed.is_empty(), "the spray landed silently");
    assert!(landed.iter().all(|c| voice(m.get(c.mat)) == Voice::Liquid));
}

#[test]
fn a_reset_does_not_replay_old_sounds() {
    let m = mats();
    let (mut s, _) = scene::load("size 120 80\nfill air\nrect concrete 0 70 120 10\nbody rect iron 55 40 8 6\n", &m, None).into_sim(&m);
    for _ in 0..120 {
        s.step(&m);
    }
    let snap = s.snapshot();
    let again = grave_engine::Sim::restore(&snap, &m);
    assert!(again.world.sounds.is_empty());
}

#[test]
fn a_material_can_choose_its_sound() {
    let mut m = mats();
    let wood = m.id("wood").unwrap();
    m.list[wood as usize].sound = "metal".into();
    assert_eq!(voice(m.get(wood)), Voice::Metal);
    assert_eq!(Voice::parse("glass"), Some(Voice::Glass));
    assert_eq!(Voice::parse("kazoo"), None);
}

