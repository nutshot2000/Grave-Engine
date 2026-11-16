# Ember Rise

A one-level platformer built entirely out of scene data — no engine code
changed. A tower is burning; you start on the docks and climb to the roof
before it comes down.

## Play it

```bash
Play/Ember-Rise.cmd          # builds the engine and opens the level
```

In the editor press **Space** to play, **R** to reset to the start.

## Controls

| Key | What |
| --- | --- |
| A / D or ← / → | move |
| W / ↑ | jump — hold it at a wall to grab the lip |
| release W, press W again | pull up onto the ledge |
| S / ↓ | let go of a ledge |
| C | crouch (also gets you under the low beam) |
| X | roll |

## The route

1. **The docks.** A fire, a harbour you can fall into and swim out of, and a
   fuel barrel sitting under the gantry.
2. **Scaffolding.** Three concrete roofs, 2.4 m apart — too high to jump, so
   you grab the wooden lip and pull up. Three climbs.
3. **The gantry.** A plank bridge over the drop. Step onto it and the far end
   is already going up: a trigger heats the planks behind you and the wind
   carries the fire back the way you came. Cross before it goes.
4. **The cistern.** A pool carved into the tower, 2.4 m deep. Wade and swim
   across.
5. **The low beam.** A stone doorway with 1.4 m of headroom — crouch or roll
   through.
6. **The shaft.** Two staggered columns of ledges, 1.2 m apart, hopping
   left-right up the tower. The basin below breaks your fall.
7. **The sealed door.** A brick wall on the roof's edge with a gunpowder
   charge inside it. Step onto the fifth ledge and the charge goes off
   above you — shake, slow motion, and a hole to climb through.
8. **The roof.** Reach the far end: it is dawn, the sky warms up, the dust
   clears, and the level finishes.

## How it is built

Everything is plain text in `scenes/ember-rise.txt`:

- Terrain from `rect` shapes; floating ledges are pinned with `anchor` so
  they never collapse.
- **Ledge climbing** (`player ... climb 2 reach 6`) — the reusable
  traversal from the [Climbing Lab](LEDGE-CLIMBING.md), using the rigged
  Mixamo character look (`mixamo-fbx-local`) with hang, climb, roll and
  swim animations.
- **Fire, not animation.** The gantry is real wood cells; a trigger sets
  their temperature with `heat circle`, and the material rules do the
  rest — it burns, weakens, and collapses on its own. The barrel below can
  be set off by falling embers.
- **Water and breath.** The cistern is real water; the actor's breath runs
  down while the head is under.
- **Props from the Asset Workshop** (`assets/workshop/`): the explosive
  barrel and a hanging lantern, placed with the `asset` command.
- **Triggers** carry the level: six checkpoints worth 100 each, story
  messages, the gantry fire, the demolition with `shake` and `slowmo`,
  and the `finish` at the top — 1600 points in total.
- **Lighting** is a blue night (`lighting 0.45`) with warm flickering lamps
  along the route, and the ending fades the sky and light to dawn.

## Tuning

- The gantry burns away roughly 6–8 seconds after you step on it. Crossing
  takes about 3. Make the plank longer (`rect wood 560 428 100 4`) to
  make it tighter, or cool it with `weather rain 0.5`.
- Falls over about 4 m hurt. That is deliberate — checkpoints are close
  together.
- The demolition charge is `rect gunpowder 932 284 4 8`; more gunpowder
  means a bigger hole (and a louder shake).
