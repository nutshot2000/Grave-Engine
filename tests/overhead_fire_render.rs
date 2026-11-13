//! Overhead combustion reads as upright fire while retaining material detail.
use grave_engine::{
    Materials,
    overhead::{Point, Volume, view::Viewport},
    world::BURNING,
};
fn volume(m: &Materials) -> Volume {
    Volume::new(32, 24, 32, m).unwrap()
}
#[test]
fn gas_flames_are_upright_and_animate_instead_of_flat_ground_diamonds() {
    let m = Materials::load_dir("materials");
    let mut v = volume(&m);
    let at = Point::new(16., 12., 9.);
    let (y, i) = v.index(at).unwrap();
    let row = i / v.width;
    let x = i % v.width;
    v.sections[y].world.set(&m, x, row, m.id("fire").unwrap());
    let mut r = Viewport::new(240, 180);
    r.projection.scale = 3.;
    let cam = Point::new(16., 12., 0.);
    let first = r.draw(&v, &m, cam, &[], &[]).to_vec();
    let mut warm = Vec::new();
    for y in 0..180 {
        for x in 0..240 {
            let i = (y * 240 + x) * 4;
            if first[i] > 100 && first[i] as u16 > first[i + 1] as u16 * 3 / 2 {
                warm.push((x, y));
            }
        }
    }
    assert!(!warm.is_empty());
    let w = warm.iter().map(|p| p.0).max().unwrap() - warm.iter().map(|p| p.0).min().unwrap() + 1;
    let h = warm.iter().map(|p| p.1).max().unwrap() - warm.iter().map(|p| p.1).min().unwrap() + 1;
    assert!(h > w && h >= 6, "flame should stand upright: {w} × {h}");
    v.frame += 7;
    assert_ne!(first, r.draw(&v, &m, cam, &[], &[]));
}
fn painted_burn(m: &Materials, paint: [u8; 3]) -> Volume {
    let mut v = volume(m);
    v.box_part(
        "wall",
        m.id("wood").unwrap(),
        [8, 6, 3],
        [14, 8, 9],
        Some(paint),
        m,
    );
    for s in &mut v.sections {
        for i in 0..s.world.mat.len() {
            if s.world.mat[i] == m.id("wood").unwrap() {
                s.world.flags[i] |= BURNING;
                s.world.burn[i] = 120;
                s.world.temp[i] = 900.;
            }
        }
    }
    v
}
#[test]
fn burning_does_not_replace_the_whole_building_with_a_yellow_surface() {
    let m = Materials::load_dir("materials");
    let a = painted_burn(&m, [160, 60, 35]);
    let b = painted_burn(&m, [45, 110, 150]);
    let mut r = Viewport::new(260, 200);
    let camera = Point::new(16., 12., 0.);
    let red = r.draw(&a, &m, camera, &[], &[]).to_vec();
    let blue = r.draw(&b, &m, camera, &[], &[]).to_vec();
    let different = red
        .chunks_exact(4)
        .zip(blue.chunks_exact(4))
        .filter(|(a, b)| (0..3).any(|k| a[k].abs_diff(b[k]) > 8))
        .count();
    assert!(
        different > 180,
        "fuel paint/detail must still affect the burning building"
    );
    let yellow = red
        .chunks_exact(4)
        .filter(|p| p[0] > 245 && p[1] > 225)
        .count();
    assert!(yellow < 40, "avoid yellow clipping across the burning area");
}
#[test]
fn an_opaque_roof_hides_fire_beneath_it() {
    let m = Materials::load_dir("materials");
    let mut v = volume(&m);
    v.box_part(
        "roof",
        m.id("stone").unwrap(),
        [4, 2, 16],
        [25, 20, 2],
        Some([80, 80, 80]),
        &m,
    );
    let p = Point::new(10., 5., 9.);
    let (y, i) = v.index(p).unwrap();
    v.sections[y]
        .world
        .set(&m, i % v.width, i / v.width, m.id("fire").unwrap());
    let mut r = Viewport::new(240, 180);
    let px = r.draw(&v, &m, Point::new(16., 12., 0.), &[], &[]);
    let flame_pixels = px
        .chunks_exact(4)
        .filter(|p| p[0] > 130 && p[0] as u16 > p[1] as u16 * 2)
        .count();
    assert_eq!(flame_pixels, 0, "roof occludes the upright flame");
}
