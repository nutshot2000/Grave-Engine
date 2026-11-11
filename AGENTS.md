# AGENTS.md: a guide for AI agents working on Grave Engine

Read this first. It explains what Grave Engine is, how it works, where
everything lives, how to check your work, and how to work with its owner.
`README.md` is the user-facing tour; `materials/README.md` lists every
material key. Each source file starts with a doc comment explaining itself.

## The owner, and how to work with them

- They make games by "vibecoding": they are **not a programmer**. Explain in
  plain, friendly words: what changed, what they'll see, what to try. No
  jargon, no code snippets unless asked. Short lists beat walls of text.
- They test in the editor and leave feedback like "sand should be more
  absorbent" or "wood should look like wood grain". Treat those as design
  requests: make the change general (a material key or engine rule), not a
  special case for one scene.
- **They leave notes in the editor for you** (see *Notes* below). "Do my
  notes" means: read `scenes/*.notes.md`, look at each note's picture, do
  what it asks, tick it `[x]`, and add a short "(done: ...)" to its text.
- Keep Grave Engine separate from their other game, Rendgrave
  (`C:\Users\batti\Desktop\pixel-arpg`): never touch that folder.
- Commit in small steps with clear messages and push to `origin main`
  (GitHub: nutshot2000/Grave-Engine, **public**). Don't commit third-party
  asset packs (see *Licences*).

## The vision

One reusable 2D engine for **any kind of game** (racing with metal and
rubber tyres, shooters, action RPGs, platformers), where every material
works on every surface. Everything is **generic and data-driven**: materials,
scenes, templates and characters are plain text files that people and AI
agents can read and write. No special cases ("fire burns wood" is not in the
code; wood burns because its `catches_fire_at` is reached while touching
something that `feeds_fire`). Inspired by Noita (cell simulation + rigid
bodies), with a game layer on top.

## Quick start

```bash
cargo build --release
cargo test --release                       # ~110 tests, ~10 s
./target/release/grave-engine scenes/player.txt         # the editor (window)
./target/release/grave-engine --run scenes/player.txt --frames 600 --walk 1 --jump --player-view 160 --scale 6 --shot shots/out.png
tools/check.sh        # or tools\check.ps1: build + all tests + a screenshot of every scene
```

Rust 2024 edition (toolchain 1.95+). Windows is the main platform; it also
builds on Linux/macOS.

**Gotcha:** if the editor is open, Windows locks `target/release/grave-engine.exe`
and `cargo build --release` fails with "Access is denied". Close it first
(`Stop-Process -Name grave-engine` in PowerShell). Tests use other files
and still run.

## Headless mode: check your own work

Always verify changes by running scenes headless and **looking at the
screenshots** (they're PNGs: open them). The report also prints objects,
the player's state, and cells per material (counts include flying cells).

| Flag | What it does |
| --- | --- |
| `--run SCENE` | simulate a scene without a window |
| `--frames N` | how many frames (60 = one second) |
| `--shot PATH` / `--every N` | screenshot at the end / every N frames (`out_00060.png`...) |
| `--scale S` | bigger screenshots (characters are drawn sharp at this size) |
| `--player-view N` | screenshot only N cells across around the player |
| `--heat` | screenshot the heat view |
| `--walk W` / `--jump` | hold the player's controls (W = -1..1) |
| `--throttle T` | hold a car's drive motors (-1..1) |
| `--materials DIR` | another materials folder (handy for A/B looks) |
| `--make-character DIR --name N` | turn animated FBX files (e.g. Mixamo) into a character look |

Exit code 3 = it ran but material or scene files have problems (printed).

## Where things live

| Path | What |
| --- | --- |
| `src/world.rs` | the cell grid: heat, changes (melt, burn, react), movement (powder, liquid, gas), air currents, **flying cells** |
| `src/material.rs` | material files, ids, **your settings** layered over defaults (`my_settings/`) |
| `src/physics.rs` | rigid objects (Rapier 2D): read back, collapse, terrain colliders, joints, impacts, explosions, bounce pads, one-way powder |
| `src/physics/bend.rs` | metal objects bend: a hard hit levers the object round its nearest weld/hinge (or middle); if force × lever beats `yield` × thickness² × depth × firmness (heat), the far side's cells are redrawn turned (joint anchors there move too); a huge hit or > ~70° in total tears it in two (bigger half keeps the id). `BEND_K` is the tuning (a bike fork bends in a 250 km/h wall crash, not on landings). `Physics::bent(id)` = total bend |
| `src/physics/vehicle.rs` | vehicle controls: brakes, rolling resistance (material key), balance assist (`upright`) and lean, `wheel_contact`, tyre roost (`kick_up`: rear driven tyre flings top-layer loose cells back and up; only the rearmost tyre, or a front tyre's ruts trip the rear one) |
| `src/trigger.rs` | triggers: scene boxes that fire actions (heat, message, checkpoint, score) when something enters; `Sim::events` |
| `src/bin/stunt-yard.rs` | **Stunt Yard / Wreck Run**, the stunt-bike game built on the engine (`Play-Stunt-Yard.cmd`); course from `tools/make_stunt_course.py`; docs in `docs/` |
| `src/physics/actor.rs` | characters (the player): movement, swimming, breathing, health, death, animation choice, body cells |
| `src/sim.rs` | `Sim` = world + physics + template links + camera follow; `Snapshot` for reset/undo/rewind/save |
| `src/scene.rs` | scene text format: load and save (format reference at the top of the file) |
| `src/edit.rs` | editor operations: regions, clipboard, transforms, undo `History`, rewind `Timeline` |
| `src/template.rs` | reusable objects (`templates/*.txt`), copies that update when the template changes |
| `src/import.rs` | images, glTF/GLB, OBJ, FBX and .blend (via Blender), material picking by name/texture/colour, `make_character` |
| `src/look.rs` | character looks: animation frames + `character.txt`, outline masks |
| `src/notes.rs` | sticky notes: `.notes.md` format, what a spot is (`describe`), notes following objects |
| `src/render.rs` | cells to RGBA pixels (CPU), textures, glow, heat view, hidden cells |
| `src/headless.rs` | headless runs, screenshots with sharp characters |
| `src/app.rs` | the editor (egui): tools, inspector, right-click menu, notes, material settings, camera, HUD, UI tests |
| `src/main.rs` | command line |
| `materials/*.txt` | one file per material (keys in `materials/README.md`); `textures/` tiles + `CREDITS.md` |
| `materials/my_settings/` | the owner's own tweaks (git-ignored): only changed keys, over the defaults |
| `scenes/*.txt` | example scenes (see *Scenes*); `*.notes.md` + `*.notes/` = notes and their pictures |
| `templates/*.txt` | crate, plank, cart |
| `assets/` | models, images, `characters/` (looks), `motions/` (Kimodo raw motions), `imported/` (from old projects) |
| `tools/` | Blender scripts, converters, `check.sh` / `check.ps1` |
| `tests/*.rs` | integration tests (one file per area); UI click-through tests are at the bottom of `src/app.rs` |

## How a frame works

Units: **1 cell = 10 cm** (10 cells per metre), **60 frames per second**,
**y grows downwards**. Temperatures in °C, densities in kg/m³.

`Sim::step` = `World::step` then `Physics::update`:

1. **World** (`world.rs`): heat spreads between neighbours (each edge
   conducts as well as the worse side); then *changes* per cell (thresholds:
   melt/boil/freeze/condense; lifetime; explosives; burning; one reaction
   with a random neighbour); then *movement* (powders fall and slide
   (`slides`), liquids flow, gases rise, heavier sinks through lighter);
   then *flying cells* (thrown by blasts/splashes, fly in arcs, knock loose
   stuff, land back as cells).
2. **Physics** (`physics.rs`, Rapier 2D): characters' and objects' cells
   are lifted out of the grid (reading back heat/damage; objects that lost
   cells split into pieces); unsupported scenery collapses into objects;
   terrain colliders are rebuilt for changed 32x32 chunks; buoyancy; bounce
   pads; characters think (controls, damage, breath); the Rapier step;
   everything is stamped back into the grid (pushing liquid/sand aside,
   splashing); impacts crack things by `toughness` (hot metal less:
   `softens_at`) and bend metal objects (`yield`, bend.rs); explosions
   throw, heat and crack.

Key ideas:

- **Objects are cells too.** Rigid objects are groups of grid cells marked
  `OWNED`, so heat, fire and melting work on them like on scenery. Read back
  every frame, stamped again after moving.
- **Characters** are a capsule in Rapier plus a body made of `flesh` cells
  (shape = their animation frame's outline, cells flagged `HIDDEN` when they
  have a look, so only the art shows). Health falls from heat/cold per body
  cell, no air at the head (breath), hard knocks, blasts.
- **Loose powder is one-way** for characters (a Rapier contact hook,
  `OneWayPowder`): it holds them up from below only.
- **Determinism:** the same scene gives the same result. Use `BTreeMap`
  (not `HashMap`) wherever iteration order affects the simulation, and the
  world's own random number generator. A test checks this.
- **Snapshots** (`Sim::snapshot` / `Sim::restore`) carry the world (without
  owned cells), objects, template links, joints (by object index + 1, 0 =
  ground), camera follow, characters. Used by Reset, undo, rewind and save.
- Object ids restart in each restored physics world; `Physics::run_id()`
  tells worlds apart (notes use it to re-attach).

## Data formats (all plain text)

- **Materials:** `materials/NAME.txt`, `key = value` lines; every key is
  documented in `materials/README.md`. Live-reloaded by the editor.
  `reaction = water -> wet_sand + air, chance 0.3` means: touching water,
  I become wet sand and the water becomes air. One reaction per material
  touched (a later line replaces an earlier one).
- **Your settings:** `materials/my_settings/NAME.txt` holds only changed
  keys, applied after the material's own file. `Materials::defaults` keeps
  the untouched values; the editor's material window writes these files.
  Delete a file = back to defaults.
- **Scenes:** `scenes/*.txt`. Commands: `size`, `seed`, `wind`,
  `turbulence`, `ambient`, `fill`, `rect`, `circle`, `line`, `heat`,
  `legend` + `grid`, `body`, `object FILE x y [scale S] [static] [material M]`,
  `place TEMPLATE x y [angle A] [material M] [flip]`, `name`, joints
  (`hinge`, `wheel`, `weld`, `rope`, optional `break KN`), `follow`,
  `upright NAME strength S` (balance assist), `player x y [look L] [zoom Z]`,
  level shapes `slope`, `hill`, `curve`, `poly` (filled below, optional
  `to Y` and `cover MAT DEPTH`), `anchor x y w h` (pinned scenery), and
  `trigger X Y W H [by NAME|player|any] [repeat] : heat circle|rect ... ;
  message TEXT ; checkpoint [X Y] ; score N`.
  Full reference at the top of `src/scene.rs`. Shapes outside the world are
  clipped; sizes are capped (agents' typos can't hang the engine).
- **Templates:** `templates/NAME.txt`, a tiny scene whose first object is the template.
- **Character looks:** `assets/characters/NAME/` with `idle_00.png...`,
  `walk_..`, `jump_..`, `fall_..`, `swim_..`, `die_..` and `character.txt`
  (`figure`, `feet`, `faces`, per-animation `N frames, F fps, loop`).
  Missing animations fall back to idle. Reference: top of `src/look.rs`.
- **Notes:** `scenes/NAME.notes.md`, one line per note:
  `- [ ] at X,Y | on: WHAT | picture: NAME.notes/note-N.png | TEXT`.
  `on:` says exactly what was clicked (object index in the scene's body
  list, materials, size, centre, template; or the scenery material, or
  empty space). The picture is a close-up with the object outlined in
  yellow and a red pin at the spot. The editor reloads ticks live.

## The game project: Stunt Yard / Wreck Run

The owner and their agents are building a stunt-bike game (`src/bin/stunt-yard.rs`,
`Play-Stunt-Yard.cmd`, docs in `docs/WRECK-RUN.md` and
`docs/STUNT-YARD-FEEDBACK.md`) **to find out what the engine lacks, then fix
it in the engine**. Rule: when the game needs something general, build it in
the engine (with a test and docs) and make the game use it, rather than
writing more game-only code. Done so far from its wish list: vehicle
controls (brakes, rolling resistance, balance), level shapes and anchors,
triggers and checkpoints. Next on the list: a physical rider and ragdoll,
sound, active simulation regions, regression scenes, contact inspector.
`cargo test --release --bin stunt-yard` runs its 9 tests (~1 minute;
includes riding the whole course).

## Workflows

### Do the owner's notes
1. Read every `scenes/*.notes.md`; open each unticked note's picture.
2. Make the change. Prefer a general solution (a new material key or rule)
   over a one-off hack; edit the scene file for placement. The editor
   reloads the scene and materials live when files change on disk.
3. Check it headless (screenshots, the player report).
4. Tick it: `- [x] ... | TEXT (done: what you did)`. Commit and push.

### Add or change a material
Write `materials/NAME.txt` (copy a similar one). Run any scene: material
problems are printed with file and line. Texture tiles go in
`materials/textures/` (small seamless tiles, one pixel per cell, credit them
in `CREDITS.md`). For a new behaviour, add a key in `material.rs` (`Material`
field, default in `blank`, parse in `apply`), use it in `world.rs` or
`physics.rs`, document it in `materials/README.md`, add a slider in
`material_settings_window` in `app.rs` if it's tweakable, and test it.

### Characters
- **The hero** (default look): built and animated in Blender by
  `tools/make_hero_in_blender.py` (empties as joints; renders 256 px frames).
- **From Mixamo or any rigged FBX:** one FBX per animation in a folder, then
  `grave-engine --make-character FOLDER --name NAME` (runs Blender in the
  background with `tools/fbx_to_character.py`; faces right, removes root
  motion). File names pick the animation (die/swim/fall/jump/walk/idle,
  most specific first: "Falling Idle" is a fall).
- **From sentences with NVIDIA Kimodo** (installed by Codex at
  `C:\Users\batti\Documents\Codex\2026-10-20\https-www-youtube-com-watch-v\outputs\kimodo-cpp`):
  `build/Release/kmd-generate.exe MODEL.gguf TEXT_ENCODER.gguf prompt.txt FRAMES STEPS SEED OUT_DIR`
  (see its `Generate-Motion.ps1`; ~1 minute per 4 s clip on the CPU) into
  `assets/motions/NAME/`, then
  `blender -b --factory-startup -P tools/kimodo_to_character.py -- assets/motions assets/characters/kimodo`.
  Human skeleton only (SOMA 30 joints). Models are ~6 GB and not part of the
  engine; NVIDIA Open Model License + Meta Llama 3 licence apply to the model.

### Blender
Blender 5.2 CLI: `C:\Program Files\Blender Foundation\Blender 5.2\blender.exe`
(or set `GRAVE_BLENDER`). The owner often has Blender open with a
"blender-mcp" add-on on `127.0.0.1:9876`: send JSON
`{"type": "execute_code", "params": {"code": "..."}}` over a raw TCP socket
and read until the reply parses as JSON. Build in a separate scene so their
work is untouched. Prefer the CLI in the background for batch work.

### Imports
Drop files on the editor, or `object FILE x y` in a scene. A part named
after a material becomes it; else its texture's colours, else its colour,
pick the nearest material. Metallic parts (glTF/Blender Metallic, OBJ `Pm`)
only pick `metal = yes` materials. FBX and .blend go through Blender.
Unreal `.uasset` can't be read: export to FBX from Unreal.

## Testing

- `cargo test --release` runs everything. Areas: `sim` (cells, heat,
  fire), `physics` (objects, breaking, explosions, determinism), `joints`,
  `editor` (undo, rewind), `templates`, `imports`, `flying`, `player`
  (movement, health, looks, bounce pads, burial), `wet_sand`, `my_settings`,
  `robust` (silly input), plus UI click-through tests in `src/app.rs`
  (egui_kittest: real mouse and key events).
- Write a test for every behaviour you add, in the matching file. Tests load
  the real `materials/` folder, so material changes can break them: that's
  intended (check the change was meant).
- `tools/check.sh` / `tools\check.ps1`: build + tests + a screenshot of every
  scene into `shots/check/` (git-ignored). Look at them.

## Conventions and gotchas

- Rust, small focused functions, doc comments in plain English at the top of
  every file and on public items. Comments explain *why*.
- Keep it general and data-driven; no material names in engine code except
  sensible fallbacks.
- `*.png` is git-ignored except `assets/**`, `materials/textures/`, `docs/`.
  Note pictures and `shots/` aren't committed.
- On Windows the Bash tool's heredocs choke on some quoting: for multi-line
  edits, write a small Python patch script, or use an editor tool.
- Line endings: files are LF in git (git warns about CRLF; harmless).
- Performance: ~3-4 ms per frame at 768x288 on the owner's PC. Big worlds
  need chunked updates (roadmap).

## Licences

- Code: the owner's.
- Material tiles: "Textures" by piiixl (https://piiixl.itch.io/textures):
  free for commercial use, **may not be redistributed as-is**. Only the
  tiles in use are in `materials/textures/` (credited). The pack itself
  stays in the git-ignored `/textures/` folder: never commit it.
- `assets/imported/`: the owner's own old projects (rock models from their
  Godot project, the "Cascade Engine" machine from their Blender file).
- Don't import Epic's Unreal sample content (StackOBot, templates): Unreal-only licence.
- Kimodo-made motions are the owner's output; the model isn't shipped.

## Known limits and what's next

Known limits:
- Wood grain always runs the same way (even along horizontal beams).
- Re-rendered character frames need an editor restart (looks are cached).
- All characters follow the same controls (no enemies yet); the camera
  stays locked on the player while playing.

Roadmap (see the end of `README.md`): game layer (enemies, triggers,
scripting), big worlds (chunked, multithreaded), coatings (wet/oily
surfaces), projectiles and a top-down mode (shooter test), material sounds,
an agent command channel for the open editor, level-design guard rails.
