# Crouching and rolling

Open **Play-Mobility-Lab.cmd**. Press Play in the editor.

- A/D or arrows: move. W/Up: jump, grab ledges, or swim upwards.
- Hold C: crouch and walk slowly. Release C: stand when there is room.
- Press X: roll forwards. Face the desired direction first. Holding X does
  not repeat rolls; release it before rolling again.
- Hold C during a roll to remain crouched afterwards.
- S/Down: drop from a ledge. Space: pause. R: reset.

The first beam requires crouching. Try releasing C underneath: the character
stays low until clear. The short second beam is for rolling. The platforms
beyond it test the existing jump/grab/climb transitions.

Keyboard and controller bindings are editable in `assets/input/editor.input`.
Controller defaults: left stick moves, A jumps, X crouches, RB rolls, B drops.

## Reusing the mechanics

Add `moves` to any scene's player line. It enables real, collision-aware low
stances independently of the character artwork. Optional settings follow:

```text
player 50 236 look hero moves crouch_height 9 roll_height 8 crouch_speed 1.6 roll_speed 5.5 roll_seconds 0.76
```

Heights are cells; speeds are metres/second; times are seconds. Settings are
saved in scenes, and current stance, roll direction, timers and button edges
are preserved in snapshots/rewind. Capsule height changes keep the feet and
mass fixed. Walls stop rolling, and rolling grants no damage immunity.

The animation graph receives `crouched` and `rolling` alongside the existing
movement parameters. The local imported character uses the supplied UAL crouch
idle, crouch walk and roll clips. A roll plays at twice its original speed to
match the default 0.76-second action. Changing the physical roll duration should
also change the graph's roll speed and rebake its incoming blends.

The Mixamo graph includes real skeletal pose blends at the current source
phase, rather than a crossfade of two pictures. Native locomotion, crouch,
roll and their dry airborne exits use these banks. Death and entering water
can interrupt immediately. The original clips and rig remain local and intact.
The public hero placeholder has no matching crouch/roll artwork; use a look
with the appropriate graph for finished animation.

`tools/prepare_climbing_lab.py --scene mobility-lab` chooses the local imported
look when available. `tools/bake_movement_blends.py --look LOOK --graph GRAPH
--reuse-existing` skips complete compatible banks. Independent workers can
use `--shards N --shard INDEX --no-install` on disjoint banks; install the graph
only after all workers finish. Generated character imagery stays local.
