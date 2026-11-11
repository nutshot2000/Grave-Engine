//! Embers: burning things fling glowing bits that drift on the air, land,
//! and react: spot fires on wood, a hiss on water.

use grave_engine::{Materials, Sim, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn sim(m: &Materials, text: &str) -> Sim {
    let (s, p) = scene::load(text, m, None).into_sim(m);
    assert!(p.is_empty(), "{p:?}");
    s
}

const CAMP: &str = "size 300 160\nseed 5\nfill air\nrect stone 0 140 300 20\nrect wood 120 132 40 8\nheat circle 140 134 4 900\n";

/// Where the embers flying right now are, on average (x), and how many.
fn embers(s: &Sim, m: &Materials) -> (f32, usize) {
    let e = m.id("ember").unwrap();
    let xs: Vec<f32> = s.world.flying.iter().filter(|f| f.mat == e).map(|f| f.x).collect();
    (xs.iter().sum::<f32>() / xs.len().max(1) as f32, xs.len())
}

#[test]
fn a_fire_throws_embers_and_the_wind_carries_them() {
    let m = mats();
    let drift = |wind: &str| {
        let mut s = sim(&m, &format!("{CAMP}wind {wind}\n"));
        let (mut sum, mut n) = (0.0, 0);
        for f in 0..400 {
            s.step(&m);
            if f > 100 {
                let (x, k) = embers(&s, &m);
                sum += x * k as f32;
                n += k;
            }
        }
        (sum / n.max(1) as f32, n)
    };
    let ((right, nr), (left, nl)) = (drift("4"), drift("-4"));
    println!("wind right: embers average x {right} ({nr} seen); wind left: {left} ({nl} seen)");
    assert!(nr > 50 && nl > 50, "the fire throws plenty of embers");
    assert!(right > left + 5.0, "wind blows them its way");
}

#[test]
fn embers_start_spot_fires_downwind() {
    let m = mats();
    // A wooden post 50 cells downwind of the campfire, not touching it.
    let mut s = sim(&m, &format!("{CAMP}wind 3\nrect wood 205 110 6 30\n"));
    let post = |s: &Sim| (110..140).flat_map(|y| (205..211).map(move |x| (x, y))).any(|(x, y)| s.world.is_burning(s.world.idx(x, y)));
    let mut caught = None;
    for f in 0..1800 {
        s.step(&m);
        if post(&s) {
            caught = Some(f);
            break;
        }
    }
    println!("the post caught fire at frame {caught:?}");
    assert!(caught.is_some(), "an ember set the post alight");
}

#[test]
fn an_ember_landing_in_water_goes_out_with_a_hiss() {
    let m = mats();
    let mut s = sim(&m, "size 60 60\nfill air\nrect stone 0 50 60 10\nrect water 0 40 60 10\n");
    let e = m.id("ember").unwrap();
    for k in 0..5 {
        assert!(s.world.spawn_flyer(&m, e, 20.0 + k as f32 * 4.0, 20.0, 0.0, 0.5));
    }
    // Watch for steam the whole time (it turns back into water quickly).
    let st = m.id("steam").unwrap();
    let mut steam = 0;
    for _ in 0..500 {
        s.step(&m);
        steam = steam.max(s.world.mat.iter().filter(|&&c| c == st).count());
    }
    let left = s.world.mat.iter().filter(|&&c| c == e).count() + s.world.flying.iter().filter(|f| f.mat == e).count();
    println!("embers left {left}, steam {steam}");
    assert_eq!(left, 0, "the embers went out");
    assert!(steam > 0, "with a hiss of steam");
}
