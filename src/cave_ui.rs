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
            "ROPE READY · aim at solid rock".into()
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
            r.center_bottom() - Vec2::new(width * 0.5, if r.width() < 1050. { 218. } else { 125. });
        card(p, Rect::from_min_size(at, Vec2::new(width, 86.)), GOLD);
        let attached = !rope.pins.is_empty();
        label(
            p,
            at + Vec2::new(14., 10.),
            if attached {
                "ROPE ATTACHED — hold to pull yourself up"
            } else {
                "ROPE — aim above you at the gold hook target"
            },
            14.,
            GOLD,
        );
        label(
            p,
            at + Vec2::new(14., 35.),
            if pad {
                if attached {
                    "Hold D-pad UP: pull up   DOWN: lower yourself"
                } else {
                    "Click RIGHT STICK once to attach"
                }
            } else if attached {
                "Hold SPACE: pull up   Hold S: lower yourself"
            } else {
                "RIGHT CLICK or Q once to attach (no need to hold)"
            },
            13.,
            INK,
        );
        label(
            p,
            at + Vec2::new(14., 59.),
            if attached {
                if pad {
                    "A/D or left stick: swing · R3 again: release"
                } else {
                    "A/D: swing · RIGHT CLICK / Q again: release"
                }
            } else {
                "Then hold SPACE to climb the rope. A/D swings."
            },
            12.,
            MUTED,
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
                "EQUIPPED  /  LB change"
            } else {
                "EQUIPPED  /  F change"
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
                "L3  Inventory & workshops"
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
        "LS Move   A Jump   R3 Rope   D-pad Reel   LT Tool   LB Equip   L3 Cargo"
    } else {
        "A/D Move   W Jump/climb   Q / right click Rope   Space/S Reel   Click Tool   F Equip   B Cargo   P Pause"
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
                let start = point(from);
                let length = (shown - start).length();
                for i in 0..((length / 16.).ceil() as usize).min(80) {
                    let t = i as f32 * 16. / length.max(1.);
                    let end = (t + 5. / length.max(1.)).min(1.);
                    p.line_segment(
                        [start.lerp(shown, t), start.lerp(shown, end)],
                        (1., GOLD.gamma_multiply(0.45)),
                    );
                }
                p.circle_filled(shown, 9., Color32::from_black_alpha(220));
                p.circle_stroke(shown, 7., (2., GOLD));
                p.line_segment(
                    [shown - Vec2::new(4., 0.), shown + Vec2::new(4., 0.)],
                    (1.5, GOLD),
                );
                p.line_segment(
                    [shown - Vec2::new(0., 4.), shown + Vec2::new(0., 4.)],
                    (1.5, GOLD),
                );
                let text = if clip.contains(actual) {
                    "ROPE HOOK · right click / Q"
                } else {
                    "ROPE HOOK OFFSCREEN · aim this way"
                };
                p.text(
                    shown + Vec2::new(0., 15.),
                    Align2::CENTER_TOP,
                    text,
                    FontId::proportional(12.),
                    GOLD,
                );
            } else {
                let at = point((a.x, a.y - 15.));
                p.text(
                    at,
                    Align2::CENTER_BOTTOM,
                    "ROPE: no solid rock in this direction / range",
                    FontId::proportional(12.),
                    GOLD,
                );
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
    let hit = crate::mining::surface(&s.world, m, pose.origin, aim, reach);
    let target = hit
        .map(|i| ((i % s.world.w) as f32 + 0.5, (i / s.world.w) as f32 + 0.5))
        .unwrap_or((
            pose.origin.0 + pose.aim.0 * reach,
            pose.origin.1 + pose.aim.1 * reach,
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
        for (j, _) in crate::mining::candidates(&s.world, m, pose.origin, i, d) {
            let at = point(((j % s.world.w) as f32, (j / s.world.w) as f32));
            p.rect_filled(
                Rect::from_min_size(at, Vec2::splat(scale)),
                0.,
                target_col.gamma_multiply(if active { 0.65 } else { 0.36 }),
            );
        }
    }
    let radius = drill.map_or(6., |d| d.radius * scale);
    p.circle_stroke(center, radius + 1., (3., Color32::from_black_alpha(230)));
    p.circle_stroke(center, radius, (1.5, target_col));
    for dir in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
        p.line_segment(
            [center + dir * (radius + 3.), center + dir * (radius + 9.)],
            (3., Color32::BLACK),
        );
        p.line_segment(
            [center + dir * (radius + 3.), center + dir * (radius + 9.)],
            (1.4, INK),
        );
    }
    p.circle_filled(center, 1.5, INK);
    if let Some((i, _)) = hit.zip(drill) {
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
        let at = center - Vec2::new(galley.size().x / 2., radius + 29.);
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
        p.circle_filled(end, 1.8 * scale, target_col);
        if drill.is_some() {
            p.line_segment([end, center], (1.2, target_col));
        }
    }
}
