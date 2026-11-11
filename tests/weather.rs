//! Weather: snow that builds up, rain, hail, dust, fog, gusts, and lightning
//! that picks the tallest thing and starts fires.

use grave_engine::render::{Renderer, View};
use grave_engine::weather::{Kind, Weather};
use grave_engine::{Materials, Sim, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn sim(m: &Materials, text: &str) -> Sim {
    let (s, p) = scene::load(text, m, None).into_sim(m);
    assert!(p.is_empty(), "{p:?}");
    s
}

fn count(s: &Sim, m: &Materials, name: &str) -> usize {
    let id = m.id(name).unwrap();
    s.world.mat.iter().filter(|&&c| c == id).count() + s.world.flying.iter().filter(|f| f.mat == id).count()
}

fn burning(s: &Sim) -> usize {
    (0..s.world.mat.len()).filter(|&i| s.world.is_burning(i)).count()
}

/// A cold world: a floor and a shelf, so snow has somewhere to settle.
const COLD: &str = "size 200 120\nseed 4\nfill air\nrect stone 0 110 200 10\nrect stone 60 70 80 4\nambient -6\nheat rect 0 0 200 120 -6\n";

#[test]
fn snow_builds_up_on_the_ground_and_on_ledges() {
    let m = mats();
    let mut s = sim(&m, &format!("{COLD}weather snow 0.8\n"));
    for _ in 0..900 {
        s.step(&m);
    }
    let snow = m.id("snow").unwrap();
    let on_shelf = (60..140).flat_map(|x| (60..70).map(move |y| (x, y))).filter(|&(x, y)| s.world.mat[s.world.idx(x, y)] == snow).count();
    println!("snow cells: {}, on the shelf: {on_shelf}", count(&s, &m, "snow"));
    assert!(count(&s, &m, "snow") > 1500, "snow piles up");
    assert!(on_shelf > 200, "and settles on the ledge: {on_shelf}");
    // Off means off: nothing more falls (what's in the air lands).
    s.weather.on = false;
    for _ in 0..800 {
        s.step(&m);
    }
    let before = count(&s, &m, "snow");
    for _ in 0..300 {
        s.step(&m);
    }
    assert_eq!(count(&s, &m, "snow"), before, "with the weather off, no more snow");
}

#[test]
fn heavier_weather_drops_more() {
    let m = mats();
    let total = |v: f32| {
        let mut s = sim(&m, &format!("{COLD}weather snow {v}\n"));
        for _ in 0..600 {
            s.step(&m);
        }
        count(&s, &m, "snow")
    };
    let (light, heavy) = (total(0.2), total(0.9));
    println!("snow after 10 s: light {light}, heavy {heavy}");
    assert!(heavy > light * 3, "heavier means more: {light} vs {heavy}");
}

#[test]
fn rain_puts_out_a_fire_and_fills_a_puddle() {
    let m = mats();
    let text = "size 200 120\nseed 4\nfill air\nrect stone 0 110 200 10\nrect stone 40 100 3 10\nrect stone 100 100 3 10\nrect wood 120 90 30 20\nheat rect 120 90 30 8 700\n";
    let mut dry = sim(&m, text);
    let mut wet = sim(&m, &format!("{text}weather rain 0.9\n"));
    for _ in 0..500 {
        dry.step(&m);
        wet.step(&m);
    }
    let (wood_dry, wood_wet) = (count(&dry, &m, "wood"), count(&wet, &m, "wood"));
    println!("wood left: dry {wood_dry}, in the rain {wood_wet} (of 600); water in the pit: {}", count(&wet, &m, "water"));
    assert!(wood_dry < 580, "the fire burns the wood in the dry: {wood_dry} left");
    assert!(wood_wet > wood_dry + 30, "rain puts the fire out and saves the wood: {wood_wet} vs {wood_dry}");
    assert!(count(&wet, &m, "water") > 300, "rain collects as water");
}

#[test]
fn hail_and_dust_fall_and_pile() {
    let m = mats();
    let mut s = sim(&m, &format!("{COLD}weather hail 0.8\nweather dust 0.8\nwind 4\n"));
    for _ in 0..700 {
        s.step(&m);
    }
    assert!(count(&s, &m, "hail") > 100, "hail: {}", count(&s, &m, "hail"));
    assert!(count(&s, &m, "dust") > 100, "dust blown in: {}", count(&s, &m, "dust"));
}

#[test]
fn gusty_wind_comes_and_goes_and_the_scene_wind_comes_back() {
    let m = mats();
    let mut s = sim(&m, &format!("{COLD}wind 2\nweather gusts 0.8\n"));
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for _ in 0..600 {
        s.step(&m);
        lo = lo.min(s.world.wind);
        hi = hi.max(s.world.wind);
    }
    println!("wind ranged {lo} .. {hi}");
    assert!(hi - lo > 6.0, "the wind gusts: {lo}..{hi}");
    let saved = scene::save(&s.snapshot(), &m);
    assert!(saved.contains("wind 2\n"), "the scene's own wind is what's saved");
    s.weather.set(Kind::Gusts, 0.0, 0.0);
    s.step(&m);
    assert_eq!(s.world.wind, 2.0, "and it returns when the gusts stop");
}

#[test]
fn lightning_strikes_the_tallest_thing_and_sets_it_on_fire() {
    let m = mats();
    // A wooden tower and a low iron post, with the view covering both.
    let text = "size 200 160\nseed 4\nfill air\nrect stone 0 150 200 10\nrect wood 20 60 100 90\nrect iron 150 120 4 30\n";
    let mut s = sim(&m, text);
    s.weather_view = Some((0.0, 0.0, 200.0, 160.0));
    let mut hits_tower = 0;
    for _ in 0..8 {
        s.weather_run.strike_now = true;
        let mut bolt_end = None;
        for _ in 0..40 {
            s.step(&m);
            if !s.weather_run.bolt.is_empty() && bolt_end.is_none() {
                bolt_end = s.weather_run.bolt.last().copied();
            }
        }
        let (bx, _) = bolt_end.expect("a bolt was drawn");
        if (20..120).contains(&bx) {
            hits_tower += 1;
        }
    }
    println!("{hits_tower} of 8 strikes hit the tall tower");
    assert!(hits_tower >= 6, "it goes for the tallest thing: {hits_tower} of 8");
    assert!(burning(&s) > 5 || count(&s, &m, "wood") < 100 * 90, "the wooden tower caught fire or broke");
}

#[test]
fn a_strike_has_a_warning_a_flash_a_shake_and_fuses_sand_to_glass() {
    let m = mats();
    let mut s = sim(&m, "size 120 80\nseed 4\nfill air\nrect stone 0 70 120 10\nrect sand 40 55 40 15\n");
    s.weather_view = Some((40.0, 0.0, 80.0, 80.0));
    s.weather_run.strike_now = true;
    let (mut warned, mut flashed, mut shook) = (false, false, false);
    for _ in 0..40 {
        s.step(&m);
        warned |= s.weather_run.pending.is_some() && s.weather_run.flash > 0.1;
        flashed |= s.weather_run.flash > 0.9;
        shook |= s.camera.shake > 0.5;
    }
    assert!(warned, "a flicker of warning first");
    assert!(flashed, "then the flash");
    assert!(shook, "the view shakes");
    assert!(count(&s, &m, "glass") > 0 || count(&s, &m, "molten_glass") > 0, "sand fused to glass");
}

#[test]
fn triggers_change_the_weather_over_time_and_it_saves() {
    let m = mats();
    let mut s = sim(&m, &format!("{COLD}body circle iron 20 20 3\nname ball\ntrigger 0 60 200 60 by ball : weather rain 0.8 1 ; weather lightning 12 ; weather fog 0.4\n"));
    assert!(s.weather.is_clear());
    for _ in 0..200 {
        s.step(&m);
        if s.triggers[0].fired {
            break;
        }
    }
    for _ in 0..70 {
        s.step(&m);
    }
    assert!((s.weather.rain - 0.8).abs() < 0.02 && s.weather.lightning == 12.0 && s.weather.fog == 0.4, "{:?}", s.weather);
    let line = s.triggers[0].to_line("ball");
    assert!(line.contains("weather rain 0.8 1") && line.contains("weather lightning 12 0"), "{line}");
    let text = scene::save(&s.snapshot(), &m);
    let again = sim(&m, &text);
    assert_eq!(again.weather.fog, 0.4);
    assert_eq!(again.weather.lightning, 12.0);
    // `weather clear` fades everything out.
    s.weather.clear(1.0);
    for _ in 0..70 {
        s.step(&m);
    }
    assert!(s.weather.is_clear(), "{:?}", s.weather);
    // Bad lines are explained.
    let bad = scene::load("size 50 50\nfill air\nweather hurricane 3\n", &m, None);
    assert!(!bad.problems.is_empty());
    let bad = scene::load("size 50 50\nfill air\nweather snow\n", &m, None);
    assert!(!bad.problems.is_empty());
}

#[test]
fn fog_hides_the_distance_and_presets_exist() {
    let m = mats();
    let text = "size 100 60\nseed 1\nfill air\nrect stone 0 50 100 10\nsky #1040c0 #1040c0\nlayer mountains #101010 depth 0.1 y 50 height 40\n";
    let draw = |fog: f32| {
        let mut s = sim(&m, text);
        s.weather.fog = fog;
        let mut r = Renderer::default();
        r.light(&s);
        let px = r.draw(&s.world, &m, View::default()).to_vec();
        let o = (45 * 100 + 50) * 4; // inside the mountain layer
        [px[o], px[o + 1], px[o + 2]]
    };
    let (clear, foggy) = (draw(0.0), draw(0.9));
    println!("mountain pixel: clear {clear:?}, foggy {foggy:?}");
    assert!(foggy[0] as i32 > clear[0] as i32 + 100, "fog washes the mountain out: {clear:?} -> {foggy:?}");
    let presets = Weather::presets();
    assert!(presets.len() >= 6 && presets.iter().any(|(n, w)| n.contains("Storm") && w.lightning > 0.0 && w.rain > 0.0));
}
