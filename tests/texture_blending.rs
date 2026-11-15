//! Visual rock contacts: soft artwork joins never mix actual material cells.
use grave_engine::{Materials, World};
use grave_engine::render::{Renderer, View};

fn setup() -> (Materials, World, u16, u16) {
    let mut m = Materials::load_dir("materials");
    let (a, b) = (m.id("rock").unwrap(), m.id("shale").unwrap());
    for (id, color) in [(a, [190, 40, 30]), (b, [30, 60, 190])] {
        let mat = &mut m.list[id as usize];
        mat.texture = None;
        mat.color = color;
        mat.color_variation = 0.0;
        mat.texture_blend = 5.0;
    }
    let mut w = World::new(64, 32);
    for y in 0..32 { for x in 0..64 { w.set(&m, x, y, if x < 32 { a } else { b }); } }
    (m, w, a, b)
}

#[test]
fn opted_in_rocks_soften_only_near_the_contact_without_changing_cells() {
    let (m, w, _, _) = setup();
    let before = w.mat.clone();
    let mut r = Renderer::default();
    let px = r.draw(&w, &m, View::default()).to_vec();
    let rgb = |x: usize| &px[(16 * 64 + x) * 4..(16 * 64 + x) * 4 + 3];
    assert_eq!(rgb(10), &[190, 40, 30]);
    assert_eq!(rgb(50), &[30, 60, 190]);
    assert!(rgb(31)[2] > 70 && rgb(31)[0] < 170, "left rock should borrow some right texture");
    assert!(rgb(32)[0] > 70 && rgb(32)[2] < 170, "right rock should borrow some left texture");
    assert_eq!(w.mat, before);
    let crop = r.draw_window(&w, &m, View::default(), (24, 8, 16, 16)).to_vec();
    for y in 0..16 { for x in 0..16 {
        assert_eq!(&crop[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4], &px[((y + 8) * 64 + x + 24) * 4..((y + 8) * 64 + x + 24) * 4 + 4]);
    } }
}

#[test]
fn crisp_materials_objects_and_air_gaps_do_not_bleed() {
    let (mut m, mut w, a, b) = setup();
    m.list[b as usize].texture_blend = 0.0;
    let mut r = Renderer::default();
    let crisp = r.draw(&w, &m, View::default()).to_vec();
    assert_eq!(&crisp[(16 * 64 + 31) * 4..(16 * 64 + 31) * 4 + 3], &[190, 40, 30]);
    m.list[b as usize].texture_blend = 5.0;
    for y in 0..32 { w.set(&m, 32, y, 0); }
    let gap = r.draw(&w, &m, View::default()).to_vec();
    assert_eq!(&gap[(16 * 64 + 31) * 4..(16 * 64 + 31) * 4 + 3], &[190, 40, 30]);
    for y in 0..32 { w.set(&m, 32, y, b); }
    let i = w.idx(31, 16);
    w.flags[i] |= grave_engine::world::OWNED;
    let object = r.draw(&w, &m, View::default()).to_vec();
    assert_eq!(&object[i * 4..i * 4 + 3], &[190, 40, 30]);
    assert_eq!(w.mat[i], a);
}
