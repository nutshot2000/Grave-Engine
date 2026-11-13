//! Optional ledge grabbing and a checked two-stage climb for any actor.
//! Only solid scenery supports a grab. Support and clearance are rechecked
//! throughout traversal, so burned ledges and newly blocked paths release it.

use super::*;

/// Reusable per-character traversal settings (reach in cells, time in seconds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClimbSettings {
    pub reach: f32,
    pub seconds: f32,
}
impl Default for ClimbSettings {
    fn default() -> Self { Self { reach: 6.0, seconds: 2.0 } }
}

/// A held corner and progress, included in snapshots and rewind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LedgeHold {
    pub edge: (i32, i32),
    pub side: f32,
    pub climbing: bool,
    pub elapsed: f32,
}
impl LedgeHold {
    fn hanging(self) -> (f32, f32) { (self.edge.0 as f32 - self.side * 3.7, self.edge.1 as f32 + 13.0) }
    fn top(self) -> (f32, f32) { (self.hanging().0, self.edge.1 as f32 - H as f32 / 2.0 - 0.25) }
    fn landing(self) -> (f32, f32) { (self.edge.0 as f32 + self.side * 5.0, self.top().1) }
    pub(super) fn position(self, seconds: f32) -> (f32, f32) {
        if !self.climbing { return self.hanging(); }
        let p = (self.elapsed / seconds).clamp(0.0, 1.0);
        let (a, b, t) = if p < 0.7 { (self.hanging(), self.top(), p / 0.7) } else { (self.top(), self.landing(), (p - 0.7) / 0.3) };
        let t = t * t * (3.0 - 2.0 * t);
        (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
    }
}

fn solid(world: &World, mats: &Materials, x: i32, y: i32) -> bool {
    world.in_bounds(x, y) && {
        let i = world.idx(x as usize, y as usize);
        !world.is_owned(i) && mats.get(world.mat[i]).state == State::Solid && !mats.get(world.mat[i]).crushable
    }
}

fn supported(h: LedgeHold, w: &World, m: &Materials) -> bool {
    let sx = h.edge.0 - (h.side < 0.0) as i32;
    solid(w, m, sx, h.edge.1) && !solid(w, m, sx, h.edge.1 - 1)
        && !solid(w, m, sx - h.side as i32, h.edge.1 + 1)
        && (-2..=2).all(|dx| solid(w, m, h.landing().0 as i32 + dx, h.edge.1))
}

fn clear(at: (f32, f32), own: ColliderHandle, w: &World, m: &Materials, colliders: &ColliderSet, ground: RigidBodyHandle) -> bool {
    let (x0, x1) = (at.0 - W as f32 / 2.0 + 0.1, at.0 + W as f32 / 2.0 - 0.1);
    let (y0, y1) = (at.1 - H as f32 / 2.0 + 0.1, at.1 + H as f32 / 2.0 - 0.1);
    if x0 < 0.0 || y0 < 0.0 || x1 >= w.w as f32 || y1 >= w.h as f32 { return false; }
    for y in y0.floor() as i32..=y1.floor() as i32 {
        for x in x0.floor() as i32..=x1.floor() as i32 {
            if solid(w, m, x, y) { return false; }
        }
    }
    // Moving props and other actors also block the route. Ground polyline
    // bounds include empty space around corners: exclude that shared body,
    // since current terrain cells were already checked precisely above.
    !colliders.iter().any(|(id, c)| {
        if id == own || c.parent().is_none() || c.parent() == Some(ground) || c.is_sensor() { return false; }
        let b = c.compute_aabb();
        b.mins.x < x1 * CELL && b.maxs.x > x0 * CELL && b.mins.y < y1 * CELL && b.maxs.y > y0 * CELL
    })
}

fn path(from: (f32, f32), to: (f32, f32), own: ColliderHandle, w: &World, m: &Materials, cs: &ColliderSet, ground: RigidBodyHandle) -> bool {
    let n = (((to.0 - from.0).abs().max((to.1 - from.1).abs()) * 2.0).ceil() as usize).max(1);
    (0..=n).all(|i| { let t = i as f32 / n as f32; clear((from.0 + (to.0-from.0)*t, from.1 + (to.1-from.1)*t), own, w, m, cs, ground) })
}

fn route(h: LedgeHold, own: ColliderHandle, w: &World, m: &Materials, cs: &ColliderSet, ground: RigidBodyHandle) -> bool {
    path(h.hanging(), h.top(), own, w, m, cs, ground) && path(h.top(), h.landing(), own, w, m, cs, ground)
}

/// Release is shared by dropping, destroyed support, damage and blocked climbs.
pub(super) fn release(a: &mut Actor, rb: &mut RigidBody) {
    a.spec.ledge = None;
    a.spec.grab_cooldown = 30;
    a.since_ground = COYOTE + 9;
    rb.set_body_type(RigidBodyType::Dynamic, true);
    rb.set_linvel(Vector::new(0.0, 0.0), true);
    if let Some(anim) = &mut a.spec.animation { anim.trigger("release"); }
}

/// Returns true while traversal owns movement; health/temperature still run.
pub(super) fn update(a: &mut Actor, bodies: &mut RigidBodySet, cs: &ColliderSet, w: &World, m: &Materials, controls: Controls, knocked: bool, ground: RigidBodyHandle) -> bool {
    let pressed = controls.jump && !a.spec.up_held;
    a.spec.up_held = controls.jump;
    a.spec.grab_cooldown = a.spec.grab_cooldown.saturating_sub(1);
    let Some(settings) = a.spec.climb else { return false; };
    let rb = &mut bodies[a.handle];
    if let Some(mut h) = a.spec.ledge {
        if controls.drop || knocked || a.pending >= a.spec.health || !supported(h, w, m) {
            release(a, rb); return false;
        }
        if !h.climbing && pressed && route(h, a.collider, w, m, cs, ground) {
            h.climbing = true; h.elapsed = 0.0;
        }
        if h.climbing {
            h.elapsed += DT;
            let next = h.position(settings.seconds);
            let old = rb.translation() / CELL;
            if !path((old.x, old.y), next, a.collider, w, m, cs, ground) {
                release(a, rb); return false;
            }
            rb.set_next_kinematic_translation(Vector::new(next.0 * CELL, next.1 * CELL));
            if h.elapsed >= settings.seconds {
                rb.set_translation(Vector::new(next.0 * CELL, next.1 * CELL), true);
                release(a, rb);
                // Success can use a pose transition; dropping/cancellation must
                // remain immediate and must not play a standing-up animation.
                if let Some(anim) = &mut a.spec.animation { anim.trigger("climb_done"); }
                a.jump_wait = 20;
                a.since_ground = 0;
                a.v_before = Vector::ZERO;
                return true;
            }
        } else {
            let p = h.hanging();
            if !clear(p, a.collider, w, m, cs, ground) { release(a, rb); return false; }
            rb.set_next_kinematic_translation(Vector::new(p.0 * CELL, p.1 * CELL));
        }
        a.spec.ledge = Some(h);
        a.v_before = rb.linvel();
        return true;
    }
    if !controls.jump || controls.drop || knocked || a.spec.grab_cooldown > 0 || a.since_ground == 0 || a.wet > 0.4 { return false; }
    let p = rb.translation() / CELL;
    let side = if controls.walk.abs() > 0.1 { controls.walk.signum() } else { a.spec.facing };
    let reach = settings.reach.ceil() as i32;
    for d in 0..=reach {
        let x = (p.x + side * (W as f32 / 2.0 + 0.5 + d as f32)).floor() as i32;
        // A lip may be above the head or beside the upper body during a
        // running jump. Restricting it to head height misses reachable roofs.
        for dy in -reach..=H as i32 / 2 + 2 {
            let y = (p.y - H as f32 / 2.0).round() as i32 + dy;
            let h = LedgeHold { edge: (x + (side < 0.0) as i32, y), side, climbing: false, elapsed: 0.0 };
            if !supported(h, w, m) || !path((p.x, p.y), h.hanging(), a.collider, w, m, cs, ground) || !route(h, a.collider, w, m, cs, ground) { continue; }
            a.spec.facing = side;
            a.spec.ledge = Some(h);
            let at = h.hanging();
            rb.set_body_type(RigidBodyType::KinematicPositionBased, true);
            rb.set_translation(Vector::new(at.0 * CELL, at.1 * CELL), true);
            rb.set_next_kinematic_translation(Vector::new(at.0 * CELL, at.1 * CELL));
            rb.set_linvel(Vector::ZERO, true);
            a.v_before = Vector::ZERO;
            return true;
        }
    }
    false
}
