//! Same deterministic fire, with timing of simulation, rendering and rewind.
use grave_engine::{
    Materials,
    edit::Timeline,
    profile,
    render::{Renderer, View},
    scene,
};
use std::{path::Path, time::Instant};
fn main() {
    let m = Materials::load_dir("materials");
    let text = std::fs::read_to_string("scenes/cinder-lane.txt").unwrap();
    let (mut s, p) = scene::load(&text, &m, Some(Path::new("scenes"))).into_sim(&m);
    assert!(p.is_empty());
    let mut r = Renderer::default();
    let mut timeline = Timeline::new();
    profile::enable(true);
    let mut start = Instant::now();
    for f in 0..1080 {
        if (180..270).contains(&f) && f % 15 == 0 {
            s.world.heat_circle(164, 185, 20, 950.);
        }
        {
            let _p = profile::scope("frame.step");
            s.step(&m);
        }
        {
            let _p = profile::scope("editor.rewind");
            timeline.record(&mut s);
        }
        if f % 2 == 0 {
            r.light(&s);
            r.draw(&s.world, &m, View::default());
        }
        if (f + 1) % 180 == 0 {
            println!(
                "FRAMES {}-{}: {:.2}s, {} bodies total\n{}",
                f - 179,
                f,
                start.elapsed().as_secs_f64(),
                s.physics.body_count()
                    + s.layers
                        .iter()
                        .map(|l| l.physics.body_count())
                        .sum::<usize>(),
                profile::report()
            );
            profile::reset();
            start = Instant::now();
        }
    }
}
