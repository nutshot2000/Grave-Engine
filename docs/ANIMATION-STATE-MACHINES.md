# Animation state machines

Click **Animations** in the editor toolbar. Select a character, edit the
states and transitions, then **Restart preview** to test your draft.
Preview inputs are separate from the game: change speed, airborne,
swimming or dead to see the transitions without moving the character.
The preview shows the current state, clip, clock and emitted events.

**Apply to character** installs the validated draft on that character.
This is undoable and also updates its Reset configuration. Saving the
scene preserves its custom graph. **Save graph + apply** additionally
writes a reusable graph file; the default location is the character's
`animation.graph`. Every scene using that look will load this file.
Invalid graphs or missing clips report an error before being applied.

Expand a state to edit its name, clip, looping, speed and optional duration.
Add timed events for sounds or gameplay. Expand a transition to edit its
source, destination, priority, minimum state time and conditions. Delete
unused states and repair their transitions before applying. Names use
letters, digits, underscores or hyphens. Clip names must exist in the look.

The default graph has idle, walk, jump, fall, landing, swim and die states.
Landing currently uses the idle clip for a short settling interval; assign
a dedicated landing clip when one is available. Walking playback follows
movement speed. Death and swimming interrupt lower-priority actions.

## Reusable files

`assets/animations/locomotion.graph` is the shared default, embedded when
the engine is built. A look's `animation.graph` overrides it. Use the
editor's Load button to bring any graph file into a draft.

A scene can assign a reusable graph after a player:

```text
player 40 130 look hero
animation ../assets/animations/action-demo.graph
```

Paths resolve beside the scene, then from the working directory.
An inline block is also supported:

```text
player 40 130 look hero
animation
initial standing
param speed 0
state standing clip idle loop speed 1
state moving clip walk loop speed 1
transition standing moving priority 0 after 0 when speed > 0.4
transition moving standing priority 0 after 0 when speed <= 0.4
endanimation
```

Scene saving embeds per-scene overrides, so those do not depend on an
external file remaining in place. Shared character-folder graphs remain
part of the look. Scenes start at the initial state; in-memory
snapshots and rewind preserve each character's current state, parameters,
pending triggers and playback clocks.

## Graph format

```text
initial idle
param speed 0
param dead 0
trigger attack
state idle clip idle loop speed 1
state action clip walk once speed 1
event action 0.1 hit
state dead clip die once speed 1
transition * dead priority 100 after 0 when dead > 0
transition idle action priority 10 after 0 when attack > 0
transition action idle priority 0 after 0.2 when finished > 0
```

The action in this example deliberately reuses walk as a placeholder;
assign an actual action clip for a game. Custom parameters and triggers
can be added in the editor's **Parameters, triggers and text format** section.
Export the draft before editing its text, then Import to validate it.

- `*` matches any source state. Transitions to the current state are ignored.
- Higher priority wins; equal priorities use file/list order. Only one
  transition happens per update, preventing instant transition cycles.
- `after` is minimum real time in the source state, independent of clip speed.
- `finished` becomes 1 at the end of a once clip. Loop states never finish.
- Conditions support `> >= < <= == !=`, joined with `&` (AND). Separate
  transitions express OR. An empty condition always passes.
- A trigger is 1 for the next update only, even if no transition uses it.
- Once clips hold their last frame. Loop clips wrap. `duration SECONDS`
  optionally overrides the state clip duration.
- `rate PARAM SCALE MIN MAX` multiplies playback by a clamped parameter
  value. The default walk uses `rate speed 0.25 0.3 1.5`.
- Events use clip seconds and repeat with loops. A time-zero event runs
  on the next update after entry. `enter` is emitted when a transition
  selects a new state. Across an unusually large update, each marker is
  emitted at most once to bound the work.

Built-in actor inputs are `speed` (horizontal m/s, absolute), `vy` (vertical
m/s, negative upward), and 0/1 values `grounded`, `airborne`, `swimming`,
`dead`, `hurt`. These update automatically when declared in a graph.
Other parameters belong to the game. Airborne has a short contact grace
period, with immediate takeoff for actual jumps and bounce pads.

## Game integration

`grave_engine::animation::{Graph, Animator}` is independent of the
character controller, physics and renderer. Parse a graph, share it with
`Arc`, create one Animator per entity, set inputs, trigger actions and call
`advance(dt, clip_duration)`. Read `state().clip` and `sample_time` to drive
your renderer. Handle the returned events in the game; the graph never
executes gameplay or audio itself. Graph comparisons compile once per
Animator rather than parsing text every tick.

The built-in actor adapter exposes `Physics::set_actor_graph`,
`set_actor_animation_parameter`, `trigger_actor_animation` and
`take_actor_animation_events`. Its event queue is capped at 256; drain it
regularly when using markers for gameplay. Each actor has independent
parameters, triggers and clocks, even when several share the same graph.

This update controls selection, timing and interruption of clips. The
current 2D character renderer still uses rendered frames and switches
between clips; it does not blend skeletal poses or improve source motion.
