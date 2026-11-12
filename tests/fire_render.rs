//! Large fires must retain fuel detail and coloured flame tongues without changing simulation.
use grave_engine::{
    Materials, Sim,
    render::{Renderer, View},
    world::BURNING,
};

#[test]
fn broad_burning_walls_keep_colour_variation_and_animation_instead_of_clipping_yellow() {
    let m = Materials::load_dir("materials");
    let mut s = Sim::empty(140, 100, &m);
    let wood = m.id("wood").unwrap();
    for y in 18..82 {
        for x in 18..122 {
            s.world.set(&m, x, y, wood);
            let i = y * 140 + x;
            s.world.flags[i] |= BURNING;
            s.world.burn[i] = 120;
            s.world.temp[i] = 850.;
            s.world.paint[i] = grave_engine::asset::pack_paint(if y % 6 == 0 {
                [45, 26, 15]
            } else {
                [150, 105, 65]
            });
        }
    }
    let state = (
        s.world.mat.clone(),
        s.world.temp.clone(),
        s.world.burn.clone(),
    );
    let mut r = Renderer::default();
    r.light(&s);
    let full = r.draw(&s.world, &m, View::default()).to_vec();
    let pixels: Vec<_> = (18..82)
        .flat_map(|y| (18..122).map(move |x| (y * 140 + x) * 4))
        .map(|i| &full[i..i + 3])
        .collect();
    let yellow = pixels.iter().filter(|p| p[0] >= 250 && p[1] >= 230).count();
    let dark = pixels.iter().map(|p| p[1]).min().unwrap();
    let bright = pixels.iter().map(|p| p[1]).max().unwrap();
    let orange = pixels
        .iter()
        .filter(|p| p[0] > 135 && p[0] as u16 * 10 > p[1] as u16 * 13)
        .count();
    assert!(
        yellow < pixels.len() / 30,
        "bloom must not flatten burning walls into yellow"
    );
    assert!(
        bright - dark > 60,
        "fine flame and fuel variation remains visible"
    );
    assert!(
        orange > pixels.len() / 20,
        "hot flame patches remain visible"
    );
    let crop = r
        .draw_window(&s.world, &m, View::default(), (40, 30, 50, 30))
        .to_vec();
    for y in 0..30 {
        for x in 0..50 {
            assert_eq!(
                &crop[(y * 50 + x) * 4..(y * 50 + x + 1) * 4],
                &full[((y + 30) * 140 + x + 40) * 4..((y + 30) * 140 + x + 41) * 4]
            );
        }
    }
    assert_eq!(
        state,
        (
            s.world.mat.clone(),
            s.world.temp.clone(),
            s.world.burn.clone()
        )
    );
    s.world.frame += 8;
    assert_ne!(full, r.draw(&s.world, &m, View::default()));
}

#[test]
fn dense_combustion_gas_has_dark_red_and_orange_regions() {
    let m = Materials::load_dir("materials");
    let mut s = Sim::empty(100, 80, &m);
    for y in 15..65 {
        for x in 15..85 {
            s.world.set(&m, x, y, m.id("fire").unwrap());
        }
    }
    let mut r = Renderer::default();
    r.light(&s);
    let px = r.draw(&s.world, &m, View::default());
    let greens: Vec<_> = (20..60)
        .flat_map(|y| (20..80).map(move |x| px[(y * 100 + x) * 4 + 1]))
        .collect();
    assert!(
        greens.iter().max().unwrap() - greens.iter().min().unwrap() > 50,
        "flame tongues need tonal range"
    );
    assert!(greens.iter().filter(|g| **g >= 230).count() < greens.len() / 30);
}
