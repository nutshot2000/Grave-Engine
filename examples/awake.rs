//! How much of a scene's world is still awake after it settles (the real
//! cost driver: sleeping areas cost nothing). Level design aid.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let m = grave_engine::Materials::load_dir("materials");
    let (mut s, _) = grave_engine::scene::load(&std::fs::read_to_string(&a[1]).unwrap(), &m, Some(std::path::Path::new("scenes"))).into_sim(&m);
    let mut sum = 0.0;
    for f in 0..900 { s.step(&m); if f >= 300 { sum += s.world.awake_fraction(); } }
    println!("awake on average after settling: {:.1}%", sum / 600.0 * 100.0);
}
