# DEEPWARD — cave expedition prototype

Double-click **Play/Cave-Explorer.cmd**. The title screen resumes the saved
expedition; press **Enter** (controller A) to start. This is a playable first
loop built on Grave Engine, not a finished or balanced game.

Recover the cyan and violet sample quotas shown in the HUD, then return to
the amber lift marker. The signal points to the nearest needed crystal,
and switches home once both quotas are met. Hold the drill to mine actual
material cells; you can bore your own passages through blocked routes.
Health, swimming, heat, collisions and the material simulation still apply.

| Action | Keyboard / mouse | Controller |
| --- | --- | --- |
| Walk | A / D, arrows | Left stick / d-pad |
| Jump / grab / climb | Space / W | A |
| Rope | Hold right mouse; release to let go | Hold LT |
| Pull up / lower | Scroll wheel while attached | D-pad up/down |
| Fast descent / release ledge | S / Down | D-pad down |
| Crouch / roll | C / X | X / B |
| Aim drill | Mouse, or I/J/K/L | Right stick |
| Drill | Hold left click or E | RT |
| Survey map | M or Tab | Back |
| Pause (also saves) | Escape or P | Start |
| Save | F5 | Y |
| New expedition (title, paused or won) | R | LB |

Attach to rock, jump and swing, then reel to reach a shelf. Release the rope
to keep your momentum. The headlamp follows aim. Every connected crystal patch lights the scene,
and the pillars and shelves cast real shadows.

Your drilled terrain, sample inventory, diver position, rope and surveyed
map are saved in `saves/cave-explorer/run.txt`. Saving happens every 30 seconds,
on pause, F5/Y and normal window close. Scene loading currently restores the
suit's health/breath; it is not a full survival-state save. Death returns you
to camp after a short delay, retaining the run's terrain and sample count.

## Current starter kit

New expeditions use the full local rig when installed and a grappling rope,
with a shared game HUD, hand-mounted tools, bright mining preview and a
craftable gas collector. **Hold right mouse** for the rope, release to let go;
**scroll up/down** changes its length. **Space** jumps. See [Cave kit](CAVE-KIT.md) for the current controls
and the Lantern Cut editor playtest. The jetpack remains an optional later loadout and is preserved in old saves.

## Everything to edit

- `scenes/cave-expedition.txt`: the level, seed, cave settings, camp, lamps
  and player loadout. The player line sets walk and roll speed in metres per second. Open it in the editor; normal painting, lights, notes,
  undo and scene saving work. Start a new expedition to use a changed source
  scene rather than the previous saved terrain.
- `assets/cave-explorer/game.txt`: objectives, camp location and camera width.
- `assets/crafting/cave-tools.txt` sets drill reach, radius, power and grade. Material quantities are actual mined cells.
- `assets/input/cave-explorer.input`: keyboard and controller remapping.
- `assets/characters/explorer/`: original animated explorer look and editable
  animation graph. `tools/make_explorer_in_blender.py` rebuilds the helmet,
  suit, utility pack and drill from the engine's existing editable hero rig.
  It also writes a local `.blend` for sculpting; that file is not distributed.
- The original `caves.txt` and `endless-caves.txt` now use the same explorer,
  rope and headlamp in editor Play mode. **P** pauses; the wheel reels while right mouse is held.

The starter traversal is reusable: `player ... rope rope_range 160
rope_elasticity 0.06 headlamp 125`. Focused surface mining, inventories and
material tools are shared by editor Play and the standalone game.

## Verification

`cargo test --release --test explorer_movement` covers sustained flight,
fuel/recharge, safe descent, snapshots, headlamp and excavation protection.
The input test covers a complete press/release arriving in one render frame.

`cargo test --release --test cave_kit` covers the new starter rope, gas tank
and hand attachments. The editor tests exercise actual pointer input, mining,
crafting, mission return and reset. Native checks cover the game HUD, mining
highlight, weapon pose, rope attachment and workshop screen. Earlier flight
autopilot results do not validate a winning route with the new rope kit;
human playtesting and balance remain to be done.

## Mining in editor Play mode

Open any scene with a player and press Play. Leave **Player tools** checked
in the toolbar. Aim inside the level canvas and hold **left mouse / E** to
drill; I/J/K/L or the controller right stick can aim too, with RT drilling.
Hold right mouse for the rope; scroll changes its length, P pauses, and Reset restores the
pre-play terrain. The cutter reaches three metres. Highlighted pixels show the
exposed material under the crosshair. Holding fire with no target shows “No reachable rock”. Mining uses the same focused surface drilling and material hardness rules
as DEEPWARD, and feeds the scene's crystal mission inventory. See the
[mining trial](MINING-TRIAL.md) for a small cavern designed to test the feel.

Pause to select/paint normally, or turn **Player tools** off to use editor
tools while simulation continues. Clicks outside the canvas do not mine.

## Full mission testing in the editor

Keep **Player tools** and **Crystal mission** checked in the toolbar, then
press Play. Mine cyan and violet crystals until both quota bars fill, follow
the signal to the amber camp ring, and enter it alive to complete the mission.
The editor continues simulating after success so you can keep testing.
The cave examples include this mission; the small caves demo includes two
accessible seams so both sample colours are available.

Pause/resume retains progress. Reset returns terrain and progress to the
Play reset point. Rewind restores sample counts with the corresponding
terrain, preventing duplicate rewards. Scene Save/Load and engine updates
carry mission progress too. The editor does not read or overwrite the
standalone game's save file.

Mission definitions are editable scene data: `mission HOME_X HOME_Y RADIUS`
followed by `sample MATERIAL TARGET COLLECTED` for each requested material.
Targets, counts and completion are saved. Explicit scene goals take priority
in the standalone game too; its tuning file remains the fallback for scenes
without a mission. Both use the same `Mission` collection/readiness/return
rules. With no mission defined, ordinary editor scenes behave as before;
checking **Crystal mission** adds the two default goals at the player's spawn.

## Materials, tools and workshops

B opens [storage and workshops](CAVE-WORKSHOPS.md); F cycles built tools.
The same inventory, crafting and material-tool rules work in editor Play.
Every drilled terrain cell is stored under its material. Crafting and ammo
spend the recorded quantities, and spending crystals lowers carried mission
quotas until you mine replacements. Tool tuning and recipes are in
`assets/crafting/`; `assets/cave-explorer/game.txt` sets the expedition scene,
home, targets and view. Use the small Mining Trial for the authored workshop
progression; generated caves now include mineable iron ore seams.

## Elemental cave guards

[Crawlers](CAVE-CREATURES.md) guard the authored bonus gallery. They patrol
floors, walls and ceilings, investigate noise and warn before biting. Oil
and acid variants spill real material on death; other variants are in the
elemental gallery. Their state and spent death payloads are saved with the
scene. Editor Play uses the same brains, movement and material reactions.
