//! Lighting: overall light, sun with shadows, lamps, light through glass and
//! water, lamps on objects, fading from triggers, saving.

use grave_engine::render::{Renderer, View};
use grave_engine::{Materials, Sim, scene};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn sim(m: &Materials, text: &str) -> Sim {
    let (s, p) = scene::load(text, m, None).into_sim(m);
    assert!(p.is_empty(), "{p:?}");
    s
}

/// The picture, and how bright the pixel at (x, y) is (0..765).
fn picture(m: &Materials, s: &Sim) -> Vec<u8> {
    let mut r = Renderer::default();
    r.light(s);
    r.draw(&s.world, m, View::default()).to_vec()
}

fn bright(px: &[u8], s: &Sim, x: usize, y: usize) -> u32 {
    let o = (y * s.world.w + x) * 4;
    px[o] as u32 + px[o + 1] as u32 + px[o + 2] as u32
}

const ROOM: &str = "size 120 60\nseed 3\nfill air\nrect stone 0 50 120 10\n";

#[test]
fn a_scene_without_lighting_looks_exactly_as_before() {
    let m = mats();
    let plain = sim(&m, ROOM);
    let full = sim(&m, &format!("{ROOM}lighting 1\n"));
    let mut r = Renderer::default();
    let before = r.draw(&plain.world, &m, View::default()).to_vec();
    assert_eq!(picture(&m, &plain), before);
    assert_eq!(picture(&m, &full), before);
}

#[test]
fn lamps_light_up_whats_near_them_in_the_dark() {
    let m = mats();
    let s = sim(&m, &format!("{ROOM}lighting 0.1\nlight 30 40 30 strength 2\n"));
    let px = picture(&m, &s);
    let (near, far) = (bright(&px, &s, 30, 50), bright(&px, &s, 100, 50));
    println!("ground near the lamp {near}, far away {far}");
    assert!(near > far * 3, "near {near} far {far}");
}

#[test]
fn the_sun_casts_shadows_and_glass_lets_it_through() {
    let m = mats();
    // Three roofs: stone, glass, and none. The floor under each.
    let s = sim(&m, &format!("{ROOM}lighting 0.1\nsun 0 1\nrect stone 10 20 20 3\nrect glass 50 20 20 3\n"));
    let px = picture(&m, &s);
    let (under_stone, under_glass, open) = (bright(&px, &s, 20, 50), bright(&px, &s, 60, 50), bright(&px, &s, 95, 50));
    println!("under stone {under_stone}, under glass {under_glass}, open {open}");
    assert!(under_stone * 3 < open, "stone casts a shadow");
    assert!(under_glass > under_stone * 2 && under_glass <= open, "glass lets most of the light through");
}

#[test]
fn deep_water_gets_darker() {
    let m = mats();
    let s = sim(&m, "size 60 80\nseed 3\nfill air\nrect stone 0 70 60 10\nrect water 0 10 60 60\nlighting 0.05\nsun 0 1\n");
    let px = picture(&m, &s);
    let (shallow, deep) = (bright(&px, &s, 30, 12), bright(&px, &s, 30, 65));
    println!("shallow {shallow} deep {deep}");
    assert!(shallow > deep * 2);
}

#[test]
fn a_lamp_on_an_object_moves_with_it() {
    let m = mats();
    let mut s = sim(&m, &format!("{ROOM}lighting 0.1\nbody rect iron 20 20 6 4\nname cart\nlight 26 22 20 on cart\n"));
    let before = s.lamps()[0];
    let id = s.physics.ids()[0];
    s.physics.edit(&mut s.world, &m, id, |b| b.x += 50.0);
    let after = s.lamps()[0];
    assert!((after.x - before.x - 50.0).abs() < 0.5 && (after.y - before.y).abs() < 0.5, "{before:?} -> {after:?}");
}

#[test]
fn a_trigger_fades_the_lights_and_lighting_survives_saving() {
    let m = mats();
    let mut s = sim(&m, &format!("{ROOM}lighting 0.8 #ffeedd\nsun 30 0.7 #ffe0b0\nbody circle iron 20 20 3\nname ball\nlight 60 30 25 strength 1.5 color #ffb060 cone 90 30 flicker 0.2 on ball\ntrigger 0 30 120 20 by ball : lighting 0.1 1\n"));
    let text = scene::save(&s.snapshot(), &m);
    let (again, p) = scene::load(&text, &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}\n{text}");
    assert_eq!(again.lighting.sun, s.lighting.sun);
    assert_eq!(again.lighting.ambient_color, [0xff, 0xee, 0xdd]);
    let (a, b) = (&again.lighting.lights[0], &s.lighting.lights[0]);
    assert!((a.x - b.x).abs() < 0.2 && (a.y - b.y).abs() < 0.2 && a.cone == b.cone && a.flicker == b.flicker && a.on.is_some(), "{a:?} vs {b:?}");
    for _ in 0..200 {
        s.step(&m);
        if s.triggers[0].fired {
            break;
        }
    }
    assert!(s.triggers[0].fired);
    for _ in 0..61 {
        s.step(&m);
    }
    assert!((s.lighting.ambient - 0.1).abs() < 0.02, "faded to dark: {}", s.lighting.ambient);
}

#[test]
fn drawing_just_the_view_matches_drawing_everything() {
    let m = mats();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/scenes");
    for file in ["house-night.txt", "house.txt", "stunt-yard.txt"] {
        let text = std::fs::read_to_string(format!("{dir}/{file}")).unwrap();
        let (mut s, _) = scene::load(&text, &m, Some(std::path::Path::new(dir))).into_sim(&m);
        // Some fire, so glow and embers are in the picture too.
        let (w, h) = (s.world.w, s.world.h);
        for y in h / 3..h / 3 + 4 {
            for x in w / 2..w / 2 + 6 {
                let i = s.world.idx(x, y);
                s.world.temp[i] = 900.0;
            }
        }
        for _ in 0..60 {
            s.step(&m);
        }
        let mut r = Renderer::default();
        r.light(&s);
        r.set_camera(w as f32 * 0.4, h as f32 * 0.5);
        let all = r.draw(&s.world, &m, View::default()).to_vec();
        let win = (w / 3, h / 4, (w / 3).min(300), h / 2);
        let part = r.draw_window(&s.world, &m, View::default(), win).to_vec();
        assert_eq!(part.len(), win.2 * win.3 * 4);
        let mut wrong = 0;
        for y in 0..win.3 {
            for x in 0..win.2 {
                let (a, b) = ((y * win.2 + x) * 4, ((win.1 + y) * w + win.0 + x) * 4);
                if part[a..a + 3] != all[b..b + 3] {
                    wrong += 1;
                }
            }
        }
        assert_eq!(wrong, 0, "{file}: {wrong} pixels differ");
    }
}

#[test]
fn remembered_sunlight_updates_when_a_new_shadow_appears() {
    let m = mats();
    let mut s = sim(&m, &format!("{ROOM}lighting 0.1\nsun 0 1\n"));
    for _ in 0..120 {
        s.step(&m);
    }
    let mut r = Renderer::default();
    r.light(&s);
    let open = bright(&r.draw(&s.world, &m, View::default()).to_vec(), &s, 60, 49);
    // Still nothing changed: drawn again, the same.
    s.step(&m);
    r.light(&s);
    assert_eq!(bright(&r.draw(&s.world, &m, View::default()).to_vec(), &s, 60, 49), open);
    // A roof appears high above (as if built there): the floor goes into shadow.
    s.world.paint_circle(&m, 60, 10, 6, m.id("stone").unwrap());
    s.step(&m);
    r.light(&s);
    let shaded = bright(&r.draw(&s.world, &m, View::default()).to_vec(), &s, 60, 49);
    println!("floor in the sun {open}, under the new roof {shaded}");
    assert!(shaded * 2 < open, "the new roof casts a shadow at once");
}

#[test]
fn the_drawn_view_keeps_its_size_and_covers_the_screen() {
    let m = mats();
    let s = sim(&m, "size 3000 900\nfill air\n");
    let mut sizes = std::collections::HashSet::new();
    for (cx, cy) in [(0.0, 0.0), (1500.0, 450.0), (2999.0, 899.0), (733.3, 120.7), (1500.5, 451.2)] {
        let win = grave_engine::render::view_window(&s.world, (cx, cy), (180.0, 110.0), 4.0);
        sizes.insert((win.2, win.3));
        assert!(win.0 + win.2 <= 3000 && win.1 + win.3 <= 900, "inside the world: {win:?}");
        let (vx0, vx1) = ((cx - 180.0).max(0.0), (cx + 180.0).min(3000.0));
        let (vy0, vy1) = ((cy - 110.0).max(0.0), (cy + 110.0).min(900.0));
        assert!(win.0 as f32 <= vx0 && (win.0 + win.2) as f32 >= vx1 && win.1 as f32 <= vy0 && (win.1 + win.3) as f32 >= vy1, "covers the view at {cx},{cy}: {win:?}");
    }
    assert_eq!(sizes.len(), 1, "always the same size: {sizes:?}");
}

#[test]
fn a_trigger_moves_the_sun_and_finish_reports_the_end() {
    let m = mats();
    let mut s = sim(&m, &format!("{ROOM}sun 20 1 #ffffff\nbody circle iron 20 20 3\nname ball\ntrigger 0 30 120 20 by ball : sun 70 0.2 #ff8040 1 ; finish\n"));
    for _ in 0..200 {
        s.step(&m);
        if s.triggers[0].fired {
            break;
        }
    }
    assert!(s.events.contains(&grave_engine::trigger::Event::Finish), "the finish line was reported");
    for _ in 0..61 {
        s.step(&m);
    }
    let sun = s.lighting.sun.unwrap();
    assert!((sun.angle - 70.0).abs() < 0.5 && (sun.strength - 0.2).abs() < 0.01 && sun.color[2] < 80, "the sun set: {sun:?}");
    let line = s.triggers[0].to_line("ball");
    assert!(line.contains("sun 70 0.2 #ff8040 1") && line.contains("finish"), "{line}");
}

#[test]
fn a_trigger_turns_dusk_into_night_sky_tint_and_all() {
    let m = mats();
    let mut s = sim(&m, &format!("{ROOM}sky #3060c0 #e0a070\nlighting 0.8 #ffd0a0\nbody circle iron 20 20 3\nname ball\ntrigger 0 30 120 20 by ball : sky #000010 #101830 1 ; lighting 0.2 1 #6080ff\n"));
    for _ in 0..200 {
        s.step(&m);
        if s.triggers[0].fired {
            break;
        }
    }
    assert!(s.triggers[0].fired);
    for _ in 0..61 {
        s.step(&m);
    }
    let (top, bottom, _) = s.backdrop.sky.unwrap();
    assert!(top[2] < 30 && bottom[2] < 60, "the sky went dark: {top:?} {bottom:?}");
    assert_eq!(s.lighting.ambient_color, [0x60, 0x80, 0xff], "and the light turned moonlight blue");
    assert!((s.lighting.ambient - 0.2).abs() < 0.02);
    let line = s.triggers[0].to_line("ball");
    assert!(line.contains("sky #000010 #101830 1") && line.contains("lighting 0.2 1 #6080ff"), "{line}");
    // Saved mid-fade, it's saved as where it's heading.
    let mut s2 = sim(&m, &format!("{ROOM}sky #3060c0 #e0a070\n"));
    s2.backdrop.sky_to([0, 0, 16], [16, 24, 48], 5.0);
    for _ in 0..30 {
        s2.step(&m);
    }
    let text = scene::save(&s2.snapshot(), &m);
    assert!(text.contains("sky #000010 #101830"), "{text}");
}
