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

/// Drop a bar of `material`, `thick` cells thick, onto a post at speed;
/// returns how far it bent (radians) and how many pieces there are after.
fn drop_bar(m: &Materials, material: &str, thick: u32, extra: &str, speed: f32) -> (f32, usize, usize, usize) {
    let mut s = sim(m, &format!("size 200 140\nfill air\nrect stone 0 130 200 10\nrect stone 112 90 4 40\nbody rect {material} 60 60 60 {thick}\ndepth 0.1\n{extra}"));
    let id = s.physics.ids()[0];
    let cells = s.physics.spec_of(&s.world, id).unwrap().cell_count();
    s.physics.edit(&mut s.world, m, id, |b| b.vy = speed);
    for _ in 0..40 {
        s.step(m);
    }
    let after = s.physics.spec_of(&s.world, id).map(|b| b.cell_count()).unwrap_or(0);
    (s.physics.bent(id), s.physics.body_count(), cells, after)
}

#[test]
fn a_thin_iron_bar_bends_on_a_post_and_stays_in_one_piece() {
    let m = mats();
    let (bent, bodies, before, after) = drop_bar(&m, "iron", 2, "", 400.0);
    println!("thin: bent {bent} bodies {bodies} cells {before} -> {after}");
    assert!(bent > 0.05, "it bends");
    assert_eq!(bodies, 1, "still one piece");
    assert!(after * 10 >= before * 9, "hardly any metal lost ({before} -> {after})");
}

#[test]
fn a_thick_iron_bar_bends_far_less_than_a_thin_one() {
    let m = mats();
    let (thin, _, _, _) = drop_bar(&m, "iron", 2, "", 400.0);
    let (thick, _, _, _) = drop_bar(&m, "iron", 8, "", 400.0);
    println!("thin {thin} thick {thick}");
    assert!(thick < thin / 4.0, "thick {thick} vs thin {thin}");
    let (gentle, _, _, _) = drop_bar(&m, "iron", 2, "", 0.0); // just falls 3 m
    println!("gentle {gentle}");
    assert_eq!(gentle, 0.0, "a short fall doesn't bend it");
}

#[test]
fn red_hot_iron_gives_way_where_cold_iron_bends_and_wood_never_bends() {
    let m = mats();
    let (cold, cold_pieces, _, _) = drop_bar(&m, "iron", 2, "", 400.0);
    let (hot, hot_pieces, _, _) = drop_bar(&m, "iron", 2, "heat rect 60 60 60 2 1100
", 400.0);
    println!("cold {cold} ({cold_pieces} pieces) hot {hot} ({hot_pieces} pieces)");
    assert_eq!(cold_pieces, 1);
    assert!(hot_pieces > 1 || hot > cold, "red hot: it tears or bends further");
    let (wood, _, _, _) = drop_bar(&m, "wood", 2, "", 400.0);
    assert_eq!(wood, 0.0);
}

#[test]
fn a_huge_hit_tears_thin_iron_in_two() {
    let m = mats();
    let (_, pieces, before, _) = drop_bar(&m, "iron", 1, "", 1500.0);
    println!("torn into {pieces} pieces ({before} cells)");
    assert!(pieces >= 2, "it tears");
}
