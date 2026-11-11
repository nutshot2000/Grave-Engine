//! Metals: softening with heat, and bending.

use grave_engine::{Materials, Sim, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn sim(m: &Materials, text: &str) -> Sim {
    let (s, p) = scene::load(text, m, None).into_sim(m);
    assert!(p.is_empty(), "{p:?}");
    s
}

#[test]
fn a_red_hot_iron_beam_sags_where_a_cold_one_holds() {
    let m = mats();
    // An iron beam sticking 100 cells out of a stone pillar (iron is strong
    // enough for that cold).
    let beam = "size 200 100\nfill air\nrect stone 0 90 200 10\nrect stone 0 30 6 60\nrect iron 6 30 100 3\n";
    let pieces = |extra: &str| {
        let mut s = sim(&m, &format!("{beam}{extra}"));
        for _ in 0..120 {
            s.step(&m);
        }
        s.physics.body_count()
    };
    assert_eq!(pieces(""), 0, "cold: it holds");
    assert!(pieces("heat rect 40 30 66 3 1300\n") > 0, "red hot: it gives way");
    let iron = m.get(m.id("iron").unwrap());
    assert!(iron.firmness(20.0) == 1.0 && iron.firmness(1000.0) < 0.5 && iron.firmness(1500.0) == 0.0);
}
