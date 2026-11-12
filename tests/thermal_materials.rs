//! Thermal exchange and the material presets changed after the material audit.
use grave_engine::{Materials, World, layers, material::State};
fn inert(cap: f32) -> String {
    format!("state = solid\ndensity = 1000\nconducts_heat = 1\nheat_capacity = {cap}\n")
}
fn heat_total(w: &World, m: &Materials) -> f64 {
    w.temp
        .iter()
        .zip(&w.mat)
        .map(|(t, id)| *t as f64 * m.get(*id).heat_capacity as f64)
        .sum()
}
#[test]
fn different_capacities_conserve_heat_and_approach_the_weighted_equilibrium() {
    let m = Materials::from_texts(&[("light", &inert(0.25)), ("heavy", &inert(4.))]);
    let mut w = World::new(2, 1);
    w.set(&m, 0, 0, m.id("light").unwrap());
    w.set(&m, 1, 0, m.id("heavy").unwrap());
    w.temp = vec![100., 0.];
    let before = heat_total(&w, &m);
    for _ in 0..100 {
        w.step(&m);
        assert!((heat_total(&w, &m) - before).abs() < 0.0002);
        assert!(w.temp.iter().all(|t| *t >= 0. && *t <= 100.));
    }
    let expected = 25. / 4.25;
    assert!((w.temp[0] - expected).abs() < 0.01);
    assert!((w.temp[1] - expected).abs() < 0.01);
}
#[test]
fn capacities_below_one_really_change_the_heating_rate() {
    let run = |cap| {
        let m = Materials::from_texts(&[("test", &inert(cap))]);
        let mut w = World::new(2, 1);
        w.fill(&m, m.id("test").unwrap());
        w.temp = vec![100., 0.];
        w.step(&m);
        w.temp[1]
    };
    assert!(run(0.25) > run(1.) + 15.);
}
#[test]
fn dense_low_capacity_grid_stays_bounded_and_conserves_heat() {
    let m = Materials::from_texts(&[("a", &inert(0.1)), ("b", &inert(0.25)), ("c", &inert(4.))]);
    let mut w = World::new(67, 38);
    let ids = [m.id("a").unwrap(), m.id("b").unwrap(), m.id("c").unwrap()];
    for y in 0..w.h {
        for x in 0..w.w {
            w.set(&m, x, y, ids[(x + y * 3) % 3]);
            let i = w.idx(x, y);
            w.temp[i] = ((x * 31 + y * 71) % 1000) as f32;
        }
    }
    let before = heat_total(&w, &m);
    for _ in 0..100 {
        w.step(&m);
        assert!(
            w.temp
                .iter()
                .all(|t| t.is_finite() && *t >= 0. && *t <= 999.)
        );
    }
    assert!((heat_total(&w, &m) - before).abs() / before < 0.00001);
}
#[test]
fn sleeping_block_boundaries_do_not_lose_or_generate_heat() {
    let m = Materials::from_texts(&[("test", &inert(0.25))]);
    let mut w = World::new(224, 32);
    w.fill(&m, m.id("test").unwrap());
    for _ in 0..180 {
        w.step(&m);
    }
    assert!(w.awake_fraction() < 0.1);
    w.temp[15] = 100.;
    w.wake_at(15);
    let before = heat_total(&w, &m);
    for _ in 0..240 {
        w.step(&m);
    }
    assert!((heat_total(&w, &m) - before).abs() / before < 0.00001);
}
#[test]
fn cross_plane_exchange_is_also_conservative() {
    let m = Materials::from_texts(&[("a", &inert(0.25)), ("b", &inert(4.))]);
    let (mut a, mut b) = (World::new(1, 1), World::new(1, 1));
    a.fill(&m, m.id("a").unwrap());
    b.fill(&m, m.id("b").unwrap());
    a.temp[0] = 100.;
    b.temp[0] = 0.;
    for _ in 0..100 {
        layers::exchange_heat(&mut a, &mut b, &m);
    }
    assert!((heat_total(&a, &m) + heat_total(&b, &m) - 25.).abs() < 0.0002);
}
#[test]
fn concrete_loses_strength_in_fire_without_becoming_flammable() {
    let m = Materials::load_dir("materials");
    let c = m.get(m.id("concrete").unwrap());
    assert_eq!(c.firmness(20.), 1.);
    assert!(c.firmness(600.) < 0.6);
    assert!(c.firmness(900.) < 0.3);
    assert!(c.catches_fire_at.is_none());
    assert_eq!(c.state, State::Solid);
}
#[test]
fn snow_keeps_the_low_meltwater_yield_and_matching_light_density() {
    let m = Materials::load_dir("materials");
    let snow = m.get(m.id("snow").unwrap());
    let water = m.get(m.id("water").unwrap());
    assert_eq!(snow.melt_yield, 0.12);
    assert!((snow.density / water.density - snow.melt_yield).abs() < 0.001);
    let mut w = World::new(128, 64);
    w.fill(&m, m.id("snow").unwrap());
    w.temp.fill(10.);
    w.ambient = 10.;
    w.step(&m);
    let n = w
        .mat
        .iter()
        .filter(|v| **v == m.id("water").unwrap())
        .count();
    let ratio = n as f32 / w.mat.len() as f32;
    assert!((0.09..0.15).contains(&ratio), "meltwater {ratio}");
}
#[test]
fn melted_silica_sand_cools_into_glass() {
    let m = Materials::load_dir("materials");
    let mut w = World::new(16, 16);
    w.fill(&m, m.id("sand").unwrap());
    w.temp.fill(1800.);
    w.ambient = 1800.;
    w.step(&m);
    assert!(w.mat.iter().all(|v| *v == m.id("molten_glass").unwrap()));
    w.temp.fill(900.);
    w.ambient = 900.;
    w.wake_all();
    w.step(&m);
    assert!(w.mat.iter().all(|v| *v == m.id("glass").unwrap()));
}
#[test]
fn heated_concrete_overhang_gives_way_while_the_cold_one_holds() {
    let m = Materials::load_dir("materials");
    let run = |hot: bool| {
        let text = format!(
            "size 120 100\nfill air\nrect stone 0 90 120 10\nrect stone 0 35 6 55\nrect concrete 6 35 30 3\n{}",
            if hot { "heat rect 6 35 30 3 900\n" } else { "" }
        );
        let (mut s, p) = grave_engine::scene::load(&text, &m, None).into_sim(&m);
        assert!(p.is_empty());
        for _ in 0..120 {
            s.step(&m);
        }
        s.physics.body_count()
    };
    assert_eq!(run(false), 0, "cold concrete beam must remain supported");
    assert!(
        run(true) > 0,
        "fire-heated concrete beam must lose support strength"
    );
}
