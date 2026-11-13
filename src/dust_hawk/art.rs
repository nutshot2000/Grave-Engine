//! Shared original helicopter artwork.
use grave_engine::overhead::{Point, view::VisualCell};
pub fn helicopter(frame: u32) -> Vec<VisualCell> {
    let mut v = Vec::new();
    let mut add = |x: f32, y: f32, z: f32, c: [u8; 3]| {
        v.push(VisualCell {
            at: Point::new(x, y, z),
            color: c,
        })
    };
    for x in -7i32..=7 {
        for y in -3i32..=3 {
            for z in 0..4 {
                if (x as f32 / 8.).powi(2)
                    + (y as f32 / 4.).powi(2)
                    + ((z as f32 - 1.5) / 2.8).powi(2)
                    < 1.2
                {
                    let color = if x > 2 && z >= 2 {
                        [65, 134, 153]
                    } else if z == 3 {
                        [117, 132, 78]
                    } else {
                        [68, 85, 49]
                    };
                    add(x as f32, y as f32, z as f32, color);
                }
            }
        }
    }
    for x in -18..-6 {
        add(x as f32, 0., 2., [87, 105, 59]);
        add(x as f32, 0., 3., [105, 120, 67]);
    }
    for z in 2..8 {
        add(-17., 0., z as f32, [110, 124, 70]);
    }
    for side in [-4., 4.] {
        for x in -5..6 {
            add(x as f32, side, -1., [55, 61, 62]);
        }
        for x in [-4., 4.] {
            add(x, side, 0., [85, 91, 84]);
        }
    }
    for z in 4..7 {
        add(0., 0., z as f32, [55, 60, 56]);
    }
    let a = frame as f32 * 0.73;
    let (s, c) = a.sin_cos();
    for n in -15..16 {
        let f = n as f32;
        add(f * c, f * s, 7., [90, 94, 86]);
        add(-f * s, f * c, 7., [118, 120, 106]);
    }
    add(0., 0., 7., [174, 174, 139]);
    v
}
