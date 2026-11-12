//! Frontier town authored with native named material parts and depth planes.
use grave_engine::{
    Materials,
    asset::{Asset, Brush, Weld, pack_paint},
    headless,
    render::Renderer,
    scene,
};
use std::{collections::BTreeMap, path::Path};
fn rect(a: &mut Asset, p: usize, x: i32, y: i32, w: i32, h: i32, mat: u16) {
    a.draw(p, Brush::Rectangle, (x, y), (x + w - 1, y + h - 1), mat);
}
fn line(a: &mut Asset, p: usize, a0: (i32, i32), b: (i32, i32), mat: u16) {
    a.draw(p, Brush::Line, a0, b, mat);
}
fn part(a: &mut Asset, name: &str, z: usize) -> usize {
    let p = a.add_part(name).unwrap();
    a.parts[p].layer = z;
    p
}
fn blank(name: &str, w: usize, h: usize, layers: &[&str]) -> Asset {
    let mut a = Asset::blank(name, w, h).unwrap();
    a.parts.clear();
    a.layers = layers
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if i == 0 {
                "front".into()
            } else {
                s.to_string()
            }
        })
        .collect();
    a.pivot = (w as f32 / 2., h as f32 - 1.);
    a.depth = 0.05;
    a
}
fn coat(a: &mut Asset, p: usize, c: [u8; 3]) {
    for i in 0..a.w * a.h {
        if a.parts[p].cells[i] != 0 {
            a.parts[p].paint[i] = pack_paint(c);
        }
    }
}
fn boards(a: &mut Asset, p: usize, c: [u8; 3], vertical: bool) {
    for i in 0..a.w * a.h {
        if a.parts[p].cells[i] == 0 {
            continue;
        }
        let (x, y) = (i % a.w, i / a.w);
        let (cross, along) = if vertical { (x, y) } else { (y, x) };
        let seam = cross % 5 == 0;
        let nail = cross % 5 == 1 && (along + (cross / 5) * 7) % 23 == 2;
        let k = if nail {
            0.35
        } else if seam {
            0.70
        } else {
            0.94 + ((cross / 5 * 13) % 7) as f32 * 0.02
        };
        a.parts[p].paint[i] = pack_paint(c.map(|v| (v as f32 * k).min(255.) as u8));
    }
}
fn weld(a: &mut Asset, x: usize, y: usize, at: (f32, f32), force: f32) {
    assert_eq!(a.parts[x].layer, a.parts[y].layer);
    a.welds.push(Weld {
        a: a.parts[x].name.clone(),
        b: a.parts[y].name.clone(),
        x: at.0,
        y: at.1,
        break_force: Some(force),
    });
}
fn gable(a: &mut Asset, p: usize, left: i32, right: i32, base: i32, peak: i32, wood: u16) {
    let mid = (left + right) / 2;
    for x in left..=right {
        let y = peak + (base - peak) * (x - mid).abs() / ((right - left) / 2).max(1);
        rect(a, p, x, y, 1, base - y + 1, wood);
    }
}
fn window(
    a: &mut Asset,
    m: &Materials,
    facade: usize,
    frame: usize,
    name: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    let wood = m.id("weathered_wood").unwrap();
    let glass = m.id("glass").unwrap();
    let z = a.parts[facade].layer;
    rect(a, facade, x - 2, y - 2, w + 4, h + 4, wood);
    rect(a, facade, x, y, w, h, 0);
    let p = part(a, name, z);
    rect(a, p, x, y, w, h, glass);
    coat(a, p, [95, 126, 131]);
    for k in 2..w - 2 {
        if k % 7 == 0 {
            a.paint_surface(
                p,
                Brush::Line,
                (x + k, y + 2),
                (x + k - 4, y + 8),
                Some([170, 187, 173]),
            );
        }
    }
    // Mullions sit immediately behind the connected glass pane.
    let fy = if y < 60 {
        39
    } else if a.h == 128 {
        85
    } else {
        a.h as i32 - 42
    };
    line(a, frame, (x + w / 2, fy), (x + w / 2, y + h), wood);
    line(a, frame, (x - 2, y + h / 2), (x + w + 1, y + h / 2), wood);
    weld(a, facade, p, (x as f32, y as f32), 75.);
}
fn building(m: &Materials, kind: &str) -> Asset {
    let saloon = kind == "saloon";
    let ranch = kind == "ranch";
    let sheriff = kind == "sheriff";
    let (w, h) = if saloon {
        (128, 128)
    } else if ranch {
        (128, 108)
    } else if sheriff {
        (104, 104)
    } else {
        (112, 108)
    };
    let name = format!("west_{kind}");
    let mut a = blank(
        &name,
        w,
        h,
        &["porch", "timber_front", "interior", "backboards"],
    );
    let (w, h) = (w as i32, h as i32);
    let wood = m.id("weathered_wood").unwrap();
    let iron = m.id("iron").unwrap();
    let cloth = m.id("cloth").unwrap();
    let colour = if saloon {
        [139, 101, 69]
    } else if ranch {
        [151, 126, 86]
    } else if sheriff {
        [132, 119, 95]
    } else {
        [114, 137, 116]
    };
    let deck = part(&mut a, "boardwalk", 0);
    rect(&mut a, deck, 2, h - 5, w - 4, 3, wood);
    boards(&mut a, deck, [139, 109, 73], true);
    let frame = part(&mut a, "porch_posts", 0);
    let canopy = if saloon { 83 } else { h - 44 };
    rect(&mut a, frame, 5, canopy, w - 10, 3, wood);
    for x in [6, w / 3, 2 * w / 3, w - 9] {
        rect(&mut a, frame, x, canopy + 3, 3, h - 8 - canopy, wood);
    }
    boards(&mut a, frame, [99, 75, 50], true);
    weld(&mut a, deck, frame, (8., (h - 5) as f32), 1400.);
    let canopy_p = part(
        &mut a,
        if saloon {
            "balcony_deck"
        } else {
            "porch_canopy"
        },
        0,
    );
    rect(&mut a, canopy_p, 1, canopy - 3, w - 2, 3, wood);
    boards(&mut a, canopy_p, [100, 77, 53], true);
    weld(&mut a, frame, canopy_p, (8., canopy as f32), 900.);
    let roof = part(&mut a, "roof_and_falsefront", 0);
    if ranch {
        gable(&mut a, roof, 5, w - 6, 34, 14, wood);
        gable(&mut a, roof, 39, 89, 49, 4, wood);
    } else if saloon {
        gable(&mut a, roof, 5, w - 6, 39, 4, wood);
    } else {
        rect(&mut a, roof, 6, 20, w - 12, 18, wood);
        rect(&mut a, roof, 13, 12, w - 26, 8, wood);
        rect(&mut a, roof, 21, 7, w - 42, 5, wood);
    }
    if ranch {
        let p = part(&mut a, "gable_posts", 0);
        rect(&mut a, p, 42, 50, 45, 2, wood);
        for x in [42, 84] {
            rect(&mut a, p, x, 52, 3, canopy - 55, wood);
        }
        boards(&mut a, p, [112, 87, 55], true);
        weld(&mut a, roof, p, (64., 50.), 600.);
        weld(&mut a, canopy_p, p, (43., (canopy - 3) as f32), 600.);
    }
    if !ranch && !saloon {
        rect(&mut a, roof, 8, 12, w - 16, 3, wood);
        for x in [8, w - 11] {
            rect(&mut a, roof, x, 15, 3, 23, wood);
        }
    }
    boards(&mut a, roof, [100, 75, 50], false);
    weld(&mut a, frame, roof, (w as f32 / 2., canopy as f32), 1600.);
    let sign = part(
        &mut a,
        if ranch {
            "longhorn_emblem"
        } else {
            "painted_sign"
        },
        0,
    );
    if ranch {
        rect(&mut a, sign, 52, 26, 24, 15, wood);
        boards(&mut a, sign, [90, 66, 45], false);
        a.paint_surface(sign, Brush::Line, (55, 29), (59, 32), Some([223, 211, 174]));
        a.paint_surface(sign, Brush::Line, (59, 32), (68, 32), Some([223, 211, 174]));
        a.paint_surface(sign, Brush::Line, (68, 32), (72, 29), Some([223, 211, 174]));
        a.paint_surface(
            sign,
            Brush::Rectangle,
            (61, 31),
            (66, 37),
            Some([223, 211, 174]),
        );
    } else {
        let sy = if saloon { 17 } else { 15 };
        rect(&mut a, sign, 13, sy, w - 26, 22, wood);
        coat(
            &mut a,
            sign,
            if sheriff {
                [188, 171, 128]
            } else {
                [75, 61, 43]
            },
        );
        let title = if saloon {
            "COPPER SPUR"
        } else if sheriff {
            "SHERIFF"
        } else {
            "CREEK TRADING"
        };
        let ty = if saloon { 19 } else { 18 };
        a.paint_text(
            sign,
            ((w - (title.len() as i32 * 6 - 1)) / 2, ty),
            title,
            1,
            if sheriff {
                [48, 48, 40]
            } else {
                [232, 207, 150]
            },
        );
        if saloon {
            a.paint_text(sign, (47, 29), "SALOON", 1, [231, 206, 148]);
        } else if !sheriff {
            a.paint_text(sign, (23, 28), "DRY GOODS", 1, [216, 190, 137]);
        }
    }
    weld(&mut a, roof, sign, (w as f32 / 2., 34.), 200.);
    if saloon {
        let rail = part(&mut a, "balcony_rail", 0);
        rect(&mut a, rail, 4, 66, w - 8, 2, wood);
        rect(&mut a, rail, 4, 78, w - 8, 2, wood);
        for x in (5..w - 4).step_by(7) {
            rect(&mut a, rail, x, 68, 1, 10, wood);
        }
        boards(&mut a, rail, [139, 108, 73], true);
        weld(&mut a, canopy_p, rail, (8., 80.), 180.);
    }
    for (name, x, ww) in [
        ("left_porch_rail", 8, w / 3 - 13),
        ("right_porch_rail", 2 * w / 3 + 4, w / 3 - 13),
    ] {
        let p = part(&mut a, name, 0);
        rect(&mut a, p, x, h - 19, ww, 2, wood);
        rect(&mut a, p, x, h - 9, ww, 2, wood);
        for px in [x, x + ww - 2] {
            rect(&mut a, p, px, h - 17, 2, 8, wood);
        }
        boards(&mut a, p, [130, 102, 65], false);
        weld(&mut a, frame, p, (x as f32, (h - 17) as f32), 140.);
    }
    let lower = part(&mut a, "lower_timber_front", 1);
    let wall_top = if saloon { 86 } else { 38 };
    rect(&mut a, lower, 9, wall_top, w - 18, h - 2 - wall_top, wood);
    boards(&mut a, lower, colour, false);
    let inside = part(&mut a, "interior_frame", 2);
    rect(&mut a, inside, 9, 34, 4, h - 36, wood);
    rect(&mut a, inside, w - 13, 34, 4, h - 36, wood);
    rect(&mut a, inside, 9, h - 5, w - 18, 3, iron);
    rect(&mut a, inside, 9, 34, w - 18, 3, wood);
    if saloon {
        rect(&mut a, inside, 9, 83, w - 18, 3, iron);
    }
    boards(&mut a, inside, [98, 75, 53], false);
    let back = part(&mut a, "room_backboards", 3);
    rect(&mut a, back, 13, 37, w - 26, h - 38, wood);
    boards(&mut a, back, [79, 66, 49], false);
    let wy = if saloon { 94 } else { h - 38 };
    window(&mut a, m, lower, frame, "left_display", 16, wy, 21, 19);
    window(&mut a, m, lower, frame, "right_display", w - 37, wy, 21, 19);
    let dx = w / 2 - 9;
    let dy = h - 34;
    rect(&mut a, lower, dx, dy, 18, 29, 0);
    if saloon {
        for (name, x) in [("left_swing_door", dx), ("right_swing_door", dx + 9)] {
            let p = part(&mut a, name, 1);
            rect(&mut a, p, x, dy + 9, 8, 18, wood);
            boards(&mut a, p, [96, 60, 38], true);
            weld(&mut a, lower, p, (x as f32, (dy + 9) as f32), 110.);
        }
    } else {
        let p = part(&mut a, "entrance_door", 1);
        rect(&mut a, p, dx, dy, 18, 29, wood);
        boards(&mut a, p, [88, 72, 52], true);
        a.paint_surface(
            p,
            Brush::Rectangle,
            (dx + 3, dy + 4),
            (dx + 14, dy + 15),
            Some([65, 75, 67]),
        );
        a.paint_surface(
            p,
            Brush::Pencil,
            (dx + 14, dy + 20),
            (dx + 14, dy + 20),
            Some([204, 171, 89]),
        );
        weld(&mut a, lower, p, (dx as f32, dy as f32), 180.);
    }
    if saloon {
        let upper = part(&mut a, "upper_timber_front", 1);
        rect(&mut a, upper, 9, 36, w - 18, 50, wood);
        boards(&mut a, upper, colour, false);
        window(&mut a, m, upper, roof, "upper_left_window", 18, 48, 21, 22);
        window(
            &mut a,
            m,
            upper,
            roof,
            "upper_right_window",
            w - 39,
            48,
            21,
            22,
        );
        rect(&mut a, upper, 56, 58, 16, 25, 0);
        let p = part(&mut a, "balcony_door", 1);
        rect(&mut a, p, 56, 58, 16, 25, wood);
        boards(&mut a, p, [93, 73, 52], true);
        weld(&mut a, upper, p, (56., 58.), 150.);
        weld(&mut a, lower, upper, (w as f32 / 2., 86.), 1300.);
    }
    // Carved timber trim and real glass/iron/cloth remain separate from colour.
    if sheriff {
        let cell = part(&mut a, "iron_jail_cell", 2);
        rect(&mut a, cell, w - 41, h - 45, 29, 2, iron);
        rect(&mut a, cell, w - 41, h - 7, 29, 2, iron);
        for x in (w - 41..w - 11).step_by(4) {
            rect(&mut a, cell, x, h - 43, 1, 36, iron);
        }
        coat(&mut a, cell, [75, 81, 78]);
        weld(
            &mut a,
            inside,
            cell,
            ((w - 13) as f32, (h - 7) as f32),
            750.,
        );
    } else {
        let p = part(
            &mut a,
            if saloon {
                "bar_counter"
            } else if ranch {
                "kitchen_table"
            } else {
                "shop_counter"
            },
            2,
        );
        rect(&mut a, p, w / 2 + 8, h - 19, w / 2 - 23, 4, wood);
        rect(&mut a, p, w / 2 + 10, h - 15, 3, 10, wood);
        rect(&mut a, p, w - 19, h - 15, 3, 10, wood);
        boards(&mut a, p, [127, 83, 46], false);
    }
    let furnishing = part(
        &mut a,
        if saloon {
            "upright_piano"
        } else if ranch {
            "bed_and_blanket"
        } else if sheriff {
            "sheriff_desk"
        } else {
            "stock_shelves"
        },
        2,
    );
    if saloon {
        rect(&mut a, furnishing, 15, h - 33, 25, 24, wood);
        rect(&mut a, furnishing, 17, h - 9, 3, 4, wood);
        rect(&mut a, furnishing, 35, h - 9, 3, 4, wood);
        boards(&mut a, furnishing, [76, 49, 32], false);
        a.paint_surface(
            furnishing,
            Brush::Rectangle,
            (16, h - 20),
            (38, h - 17),
            Some([208, 200, 162]),
        );
        for x in (17..39).step_by(3) {
            a.paint_surface(
                furnishing,
                Brush::Line,
                (x, h - 20),
                (x, h - 18),
                Some([30, 26, 22]),
            );
        }
    } else if ranch {
        rect(&mut a, furnishing, 16, h - 18, 26, 9, wood);
        rect(&mut a, furnishing, 18, h - 9, 3, 4, wood);
        rect(&mut a, furnishing, 37, h - 9, 3, 4, wood);
        rect(&mut a, furnishing, 17, h - 20, 24, 7, cloth);
        coat(&mut a, furnishing, [131, 104, 79]);
    } else if sheriff {
        rect(&mut a, furnishing, 15, h - 19, 26, 3, wood);
        rect(&mut a, furnishing, 17, h - 16, 3, 11, wood);
        rect(&mut a, furnishing, 36, h - 16, 3, 11, wood);
        boards(&mut a, furnishing, [122, 85, 51], false);
    } else {
        rect(&mut a, furnishing, 15, h - 46, 3, 41, wood);
        rect(&mut a, furnishing, 40, h - 46, 3, 41, wood);
        for y in [h - 44, h - 30, h - 16] {
            rect(&mut a, furnishing, 15, y, 28, 2, wood);
        }
        boards(&mut a, furnishing, [139, 100, 56], false);
    }
    if saloon {
        let p = part(&mut a, "upstairs_bed", 2);
        rect(&mut a, p, 18, 72, 27, 7, cloth);
        rect(&mut a, p, 16, 78, 31, 2, wood);
        rect(&mut a, p, 18, 80, 3, 3, wood);
        rect(&mut a, p, 42, 80, 3, 3, wood);
        coat(&mut a, p, [138, 91, 70]);
    }
    // Repaint frame mullions after all window details have been added.
    boards(&mut a, inside, [102, 79, 54], false);
    split_crest(&mut a, roof, sign);
    physical_boards(&mut a);
    a
}
// Painted seams become connected physical boards with local nail joints.
fn physical_boards(a: &mut Asset) {
    let old = std::mem::take(&mut a.parts);
    let joints = std::mem::take(&mut a.welds);
    let mut mapping = std::collections::BTreeMap::<String, Vec<usize>>::new();
    for p in old {
        let split = p.layer == 1 && p.name.contains("timber_front")
            || p.layer == 3
            || p.name.starts_with("roof_")
            || p.name == "porch_posts"
            || p.name == "interior_frame"
            || p.name == "boardwalk"
            || p.name == "porch_canopy"
            || p.name == "balcony_deck";
        let (tw, th) = if p.name == "porch_posts" || p.name == "interior_frame" {
            (32, 32)
        } else {
            (40, 6)
        };
        let mut seen = vec![false; a.w * a.h];
        let mut pieces = Vec::new();
        for start in 0..p.cells.len() {
            if seen[start] || p.cells[start] == 0 {
                continue;
            }
            let tile = (start % a.w / tw, start / a.w / th);
            let mut cells = Vec::new();
            let mut queue = vec![start];
            seen[start] = true;
            while let Some(i) = queue.pop() {
                cells.push(i);
                let (x, y) = (i % a.w, i / a.w);
                for n in [
                    x.checked_sub(1).map(|_| i - 1),
                    (x + 1 < a.w).then_some(i + 1),
                    y.checked_sub(1).map(|_| i - a.w),
                    (y + 1 < a.h).then_some(i + a.w),
                ]
                .into_iter()
                .flatten()
                {
                    if !seen[n]
                        && p.cells[n] != 0
                        && (!split || (n % a.w / tw, n / a.w / th) == tile)
                    {
                        seen[n] = true;
                        queue.push(n);
                    }
                }
            }
            let mut board = p.clone();
            board.cells.fill(0);
            board.paint.fill(0);
            board.name = if pieces.is_empty() {
                p.name.clone()
            } else {
                format!("{}_b{}", p.name, pieces.len())
            };
            for i in cells {
                board.cells[i] = p.cells[i];
                board.paint[i] = p.paint[i];
            }
            pieces.push(a.parts.len());
            a.parts.push(board);
        }
        mapping.insert(p.name, pieces);
    }
    // Preserve authored fittings, attaching them to the nearest actual board.
    for mut j in joints {
        let nearest = |name: &str| {
            *mapping[name]
                .iter()
                .min_by_key(|p| {
                    a.parts[**p]
                        .cells
                        .iter()
                        .enumerate()
                        .filter(|(_, m)| **m != 0)
                        .map(|(i, _)| {
                            let dx = i % a.w;
                            let dy = i / a.w;
                            ((dx as f32 - j.x).powi(2) + (dy as f32 - j.y).powi(2)) as u32
                        })
                        .min()
                        .unwrap_or(u32::MAX)
                })
                .unwrap()
        };
        let x = nearest(&j.a);
        let y = nearest(&j.b);
        j.a = a.parts[x].name.clone();
        j.b = a.parts[y].name.clone();
        a.welds.push(j);
    }
    // A nail joins neighboring boards, never an entire frontage to one remote anchor.
    let mut edges = std::collections::BTreeMap::<(usize, usize), (usize, usize, usize)>::new();
    for z in 0..a.layers.len() {
        let mut owner = vec![usize::MAX; a.w * a.h];
        for (p, b) in a.parts.iter().enumerate().filter(|(_, b)| b.layer == z) {
            for (i, m) in b.cells.iter().enumerate() {
                if *m != 0 {
                    owner[i] = p;
                }
            }
        }
        for y in 0..a.h {
            for x in 0..a.w {
                let i = y * a.w + x;
                let p = owner[i];
                if p == usize::MAX {
                    continue;
                }
                for n in [
                    (x + 1 < a.w).then_some(i + 1),
                    (y + 1 < a.h).then_some(i + a.w),
                ]
                .into_iter()
                .flatten()
                {
                    let q = owner[n];
                    if q == usize::MAX || p == q {
                        continue;
                    }
                    let e = edges.entry((p.min(q), p.max(q))).or_default();
                    e.0 += x;
                    e.1 += y;
                    e.2 += 1;
                }
            }
        }
    }
    for ((p, q), (x, y, n)) in edges {
        if a.welds.iter().any(|j| {
            (j.a == a.parts[p].name && j.b == a.parts[q].name)
                || (j.b == a.parts[p].name && j.a == a.parts[q].name)
        }) {
            continue;
        }
        weld(a, p, q, (x as f32 / n as f32, y as f32 / n as f32), 90.);
    }
    assert!(
        a.parts.len() <= grave_engine::asset::MAX_PARTS,
        "{} has {} boards",
        a.name,
        a.parts.len()
    );
}
fn split_crest(a: &mut Asset, roof: usize, sign: usize) {
    let mut seen = vec![false; a.w * a.h];
    let mut islands = Vec::new();
    for start in 0..seen.len() {
        if seen[start] || a.parts[roof].cells[start] == 0 {
            continue;
        }
        let mut cells = vec![start];
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            let (x, y) = (i % a.w, i / a.w);
            for (ok, j) in [
                (x > 0, i.wrapping_sub(1)),
                (x + 1 < a.w, i + 1),
                (y > 0, i.wrapping_sub(a.w)),
                (y + 1 < a.h, i + a.w),
            ] {
                if ok && !seen[j] && a.parts[roof].cells[j] != 0 {
                    seen[j] = true;
                    cells.push(j);
                    stack.push(j);
                }
            }
        }
        islands.push(cells);
    }
    islands.sort_by_key(|p| std::cmp::Reverse(p.len()));
    for (n, cells) in islands.into_iter().enumerate().skip(1) {
        let p = part(a, &format!("roof_crest_{n}"), 0);
        for i in cells {
            a.parts[p].cells[i] = a.parts[roof].cells[i];
            a.parts[p].paint[i] = a.parts[roof].paint[i];
            a.parts[roof].cells[i] = 0;
            a.parts[roof].paint[i] = 0;
        }
        weld(a, sign, p, (a.w as f32 / 2., 18.), 250.);
    }
}
fn duplicate_rear(a: &mut Asset) {
    let parts = a.parts.clone();
    let joints = a.welds.clone();
    a.layers.push("rear_support".into());
    for mut p in parts {
        p.name = format!("rear_{}", p.name);
        p.layer = 1;
        a.parts.push(p);
    }
    for mut j in joints {
        j.a = format!("rear_{}", j.a);
        j.b = format!("rear_{}", j.b);
        a.welds.push(j);
    }
}
fn props(m: &Materials) -> Vec<Asset> {
    let wood = m.id("weathered_wood").unwrap();
    let iron = m.id("iron").unwrap();
    let mut out = Vec::new();
    for explosive in [false, true] {
        let mut a = blank(
            if explosive {
                "west_powder_keg"
            } else {
                "west_barrel"
            },
            18,
            26,
            if explosive {
                &["barrel", "powder"]
            } else {
                &["barrel"]
            },
        );
        let p = part(&mut a, "oak_staves", 0);
        for y in 1..25 {
            let inset = if !(4..22).contains(&y) { 2 } else { 1 };
            rect(&mut a, p, inset, y, 18 - inset * 2, 1, wood);
        }
        boards(&mut a, p, [126, 88, 48], true);
        for y in [6, 19] {
            rect(&mut a, p, 2, y, 14, 2, iron);
        }
        a.paint_surface(p, Brush::Rectangle, (2, 6), (15, 7), Some([81, 85, 78]));
        a.paint_surface(p, Brush::Rectangle, (2, 19), (15, 20), Some([81, 85, 78]));
        if explosive {
            a.paint_surface(p, Brush::Rectangle, (1, 10), (16, 17), Some([141, 49, 33]));
            a.paint_text(p, (1, 11), "TNT", 1, [225, 196, 131]);
            let c = part(&mut a, "rear_keg_case", 1);
            rect(&mut a, c, 1, 1, 16, 24, wood);
            rect(&mut a, c, 3, 3, 12, 20, 0);
            let q = part(&mut a, "gunpowder_core", 1);
            rect(&mut a, q, 7, 16, 3, 6, m.id("gunpowder").unwrap());
            weld(&mut a, c, q, (3., 10.), 40.);
        }
        out.push(a);
    }
    let mut a = blank("west_hay_bale", 22, 15, &["front"]);
    let p = part(&mut a, "straw_and_twine", 0);
    rect(&mut a, p, 1, 1, 20, 13, m.id("straw").unwrap());
    boards(&mut a, p, [185, 151, 75], false);
    for x in [5, 15] {
        rect(&mut a, p, x, 1, 1, 13, m.id("cloth").unwrap());
        a.paint_surface(p, Brush::Line, (x, 1), (x, 13), Some([104, 86, 53]));
    }
    duplicate_rear(&mut a);
    out.push(a);
    let mut a = blank("west_hitching_rail", 32, 25, &["front"]);
    let p = part(&mut a, "timber_rail", 0);
    rect(&mut a, p, 1, 7, 30, 3, wood);
    for x in [2, 27] {
        rect(&mut a, p, x, 4, 3, 20, wood);
    }
    boards(&mut a, p, [110, 85, 54], true);
    out.push(a);
    let mut a = blank("west_wagon", 64, 40, &["front"]);
    let frame = part(&mut a, "wagon_chassis", 0);
    rect(&mut a, frame, 5, 24, 49, 4, wood);
    rect(&mut a, frame, 52, 24, 12, 2, wood);
    boards(&mut a, frame, [97, 68, 39], false);
    let bed = part(&mut a, "wagon_box", 0);
    rect(&mut a, bed, 8, 9, 43, 15, wood);
    rect(&mut a, bed, 11, 9, 37, 12, 0);
    boards(&mut a, bed, [145, 108, 63], false);
    weld(&mut a, frame, bed, (28., 24.), 400.);
    for (name, cx) in [("rear_wheel", 15), ("front_wheel", 46)] {
        let p = part(&mut a, name, 0);
        a.draw(p, Brush::Circle, (cx - 7, 25), (cx + 7, 39), iron);
        a.draw(p, Brush::Circle, (cx - 6, 26), (cx + 6, 38), wood);
        a.draw(p, Brush::Circle, (cx - 4, 28), (cx + 4, 36), 0);
        for (dx, dy) in [(0, 6), (6, 0), (4, 4), (-4, 4)] {
            for off in 0..2 {
                line(
                    &mut a,
                    p,
                    (cx - dx + off, 32 - dy),
                    (cx + dx + off, 32 + dy),
                    wood,
                );
            }
        }
        rect(&mut a, p, cx - 1, 31, 3, 3, iron);
        coat(&mut a, p, [133, 105, 66]);
        weld(&mut a, frame, p, (cx as f32, 27.), 100.);
    }
    duplicate_rear(&mut a);
    out.push(a);
    let mut a = blank("west_lantern", 8, 15, &["front"]);
    let p = part(&mut a, "iron_frame", 0);
    rect(&mut a, p, 2, 1, 4, 4, iron);
    rect(&mut a, p, 3, 2, 2, 2, 0);
    rect(&mut a, p, 1, 5, 6, 9, iron);
    rect(&mut a, p, 2, 6, 4, 6, 0);
    coat(&mut a, p, [74, 65, 46]);
    let q = part(&mut a, "lamp_glass", 0);
    rect(&mut a, q, 2, 6, 4, 6, m.id("lamp_glass").unwrap());
    coat(&mut a, q, [255, 207, 117]);
    weld(&mut a, p, q, (2., 6.), 40.);
    a.pivot = (4., 0.);
    out.push(a);
    let mut a = blank("west_water_trough", 24, 18, &["front"]);
    let p = part(&mut a, "wooden_trough", 0);
    rect(&mut a, p, 0, 3, 24, 12, wood);
    rect(&mut a, p, 3, 3, 18, 8, 0);
    rect(&mut a, p, 2, 15, 3, 2, wood);
    rect(&mut a, p, 19, 15, 3, 2, wood);
    boards(&mut a, p, [99, 86, 62], false);
    out.push(a);
    let mut a = blank("west_water_tower", 48, 88, &["front", "tank_inside"]);
    let frame = part(&mut a, "tower_legs", 0);
    for x in [5, 39] {
        rect(&mut a, frame, x, 25, 4, 62, wood);
    }
    for y in [27, 53, 78] {
        rect(&mut a, frame, 5, y, 38, 3, wood);
    }
    for off in 0..2 {
        line(&mut a, frame, (9 + off, 30), (39 + off, 78), wood);
        line(&mut a, frame, (9 + off, 78), (39 + off, 30), wood);
    }
    boards(&mut a, frame, [102, 74, 45], true);
    let tank = part(&mut a, "tank_staves", 0);
    rect(&mut a, tank, 3, 4, 42, 22, wood);
    boards(&mut a, tank, [133, 107, 68], true);
    for y in [7, 22] {
        rect(&mut a, tank, 4, y, 40, 2, iron);
        a.paint_surface(
            tank,
            Brush::Rectangle,
            (4, y),
            (43, y + 1),
            Some([76, 78, 68]),
        );
    }
    a.paint_text(tank, (10, 12), "WATER", 1, [221, 203, 148]);
    weld(&mut a, frame, tank, (24., 26.), 800.);
    let roof = part(&mut a, "tank_roof", 0);
    gable(&mut a, roof, 0, 47, 4, 0, wood);
    boards(&mut a, roof, [91, 65, 43], false);
    weld(&mut a, tank, roof, (24., 4.), 400.);
    let inside = part(&mut a, "rear_legs_and_tank", 1);
    for x in [5, 39] {
        rect(&mut a, inside, x, 24, 4, 63, wood);
    }
    rect(&mut a, inside, 3, 4, 3, 23, wood);
    rect(&mut a, inside, 42, 4, 3, 23, wood);
    rect(&mut a, inside, 3, 24, 42, 3, wood);
    rect(&mut a, inside, 5, 78, 38, 3, wood);
    boards(&mut a, inside, [81, 70, 49], true);
    out.push(a);
    let mut a = blank("west_dry_tree", 36, 52, &["front"]);
    let p = part(&mut a, "dry_branches", 0);
    rect(&mut a, p, 17, 21, 3, 30, wood);
    for (x, y) in [(4, 5), (10, 15), (28, 2), (32, 18), (4, 27)] {
        for off in 0..2 {
            line(&mut a, p, (18 + off, 34), (x + off, y), wood);
        }
    }
    boards(&mut a, p, [101, 79, 51], true);
    out.push(a);
    out
}
fn save(a: &Asset, m: &Materials) {
    let p = a.validate(m);
    assert!(p.is_empty(), "{}: {p:?}", a.name);
    a.save(
        &Path::new("assets/workshop").join(format!("{}.asset", a.name)),
        m,
    )
    .unwrap();
}
fn main() {
    let mut rgba = Vec::new();
    for y in 0..16 {
        for x in 0..16 {
            let grain = ((x * 13 + y * 7) % 5) as i32 - 2;
            for c in [122, 74, 38] {
                rgba.push((c + grain) as u8);
            }
            rgba.push(255);
        }
    }
    headless::write_png(
        Path::new("materials/textures/weathered_wood.png"),
        16,
        16,
        &rgba,
        1,
    )
    .unwrap();
    let m = Materials::load_dir("materials");
    assert!(m.problems.is_empty(), "{:?}", m.problems);
    for kind in ["ranch", "store", "saloon", "sheriff"] {
        let a = building(&m, kind);
        println!("{}: {} parts", a.name, a.parts.len());
        save(&a, &m);
    }
    for a in props(&m) {
        save(&a, &m);
    }
    let mut s = String::from(
        "# Copper Creek: frontier timber town, no fire is pre-lit.\nsize 830 260\nseed 73\noutside_temperature 24\nfill air\nwind 0.15\nturbulence 0.7\nsky #31465e #d39b64\nlayer mountains #72646b depth 0.15 y 196 height 72\nlayer hills #938268 depth 0.35 y 208 height 33\nlighting 0.72 #ffe3bb\nsun 38 0.32 #ffddaa\nrect dirt 0 202 830 58\nanchor 0 202 830 58\n",
    );
    for y in 202..260 {
        for x in 0..830 {
            let base = if y < 206 {
                [153i32, 125, 81]
            } else {
                [135, 106, 70]
            };
            let k = ((x * 13 + y * 29) % 13) as i32 - 6;
            let c = base.map(|v| (v + k).clamp(0, 255));
            s += &format!("paint {x} {y} #{:02x}{:02x}{:02x}\n", c[0], c[1], c[2]);
        }
    }
    for (name, x, alias) in [
        ("west_ranch", 90, "ranch"),
        ("west_store", 256, "store"),
        ("west_saloon", 430, "saloon"),
        ("west_sheriff", 600, "sheriff"),
        ("west_water_tower", 786, "tower"),
        ("west_wagon", 710, "wagon"),
        ("west_hitching_rail", 177, "hitch"),
        ("west_water_trough", 14, "trough"),
        ("west_dry_tree", 819, "tree"),
    ] {
        s += &format!("asset ../assets/workshop/{name}.asset {x} 202 as {alias}\n");
    }
    s += "weld tree.dry_branches ground at 819 201 break 180\n";
    // Separate alley props fit between the physical porch outlines.
    for (name, x, y, alias) in [
        ("west_hay_bale", 335, 202, "hay1"),
        ("west_powder_keg", 335, 188, "keg1"),
        ("west_barrel", 354, 202, "barrel1"),
        ("west_barrel", 536, 202, "barrel2"),
        ("west_hay_bale", 663, 202, "hay2"),
        ("west_powder_keg", 710, 185, "wagon_charge"),
    ] {
        s += &format!("asset ../assets/workshop/{name}.asset {x} {y} as {alias}\n");
    }
    for (x, y, alias, parent) in [
        (46, 158, "lantern_ranch", "ranch"),
        (213, 160, "lantern_store", "store"),
        (378, 160, "lantern_saloon", "saloon"),
        (640, 160, "lantern_law", "sheriff"),
    ] {
        s += &format!(
            "asset ../assets/workshop/west_lantern.asset {x} {y} as {alias}\nweld {alias}.iron_frame {parent}.porch_posts at {x} {y} break 50\nlight {x} {} 46 strength 0.9 color #ffd18b unshadowed on {alias}.lamp_glass\n",
            y + 9
        );
    }
    s += "rect water 5 191 18 6\nrect oil 333 200 7 1\n";
    // Window glow comes from rooms, not electric street fixtures.
    s += "plane 1 timber_front\nsize 830 260\nfill air\nrect dirt 0 202 830 58\nanchor 0 202 830 58\nlighting 0.68 #ffe1b0\nrect water 768 121 36 17\nendplane\nplane 2 interior\nsize 830 260\nfill air\nrect dirt 0 202 830 58\nanchor 0 202 830 58\nlighting 0.45 #d1ae7b\n";
    for x in [90, 256, 430, 600] {
        s += &format!("light {x} 177 45 strength 0.65 color #ffd08b unshadowed\n");
    }
    s += "light 430 143 45 strength 0.5 color #ffbf78 unshadowed\nendplane\nplane 3 backboards\nsize 830 260\nfill air\nrect dirt 0 202 830 58\nanchor 0 202 830 58\nlighting 0.9 #e8e6df\nendplane\n";
    std::fs::write("scenes/copper-creek.txt", s).unwrap();
    if std::env::args().any(|a| a == "--verify") {
        verify(&m);
    }
}
fn verify(m: &Materials) {
    let text = std::fs::read_to_string("scenes/copper-creek.txt").unwrap();
    let (mut sim, p) = scene::load(&text, m, Some(Path::new("scenes"))).into_sim(m);
    assert!(p.is_empty(), "{p:?}");
    let counts = |s: &grave_engine::Sim| -> BTreeMap<String, usize> {
        let mut out = BTreeMap::new();
        for z in 0..=s.layers.len() {
            let p = grave_engine::assemblies::plane(s, z);
            for id in p.physics.ids() {
                if let Some(g) = p.physics.assembly(id) {
                    *out.entry(format!("{z}:{g}")).or_default() += 1;
                }
            }
        }
        out
    };
    let initial = counts(&sim);
    let mut r = Renderer::default();
    r.light(&sim);
    headless::shot(
        &sim.world,
        m,
        &mut r,
        Path::new("shots/copper-creek-initial.png"),
        2,
        false,
    )
    .unwrap();
    for _ in 0..180 {
        sim.step(m);
    }
    r.light(&sim);
    headless::shot(
        &sim.world,
        m,
        &mut r,
        Path::new("shots/copper-creek-standing.png"),
        2,
        false,
    )
    .unwrap();
    let after = counts(&sim);
    for (g, n) in &initial {
        assert_eq!(after.get(g), Some(n), "unlit asset changed: {g}");
    }
    let count = |s: &grave_engine::Sim, mat| {
        std::iter::once(&s.world)
            .chain(s.layers.iter().map(|l| &l.world))
            .map(|w| w.mat.iter().filter(|v| **v == mat).count())
            .sum::<usize>()
    };
    let wood = count(&sim, m.id("weathered_wood").unwrap());
    let powder = count(&sim, m.id("gunpowder").unwrap());
    let mut inside = false;
    let roof = sim
        .physics
        .ids()
        .into_iter()
        .find(|id| {
            sim.physics.assembly(*id) == Some("saloon")
                && sim.physics.spec_of(&sim.world, *id).is_some_and(|b| {
                    let cells: Vec<_> = b
                        .cells
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| c.is_some())
                        .collect();
                    let avg = cells.iter().map(|(i, _)| (*i / b.w) as f32).sum::<f32>()
                        / cells.len() as f32;
                    b.y - b.h as f32 / 2. + avg < 125.
                })
        })
        .unwrap();
    let roof_y = sim.physics.centre(roof).unwrap().1;
    for f in 0..2000 {
        if f < 120 && f % 15 == 0 {
            sim.world.heat_circle(335, 184, 20, 950.);
        }
        sim.step(m);
        inside |= sim.layers[1]
            .world
            .flags
            .iter()
            .any(|v| v & grave_engine::world::BURNING != 0);
        if f == 180 {
            r.light(&sim);
            headless::shot(
                &sim.world,
                m,
                &mut r,
                Path::new("shots/copper-creek-fire.png"),
                2,
                false,
            )
            .unwrap();
        }
    }
    r.light(&sim);
    headless::shot(
        &sim.world,
        m,
        &mut r,
        Path::new("shots/copper-creek-aftermath.png"),
        2,
        false,
    )
    .unwrap();
    let wood_after = count(&sim, m.id("weathered_wood").unwrap());
    let powder_after = count(&sim, m.id("gunpowder").unwrap());
    println!(
        "wood {wood}->{wood_after}; powder {powder}->{powder_after}; interior fire {inside}; roof {roof_y}->{:?}",
        sim.physics.centre(roof)
    );
    assert!(
        wood_after < wood,
        "the fire must consume real construction timber"
    );
    assert!(
        wood_after > wood / 2,
        "one localized ignition must not erase the town"
    );
    assert!(powder_after < powder);
    assert!(inside);
}
