//! Draws every creature species big, side by side: shots/creature-sheet.png.
fn main() {
    let kinds = grave_engine::creatures::species().unwrap();
    let (cw, ch, s) = (20usize, 16usize, 10usize);
    let mut img = image::RgbImage::from_pixel((cw * s * kinds.len()) as u32, (ch * s * 2) as u32, image::Rgb([24, 26, 32]));
    for (k, sp) in kinds.iter().enumerate() {
        let mut c = grave_engine::creatures::Creature::new(sp.clone(), (0.0, 0.0));
        c.element = Some(match sp.id.as_str() { "mite" => "crystal_violet", "bloater" => "volatile_gas", "worm" => "gravel", "beetle" => "ember", _ => "fire" }.into());
        for (row, dead) in [(0, false), (1, true)] {
            for (x, y, rgb) in grave_engine::creatures::pixels(&c, 7, dead) {
                for dy in 0..s { for dx in 0..s {
                    img.put_pixel(((k * cw + x) * s + dx) as u32, ((row * ch + y) * s + dy) as u32, image::Rgb(rgb));
                } }
            }
        }
    }
    img.save("shots/creature-sheet.png").unwrap();
    println!("{}", kinds.iter().map(|k| k.id.clone()).collect::<Vec<_>>().join(", "));
}
