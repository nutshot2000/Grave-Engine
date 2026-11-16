//! Animated hand sockets and one shared tool origin for presentation and gameplay.
use crate::{Materials, Sim, material::State, physics::ActorStatus};

pub struct Pose {
    pub hand: (f32, f32),
    pub origin: (f32, f32),
    pub muzzle: (f32, f32),
    pub aim: (f32, f32),
}
pub fn shoulder(a: &ActorStatus) -> (f32, f32) {
    (a.x, a.y + (16. - a.body_height) * 0.5 - 3.)
}
pub fn pose(a: &ActorStatus, aim: (f32, f32)) -> Pose {
    let n = aim.0.hypot(aim.1).max(0.001);
    let aim = (aim.0 / n, aim.1 / n);
    let hand = a
        .look
        .as_ref()
        .and_then(|l| {
            let (_, i) = l.frame(&a.anim, a.anim_time);
            let &(mut x, mut y) = l.hands.get(&(a.anim.clone(), i))?;
            if let Some(b) = &a.blend {
                let (_, j) = l.frame(&b.from_clip, b.source_time);
                if let Some(&(sx, sy)) = l.hands.get(&(b.from_clip.clone(), j)) {
                    let t = (b.elapsed / b.seconds.max(0.001)).clamp(0., 1.);
                    let t = t * t * (3. - 2. * t);
                    x = sx + (x - sx) * t;
                    y = sy + (y - sy) * t;
                }
            }
            let cpp = l.cells_per_px(16);
            Some((
                a.x + a.visual_offset.0 + (x - 128.) * cpp * a.facing,
                a.y + 8. + a.visual_offset.1 + (y - l.feet) * cpp,
            ))
        })
        .unwrap_or_else(|| {
            let s = shoulder(a);
            (s.0 + aim.0 * 5., s.1 + aim.1 * 5.)
        });
    Pose {
        hand,
        origin: (hand.0 - aim.0 * 5., hand.1 - aim.1 * 5.),
        muzzle: (hand.0 + aim.0 * 5., hand.1 + aim.1 * 5.),
        aim,
    }
}
impl Sim {
    /// Vent a stored gas cell through the equipped collector. Finite tank, real gas.
    pub fn vent_gas(
        &mut self,
        m: &Materials,
        c: &crate::supplies::Catalog,
        aim: (f32, f32),
    ) -> bool {
        if self.world.frame % 3 != 0
            || !c
                .tool(&self.supplies.equipped)
                .is_some_and(|t| matches!(t.mode, crate::supplies::Mode::Vacuum { .. }))
        {
            return false;
        }
        let Some(a) = self
            .physics
            .player_status()
            .filter(|a| !a.dead && !a.hanging && !a.climbing && !a.rolling)
        else {
            return false;
        };
        let Some((&id, _)) = self.supplies.gas.iter().find(|(_, n)| **n > 0) else {
            return false;
        };
        let pose = pose(&a, aim);
        // A blocked nozzle retains its contents; the gas cannot appear through a wall.
        let n = (pose.muzzle.0 - pose.origin.0).hypot(pose.muzzle.1 - pose.origin.1);
        for k in 0..=(n * 2.) as usize {
            let t = k as f32 * 0.5;
            let (x, y) = (
                (pose.origin.0 + pose.aim.0 * t) as i32,
                (pose.origin.1 + pose.aim.1 * t) as i32,
            );
            if !self.world.in_bounds(x, y) {
                return false;
            }
            let i = self.world.idx(x as usize, y as usize);
            if !self.world.is_owned(i) && m.get(self.world.mat[i]).state != State::Gas {
                return false;
            }
        }
        if self.world.spawn_flyer(
            m,
            id,
            pose.muzzle.0,
            pose.muzzle.1,
            pose.aim.0 * 1.2,
            pose.aim.1 * 1.2,
        ) {
            *self.supplies.gas.get_mut(&id).unwrap() -= 1;
            return true;
        }
        false
    }
    /// All user tools use the same animated hand. Traversal frees both hands.
    pub fn use_player_tool(
        &mut self,
        m: &Materials,
        c: &crate::supplies::Catalog,
        aim: (f32, f32),
    ) -> std::collections::BTreeMap<crate::MatId, usize> {
        let Some(a) = self
            .physics
            .player_status()
            .filter(|a| !a.dead && !a.hanging && !a.climbing && !a.rolling)
        else {
            return Default::default();
        };
        // The crosshair is aimed from the shoulder. Wrist animation only moves the visible tool.
        if c.tool(&self.supplies.equipped).is_some_and(|t| matches!(t.mode, crate::supplies::Mode::Drill(_))) {
            return self.use_tool_from(m, c, shoulder(&a), aim, true);
        }
        let p = pose(&a, aim);
        let s = shoulder(&a);
        let d = (p.origin.0 - s.0, p.origin.1 - s.1);
        let n = d.0.hypot(d.1);
        // An animated hand must never move the emitter through a thin wall.
        for k in 0..=(n * 2.) as usize {
            let t = k as f32 * 0.5 / n.max(0.01);
            let (x, y) = ((s.0 + d.0 * t) as i32, (s.1 + d.1 * t) as i32);
            if !self.world.in_bounds(x, y) {
                return Default::default();
            }
            let i = self.world.idx(x as usize, y as usize);
            if !self.world.is_owned(i) && m.get(self.world.mat[i]).state != State::Gas {
                return Default::default();
            }
        }
        self.use_tool(m, c, p.origin, p.aim)
    }
    pub fn aim_tool(&mut self, aim: (f32, f32), active: bool, firing: bool) {
        self.physics
            .set_actor_animation_parameter(0, "drilling", firing as u8 as f32);
        self.physics
            .set_actor_animation_parameter(0, "tool_active", active as u8 as f32);
        self.physics
            .set_actor_animation_parameter(0, "aim_y", aim.1);
        self.physics
            .set_actor_animation_parameter(0, "aim_x", aim.0);
        if active && aim.0.abs() > 0.1 {
            self.physics.edit_actor(0, |a| a.facing = aim.0.signum());
        }
    }
}
