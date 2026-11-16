//! Magma pools stay molten; lava geysers (`magma_vent`) erupt in bursts.
use grave_engine::{Materials, scene};

fn count(s: &grave_engine::Sim, m: &Materials, name: &str) -> usize {
    let id = m.id(name).unwrap();
    s.world.mat.iter().filter(|&&c| c == id).count()
}

#[test]
fn magma_stays_molten_and_geysers_erupt() {
    let m = Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"));
    assert!(m.problems.is_empty(), "{:?}", m.problems);
    let text = "size 240 160\nfill air\nrect rock 0 120 240 40\nanchor 0 120 240 40\nrect magma 30 116 40 4\nrect rock 26 110 4 10\nrect rock 70 110 4 10\nrect magma_vent 160 120 8 3\n";
    let (mut s, problems) = scene::load(text, &m, None).into_sim(&m);
    assert!(problems.is_empty(), "{problems:?}");
    let magma = count(&s, &m, "magma");
    let lava = m.id("lava").unwrap();
    let mut thrown = 0;
    for _ in 0..620 {
        s.step(&m);
        thrown = thrown.max(s.world.flying.iter().filter(|f| f.mat == lava).count());
    }
    assert_eq!(count(&s, &m, "magma"), magma, "the pool is still all molten magma");
    assert!(thrown >= 5, "the geyser spat lava into the air ({thrown} lumps at once)");
}
