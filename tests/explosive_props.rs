//! Check reusable prop data against the real material/physics simulation.
use grave_engine::{Materials, Sim, asset::Asset, scene};
use std::path::Path;

#[test]
fn explosive_props_are_reusable_stable_and_really_explode() {
    let m = Materials::load_dir("materials");
    for name in ["tnt_box", "explosive_barrel"] {
        let a = Asset::load_file(
            &Path::new("assets/workshop").join(format!("{name}.asset")),
            &m,
        )
        .unwrap();
        assert!(a.validate(&m).is_empty(), "{name}: {:?}", a.validate(&m));
        assert_eq!(Asset::parse(&a.to_text(&m), &m).unwrap(), a);
        assert!(
            a.parts
                .iter()
                .any(|p| p.cells.contains(&m.id("gunpowder").unwrap()))
        );
        let mut s = Sim::empty(180, 140, &m);
        for y in 110..140 {
            for x in 0..180 {
                s.world.set(&m, x, y, m.id("stone").unwrap());
            }
        }
        let count = a.instantiate(&mut s, &m, 90., 84.).unwrap().len();
        for _ in 0..180 {
            s.step(&m);
        }
        assert_eq!(
            s.physics.body_count(),
            count,
            "{name} should survive this short drop"
        );
        assert_eq!(
            s.physics.joints().len(),
            a.welds.len(),
            "{name} attachments broke during a short drop"
        );
        let snapshot = s.snapshot();
        let (_, problems) = scene::load(&scene::save(&snapshot, &m), &m, None).into_sim(&m);
        assert!(problems.is_empty(), "{name}: {problems:?}");
        s.world.heat_circle(90, 85, 35, 1000.);
        let mut blasts = 0;
        for _ in 0..60 {
            s.world.step(&m);
            blasts += s.world.blasts.len();
            s.physics.update(&mut s.world, &m);
        }
        assert!(blasts > 0, "{name} must use real explosive material");
    }
}
