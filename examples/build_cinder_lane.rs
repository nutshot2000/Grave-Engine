//! Native street authoring: uses the same building/asset tools as Editor Studio.
use grave_engine::{
    Materials,
    asset::{Asset, Brush, Weld},
    building::Building,
    headless,
    render::Renderer,
    scene,
};
use std::path::Path;
fn rect(a: &mut Asset, p: usize, x: i32, y: i32, w: i32, h: i32, mat: u16) {
    a.draw(p, Brush::Rectangle, (x, y), (x + w - 1, y + h - 1), mat);
}
fn paint(a: &mut Asset, p: usize, color: [u8; 3]) {
    a.paint_surface(
        p,
        Brush::Rectangle,
        (0, 0),
        (a.w as i32 - 1, a.h as i32 - 1),
        Some(color),
    );
}
fn save(a: &Asset, m: &Materials) {
    let problems = a.validate(m);
    assert!(problems.is_empty(), "{}: {problems:?}", a.name);
    a.save(
        &Path::new("assets/workshop").join(format!("{}.asset", a.name)),
        m,
    )
    .unwrap();
}
fn main() {
    let m = Materials::load_dir("materials");
    let wood = m.id("wood").unwrap();
    let iron = m.id("iron").unwrap();
    let powder = m.id("gunpowder").unwrap();
    let cloth = m.id("cloth").unwrap();
    for (name, facade, roof) in [
        ("street_rose_house", [178, 116, 92], [113, 54, 43]),
        ("street_sage_house", [106, 143, 130], [63, 79, 95]),
    ] {
        let mut a = Building {
            name: name.into(),
            width: 128,
            height: 120,
            facade: true,
            ..Default::default()
        }
        .build(&m)
        .unwrap();
        paint(&mut a, 0, facade);
        let p = a.parts.iter().position(|p| p.name == "roof").unwrap();
        paint(&mut a, p, roof);
        for (name, x, y, w, h) in [
            ("sofa", 12, 100, 27, 12),
            ("bed", 12, 54, 29, 13),
            ("table", 82, 98, 22, 14),
        ] {
            let p = a.add_part(name).unwrap();
            a.parts[p].layer = 1;
            rect(&mut a, p, x, y, w, 3, wood);
            rect(&mut a, p, x, y + 3, 3, h - 3, wood);
            rect(&mut a, p, x + w - 3, y + 3, 3, h - 3, wood);
            if name != "table" {
                rect(&mut a, p, x + 3, y + 3, w - 6, 6, cloth);
            }
            paint(
                &mut a,
                p,
                if name == "sofa" {
                    [173, 100, 66]
                } else if name == "bed" {
                    [93, 139, 160]
                } else {
                    [119, 78, 47]
                },
            );
        }
        save(&a, &m);
    }
    for (name, title, color) in [
        ("street_night_owl", "NIGHT OWL", [35, 92, 82]),
        ("street_fuel_depot", "FUEL DEPOT", [118, 46, 40]),
    ] {
        let mut a = Asset::load_file(Path::new("assets/workshop/timber_shop.asset"), &m).unwrap();
        a.name = name.into();
        let sign = a.parts.iter().position(|p| p.name == "shop_sign").unwrap();
        paint(&mut a, sign, color);
        a.paint_text(
            sign,
            ((128 - (title.len() as i32 * 6 - 1)) / 2, 45),
            title,
            1,
            [255, 225, 145],
        );
        let trim = a
            .parts
            .iter()
            .position(|p| p.name == "timber_shopfront")
            .unwrap();
        paint(&mut a, trim, color);
        save(&a, &m);
    }
    let mut a = Asset::blank("street_powder_crate", 22, 18).unwrap();
    a.depth = 0.12;
    a.pivot = (11., 18.);
    a.rename_part(0, "wooden_case").unwrap();
    rect(&mut a, 0, 0, 0, 22, 18, wood);
    rect(&mut a, 0, 3, 3, 16, 12, 0);
    paint(&mut a, 0, [143, 92, 49]);
    // A wooden front label remains wood, while the hidden core is real powder.
    rect(&mut a, 0, 0, 5, 22, 8, wood);
    a.paint_surface(0, Brush::Rectangle, (0, 5), (21, 12), Some([185, 42, 32]));
    a.paint_text(0, (2, 6), "TNT", 1, [24, 20, 17]);
    let p = a.add_part("powder_core").unwrap();
    rect(&mut a, p, 3, 3, 16, 2, powder);
    rect(&mut a, p, 3, 13, 16, 2, powder);
    // Keep the powder one connected body through an open side channel.
    rect(&mut a, 0, 3, 3, 2, 12, 0);
    rect(&mut a, p, 3, 3, 2, 12, powder);
    a.welds.push(Weld {
        a: "wooden_case".into(),
        b: "powder_core".into(),
        x: 3.,
        y: 8.,
        break_force: Some(25.),
    });
    save(&a, &m);
    let mut a = Asset::blank("street_powder_barrel", 18, 26).unwrap();
    a.depth = 0.10;
    a.pivot = (9., 26.);
    a.rename_part(0, "steel_shell").unwrap();
    rect(&mut a, 0, 1, 1, 16, 24, iron);
    rect(&mut a, 0, 3, 3, 12, 20, 0);
    paint(&mut a, 0, [171, 53, 37]);
    for y in [4, 20] {
        a.paint_surface(
            0,
            Brush::Rectangle,
            (1, y),
            (16, y + 1),
            Some([79, 94, 100]),
        );
    }
    let p = a.add_part("powder_core").unwrap();
    rect(&mut a, p, 3, 3, 12, 20, powder);
    rect(&mut a, 0, 3, 9, 12, 8, iron);
    a.paint_surface(0, Brush::Rectangle, (1, 9), (16, 16), Some([204, 146, 55]));
    a.paint_text(0, (1, 10), "TNT", 1, [25, 22, 19]);
    rect(&mut a, 0, 3, 3, 1, 20, 0);
    rect(&mut a, p, 3, 3, 1, 20, powder);
    a.welds.push(Weld {
        a: "steel_shell".into(),
        b: "powder_core".into(),
        x: 3.,
        y: 13.,
        break_force: Some(30.),
    });
    save(&a, &m);
    let mut a = Asset::blank("street_lamp", 24, 90).unwrap();
    a.depth = 0.04;
    a.pivot = (12., 89.);
    a.rename_part(0, "iron_post").unwrap();
    rect(&mut a, 0, 8, 7, 3, 82, iron);
    rect(&mut a, 0, 5, 85, 9, 4, iron);
    rect(&mut a, 0, 8, 5, 15, 3, iron);
    rect(&mut a, 0, 17, 8, 6, 2, iron);
    paint(&mut a, 0, [60, 75, 87]);
    let p = a.add_part("warm_lantern").unwrap();
    rect(&mut a, p, 17, 10, 6, 3, m.id("lamp_glass").unwrap());
    paint(&mut a, p, [255, 215, 143]);
    a.welds.push(Weld {
        a: "iron_post".into(),
        b: "warm_lantern".into(),
        x: 20.,
        y: 10.,
        break_force: Some(80.),
    });
    save(&a, &m);
    let mut a = Asset::blank("street_bench", 30, 19).unwrap();
    a.depth = 0.08;
    a.pivot = (15., 19.);
    a.rename_part(0, "timber_seat").unwrap();
    rect(&mut a, 0, 0, 1, 30, 3, wood);
    rect(&mut a, 0, 0, 7, 30, 3, wood);
    rect(&mut a, 0, 2, 3, 2, 7, wood);
    rect(&mut a, 0, 26, 3, 2, 7, wood);
    paint(&mut a, 0, [133, 92, 54]);
    let p = a.add_part("metal_legs").unwrap();
    rect(&mut a, p, 3, 10, 24, 2, iron);
    rect(&mut a, p, 3, 12, 3, 7, iron);
    rect(&mut a, p, 24, 12, 3, 7, iron);
    paint(&mut a, p, [65, 78, 84]);
    a.welds.push(Weld {
        a: "timber_seat".into(),
        b: "metal_legs".into(),
        x: 15.,
        y: 10.,
        break_force: Some(120.),
    });
    save(&a, &m);
    let mut s = String::from(
        "# Cinder Lane: a fresh demolition playground.\nsize 770 250\nseed 41\noutside_temperature 12\nfill air\nsky #090e22 #34445c stars\nlayer city #192a42 depth 0.25 y 200 height 60 windows\nlighting 0.48 #b4c3de\nrect stone 0 200 770 50\nanchor 0 200 770 50\n",
    );
    for y in 200..250 {
        for x in 0..770 {
            let c = if y < 204 {
                "#71808b"
            } else if y < 208 {
                "#3b4855"
            } else if y == 229 && (x % 46 < 24) {
                "#c5b88d"
            } else {
                "#202d3b"
            };
            s += &format!("paint {x} {y} {c}\n");
        }
    }
    for (x, name, alias) in [
        (90, "street_rose_house", "rose"),
        (256, "street_night_owl", "owl"),
        (430, "street_fuel_depot", "depot"),
        (600, "street_sage_house", "sage"),
    ] {
        s += &format!("asset ../assets/workshop/{name}.asset {x} 200 as {alias}\n");
        if name.contains("house") {
            s += &format!("light {x} 144 65 strength 0.85 color #ffc58c unshadowed\n");
        } else {
            s += &format!(
                "weld ground {alias}.timber_facade at {} 197 break 1800\nweld ground {alias}.timber_facade at {} 197 break 1800\nlight {} 144 57 strength 0.95 color {} unshadowed on {alias}.shop_sign\n",
                x - 54,
                x + 54,
                x,
                if alias == "owl" { "#77dfcf" } else { "#ffad73" }
            );
        }
    }
    for (i, x) in [12, 180, 350, 522, 688].iter().enumerate() {
        s += &format!(
            "asset ../assets/workshop/street_lamp.asset {x} 200 as lamp{i}\nweld ground lamp{i}.iron_post at {} 198 break 220\nlight {} 125 100 strength 1.8 color #ffd397 cone 90 76 on lamp{i}.warm_lantern\n",
            x - 3,
            x + 8
        );
    }
    for (name, x, y, alias) in [
        ("street_powder_crate", 164, 200, "charge1"),
        ("street_powder_barrel", 164, 182, "barrel1"),
        ("street_powder_crate", 330, 200, "charge2"),
        ("street_powder_barrel", 330, 182, "barrel2"),
        ("street_powder_crate", 506, 200, "charge3"),
        ("street_powder_barrel", 506, 182, "barrel3"),
        ("street_powder_crate", 672, 200, "charge4"),
    ] {
        s += &format!("asset ../assets/workshop/{name}.asset {x} {y} as {alias}\n");
    }
    // Oil is poured into unoccupied gaps after stamping; no fire is pre-lit.
    for x in [149, 317, 491] {
        s += &format!("rect oil {x} 198 40 2\n");
    }
    for (x, name) in [(710, "bench_west"), (750, "bench_east")] {
        s += &format!("asset ../assets/workshop/street_bench.asset {x} 200 as {name}\n");
    }
    for z in 1..=2 {
        s += &format!(
            "plane {z} {}\nsize 770 250\nfill air\nrect stone 0 200 770 50\nanchor 0 200 770 50\nlighting 0.5 #d9c6ab\n",
            if z == 1 { "interior" } else { "back_wall" }
        );
        if z == 1 {
            for x in [90, 256, 430, 600] {
                s += &format!("light {x} 150 53 strength 0.7 color #ffd39c unshadowed\n");
            }
        }
        s += "endplane\n";
    }
    std::fs::write("scenes/cinder-lane.txt", s).unwrap();
    if std::env::args().any(|a| a == "--verify") {
        verify(&m);
    }
}
fn verify(m: &Materials) {
    let text = std::fs::read_to_string("scenes/cinder-lane.txt").unwrap();
    let (mut s, p) = scene::load(&text, m, Some(Path::new("scenes"))).into_sim(m);
    assert!(p.is_empty(), "{p:?}");
    let initial_groups: std::collections::BTreeMap<_, _> = grave_engine::assemblies::names(&s)
        .into_iter()
        .map(|n| {
            let count = s
                .physics
                .ids()
                .iter()
                .filter(|id| s.physics.assembly(**id) == Some(n.as_str()))
                .count();
            (n, count)
        })
        .collect();
    let mut r = Renderer::default();
    for _ in 0..180 {
        s.step(m);
    }
    r.light(&s);
    headless::shot(
        &s.world,
        m,
        &mut r,
        Path::new("shots/cinder-lane-standing.png"),
        2,
        false,
    )
    .unwrap();
    let counts = |s: &grave_engine::Sim, mat| {
        std::iter::once(&s.world)
            .chain(s.layers.iter().map(|l| &l.world))
            .map(|w| w.mat.iter().filter(|v| **v == mat).count())
            .sum::<usize>()
    };
    for (name, count) in &initial_groups {
        let n = s
            .physics
            .ids()
            .iter()
            .filter(|id| s.physics.assembly(**id) == Some(name.as_str()))
            .count();
        assert_eq!(n, *count, "unlit group {name} must stay intact");
    }
    let before = counts(&s, m.id("wood").unwrap());
    let powder = counts(&s, m.id("gunpowder").unwrap());
    let bodies = s.physics.body_count();
    let mut caught = false;
    for f in 0..900 {
        if f < 90 && f % 15 == 0 {
            s.world.heat_circle(164, 185, 20, 950.);
        }
        s.step(m);
        caught |= s.layers.iter().any(|l| {
            l.world
                .flags
                .iter()
                .any(|v| v & grave_engine::world::BURNING != 0)
        });
        if f == 180 {
            r.light(&s);
            headless::shot(
                &s.world,
                m,
                &mut r,
                Path::new("shots/cinder-lane-fire.png"),
                2,
                false,
            )
            .unwrap();
        }
    }
    r.light(&s);
    headless::shot(
        &s.world,
        m,
        &mut r,
        Path::new("shots/cinder-lane-aftermath.png"),
        2,
        false,
    )
    .unwrap();
    let after = counts(&s, m.id("wood").unwrap());
    let powder_after = counts(&s, m.id("gunpowder").unwrap());
    println!(
        "Wood {before}->{after}; powder {powder}->{powder_after}; root bodies {bodies}->{}; interior caught fire: {caught}",
        s.physics.body_count()
    );
    assert!(
        caught,
        "shop and house interiors must catch from the outside fire"
    );
    assert!(after < before, "fire must damage the street");
    assert!(powder_after < powder, "powder props must detonate");
}
