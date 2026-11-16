//! Finds a geyser in a scene, runs until it erupts hardest, and saves a
//! close-up: shots/geyser.png (level design aid).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let m = grave_engine::Materials::load_dir("materials");
    let (mut s, _) = grave_engine::scene::load(&std::fs::read_to_string(&a[1]).unwrap(), &m, Some(std::path::Path::new("scenes"))).into_sim(&m);
    let (vent, lava) = (m.id("magma_vent").unwrap(), m.id("lava").unwrap());
    let w = s.world.w;
    let first = s.world.mat.iter().position(|&c| c == vent).expect("no geyser");
    let (vx, vy) = (first % w, first / w);
    let (x0, y0) = (vx.saturating_sub(60), vy.saturating_sub(90));
    let near = |s: &grave_engine::Sim| (y0..(y0 + 110).min(s.world.h)).map(|y| (x0..(x0 + 120).min(w)).filter(|&x| s.world.mat[y * w + x] == lava).count()).sum::<usize>() + s.world.flying.iter().filter(|f| f.mat == lava && (f.x - vx as f32).abs() < 60.0 && (f.y - vy as f32).abs() < 90.0).count();
    let mut best = (0, 0);
    for f in 0..700 { s.step(&m); let n = near(&s); if n > best.0 { best = (n, f); } }
    let (mut s, _) = grave_engine::scene::load(&std::fs::read_to_string(&a[1]).unwrap(), &m, Some(std::path::Path::new("scenes"))).into_sim(&m);
    for _ in 0..=best.1 { s.step(&m); }
    let mut r = grave_engine::render::Renderer::default();
    grave_engine::headless::shot_with_actors(&s.world, &m, &mut r, &s.physics.actor_status(), Some((x0, y0, 120, 110)), std::path::Path::new("shots/geyser.png"), 5, false).unwrap();
    println!("geyser at ({vx}, {vy}); most lava at frame {} ({} cells)", best.1, best.0);
}
