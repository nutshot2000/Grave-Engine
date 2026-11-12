//! Parametric cell-built buildings, shared by the visual builder and AI callers.
use crate::{
    Materials,
    asset::{Asset, Brush, Weld},
};
#[derive(Clone)]
pub struct Building {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub storeys: usize,
    pub facade: bool,
    pub timber: bool,
}
impl Default for Building {
    fn default() -> Self {
        Self {
            name: "my_building".into(),
            width: 96,
            height: 110,
            storeys: 2,
            facade: false,
            timber: true,
        }
    }
}
impl Building {
    pub fn build(&self, m: &Materials) -> Result<Asset, String> {
        if !(48..=128).contains(&self.width)
            || !(64..=128).contains(&self.height)
            || !(1..=3).contains(&self.storeys)
        {
            return Err("Use width 48–128, height 64–128 and 1–3 storeys".into());
        }
        if self.height < 29 + self.storeys * 24 {
            return Err(
                "Increase the height: each storey needs at least 24 cells below the roof".into(),
            );
        }
        let mut a = Asset::blank(&self.name, self.width, self.height)?;
        a.depth = 0.06;
        a.pivot = (self.width as f32 / 2., self.height as f32 - 1.);
        a.rename_part(0, "structure")?;
        let id = |name| m.id(name).ok_or_else(|| format!("Missing material {name}"));
        let (wood, wall, glass, stone) = (
            id("wood")?,
            id(if self.timber { "wood" } else { "brick" })?,
            id("glass")?,
            id("stone")?,
        );
        let (w, h) = (self.width as i32, self.height as i32);
        let top = 24;
        let bottom = h - 5;
        let floor_h = (bottom - top) / self.storeys as i32;
        let rect = |a: &mut Asset, p, x, y, ww, hh, mat| {
            a.draw(p, Brush::Rectangle, (x, y), (x + ww - 1, y + hh - 1), mat)
        };
        if self.facade {
            rect(&mut a, 0, 4, top, w - 8, bottom - top, wall);
        } else {
            for x in [4, w / 2 - 2, w - 8] {
                rect(&mut a, 0, x, top, 4, bottom - top, wall);
            }
        }
        for floor in 0..=self.storeys as i32 {
            rect(&mut a, 0, 4, top + floor * floor_h, w - 8, 3, wood);
        }
        for floor in 0..self.storeys as i32 {
            let y = top + floor * floor_h + 5;
            let pane_h = (floor_h - 11).clamp(8, 20);
            for (side, x) in [("left", 10), ("right", w / 2 + 8)] {
                let pane_w = w / 2 - 20;
                rect(&mut a, 0, 4, y - 2, w - 8, 2, wood);
                rect(&mut a, 0, x - 2, y - 2, pane_w + 4, pane_h + 4, wood);
                rect(&mut a, 0, x, y, pane_w, pane_h, 0);
                let p = a.add_part(&format!("window_{floor}_{side}"))?;
                rect(&mut a, p, x, y, pane_w, pane_h, glass);
                a.paint_surface(
                    p,
                    Brush::Rectangle,
                    (x, y),
                    (x + pane_w - 1, y + pane_h - 1),
                    Some([115, 175, 187]),
                );
                a.welds.push(Weld {
                    a: "structure".into(),
                    b: a.parts[p].name.clone(),
                    x: x as f32,
                    y: y as f32,
                    break_force: Some(100.),
                });
            }
            let y2 = top + (floor + 1) * floor_h - 14;
            rect(&mut a, 0, w / 2 - 1, y2, 3, 14, 0);
            let p = a.add_part(&format!("door_{floor}"))?;
            rect(&mut a, p, w / 2 - 1, y2, 3, 14, wood);
            a.paint_surface(
                p,
                Brush::Rectangle,
                (w / 2 - 1, y2),
                (w / 2 + 1, y2 + 13),
                Some([73, 112, 91]),
            );
            a.welds.push(Weld {
                a: "structure".into(),
                b: a.parts[p].name.clone(),
                x: w as f32 / 2.,
                y: y2 as f32,
                break_force: Some(150.),
            });
        }
        let roof = a.add_part("roof")?;
        for (x0, y0, x1, y1) in [(1, 24, w / 2, 2), (w / 2, 2, w - 2, 24)] {
            for off in 0..3 {
                a.draw(roof, Brush::Line, (x0, y0 + off), (x1, y1 + off), wood);
            }
        }
        a.paint_surface(
            roof,
            Brush::Rectangle,
            (0, 0),
            (w - 1, 27),
            Some([115, 70, 50]),
        );
        for x in [6, w - 7] {
            a.welds.push(Weld {
                a: "structure".into(),
                b: "roof".into(),
                x: x as f32,
                y: 25.,
                break_force: Some(1200.),
            });
        }
        let base = a.add_part("foundation")?;
        rect(&mut a, base, 3, h - 5, w - 6, 4, stone);
        a.welds.push(Weld {
            a: "structure".into(),
            b: "foundation".into(),
            x: w as f32 / 2.,
            y: (h - 5) as f32,
            break_force: Some(2400.),
        });
        // Each physical cell belongs to one part; openings must be real gaps.
        for p in 1..a.parts.len() {
            for i in 0..a.w * a.h {
                if a.parts[p].cells[i] != 0 {
                    a.parts[0].cells[i] = 0;
                    a.parts[0].paint[i] = 0;
                }
            }
        }
        if self.facade {
            let mut interior = Self {
                facade: false,
                ..self.clone()
            }
            .build(m)?;
            a.layers.push("interior".into());
            for p in &mut interior.parts {
                p.name = format!("inside_{}", p.name);
                p.layer = 1;
            }
            for j in &mut interior.welds {
                j.a = format!("inside_{}", j.a);
                j.b = format!("inside_{}", j.b);
            }
            a.parts.extend(interior.parts);
            a.welds.extend(interior.welds);
        }
        Ok(a)
    }
}
