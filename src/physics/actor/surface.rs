//! Reusable surface adhesion. A short-range distance probe follows the exposed
//! contour, including inside/outside corners. No support means normal gravity.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceWalker {
    pub normal: (f32, f32),
    pub attached: bool,
}
impl Default for SurfaceWalker {
    fn default() -> Self {
        Self {
            normal: (0., -1.),
            attached: false,
        }
    }
}
pub fn tangent(normal: (f32, f32)) -> (f32, f32) {
    (-normal.1, normal.0)
}
pub fn rotate(normal: (f32, f32), v: (f32, f32)) -> (f32, f32) {
    let t = tangent(normal);
    (t.0 * v.0 - normal.0 * v.1, t.1 * v.0 - normal.1 * v.1)
}
fn nearest(w: &World, m: &Materials, c: (f32, f32), reach: f32) -> Option<((f32, f32), f32)> {
    let mut best = reach * reach;
    let mut found = None;
    for y in (c.1 - reach).floor().max(0.) as usize..=((c.1 + reach).ceil() as usize).min(w.h - 1) {
        for x in
            (c.0 - reach).floor().max(0.) as usize..=((c.0 + reach).ceil() as usize).min(w.w - 1)
        {
            let i = w.idx(x, y);
            let mat = m.get(w.mat[i]);
            if w.is_owned(i) || mat.crushable || !matches!(mat.state, State::Solid | State::Powder)
            {
                continue;
            }
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            let d = (c.0 - p.0).powi(2) + (c.1 - p.1).powi(2);
            if d < best {
                best = d;
                found = Some((p, d.sqrt()));
            }
        }
    }
    found
}
/// Advance along a contour. Iterated separation keeps the body out of both
/// faces at an inside corner; the nearest-point arc rounds an outside corner.
pub fn advance(
    w: &World,
    m: &Materials,
    c: (f32, f32),
    normal: (f32, f32),
    height: f32,
    travel: f32,
) -> Option<((f32, f32), (f32, f32))> {
    let radius = height * 0.5 + 0.7;
    let t = tangent(normal);
    let mut at = (c.0 + t.0 * travel, c.1 + t.1 * travel);
    let (q, d) = nearest(w, m, at, radius + 2.5)?;
    if d < 0.1 {
        return None;
    }
    // Stay within a small step of the existing surface, never jump a shaft.
    if d > radius + 2.0 {
        return None;
    }
    let mut hit = q;
    for _ in 0..5 {
        let (q, d) = nearest(w, m, at, radius + 2.5)?;
        if d < 0.1 {
            return None;
        }
        hit = q;
        if d < radius {
            let k = (radius - d) / d;
            at.0 += (at.0 - q.0) * k;
            at.1 += (at.1 - q.1) * k;
        } else if d > radius + 0.05 {
            let k = (radius - d) / d;
            at.0 += (at.0 - q.0) * k;
            at.1 += (at.1 - q.1) * k;
        } else {
            break;
        }
    }
    // A narrow slit may make projections alternate between two faces. Reject
    // it unless the final body really clears both, instead of clipping through.
    if nearest(w, m, at, radius + 2.5)?.1 < radius - 0.08 {
        return None;
    }
    let vx = at.0 - hit.0;
    let vy = at.1 - hit.1;
    let len = vx.hypot(vy);
    if len < 0.1 {
        return None;
    }
    let n = (vx / len, vy / len);
    let blend = (normal.0 * 0.45 + n.0 * 0.55, normal.1 * 0.45 + n.1 * 0.55);
    let len = blend.0.hypot(blend.1);
    let n = if len > 0.1 {
        (blend.0 / len, blend.1 / len)
    } else {
        n
    };
    if at.0 < radius
        || at.1 < radius
        || at.0 > w.w as f32 - radius - 1.
        || at.1 > w.h as f32 - radius - 1.
    {
        return None;
    }
    Some((at, n))
}
pub(super) fn update(
    a: &mut Actor,
    bodies: &mut RigidBodySet,
    w: &World,
    m: &Materials,
    controls: Controls,
) -> bool {
    let Some(mut surface) = a.spec.surface else {
        return false;
    };
    let rb = &mut bodies[a.handle];
    let root = rb.translation() / CELL;
    let offset = (H as f32 - a.spec.posture.height) * 0.5;
    let center = (root.x, root.y + offset);
    let height = a.spec.posture.height;
    let radius = height * 0.5 + 0.7;
    let Some((q, d)) = nearest(w, m, center, radius + 2.5) else {
        surface.attached = false;
        a.spec.surface = Some(surface);
        rb.set_body_type(RigidBodyType::Dynamic, true);
        return false;
    };
    if d < 0.1 || d > radius + 2.0 {
        surface.attached = false;
        a.spec.surface = Some(surface);
        rb.set_body_type(RigidBodyType::Dynamic, true);
        return false;
    }
    if !surface.attached {
        surface.normal = ((center.0 - q.0) / d, (center.1 - q.1) / d);
    }
    let speed = a.spec.mobility.map_or(WALK, |c| c.crouch_speed);
    let travel = controls.walk.clamp(-1., 1.) * speed * 10.0 * DT;
    let Some((at, normal)) = advance(w, m, center, surface.normal, height, travel) else {
        surface.attached = false;
        a.spec.surface = Some(surface);
        rb.set_body_type(RigidBodyType::Dynamic, true);
        return false;
    };
    a.surface_controlled = surface.attached;
    surface.normal = normal;
    surface.attached = true;
    a.spec.surface = Some(surface);
    rb.set_body_type(RigidBodyType::KinematicPositionBased, true);
    rb.set_next_kinematic_translation(Vector::new(at.0 * CELL, (at.1 - offset) * CELL));
    a.since_ground = 0;
    a.spec.posture.crouched = true;
    if controls.walk.abs() > 0.01 {
        a.spec.facing = controls.walk.signum();
        a.stride = a.stride.wrapping_add(1);
    }
    a.v_before = rb.linvel();
    true
}
