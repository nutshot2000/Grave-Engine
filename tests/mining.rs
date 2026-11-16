//! Mining resistance, occlusion, persistent fractures and the playable test cavern.
use grave_engine::{
    Materials, Sim,
    edit::{History, Timeline},
    mining::{Drill, Mining},
    render::Renderer,
    scene,
};
fn wall(name: &str) -> (Materials, Sim) {
    let m = Materials::load_dir("materials");
    assert!(m.problems.is_empty(), "{:?}", m.problems);
    let (s, p) = scene::load(
        &format!("size 120 110\nfill air\nrect {name} 60 20 35 70\nanchor 60 20 35 70\n"),
        &m,
        None,
    )
    .into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    (m, s)
}
fn strike(s: &mut Sim, m: &Materials) {
    s.mining
        .strike(&mut s.world, m, (48., 60.), (1., 0.), Drill::default());
    s.world.frame += 6;
}
#[test]
fn soft_rock_and_basalt_require_different_work_and_each_strike_only_touches_the_face() {
    let mut times = Vec::new();
    for mat in ["dirt", "sandstone", "rock", "basalt"] {
        let (m, mut s) = wall(mat);
        let i = s.world.idx(60, 60);
        let deep = s.world.idx(65, 60);
        strike(&mut s, &m);
        assert_ne!(
            s.world.mat[deep], 0,
            "a strike must not erase a sphere behind the face"
        );
        if mat == "rock" {
            assert_ne!(s.world.mat[i], 0);
            assert!(s.mining.fractures[&i].work > 0.0);
            assert!(
                s.world
                    .sounds
                    .iter()
                    .any(|s| s.kind == grave_engine::sound::Kind::Hit)
            );
        }
        let mut hits = 1;
        while s.world.mat[i] != 0 && hits < 100 {
            strike(&mut s, &m);
            hits += 1;
        }
        assert_eq!(s.world.mat[i], 0, "{mat} should eventually break");
        times.push(hits);
    }
    assert!(times.windows(2).all(|w| w[0] < w[1]), "{times:?}");
    assert!(
        times[3] > times[2] * 2,
        "basalt must be a meaningful route choice: {times:?}"
    );
}
#[test]
fn bedrock_blocks_rewards_and_owned_objects_liquids_and_remote_walls_are_safe() {
    let (m, mut s) = wall("bedrock");
    let crystal = m.id("crystal_cyan").unwrap();
    let i = s.world.idx(62, 60);
    s.world.mat[i] = crystal;
    for _ in 0..100 {
        strike(&mut s, &m);
    }
    assert_eq!(s.world.mat[i], crystal);
    assert!(s.mining.fractures.is_empty());
    assert!(s.mining.impact.as_ref().unwrap().blocked);
    let (m, mut s) = wall("rock");
    let i = s.world.idx(60, 60);
    s.world.flags[i] |= grave_engine::world::OWNED;
    let before = s.world.mat.clone();
    for _ in 0..30 {
        strike(&mut s, &m);
    }
    assert_eq!(s.world.mat, before);
    s.world.flags[i] = 0;
    s.world.mat[i] = m.id("water").unwrap();
    for _ in 0..30 {
        strike(&mut s, &m);
    }
    assert_eq!(s.world.mat[i], m.id("water").unwrap());
    let mut tool = Drill::default();
    tool.reach = 8.;
    let mut mine = Mining::default();
    assert!(
        mine.strike(&mut s.world, &m, (20., 60.), (1., 0.), tool)
            .is_empty()
    );
    assert!(mine.impact.is_none());
}
#[test]
fn fractures_survive_save_undo_rewind_and_shift_and_render_without_changing_materials() {
    let (m, mut s) = wall("rock");
    let start = s.snapshot();
    let mut history = History::new();
    history.touch(&mut s, "chip rock");
    let mut timeline = Timeline::new();
    timeline.every = 1;
    timeline.record(&mut s);
    let mut r = Renderer::default();
    r.light(&s);
    let clean = r.draw(&s.world, &m, Default::default()).to_vec();
    for _ in 0..3 {
        strike(&mut s, &m);
    }
    history.commit(&mut s);
    timeline.record(&mut s);
    let fractures = s.mining.fractures.clone();
    assert!(!fractures.is_empty());
    r.light(&s);
    let cracked = r.draw(&s.world, &m, Default::default()).to_vec();
    assert_ne!(clean, cracked);
    assert_eq!(s.world.mat, start.world.mat);
    grave_engine::headless::write_png(
        std::path::Path::new("shots/mining-fractures.png"),
        120,
        110,
        &cracked,
        5,
    )
    .unwrap();
    let saved = scene::save(&s.snapshot(), &m);
    let (loaded, p) = scene::load(&saved, &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    assert_eq!(loaded.mining.fractures, fractures);
    history.undo(&mut s, &m);
    assert!(s.mining.fractures.is_empty());
    history.redo(&mut s, &m);
    assert_eq!(s.mining.fractures, fractures);
    assert!(timeline.restore(0, &mut s, &m));
    assert!(s.mining.fractures.is_empty());
    assert!(timeline.restore(1, &mut s, &m));
    assert_eq!(s.mining.fractures, fractures);
    s.shift(-10, 5);
    assert_eq!(
        s.mining.fractures[&s.world.idx(50, 65)].material,
        m.id("rock").unwrap()
    );
    s = Sim::restore(&start, &m);
    assert!(s.mining.fractures.is_empty());
}
