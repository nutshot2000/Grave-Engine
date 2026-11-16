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
| Jump / grab / climb | W / Up | A |
| Jetpack | Hold Space | RT |
| Fast descent / release ledge | S / Down | B |
| Crouch / roll | C / X | X / RB |
| Aim drill | Mouse, or I/J/K/L | Right stick |
| Drill | Hold left click or E | LT |
| Survey map | M or Tab | Back |
| Pause (also saves) | Escape or P | Start |
| Save | F5 | Y |
| New expedition (title, paused or won) | R | LB |

Release thrust to recharge. Landing recharges faster; the pack limits
passive descent, so you can glide and recharge in large shafts. The helmet
lamp follows the explorer. Every connected crystal patch lights the scene,
and the pillars and shelves cast real shadows.

Your drilled terrain, sample inventory, explorer position/fuel and surveyed
map are saved in `saves/cave-explorer/run.txt`. Saving happens every 30 seconds,
on pause, F5/Y and normal window close. Scene loading currently restores the
suit's health/breath; it is not a full survival-state save. Death returns you
to camp after a short delay, retaining the run's terrain and sample count.

## Everything to edit

- `scenes/cave-expedition.txt`: the level, seed, cave settings, camp, lamps
  and player loadout. Open it in the editor; normal painting, lights, notes,
  undo and scene saving work. Start a new expedition to use a changed source
  scene rather than the previous saved terrain.
- `assets/cave-explorer/game.txt`: objectives, camp location, drill size/reach
  and camera width. Material quantities are actual mined cells.
- `assets/input/cave-explorer.input`: keyboard and controller remapping.
- `assets/characters/explorer/`: original animated explorer look and editable
  animation graph. `tools/make_explorer_in_blender.py` rebuilds the helmet,
  suit, utility pack and drill from the engine's existing editable hero rig.
  It also writes a local `.blend` for sculpting; that file is not distributed.
- The original `caves.txt` and `endless-caves.txt` now use the same explorer,
  pack and headlamp in editor Play mode. **P** pauses; Space flies.

The engine additions are reusable: `player ... jetpack flight_speed 6.5
jet_capacity 6 headlamp 125`, plus `Sim::dig_circle`, which reports real
removed material and preserves owned cells/liquids. They are not special
cases tied to this game or to a particular rock.

## Verification

`cargo test --release --test explorer_movement` covers sustained flight,
fuel/recharge, safe descent, snapshots, headlamp and excavation protection.
The input test covers a complete press/release arriving in one render frame.

`cargo run --release --bin cave-explorer -- --verify` runs an automatic
expedition using flight and drilling controls (no position warps), mines both
sample types, and returns to camp. It writes screenshots in `shots/` and
reports whether the expedition was won. It does not write the player's save.
The first verified route crossed over 40 metres from camp, mined thousands
of real cells, collected both sample types and returned at full health.
Native window checks also covered the title/start, HUD, map, pause and save.
This does not establish long-term balance or every possible generated seed.

## Mining in editor Play mode

Open any scene with a player and press Play. Leave **Player tools** checked
in the toolbar. Aim inside the level canvas and hold **left mouse / E** to
drill; I/J/K/L or the controller right stick can aim too, with LT drilling.
Space flies with the explorer loadout, P pauses, and Reset restores the
pre-play terrain. A ring shows the drill's fixed reach: clicking a distant
wall does not mine it remotely. Mining uses the same radius/reach and engine
excavation rule as DEEPWARD, and feeds the scene's crystal mission inventory.

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
