//! Importing models and images, and material textures.

use grave_engine::import;
use grave_engine::render::{Renderer, View};
use grave_engine::{Materials, World};
use std::path::{Path, PathBuf};

fn mats() -> Materials {
    Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/materials"))
}

fn asset(name: &str) -> PathBuf {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets")).join(name)
}

fn count(shape: &import::Shape, m: &Materials, name: &str) -> usize {
    let id = m.id(name);
    shape.cells.iter().filter(|c| **c == id).count()
}

#[test]
fn the_blender_tower_imports_from_glb_and_obj_alike() {
    let m = mats();
    let (glb, w1) = import::load(&asset("tower.glb"), &m, None).unwrap();
    let (obj, w2) = import::load(&asset("tower.obj"), &m, None).unwrap();
    assert!(w1.is_empty() && w2.is_empty(), "{w1:?} {w2:?}");
    for name in ["stone", "wood", "iron"] {
        let (a, b) = (count(&glb, &m, name), count(&obj, &m, name));
        assert!(a > 20, "{name}: {a}");
        assert!((a as i32 - b as i32).abs() <= 4, "{name}: glb {a} vs obj {b}");
    }
    assert_eq!(count(&glb, &m, "ice"), 0, "paint colours never pick things that melt at room temperature");
}

#[test]
fn fbx_goes_through_blender_when_it_is_installed() {
    let m = mats();
    if import::find_blender().is_none() {
        eprintln!("no Blender here: skipping");
        return;
    }
    let (fbx, _) = import::load(&asset("tower.fbx"), &m, None).unwrap();
    let (glb, _) = import::load(&asset("tower.glb"), &m, None).unwrap();
    assert!((count(&fbx, &m, "stone") as i32 - count(&glb, &m, "stone") as i32).abs() <= 4);
}

#[test]
fn texture_colours_pick_materials() {
    let m = mats();
    let dir = std::env::temp_dir().join(format!("grave_obj_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // A picture: left half wood brown, right half iron grey.
    let mut img = image::RgbaImage::new(2, 1);
    img.put_pixel(0, 0, image::Rgba([122, 74, 38, 255]));
    img.put_pixel(1, 0, image::Rgba([138, 143, 153, 255]));
    img.save(dir.join("half.png")).unwrap();
    std::fs::write(dir.join("quad.mtl"), "newmtl Painted\nKd 1 1 1\nmap_Kd half.png\n").unwrap();
    std::fs::write(
        dir.join("quad.obj"),
        "mtllib quad.mtl\nv 0 0 0\nv 2 0 0\nv 2 1 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nusemtl Painted\nf 1/1 2/2 3/3\nf 1/1 3/3 4/4\n",
    )
    .unwrap();
    let (shape, warnings) = import::load(&dir.join("quad.obj"), &m, None).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!((shape.w, shape.h), (20, 10));
    let left = shape.cells[5 * 20 + 3];
    let right = shape.cells[5 * 20 + 16];
    assert_eq!(left, m.id("wood"));
    assert_eq!(right, m.id("iron"));
}

#[test]
fn materials_with_a_texture_are_drawn_with_it() {
    let m = mats();
    assert!(m.get(m.id("wood").unwrap()).texture.is_some());
    let mut w = World::new(48, 24);
    w.fill(&m, m.id("wood").unwrap());
    let mut r = Renderer::default();
    let px = r.draw(&w, &m, View::default()).to_vec();
    // A flat colour would give the same few shades everywhere; the grain
    // texture has seams that are clearly darker.
    let brightness: Vec<u32> = px.chunks(4).map(|p| p[0] as u32 + p[1] as u32 + p[2] as u32).collect();
    let (lo, hi) = (*brightness.iter().min().unwrap(), *brightness.iter().max().unwrap());
    assert!(hi > lo + 60, "texture should show light grain and dark seams: {lo}..{hi}");
}
