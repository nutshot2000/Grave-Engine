//! Shared cave game presentation: readable HUD, hand-mounted equipment and targeting.
use crate::{
    Materials, Sim,
    supplies::{Catalog, Mode},
    tool_pose,
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Vec2};
const INK: Color32 = Color32::from_rgb(222, 237, 240);
const MUTED: Color32 = Color32::from_rgb(147, 169, 182);
const CYAN: Color32 = Color32::from_rgb(92, 235, 229);
const GOLD: Color32 = Color32::from_rgb(255, 201, 113);
fn label(p: &egui::Painter, at: Pos2, text: impl ToString, size: f32, col: Color32) {
    p.text(
        at,
        Align2::LEFT_TOP,
        text.to_string(),
        FontId::proportional(size),
        col,
    );
}
fn card(p: &egui::Painter, r: Rect, col: Color32) {
    p.rect_filled(r, 8., Color32::from_rgba_unmultiplied(8, 18, 27, 235));
    p.line_segment(
        [
            r.left_top() + Vec2::new(1., 8.),
            r.left_bottom() - Vec2::new(-1., 8.),
        ],
        (3., col),
    );
}
fn meter(p: &egui::Painter, at: Pos2, w: f32, v: f32, col: Color32) {
    let r = Rect::from_min_size(at, Vec2::new(w, 5.));
    p.rect_filled(r, 2., Color32::from_rgb(37, 52, 63));
    let mut f = r;
    f.set_width(w * v.clamp(0., 1.));
    p.rect_filled(f, 2., col);
}
pub fn hud(p: &egui::Painter, r: Rect, s: &Sim, m: &Materials, c: &Catalog, pad: bool) {
    let Some(a) = s.physics.player_status() else {
        return;
    };
    let compact = r.width() < 760.;
    let margin = 14.;
    let left = r.min + Vec2::splat(margin);
    let width = if compact { 215. } else { 246. };
    let status = if a.dead {
        "SIGNAL LOST"
    } else if a.health < 30. {
        "SUIT CRITICAL"
    } else {
        "DIVER / VITALS"
    };
    card(
        p,
        Rect::from_min_size(left, Vec2::new(width, 112.)),
        if a.health < 30. {
            Color32::LIGHT_RED
        } else {
            CYAN
        },
    );
    label(p, left + Vec2::new(14., 10.), status, 11., MUTED);
    label(
        p,
        left + Vec2::new(14., 29.),
        format!("{:.0}", a.health),
        31.,
        INK,
    );
    label(p, left + Vec2::new(72., 42.), "HEALTH", 11., MUTED);
    meter(
        p,
        left + Vec2::new(14., 66.),
        width - 28.,
        a.health / 100.,
        Color32::from_rgb(237, 122, 105),
    );
    let rope = a.rope.as_ref();
    let traversal = if let Some(ro) = rope {
        if ro.pins.is_empty() {
            "HOLD RIGHT MOUSE · rope".into()
        } else {
            format!("ROPE ATTACHED · {:.1} m", ro.length / 10.)
        }
    } else if let Some((f, cap)) = a.jet_fuel {
        format!("JET FUEL  {:.0}%", f / cap * 100.)
    } else {
        "ON FOOT · W jump / climb".into()
    };
    label(p, left + Vec2::new(14., 83.), traversal, 11., CYAN);
    if let Some(rope) = &a.rope {
        let width = (r.width() - 28.).min(440.);
        let at =
            r.center_bottom() - Vec2::new(width * 0.5, if r.width() < 1050. { 178. } else { 99. });
        card(p, Rect::from_min_size(at, Vec2::new(width, 58.)), GOLD);
        label(
            p,
            at + Vec2::new(14., 10.),
            if pad {
                "HOLD RIGHT MOUSE / LT: rope · release: let go"
            } else {
                "HOLD RIGHT MOUSE: rope · Release: let go"
            },
            14.,
            GOLD,
        );
        label(
            p,
            at + Vec2::new(14., 35.),
            if pad {
                "Wheel / D-pad up/down: pull up or lower"
            } else if rope.pins.is_empty() {
                "Aim at rock above you. Hold the button to attach."
            } else {
                "SCROLL UP: pull up · SCROLL DOWN: lower"
            },
            13.,
            INK,
        );
    }
    if let Some(mission) = &s.mission {
        let at = if compact {
            left + Vec2::new(0., 122.)
        } else {
            r.right_top() + Vec2::new(-310. - margin, margin)
        };
        let w = if compact { width } else { 310. };
        card(
            p,
            Rect::from_min_size(at, Vec2::new(w, 98. + mission.goals.len() as f32 * 29.)),
            GOLD,
        );
        label(
            p,
            at + Vec2::new(14., 10.),
            "EXPEDITION / OBJECTIVE",
            10.,
            MUTED,
        );
        label(
            p,
            at + Vec2::new(14., 28.),
            if mission.complete {
                "Samples delivered"
            } else if mission.ready() {
                "Return to camp"
            } else {
                "Recover crystal samples"
            },
            17.,
            INK,
        );
        for (i, g) in mission.goals.iter().enumerate() {
            let y = 59. + i as f32 * 29.;
            let mat = m.get(g.material);
            let col = Color32::from_rgb(mat.color[0], mat.color[1], mat.color[2]);
            label(
                p,
                at + Vec2::new(14., y - 6.),
                format!(
                    "{}   {} / {}",
                    mat.name.replace("crystal_", "").to_uppercase(),
                    g.collected,
                    g.target
                ),
                11.,
                col,
            );
            meter(
                p,
                at + Vec2::new(14., y + 11.),
                w - 28.,
                g.collected as f32 / g.target.max(1) as f32,
                col,
            );
        }
        if let Some((x, y)) = mission.signal(&s.world, (a.x, a.y)) {
            let d = (x - a.x, y - a.y);
            let arrow = if d.0.abs() > d.1.abs() {
                if d.0 > 0. { "RIGHT" } else { "LEFT" }
            } else if d.1 > 0. {
                "BELOW"
            } else {
                "ABOVE"
            };
            label(
                p,
                at + Vec2::new(14., 70. + mission.goals.len() as f32 * 29.),
                format!("SIGNAL  {:.0} m · {arrow}", d.0.hypot(d.1) / 10.),
                12.,
                GOLD,
            );
        }
    }
    let base = r.left_bottom() + Vec2::new(margin, -128.);
    let tools: Vec<_> = c
        .tools
        .iter()
        .filter(|t| s.supplies.tools.contains(&t.id))
        .collect();
    let tw = if compact { 230. } else { 300. };
    card(p, Rect::from_min_size(base, Vec2::new(tw, 87.)), CYAN);
    if let Some(t) = c.tool(&s.supplies.equipped) {
        label(
            p,
            base + Vec2::new(14., 10.),
            if pad {
                "EQUIPPED  /  Q / LB RB change"
            } else {
                "EQUIPPED  /  Q change"
            },
            10.,
            MUTED,
        );
        label(p, base + Vec2::new(14., 28.), &t.title, 20., INK);
        let ammo = match t.mode {
            Mode::Drill(_) => "ROCK CUTTER · no ammunition".into(),
            Mode::Spray { ammo, .. } => {
                if s.supplies.count(ammo) == 0 {
                    "EMPTY · craft ammunition at a workshop".into()
                } else {
                    format!(
                        "{}   {}",
                        s.supplies.count(ammo),
                        m.get(ammo).name.replace('_', " ").to_uppercase()
                    )
                }
            }
            Mode::Vacuum { capacity, .. } => {
                format!("TANK   {} / {} cells", s.supplies.gas_total(), capacity)
            }
        };
        label(p, base + Vec2::new(14., 59.), ammo, 12., CYAN);
    }
    for (i, t) in tools.iter().enumerate() {
        let at = base + Vec2::new(i as f32 * 17. + 14., -12.);
        p.circle_filled(
            at,
            3.,
            if s.supplies.equipped == t.id {
                CYAN
            } else {
                MUTED.gamma_multiply(0.35)
            },
        );
    }
    let stock = r.right_bottom() + Vec2::new(-228. - margin, -128.);
    if !compact {
        card(p, Rect::from_min_size(stock, Vec2::new(228., 87.)), GOLD);
        label(
            p,
            stock + Vec2::new(14., 10.),
            "CARGO / MATERIAL CELLS",
            10.,
            MUTED,
        );
        label(p, stock + Vec2::new(14., 29.), s.supplies.total(), 25., INK);
        label(
            p,
            stock + Vec2::new(14., 62.),
            if pad {
                "B / Y  Inventory & workshops"
            } else {
                "B  Inventory & workshops"
            },
            12.,
            GOLD,
        );
    }
    let mut alerts = Vec::new();
    if a.breath < 8. {
        alerts.push(format!(
            "AIR  {:.0}% — find an air pocket",
            a.breath / 10. * 100.
        ));
    }
    if let Some(soak) = a.soak {
        alerts.push(if soak.burning {
            "ON FIRE — water puts it out".into()
        } else {
            format!("Coated in {}", m.get(soak.liquid).name.replace('_', " "))
        });
    }
    if a.hanging {
        alerts.push("HANGING — release W, then W to climb; S to drop".into());
    }
    if a.dead {
        alerts.push("DIVER DOWN — returning to camp".into());
    }
    for (i, t) in alerts.iter().enumerate() {
        let at = r.center_bottom() - Vec2::new(0., 155. + i as f32 * 26.);
        p.rect_filled(
            Rect::from_center_size(at, Vec2::new(380., 24.)),
            5.,
            Color32::from_black_alpha(220),
        );
        p.text(
            at,
            Align2::CENTER_CENTER,
            t,
            FontId::proportional(13.),
            GOLD,
        );
    }
    let help = if pad {
        "LS Move   A Jump   X Crouch   B Roll   RT Tool   Hold LT Rope   LB/RB Equip   Y Cargo"
    } else {
        "A/D Move   SPACE Jump   S Crouch   SHIFT Roll   LEFT MOUSE Tool   HOLD RIGHT MOUSE Rope   Q Equip   B Cargo"
    };
    let footer = Rect::from_min_max(r.left_bottom() + Vec2::new(0., -29.), r.right_bottom());
    p.rect_filled(footer, 0., Color32::from_rgba_unmultiplied(7, 15, 23, 235));
    p.text(
        footer.center(),
        Align2::CENTER_CENTER,
        help,
        FontId::proportional(if compact { 10. } else { 12. }),
        MUTED,
    );
}

pub fn equipment(
    p: &egui::Painter,
    s: &Sim,
    m: &Materials,
    c: &Catalog,
    aim: (f32, f32),
    active: bool,
    scale: f32,
    point: impl Fn((f32, f32)) -> Pos2,
) {
    let Some(a) = s.physics.player_status().filter(|a| !a.dead) else {
        return;
    };
    let pose = tool_pose::pose(&a, aim);
    if let Some(rope) = &a.rope {
        if rope.pins.is_empty() {
            let from = tool_pose::shoulder(&a);
            let hit = crate::physics::grapple_target(&s.world, m, from, aim, rope.range);
            if let Some(hook) = hit {
                let actual = point(hook);
                let clip = p.clip_rect().shrink(24.);
                let shown = Pos2::new(
                    actual.x.clamp(clip.left(), clip.right()),
                    actual.y.clamp(clip.top(), clip.bottom()),
                );
                // A quiet ring where the hook would catch: no line or text
                // over the aim (the footer says how to use the rope).
                let alpha = if clip.contains(actual) { 0.75 } else { 0.4 };
                p.circle_stroke(shown, 4.5, (3., Color32::from_black_alpha(140)));
                p.circle_stroke(shown, 4.5, (1.3, GOLD.gamma_multiply(alpha)));
            }
        }
        if let Some(last) = rope.pins.last() {
            let mut path = rope.pins.clone();
            path.push(pose.hand);
            let fixed: f32 = path
                .windows(2)
                .map(|p| (p[0].0 - p[1].0).hypot(p[0].1 - p[1].1))
                .sum();
            let sag = (rope.length - fixed).max(0.).min(24.);
            for (i, pair) in path.windows(2).enumerate() {
                let mut prev = point(pair[0]);
                for k in 1..=18 {
                    let t = k as f32 / 18.;
                    let v = (
                        pair[0].0 + (pair[1].0 - pair[0].0) * t,
                        pair[0].1
                            + (pair[1].1 - pair[0].1) * t
                            + if i == path.len() - 2 {
                                4. * t * (1. - t) * sag * 0.45
                            } else {
                                0.
                            },
                    );
                    let next = point(v);
                    p.line_segment([prev, next], (3., Color32::from_black_alpha(200)));
                    p.line_segment([prev, next], (1.5, GOLD));
                    prev = next;
                }
            }
            p.circle_filled(point(rope.pins[0]), 3., INK);
            let _ = last;
        }
    }
    if a.hanging || a.climbing || a.rolling {
        return;
    }
    let Some(t) = c.tool(&s.supplies.equipped) else {
        return;
    };
    let col = match t.mode {
        Mode::Drill(_) => CYAN,
        Mode::Spray { material, .. } => {
            let c = m.get(material).color;
            Color32::from_rgb(c[0], c[1], c[2])
        }
        Mode::Vacuum { .. } => Color32::from_rgb(157, 195, 255),
    };
    let hand = point(pose.hand);
    let end = point(pose.muzzle);
    // One hand anchor drives both the held tool and the simulated emitter.
    p.line_segment([hand, end], (2.6 * scale, Color32::from_rgb(15, 23, 29)));
    p.line_segment([hand, end], (1.7 * scale, Color32::from_rgb(113, 133, 142)));
    p.line_segment(
        [hand.lerp(end, 0.42), hand.lerp(end, 0.65)],
        (1.9 * scale, col),
    );
    p.circle_filled(hand, 1.05 * scale, Color32::from_rgb(182, 167, 146));
    let drill = if let Mode::Drill(d) = t.mode {
        Some(d)
    } else {
        None
    };
    let reach = drill.map_or(35., |d| d.reach);
    let origin = if drill.is_some() { tool_pose::shoulder(&a) } else { pose.origin };
    let body = drill.map(|_| s.physics.player_cells());
    let hit = crate::mining::surface_from(&s.world, m, origin, aim, reach, body);
    let target = hit
        .map(|i| ((i % s.world.w) as f32 + 0.5, (i / s.world.w) as f32 + 0.5))
        .unwrap_or((
            origin.0 + pose.aim.0 * reach,
            origin.1 + pose.aim.1 * reach,
        ));
    let center = point(target);
    let blocked = hit
        .zip(drill)
        .is_some_and(|(i, d)| crate::mining::hardness(m.get(s.world.mat[i])) > d.grade);
    let target_col = if blocked {
        Color32::from_rgb(255, 113, 105)
    } else {
        col
    };
    if let Some((i, d)) = hit.zip(drill) {
        for (j, _) in crate::mining::candidates_from(&s.world, m, origin, i, d, body) {
            let at = point(((j % s.world.w) as f32, (j / s.world.w) as f32));
            p.rect_filled(
                Rect::from_min_size(at, Vec2::splat(scale)),
                0.,
                target_col.gamma_multiply(if active { 0.65 } else { 0.36 }),
            );
        }
    }
    // Small cross only: the terrain tint already shows the actual mining footprint.
    for dir in [Vec2::X, Vec2::Y] {
        p.line_segment(
            [center - dir * 3., center + dir * 3.],
            (3., Color32::from_black_alpha(220)),
        );
        p.line_segment([center - dir * 3., center + dir * 3.], (1., target_col));
    }
    // Readouts sit at the bottom of the view, never over the crosshair.
    let readout = p.clip_rect().center_bottom() - Vec2::new(0., 64.);
    if active && drill.is_some() && hit.is_none() {
        p.text(readout, Align2::CENTER_BOTTOM, "No reachable rock", FontId::proportional(12.), INK);
    }
    if let Some((i, _)) = hit.zip(drill).filter(|_| active || blocked) {
        let mat = m.get(s.world.mat[i]);
        let h = crate::mining::hardness(mat);
        let progress = s.mining.fractures.get(&i).map_or(0., |f| f.work / h);
        let text = format!(
            "{}  /  {}",
            mat.name.replace('_', " ").to_uppercase(),
            if blocked {
                "UPGRADE DRILL".into()
            } else {
                format!("{:.0}%", (progress * 100.).min(99.))
            }
        );
        let galley = p.layout_no_wrap(text, FontId::proportional(11.), INK);
        let at = readout - Vec2::new(galley.size().x / 2., galley.size().y);
        p.rect_filled(
            Rect::from_min_size(at - Vec2::splat(5.), galley.size() + Vec2::splat(10.)),
            4.,
            Color32::from_black_alpha(230),
        );
        p.galley(at, galley, target_col);
    }
    if active && matches!(t.mode, Mode::Vacuum { .. }) {
        for i in 0..14 {
            let d = ((s.world.frame as f32 * 0.7 + i as f32 * 2.7) % 30.) + 4.;
            let cross = (d * 1.3 + i as f32).sin() * (d * 0.09);
            let q = (
                pose.muzzle.0 + pose.aim.0 * d - pose.aim.1 * cross,
                pose.muzzle.1 + pose.aim.1 * d + pose.aim.0 * cross,
            );
            p.circle_filled(point(q), scale * 0.22, col.gamma_multiply(0.65));
        }
    }
    if active {
        if drill.is_some() {
            cutting_beam(p, s, m, end, center, hit, target_col, blocked, scale);
        } else {
            p.circle_filled(end, 1.8 * scale, target_col);
        }
    }
}

/// A small pseudo-random number in 0..1 (stable per frame and index).
fn jitter(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B9) ^ b.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.
}

/// The drill at work: a flickering beam with a hot core, a glow where it
/// bites, and sparks of the rock's own colour flying back off the face.
/// Too-hard rock makes it sputter red.
#[allow(clippy::too_many_arguments)]
fn cutting_beam(p: &egui::Painter, s: &Sim, m: &Materials, from: Pos2, to: Pos2, hit: Option<usize>, col: Color32, blocked: bool, scale: f32) {
    let f = s.world.frame as u32;
    if blocked && (f / 3) % 3 == 0 {
        return;
    }
    let flick = 0.8 + 0.2 * jitter(f, 1);
    let along = to - from;
    let len = along.length().max(1.);
    let side = Vec2::new(-along.y, along.x) / len;
    let white = Color32::from_rgb(235, 255, 252);
    // Soft glow, then a brighter band.
    p.line_segment([from, to], (6. * flick, col.gamma_multiply(0.13)));
    p.line_segment([from, to], (3. * flick, col.gamma_multiply(0.38)));
    // A crackling core: short segments nudged sideways each frame.
    let n = ((len / 6.).ceil() as u32).clamp(2, 24);
    let mut prev = from;
    for k in 1..=n {
        let t = k as f32 / n as f32;
        let wobble = if k == n { 0. } else { (jitter(f, k + 7) - 0.5) * 2.2 * (t * (1. - t) * 4.).min(1.) };
        let next = from + along * t + side * wobble;
        p.line_segment([prev, next], (1.3, white.gamma_multiply(0.9)));
        prev = next;
    }
    // Muzzle flare and the hot spot on the rock.
    p.circle_filled(from, 2.4 * flick, col.gamma_multiply(0.45));
    p.circle_filled(from, 1.1, white);
    if hit.is_none() {
        return;
    }
    p.circle_filled(to, 6. * flick, col.gamma_multiply(0.18));
    p.circle_filled(to, 3.2 * flick, Color32::from_rgb(255, 214, 140).gamma_multiply(0.6));
    p.circle_filled(to, 1.4, white);
    // Sparks: thrown back off the face, in the rock's colour turning hot.
    let rock = hit.map_or([200, 200, 200], |i| m.get(s.world.mat[i]).color);
    let rock = Color32::from_rgb(rock[0], rock[1], rock[2]);
    let back = -along / len;
    for i in 0..12u32 {
        let life = ((f + i * 5) % 14) as f32 / 14.;
        let seed = (f + i * 5) / 14;
        let spread = (jitter(seed, i) - 0.5) * 2.4;
        let dir = (back + side * spread).normalized();
        let speed = 6. + 12. * jitter(seed, i + 40);
        let at = to + dir * speed * life * scale.max(1.) * 0.6 + Vec2::new(0., life * life * 5.);
        let c = if i % 3 == 0 { Color32::from_rgb(255, 196, 110) } else { rock };
        p.circle_filled(at, (1.3 * (1. - life)).max(0.4), c.gamma_multiply(1. - life * 0.7));
    }
}
