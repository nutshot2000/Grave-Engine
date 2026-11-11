//! Backdrops: a sky and parallax layers behind the world.

use grave_engine::backdrop::{Backdrop, PRESETS};
use grave_engine::render::{Renderer, View};
use grave_engine::{Materials, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn backdrop(lines: &str) -> Backdrop {
    let mut b = Backdrop::default();
    for l in lines.lines() {
        let w: Vec<&str> = l.split_whitespace().collect();
        assert!(b.parse_line(&w).unwrap(), "{l}");
    }
    b
}

/// How far (pixels) a row of the picture moved between two drawings (the
/// backdrop is drawn in world cells, so it slides right against the world
/// as the camera moves right, by distance x (1 - depth)).
fn shift(a: &[[f32; 3]], b: &[[f32; 3]], w: usize, row: usize) -> usize {
    let ra = &a[row * w..(row + 1) * w];
    let rb = &b[row * w..(row + 1) * w];
    (0..80)
        .min_by_key(|&k| {
            (0..w - 80).map(|x| (0..3).map(|c| (ra[x][c] - rb[x + k][c]).abs() as u64).sum::<u64>()).sum::<u64>()
        })
        .unwrap()
}

#[test]
fn far_layers_slide_slower_than_near_ones() {
    let (w, h) = (300, 100);
    let far = backdrop("sky #000000 #000000\nlayer mountains #ffffff depth 0.2 y 100 height 90 seed 3");
    let near = backdrop("sky #000000 #000000\nlayer mountains #ffffff depth 0.8 y 100 height 90 seed 3");
    let mut pics = Default::default();
    let (mut a, mut b, mut g) = (Vec::new(), Vec::new(), Vec::new());
    let mut moved = |bd: &Backdrop| {
        bd.draw(w, h, (0, 0, w, h), 100.0, 50.0, 0, &mut pics, &mut a, &mut g);
        bd.draw(w, h, (0, 0, w, h), 150.0, 50.0, 0, &mut pics, &mut b, &mut g);
        shift(&a, &b, w, 60)
    };
    let (f, n) = (moved(&far), moved(&near));
    println!("camera moved 50: far layer {f}, near layer {n}");
    assert!((38..=42).contains(&f), "far layer moves 40 against the world: {f}");
    assert!((8..=12).contains(&n), "near layer nearly keeps up: {n}");
}

#[test]
fn the_sky_shows_only_in_open_air_and_windows_glow() {
    let m = mats();
    let text = "size 100 60\nseed 1\nfill air\nrect stone 0 40 100 20\nsky #2040a0 #2040a0\nlayer city #101010 depth 0.5 y 40 height 30 windows\n";
    let (s, p) = scene::load(text, &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    let mut r = Renderer::default();
    r.light(&s);
    let px = r.draw(&s.world, &m, View::default()).to_vec();
    let at = |x: usize, y: usize| { let o = (y * 100 + x) * 4; [px[o], px[o + 1], px[o + 2]] };
    assert_eq!(at(50, 1), [0x20, 0x40, 0xa0], "sky up top");
    let stone = at(50, 50);
    assert!(stone[2] < 150 && stone[0] > 60, "stone is still stone: {stone:?}");
    let warm = (0..100).flat_map(|x| (12..40).map(move |y| (x, y))).filter(|&(x, y)| { let c = at(x, y); c[0] > 200 && c[1] > 150 }).count();
    assert!(warm > 5, "lit windows in the city: {warm}");
}

#[test]
fn backdrops_survive_saving_and_presets_load() {
    let m = mats();
    let text = "size 100 60\nfill air\nsky #102030 #405060 stars\nlayer mountains #445566 depth 0.15 y 50 height 30 snow seed 4\nlayer clouds #ffffff depth 0.05 y 5 height 8 haze 0.2\n";
    let (mut s, p) = scene::load(text, &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    let saved = scene::save(&s.snapshot(), &m);
    let (again, p) = scene::load(&saved, &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    assert_eq!(again.backdrop, s.backdrop);
    for (name, lines) in PRESETS {
        let b = Backdrop::preset(lines, 220);
        assert!(b.sky.is_some() && b.layers.len() >= 2, "{name}");
    }
    let bad = scene::load("size 50 50\nfill air\nlayer volcano #ff0000\n", &m, None);
    assert!(!bad.problems.is_empty(), "unknown layer kinds are explained");
}

#[test]
fn picture_layers_keep_their_names_as_written() {
    let m = mats();
    let dir = std::env::temp_dir().join(format!("grave_backdrop_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/materials/textures/brick.png"), dir.join("my hills.png")).unwrap();
    let text = "size 64 32\nfill air\nlayer image \"my hills.png\" depth 0.5 y 32 scale 1\n";
    let (s, p) = scene::load(text, &m, Some(&dir)).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    // Found next to the scene and drawn.
    let mut r = Renderer::default();
    r.light(&s);
    let px = r.draw(&s.world, &m, View::default()).to_vec();
    let o = (20 * 64 + 10) * 4;
    assert!(px[o] > 60, "the picture shows: {:?}", &px[o..o + 3]);
    // Saved as written: quoted, not a full path.
    let saved = { let mut s = s; scene::save(&s.snapshot(), &m) };
    assert!(saved.contains("layer image \"my hills.png\""), "{saved}");
}
