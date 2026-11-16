# The Lantern Cut: cave kit playtest

Open **Play/Cave-Kit-Trial.cmd** and press **Play**. This is a compact,
editable cavern for testing the new kit together. The starting bench has
enough scrap for either a gas collector or a flame projector. Mine the iron
seam and sandstone nearby to build more equipment. Crystal shelves and an
oil crawler/fire shellback give you a reason to cross the cavern.

The existing Mining Trial, expedition and generated cave examples also now
start with a grappling rope. New expeditions use the rope; old saved runs
retain their previous equipment until restarted.

| Action | Keyboard / mouse | Controller |
| --- | --- | --- |
| Move / pump a swing | A/D | Left stick |
| Jump / grab / climb | W | A |
| Aim | Mouse / IJKL | Right stick |
| Attach / release rope | Q / right click | Right-stick click |
| Reel in / pay out rope | Space / S | D-pad up/down |
| Use equipped tool | Hold left click / E | LT |
| Change tool | F | LB |
| Cargo and workshops | B | Left-stick click |
| Vent collector tank | V | RT |
| Crouch / roll | C / X | X / RB |
| Pause and edit | P / top bar | Start |

Aim at solid scenery and attach the rope, then jump, swing and reel. Release
to keep your momentum. It has elastic give and wraps around nearby exposed
corners, then unwraps when the line clears them. Removing its anchor drops
the line. Rock collisions still matter: this does not let you fly through
walls. The jetpack remains supported as optional scene equipment.

The game HUD puts vitals and rope status at the upper left, sample goals and
signal at the upper right, the equipped tool at the lower left, and cargo at
the lower right. Cave Play hides editor panels; **Pause & edit** restores
them. **Reset** restores the starting terrain, enemies, inventory and rope.
Cargo pauses the simulation and shows exact counts, locked/owned tools,
recipe costs and the reason a recipe is unavailable.

The drill highlights the actual exposed pixels it will work in cyan, after
lighting so they remain visible in darkness. Red means the surface requires
a stronger drill. The progress label reflects existing fracture work. This
is a temporary display overlay and does not paint or change materials.

## Character and tools

`assets/characters/cave-diver/look.link` prefers the installed local imported
character, with the original explorer as a public fallback. The alias has
its own `assets/animations/cave-diver.graph`; the existing animation lab is
unchanged. It uses the existing jump, fall, landing, crouch, roll, ledge
grab/climb and swim clips. Stationary tool aiming selects the up, neutral
or down weapon pose; movement retains its movement animation.

`hand.sockets` in the local character directory contains an animated hand
position for each frame. The tool and emitter share that position, including
mirroring and transition interpolation. A shoulder-to-emitter obstruction
check prevents an animated hand firing through thin walls. Tools are put
away during ledge traversal and rolling. The engine still uses rendered
frames of the rig rather than a runtime 3D skeleton.

To rebuild anchors after editing the local rig, run Blender in background
on `mixamo-fbx-animations.blend` with `tools/export_character_sockets.py`.
The script exports positions without re-rendering or changing the rig.
Imported art, the Blender file and exported sockets stay local.

## Gas collection

Craft the **Gas collector & 512-cell tank** at a mechanic workshop. Aim into
the gas pocket in the right ceiling; break its sandstone seal first. Click
draws in nearby visible gas. Ordinary air is ignored, walls/liquids block
suction, and a full tank stops collecting. Different gases keep separate
exact counts under **[tank]** in Cargo. V/RT vents the first stored gas type
as real simulated cells. A blocked nozzle retains the contents.

At a chemistry workshop, **Condense gas fuel** spends 48 volatile gas cells
to make 96 oil cells for the flame projector. This is an authored processing
recipe, not an assertion of material-volume conservation during refining.
Collected gas is stored as a material resource; venting uses its default
temperature/state, not its original heat or remaining lifetime.

## Editable settings and verification

- Scene player settings: `rope rope_range 220 rope_elasticity 0.06`.
  Range is in cells; higher elasticity gives a softer catch. Saved runs also
  carry `rope_length` and `rope_path` (hook plus corner coordinates).
- Tools, collection reach, intake width and tank capacity:
  `assets/crafting/cave-tools.txt`. Recipes: `cave-recipes.txt` beside it.
- Gas properties: `materials/volatile_gas.txt`. Tank scene inventory uses
  `gas_stock MATERIAL COUNT`, independently from ordinary `stock`.
- Cave lighting remains normal editable scene lamps. The personal fill is
  broader; the headlamp follows aim, and drill strikes add a brief work light.
- `tests/cave_kit.rs` exercises swings, reeling, release momentum, corner
  wrapping, mined anchors, save/reset/shift, gas conservation/capacity,
  occlusion/refining/venting, exact mining previews and local weapon poses.

The corner search is bounded and handles nearby exposed terrain corners;
it is not an arbitrary maze pathfinder or a freely knotted rope simulation.
Automated checks do not establish final game balance or a full human winning
playthrough with the new starter kit.
