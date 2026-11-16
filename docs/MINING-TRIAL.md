# The Crooked Seam — mining experiment

Double-click **Play/Mining-Trial.cmd** to open the cavern in the editor.
Keep **Player tools** and **Crystal mission** checked, then press **Play**.
The starter kit now uses a grappling rope: hold right mouse to attach;
scroll to pull up/lower, release right mouse to let go. Space jumps. See [Cave kit](CAVE-KIT.md).

Use A/D to move, aim with the mouse, hold click or E to drill, Space/W to jump/climb,
and scroll to change the attached rope's length. P pauses. Reset restores the test.

The trial also has [material storage and workshops](CAVE-WORKSHOPS.md).
B opens exact material counts and crafting; F cycles built tools. The camp
mechanic makes a reinforced drill or flame projector; the gallery chemistry
bench makes an acid sprayer. Fuel and acid are finite crafted supplies.

[Surface-crawling guards](CAVE-CREATURES.md) protect the richer lower pocket
and cache. Their elemental deaths release real material spills. A small
violet seam near the shaft provides initial chemical supplies before them.

The point is to compare choices, rather than race to fill the sample bars:

- At camp, chip the golden sandstone. It gives way quickly.
- The grey basalt wall to the right is a possible shortcut, but needs much
  more work. Dark layered bedrock below it stops the starter drill entirely.
- The open winding passage above bypasses the basalt. Follow the torch,
  using rope swings and resting on ledges.
- Collect cyan from the shelf in the next gallery. Sweep the drill across
  a surface to open enough room for the explorer; it is a narrow tool.
- Break the soft floor near the gallery's right torch to reach the violet
  pocket below. Plan your rope anchor for the climb back out. There are
  shelves inside the shaft.
- Return to the amber camp ring with 16 cells of each colour to finish.

Rock now fractures before breaking. The aiming ring sits on the first
surface within reach, with its material, resistance and fracture percentage.
Chips and dust come from the actual contact; stone impacts and breaks use
the engine's material sound (enable the speaker button to hear them).
Hard facings protect deposits behind them. Liquids and rigid objects block
the drill. Only removed terrain counts towards the mission.

All geometry, lights, crystals and mission targets are ordinary scene data.
Right-click a material and adjust **Mining hardness (seconds)** to tune it.
Changes apply everywhere that material is used. Mining hardness is separate
from physics toughness, so balancing the drill does not weaken buildings.
The values are surface work times; tool-edge falloff and the depth of a wall
make the complete excavation take longer. The starter drill's grade is 3:
hardness above 3 requires a different tool. Tool power and grade are engine
parameters available to games; this experiment does not add an upgrade shop.

Fractures persist when you release the drill. Scene save/load, undo, rewind
and Reset carry them with the terrain. Cosmetic dust gives no extra samples.
The same drilling rule is used by DEEPWARD and editor Play. The older
`dig_circle` helper remains available for instant excavation by other tools.

This is a feel test, not finished balance. Useful feedback is whether the
slow shortcut is tempting, whether the winding route feels preferable, and
whether chipping rock is satisfying or simply tedious.
