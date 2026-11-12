//! Rebuild the native construction kit and editor demonstration scene.
use grave_engine::{
    Materials,
    asset::{Asset, Brush, Weld},
    building::Building,
    test_bay::{Recipe, TestKind},
};
use std::path::Path;
fn main() {
    let m = Materials::load_dir("materials");
    let save = |a: Asset| {
        assert!(a.validate(&m).is_empty(), "{:?}", a.validate(&m));
        a.save(
            &Path::new("assets/workshop").join(format!("{}.asset", a.name)),
            &m,
        )
        .unwrap();
    };
    for (name, mat, color) in [
        ("timber_beam", "wood", [146, 101, 61]),
        ("steel_beam", "iron", [121, 143, 155]),
        ("brick_wall", "brick", [159, 91, 67]),
    ] {
        let mut a = Asset::blank(name, 40, 12).unwrap();
        a.pivot = (20., 10.);
        a.rename_part(0, "structure").unwrap();
        a.draw(0, Brush::Rectangle, (0, 2), (39, 9), m.id(mat).unwrap());
        a.paint_surface(0, Brush::Rectangle, (0, 2), (39, 9), Some(color));
        save(a);
    }
    let mut a = Asset::blank("breakable_window", 32, 36).unwrap();
    a.rename_part(0, "wood_frame").unwrap();
    a.draw(0, Brush::Rectangle, (0, 0), (31, 35), m.id("wood").unwrap());
    a.draw(0, Brush::Rectangle, (3, 3), (28, 32), 0);
    a.paint_surface(0, Brush::Rectangle, (0, 0), (31, 35), Some([74, 112, 101]));
    let p = a.add_part("glass_pane").unwrap();
    a.draw(
        p,
        Brush::Rectangle,
        (3, 3),
        (28, 32),
        m.id("glass").unwrap(),
    );
    a.paint_surface(p, Brush::Rectangle, (3, 3), (28, 32), Some([119, 178, 192]));
    a.welds.push(Weld {
        a: "wood_frame".into(),
        b: "glass_pane".into(),
        x: 3.,
        y: 18.,
        break_force: Some(80.),
    });
    save(a);
    save(
        Building {
            name: "studio_timber_house".into(),
            ..Default::default()
        }
        .build(&m)
        .unwrap(),
    );
    save(
        Building {
            name: "studio_layered_house".into(),
            facade: true,
            ..Default::default()
        }
        .build(&m)
        .unwrap(),
    );
    std::fs::create_dir_all("assets/tests").unwrap();
    for (name, asset, kind) in [
        ("crate_burn", "fuel_crate", TestKind::Burn),
        ("house_collapse", "studio_layered_house", TestKind::Burn),
        ("barrel_impact", "explosive_barrel", TestKind::Impact),
    ] {
        let r = Recipe {
            name: name.into(),
            asset: asset.into(),
            kind,
            ..Default::default()
        };
        std::fs::write(format!("assets/tests/{name}.test"), r.text()).unwrap();
    }
    std::fs::write("scenes/editor-studio.txt", "# Construction kit playground: source assets stay reusable.\nsize 560 220\nseed 7\nfill air\nrect stone 0 190 560 30\nrect sandstone 0 190 560 3\nlighting 0.55 #b8c9e5\nsun 25 0.65 #ffe2bc\nsky #182d47 #a66d6b stars\nlayer city #283a50 depth 0.25 y 191 height 55 windows\nasset ../assets/workshop/studio_timber_house.asset 100 190 as house\nasset ../assets/workshop/studio_layered_house.asset 230 190 as layered_house\nasset ../assets/workshop/tnt_box.asset 330 190 as tnt\nasset ../assets/workshop/explosive_barrel.asset 375 190 as barrel\nasset ../assets/workshop/breakable_window.asset 440 190 as window\nasset ../assets/workshop/timber_beam.asset 500 190 as beam\nlight 100 140 65 color #ffbf80 strength 1.1 unshadowed\nlight 230 140 65 color #80cfff strength 1.1 unshadowed\nlight 370 155 60 color #ffbd82 strength 1.2\ntrigger 320 145 15 45 : message Charge zone reached!\n").unwrap();
}
