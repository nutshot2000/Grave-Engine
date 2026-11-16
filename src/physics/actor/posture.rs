//! Optional, rewindable low stances. The standing root and feet never shift
//! when changing height; only the capsule and its local offset change.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobilitySettings {
    /// Cell heights; speeds in m/s and times in seconds.
    pub crouch_height: f32,
    pub roll_height: f32,
    pub walk_speed: f32,
    pub crouch_speed: f32,
    pub roll_speed: f32,
    pub roll_seconds: f32,
}
impl Default for MobilitySettings {
    fn default() -> Self { Self { walk_speed: WALK, crouch_height: 9.0, roll_height: 8.0, crouch_speed: 1.6, roll_speed: 5.5, roll_seconds: 0.76 } }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Posture {
    pub height: f32,
    pub crouched: bool,
    pub blocked: bool,
    pub roll_left: f32,
    pub direction: f32,
    pub cooldown: f32,
    pub roll_held: bool,
}
impl Default for Posture {
    fn default() -> Self { Self { height: H as f32, crouched: false, blocked: false, roll_left: 0.0, direction: 1.0, cooldown: 0.0, roll_held: false } }
}

pub(super) fn shape(c: &mut Collider, height: f32) {
    let mass = c.mass();
    let radius = W as f32 * CELL / 2.0;
    c.set_shape(SharedShape::capsule_y((height * CELL / 2.0 - radius).max(0.0), radius));
    c.set_translation_wrt_parent(Vector::new(0.0, (H as f32 - height) * CELL / 2.0));
    c.set_mass(mass);
}

pub(super) fn update(a: &mut Actor, bodies: &mut RigidBodySet, cs: &mut ColliderSet, w: &World, m: &Materials, c: Controls, ground: RigidBodyHandle, wall: bool, knocked: bool) {
    let pressed = c.roll && !a.spec.posture.roll_held;
    a.spec.posture.roll_held = c.roll;
    let Some(settings) = a.spec.mobility else { return; };
    // A fast, low capsule can catch a terrain polyline endpoint and receive a
    // tiny upward impulse. Keep a ground roll attached to a nearby flat floor,
    // but never bridge a ledge, follow a distant floor, or suppress a blast.
    if a.spec.posture.cooldown > 0.0 && a.spec.jump_buffer == 0 && !c.jump && !knocked && a.since_ground <= 8 && a.wet <= 0.4 {
        let rb = &mut bodies[a.handle];
        let p = rb.translation() / CELL;
        let feet = p.y + H as f32 / 2.0;
        let solid = |x: i32,y: i32| w.in_bounds(x,y) && {
            let i=w.idx(x as usize,y as usize);
            !w.is_owned(i) && m.get(w.mat[i]).state==State::Solid && !m.get(w.mat[i]).crushable
        };
        for y in feet.floor() as i32..=(feet+2.0).floor() as i32 {
            if (-2..=2).all(|dx| solid(p.x.round() as i32+dx,y) && !solid(p.x.round() as i32+dx,y-1)) {
                if rb.linvel().y.abs()<2.5 {
                    rb.set_translation(Vector::new(p.x*CELL,(y as f32-H as f32/2.0)*CELL),true);
                    rb.set_linvel(Vector::new(rb.linvel().x,0.0),true);
                    a.since_ground=0;
                }
                break;
            }
        }
    }
    let rb = &bodies[a.handle];
    let pos = rb.translation() / CELL;
    let can_fit = |height| ledge::clear_height((pos.x, pos.y + (H as f32 - height) / 2.0), height, a.collider, w, m, cs, ground);
    let standing = can_fit(H as f32);
    // A pose blend still starts with the previous standing silhouette. Do not
    // shrink the capsule ahead of that picture and let it enter a low beam
    // while its head is visibly upright. Actual low poses unlock the passage.
    let pose_height = a.spec.look.as_ref().and_then(|look| {
        let anim=a.spec.animation.as_ref()?;
        if !anim.graph.parameters.contains_key("crouched") || !anim.graph.parameters.contains_key("rolling") { return None; }
        let (frame,_,_)=look.sample(&anim.state().clip,anim.sample_time(|c|look.duration(c)),anim.blend.as_ref());
        frame.mask.chunks(MASK_W).position(|row|row.iter().any(|on|*on)).map(|row|(H-row)as f32)
    }).unwrap_or(0.0);
    let p = &mut a.spec.posture;
    p.cooldown = (p.cooldown - DT).max(0.0);
    let disabled = knocked || a.wet > 0.4 || a.spec.dead_for > 0 || a.spec.ledge.is_some() || a.pending >= a.spec.health;
    p.roll_left = (p.roll_left - DT).max(0.0);
    if disabled || a.since_ground > 8 || (wall && p.roll_left < settings.roll_seconds - 0.18_f32.min(settings.roll_seconds*0.5) && rb.linvel().x.abs() < settings.roll_speed * 0.25) {
        p.roll_left = 0.0;
    }
    if pressed && !disabled && a.since_ground == 0 && rb.linvel().y.abs() < 1.0 && p.roll_left == 0.0 && p.cooldown == 0.0 && can_fit(settings.roll_height) {
        p.direction = if c.walk.abs() > 0.1 { c.walk.signum() } else { a.spec.facing };
        p.roll_left = settings.roll_seconds;
        p.cooldown = settings.roll_seconds + 0.3;
    }
    p.blocked = !standing;
    p.crouched = !disabled && p.roll_left == 0.0 && ((c.crouch && a.spec.jump_buffer == 0) || !standing);
    let low = if p.roll_left > 0.0 { settings.roll_height } else if p.crouched { settings.crouch_height } else { H as f32 };
    let target=low.max(pose_height);
    // The capsule follows the rendered tuck and rises only with clear headroom.
    let next = if p.roll_left > 0.0 { target } else { p.height + (target-p.height).clamp(-60.0*DT,60.0*DT) };
    if (next-p.height).abs() > 0.001 && (next <= p.height || can_fit(next)) {
        p.height = next;
        shape(&mut cs[a.collider], p.height);
    }
}
