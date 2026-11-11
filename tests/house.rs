//! The timber house (scenes/house.txt, built in Blender by
//! tools/make_house_in_blender.py): it stands up by itself, and things
//! thrown at it break bits off without the whole house falling.

use grave_engine::{Materials, scene};
use std::path::Path;

#[test]
fn the_timber_house_stands_and_a_cannonball_smashes_a_corner() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let m = Materials::load_dir(format!("{dir}/materials"));
    let text = std::fs::read_to_string(format!("{dir}/scenes/house.txt")).unwrap();
    let (mut s, problems) = scene::load(&text, &m, Some(Path::new(&format!("{dir}/scenes")))).into_sim(&m);
    assert!(problems.is_empty(), "{problems:?}");
    let pile = s.physics.body_count();
    let iron = m.id("iron").unwrap();
    let iron_cells = |s: &grave_engine::Sim| s.world.mat.iter().filter(|&&c| c == iron).count();
    let house_iron = iron_cells(&s);
    for _ in 0..180 {
        s.step(&m);
    }
    assert_eq!(s.physics.body_count(), pile, "nothing falls off the house by itself");
    // Throw the big cannonball at the brick wall.
    let ball = s.physics.ids()[0];
    s.physics.edit(&mut s.world, &m, ball, |b| {
        b.x = 150.0;
        b.y = 150.0;
        b.vx = 450.0;
        b.vy = -60.0;
    });
    for _ in 0..120 {
        s.step(&m);
    }
    assert!(s.physics.body_count() > pile + 3, "bits broke off ({} objects)", s.physics.body_count());
    assert!(s.physics.body_count() < pile + 80, "but the house didn't all come down ({} objects)", s.physics.body_count());
    assert!(iron_cells(&s) * 10 >= house_iron * 9, "iron isn't lost");
}
