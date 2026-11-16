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
  (the sibling folder `..\pixel-arpg`, next to this one): never touch it.
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
                      # (editor or a game open? CARGO_TARGET_DIR=target/side tools/check.sh builds elsewhere;
                      #  delete such folders when done: each is ~1.5 GB)
```

Rust 2024 edition (toolchain 1.95+). Windows is the main platform; it also
builds on Linux/macOS.

**Gotcha:** if the editor is open, Windows locks `target/release/grave-engine.exe`
and `cargo build --release` fails with "Access is denied". Close it first
(`Stop-Process -Name grave-engine` in PowerShell). Tests use other files
and still run.

## The live editor (keep it open)

The owner keeps the editor open while you work and watches changes arrive:
- It runs from a copy (`target/release/editor-copies/`), so **build normally**
  (`cargo build --release`, plain `target/`; no separate target folders) even
  while it's open. When `grave-engine.exe` is rebuilt it restarts itself into
  the new build within a few seconds, keeping the scene, its file name, notes and camera
  (it waits while the scene is playing, during a drag or a note, or while the
  asset workshop is open). `--no-copy` turns this off.
- Content reloads live with no restart: the open scene file, materials,
  templates, notes, asset files the scene places (`asset FILE`), and the asset
  open in the workshop (if it has no unsaved changes).
- So: edit a scene/asset/material file and the owner sees it at once; change
  Rust code, build, and the editor updates itself. Open it with `Play/Editor.cmd`.
- **Follow mode** (👁 Follow AI in the top bar, on by default): when a scene
  (`scenes/*.txt`) or workshop asset (`assets/workshop/*.asset`) changes on
  disk and isn't already on screen, the editor opens it (one Undo goes back).
  Off: it offers a "Go there" button instead. **Tell the owner what you're
  doing** by writing `live/agent.txt` (git-ignored) as you start each piece:
  `say Building a windmill`, `open scenes/x.txt` or `open assets/workshop/y.asset`,
  `look X Y` (point the camera), and before building the editor
  `update WHAT CHANGED` (shown as "✨ Engine updated: ..." after it restarts).
  Rewrite the whole file each time. Everything lands in the editor's
  **🤖 Activity** panel (kept in `live/activity.txt`), and areas of the open
  scene that change on disk get a fading yellow outline.
- The games (`Play/*.cmd`) still lock their own exe while running: ask before
  rebuilding a game that's open.

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

Top level: `src/` (engine, editor in `app.rs`, games in `bin/`), `materials/`, `scenes/`, `assets/` (workshop assets, characters, models), `templates/`, `docs/` (guides; index in `docs/README.md`), `tools/` (level/asset generators, Blender and character scripts, `check.sh`), `tests/`, `examples/` (level builders), `Play/` (double-click launchers: each builds first, then runs). Build into `target/` only; don't leave extra target folders behind.

| Path | What |
| --- | --- |
| `src/world.rs` | the cell grid: heat, changes (melt, burn, react), movement (powder, liquid, gas), air currents, **flying cells** (light ones drift on `flow_at`: embers, ash, snow; `spawn_flyer` for new ones), **embers** (burning cells with `sparks` throw them). **Sleeping areas**: 32x32 blocks with an awake countdown; only blocks awake or next to one are worked on. Anything that writes a cell must wake it: world helpers (`become_`, `make_air`, `swap`, `land`, `heat_circle`) do; direct writes elsewhere call `world.wake_at(i)` (physics stamping, explosions, triggers, editor `put`; `History::touch`, scene load and `Sim::restore` call `wake_all`). Physics skips terrain/collapse work in `block_still` blocks, and `snap_overhangs` (which scenery hangs too far) only re-checks boxes around awake blocks padded by the strongest material's reach (`snap_regions`/`snap_region`); a box's edge counts as ground only where that structure really connects to the world's ground or edge (flood-filled: floating islands must not look supported; `tests/sleep.rs` guards this). `world.sleeping = false` turns it off (tests compare). After `wake_all` (load, reset, undo), calm blocks (only air and plain solids: no reactions, no objects, nothing burning) drop to `CALM_FRAMES` (17, so physics' 16-frame crumb sweep still sees them) instead of 60 (`settle_calm`): big worlds don't crawl after loading. **Heat** is shared between CPU cores (`run_bands`, bands of block rows) when more than `PARALLEL_CELLS` are awake; each cell reads only last step's temperatures, so the result is bit-identical (`tests/big_worlds.rs`). Still air with nowhere to rise skips its dice in `move_gas`. **Movement** (powders, liquids, gases: `world/movement.rs`) is worked on in 64x64 patches in four checkerboard rounds (Noita's method): patches in a round are 64 apart and nothing reaches more than `REACH` (17) cells, so cores never meet; each patch rolls its own dice (`patch_seed`: frame roll + position), so results are identical on any number of cores (`set_worker_threads(1)` to compare; `tests/big_worlds.rs` checks it). Workers use raw pointers (`Grid`): any new per-cell layer that moves with a cell must be added to `Mover::swap` and `World::swap`. **Changes** (melt/boil/freeze, running out, explosives, fire, reactions: `world/changes.rs`) use the same patches and rounds (`patch_rounds`); a cell touches only itself and its 4 neighbours. Blasts and sparks go in each patch's `Outbox` and are added afterwards in patch order, so they're identical on any core count too. `Changer::become_` must match `World::become_`. Slow reactions (chance < 0.02 per frame: mud/wet sand drying) don't keep an area awake: their block is visited every 16 frames with the chance scaled up 16x |
| `src/material.rs` | material files, ids, **your settings** layered over defaults (`my_settings/`) |
| `src/physics.rs` | rigid objects (Rapier 2D): read back, collapse, terrain colliders, joints, impacts, explosions, bounce pads, one-way powder |
| `src/physics/bend.rs` | metal objects bend: a hard hit levers the object round its nearest weld/hinge (or middle); if force × lever beats `yield` × thickness² × depth × firmness (heat), the far side's cells are redrawn turned (joint anchors there move too); a huge hit or > ~70° in total tears it in two (bigger half keeps the id). `BEND_K` is the tuning (a bike fork bends in a 250 km/h wall crash, not on landings). `Physics::bent(id)` = total bend |
| `src/physics/vehicle.rs` | (balance styles: `Balance { strength, air, heavy }`; `air` < 1 = trials: on the ground follows the slope under the wheels (`ground_under`) with lean as weight shift; in the air lean spins it; `heavy` = gravity scale on the assembly)  vehicle controls: brakes, rolling resistance (material key), balance assist (`upright`) and lean, `wheel_contact`, tyre roost (`kick_up`: rear driven tyre flings top-layer loose cells back and up; only the rearmost tyre, or a front tyre's ruts trip the rear one) |
| `src/trigger.rs` | triggers: scene boxes that fire actions (heat, message, checkpoint, score, finish, lighting, sun (moves the sun over time), and camera: slowmo, zoom, focus [it/X Y], shake, camera reset) when something enters; `Sim::events` |
| `src/weather.rs` | weather: `Weather` settings on `Sim` (snow/rain/hail/dust/fog/gusts 0..1, lightning per minute, master `on`, `fades` for triggers) and `WeatherRun` (flash, bolt, strike timer). `step` (called in `Sim::step` right after `World::step`, before physics so strikes can add `Blast`s) spawns flying cells just above `Sim::weather_view` (the screen area, set each frame by the editor/games; else around the followed object; else the whole world), modulates `world.wind` for gusts (`gust_base` remembers the scene's wind: saved instead), and runs lightning: tallest of 6 sampled columns, 26-frame flicker warning, then bolt + flash + shake + heat + blast + sand to glass. Fog/bolt/flash are drawn by `render.rs` (`renderer.light(&sim)` picks them up). Scene lines `weather ...`; trigger action `weather KIND V [S]` / `weather clear [S]`; saved, undoable (☁ Weather menu in app.rs) |
| `src/backdrop.rs` | sky gradient + parallax layers (hills, mountains, city, forest, clouds, image) drawn at world resolution for the camera centre (`renderer.set_camera`): layer x = world x - camera x (1 - depth). Renderer shows it in plain AIR cells; lit by ambient + sun, not world shadows; lamp haze in front. `PRESETS` for the editor menu. Scene lines `sky`/`layer` |
| `src/light.rs` | lighting (sun shadows are swept per frame over just the slanted band that can reach the drawn window, from at most `MAX_SHADOW_ROWS` above it): `Lighting` on `Sim` (ambient level + tint, `Sun` swept row by row for shadows, `Light` lamps cast as rays with falloff, cones, flicker, `on` an object), `illuminate` (per-cell light; materials' `light_through()`), fades from the `lighting L S` trigger action. The renderer draws with it after `renderer.light(&sim)`; plain lighting = the old picture exactly. Saved as `lighting`/`sun`/`light` scene lines; in snapshots and undo |
| `src/sim.rs` depth planes | `Sim::layers` are the planes behind the front (assets' `layer` command). Heat crosses planes each step; blasts echo to the other planes one frame later, weaker with distance (`echo_blasts`) |
| `src/asset.rs` joints | Assets have rigid `weld`s and moving `joint`s (`hinge` = pivot, `motor` on a hinge, `wheel`, `rope`; second part may be `ground`). Always-on motors (no `drive`) use full strength through linkages (physics `MotorModel::ForceBased`); `drive` motors keep the tuned vehicle feel. Example: `oil_pumpjack.asset`. Guide: docs/ASSET-WORKSHOP.md |
| `src/pump.rs` | Stroke-driven liquid transfer: a body attachment's upward travel moves existing cells from an intake rectangle to a free outlet. Finite supply, preserved material state, dry/blocked protection and snapshot/save support. Scene `pump LIQUID drive NAME at X Y intake X Y W H outlet X Y W H rate N`. Iron Heron example: docs/IRON-HERON.md |
| `src/physics.rs` loads | Liquid resting on an object weighs it down where it rests (`loads`; not powders yet). Buoyancy needs water both sides, or water + a wall. Scene loading lifts objects out of scenery (an asset's parts together) and reports lifts over 2 cells as a scene problem. Example: Millbrook waterwheel (docs/MILLBROOK.md) |
| `src/input.rs` | **Controls for every game**: keyboard, mouse and game controllers (gamepads/joysticks, via gilrs) as named actions from a plain-text `.input` file (`assets/input/GAME.input`, embedded with `include_str!` as the default and re-read at start-up so players can remap). `Input::load_or`, then each frame `update(ctx)` and `pressed/down/released/value/axis("name")`. `update_from` feeds fake states for tests. Dust Hawk, Stunt Yard and the editor's Play mode (`assets/input/editor.input`) use it |
| `src/sound.rs`, `src/sound/play.rs` | **Sound from materials**, no sound files. The simulation notes cues on `World::sounds` (`Kind::Hit` from physics contacts whose push *jumps* (resting weight stays quiet), `Break` when objects split or scenery collapses, `Blast` per explosion, `Thunder` per lightning strike, `Land` for fast flying cells), each with the material, loudness 0..1 and size. Each material has a `Voice` (metal, wood, stone, glass, soft, earth, liquid, none: `sound =` in its file, else worked out by `voice()`). `sound::Player` turns cues into synthesised sound (modal sines for metal/wood/glass, filtered noise for stone/earth, bubbles for liquids, sub-bass + roar for blasts; bigger rings lower; slow motion drops the pitch) plus one endless stream for fire crackle (burning cells on screen), rain, hail and wind. Games call `player.listen(&mut sim, &mats, view, speed)` once a frame after stepping (speed 0 = paused: quiet); it always drains the cues. The editor (🔊 toolbar button, volume in View) and Stunt Yard use it; `GRAVE_NO_SOUND=1` or `Player::silent()` for tests. Tests: `tests/sound.rs` |
| `src/world/coating.rs` | **Wet and oily ground**: liquids that `leaves_film()` (not molten) leave a film on solid scenery they touch (`World::coat` = which liquid, `coat_until` = frame it's dry by, `FOREVER` = never). It dries over the liquid's `coat_lasts` seconds, worked out from the clock (`wetness`), so wet roads don't keep areas awake; still touching its liquid = soaked. Physics traces ground grip per (material, film, `coat_level` 3..0): grip = friction x `film_grip` (liquid's `coat_grip` share); `Chunk::recheck` re-traces a chunk when a drying film steps down. Flammable films (oil) burn off in a lick of flame when the ground or the air just above is hot (spreads along a slick); boiling ones vanish. A film is only replaced by the same liquid or once dry. Drawn as a glossy edge in the liquid's colour (slick films shimmer). Scene: `coat oil X Y W H [forever]` (exposed cells only), saved as runs. Tests: `tests/coatings.rs`; demo `scenes/slick-road.txt` |
| `src/physics/actor/soak.rs` | **Soaked characters** (Noita-style): `ActorSpec::soak` = `Soak { liquid, amount 0..1, burning }`. Liquid beside/under the body, or flying drops (rain, splashes), soak it; a dunk (`wet >= 0.5`) washes off a different liquid. Dries over the liquid's `coat_lasts / 3`; a hot body steams water off. `puts_out_fire` soak: heat damage x (1 - 0.6 x amount) (`heat_shield`); flammable soak (oil): lights from flames touching or a hot body, burns `BURNS_FOR` s at `FIRE_HURTS`/s with flying flames, doused by water. Good soakings drip (an oily trail can burn). Figure cells get the film look (`mark_cells` sets `world.coat`); the editor tints pictures, draws a drop/flame icon over the head and a HUD bar saying what it means. `Physics::edit_actor` lets games set it. Tests `tests/soaked.rs`; demo `scenes/soak-lab.txt` |
| `src/levelgen.rs` | **Level generator**: herringbone Wang tiles (Sean Barrett's method, as in Noita; https://nothings.org/gamedev/herringbone/). `layout(uw, uh)`: 2x1 and 1x2 bricks, staircases H at (k+4m, k), V at (k+4m, k+1), units of `tile` cells. Each brick has 6 edge pieces (`Brick::pieces`, shared keys); `piece_open` decides each once (seeded), so passages line up; area borders are sealed unless `open_sides`, plus a 2-cell solid rim. Bricks use a hand-made `Tile` that fits: exact edges first, else one with a single spare doorway onto rock (never one that blocks a needed passage). Tiles are text drawings in a folder (format: `tiles/rooms/README.md`; `legend # ground` = the level's rock; each tile also used mirrored unless `mirror no`). **The main set is `tiles/caves/`** (24 tiles, 48 with mirrors: halls, mine, bridge, pools, lava pit, vault, storeroom, waterfall, crystals, oil seep, steps, rockfall, shrine, ore den; shafts, well, tower, drop, alcove, cavern, zigzag), drawn by `tools/make_cave_tiles.py` on the built-in grid (16x8 / 8x16, doorways at 3-5 of each 8; mirrored ones shift one block, neighbours still share the middle two): edit tiles there and rerun it (it checks doorways and redraws `contact-sheet.png`); `tests/levelgen.rs` re-checks them. Editor Generate caves uses tiles/caves (checkbox). else a built-in tile (`carve`), drawn on a coarse grid (`GRID` 8 blocks per unit, like Sean's pixel tiles): 3-block openings at each open edge's middle, corridors 2-3 blocks wide (L-shaped or staircase) to a hub per half, rooms at hubs, dead-end branches, halves joined; `rooms` chance of a special room (brick vault with shelf, stone ring with ore, flooded room, lava pit, powder cache); `pools` puddles. Then `texture_ground` (noise patches of stone/sandstone, ore specks in passage walls) and `World::back` = `back` (the wall drawn behind caves). Generated solid ground is ANCHORED by default (Noita's lesson: real physics turns levels to rubble; `loose` turns it off). Objects/characters are left alone. Scene: `generate X Y W H [tile 64] [seed 7] [rock MAT] [open 0.6] [pools 0.2] [pool MAT] [rooms 0.25] [back MAT|none] [tiles DIR] [sides open] [loose]` (`Settings::new` = defaults); editor: Edit > Generate caves (Reroll). Tests `tests/levelgen.rs`; demo `scenes/caves.txt` |
| `World::back` | **Back walls**: a material per position (not per cell: it stays put when cells move), drawn dimmed behind empty/gas cells instead of the sky (render.rs; caves). Scene `backwall MAT X Y W H` (`none` clears), saved as runs. Purely visual |
| `src/endless.rs` | **Endless worlds** (Noita-style streaming). The world is a window onto an endless map in `region`-sized squares (256); `Endless::origin` = map cell at world (0,0). `update(sim, mats, focus)` each frame: within 2 regions of an edge, `prepare` generates the strip ahead on other cores (`ahead`); within 1, `scroll` one region **in place**: `save` lifts out the leaving strip's loose objects (nothing refers to them) with `Physics::remove`, keeps the region ready-made in memory (`cache`, last `KEEP`=48) and writes it as a scene file on a background thread (`saves/endless/<scene>/rX_Y.txt`); `Sim::shift` slides everything (World::shift: layers memmoved on several cores, sleeping blocks move along, only the gap is reset; physics/shift.rs: bodies and actors translated with velocity kept, stamped cell lists renumbered, ground-joint anchors moved, rock outlines moved chunk by chunk, chunks sliding off switched off and removed 64/frame (`graveyard`), new-strip chunks `lazy` (traced 48/frame); triggers, free lamps, pumps, checkpoint, camera focus point, weather view); `fill` copies in the prepared/remembered region (loads from disk after a restart). Returns the shift so cameras follow. A scroll of 1280x1024 is ~8 ms, the next frames a few ms over normal. Generation is position-only (`generate_at`; `tests/endless.rs` checks region-by-region == whole). Scene `endless seed N [tile 64] [tiles DIR] [region 256] [open] [pools] [rooms] [rock]` generates the start window (open sides); the editor reads it (`Endless::from_scene`), `new_run` clears saves on load/Reset (Reset also restores the start origin), streams in `stream_endless` after each step (clears undo/rewind), and carries a run across engine updates (`GRAVE_RESUME_ENDLESS`, `resume`). Saved regions keep shapes, not heat/fire. `Sim::shift(dx, dy)` is usable on its own (tests in `tests/endless.rs`). Demo `scenes/endless-caves.txt` |
| `src/camera.rs` | `Effects` (`Sim::camera`: slow motion, zoom, focus, shake, timed in sim frames so replays match; `time_scale()` scales how many steps games run per second) and `Camera` (smooth follow with look-ahead, applies effects). Editor and Stunt Yard both use them |
| `src/bin/stunt-yard.rs` | **Stunt Yard**, the stunt-bike game built on the engine (`Play/Stunt-Yard.cmd`). Courses are `LEVELS` (scene file + title; Wreck Run also has built-in stages/signs/finish; newer levels use triggers incl. `finish`). Trials controls (W/S/A/D). The bike's parts are found from the followed frame. `ai_controls` + `--verify-ai --level=N [--from=X] [--every=N]` = an automatic rider for checking levels (prints where it gets stuck). The editor shows a **🎮 Play as game** button for scenes listed in `game_level` (src/app.rs): add new courses there too. Courses from `tools/make_stunt_course.py` (Wreck Run) and `tools/make_mountain_run.py` (Mountain Run); docs in `docs/` |
| `src/physics/actor.rs` | characters (the player): movement, swimming, breathing, health, death, animation choice, body cells |
| `src/sim.rs` | `Sim` = world + physics + template links + camera follow; `Snapshot` for reset/undo/rewind/save |
| `src/scene.rs` | scene text format: load and save (format reference at the top of the file) |
| `src/edit.rs` | editor operations: regions, clipboard, transforms, undo `History`, rewind `Timeline` |
| `src/template.rs` | reusable objects (`templates/*.txt`), copies that update when the template changes |
| `src/import.rs` | images, glTF/GLB, OBJ, FBX and .blend (via Blender), material picking by name/texture/colour, `make_character` |
| `src/look.rs` | character looks: animation frames + `character.txt`, outline masks |
| `src/notes.rs` | sticky notes: `.notes.md` format, what a spot is (`describe`), notes following objects |
| `src/render.rs` | cells to RGBA pixels (CPU), textures, glow, heat view, hidden cells. `draw_window(x, y, w, h)` draws only that part (pick it with `view_window`: steady size, so textures are reused) (plus a 40-cell margin for glow): the editor, Stunt Yard and screenshots draw just the view (pixel-identical to a full draw; tested) |
| `src/headless.rs` | headless runs, screenshots with sharp characters |
| `src/app.rs` | the editor (egui): tools (Select drags objects; during Play letting go mid-swing throws them: `fling`, `MAX_THROW`), inspector, right-click menu, notes, material settings, camera, HUD, UI tests |
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
- **Sound never feeds back**: cues are only read by the player, so `Physics::heard` may be a `HashMap`. Add new kinds of sound as cues where the thing happens (`world.sound(Sound {..})`), not by polling state in the player.
- Object ids restart in each restored physics world; `Physics::run_id()`
  tells worlds apart (notes use it to re-attach).

## Data formats (all plain text)

For detailed asset authoring, first read **docs/AI-ASSET-BUILDER.md**. Prefer
JSON recipes with `tools/asset_builder.py --strict` over hand-writing grids.
Use `asset-doctor` to validate, simulate and screenshot a copy; run machinery
for complete cycles and inspect the images. Recipes live in `assets/recipes/`.
Keep material and surface paint separate, preserve named parts, and use the
live editor instructions above so the owner can watch.

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
`Play/Stunt-Yard.cmd`, docs in `docs/WRECK-RUN.md` and
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

### Controls for a game (keyboard + controller)

Never read keys or controller buttons directly in a game. Write
`assets/input/GAME.input` with one line per action, e.g.
`jump = key Space, pad A` or `walk = keys D A, stick LeftX, pads Right Left`
(all names: top of `src/input.rs`), then in the game:
`const INPUT: &str = include_str!("../../assets/input/GAME.input");`,
`Input::load_or("assets/input/GAME.input", INPUT)`, `input.update(ctx)` once a
frame, and ask `input.pressed("jump")`, `input.axis("walk")`. Give every action
a keyboard *and* a controller source. Show controller hints when
`input.controller()` is `Some`. `Input::update_from` with a hand-made
`PadState` tests controller play without a real controller.

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
- **The timber house** (`scenes/house.txt`, `assets/house/timber-house.glb`):
  built by `tools/make_house_in_blender.py` (a cutaway, parts named after
  materials; run it in Blender for a "Grave House" scene, or
  `blender -b --factory-startup -P tools/make_house_in_blender.py -- OUT.glb`).
  Imported scenery must touch the ground and every part must be held up
  (no long overhangs, no pixels joined only diagonally) or it falls when the
  first thing changes; `tests/house.rs` checks it stands.
- **From Mixamo or any rigged FBX:** one FBX per animation in a folder, then
  `grave-engine --make-character FOLDER --name NAME` (runs Blender in the
  background with `tools/fbx_to_character.py`; faces right, removes root
  motion). File names pick the animation (die/swim/fall/jump/walk/idle,
  most specific first: "Falling Idle" is a fall).
- **From sentences with NVIDIA Kimodo** (installed by Codex at
  `%USERPROFILE%\Documents\Codex\2026-10-20\https-www-youtube-com-watch-v\outputs\kimodo-cpp`):
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
  (movement, health, looks, bounce pads, burial), `materials` (every material complete; new ones behave), `metals` (softening, bending), `wet_sand`, `my_settings`,
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
- Performance: ~3-4 ms per frame at 768x288 on the owner's PC. Mountain Run
  (5600x760) runs ~6 ms once settled; the first 17 frames after loading
  are ~48 ms (everything awake). `GRAVE_PROFILE=1 grave-engine --run SCENE`
  prints where the time goes (`profile::scope` names: `cells.heat`,
  `cells.movement`, `physics.terrain`...). Heat, movement and changes (fire,
  reactions) use all cores; `cells.coatings` (wet films) is still one core
  (~6 ms only on the first frames after loading a huge world).

## Privacy of the public repo

The repo is public. Keep the owner's name, home folder and email out of it: use paths relative to the project (`Path(__file__)`), and note that Blender renders embed the source file's full path in PNG metadata and FBX files embed texture paths: strip them before committing (`tools/` scripts should write clean files). Never commit the third-party texture pack (`/textures/`).

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
- Films coat scenery and characters (soak.rs), not objects: crates and tyres
  aren't coated and don't carry a film onto dry ground. No electricity yet
  (wet + shock would fit soak.rs).
- Re-rendered character frames need an editor restart (looks are cached).
- Actors follow global controls unless ActorSpec.control_override or
  Physics::set_actor_controls assigns an independent controller. Games own AI
  navigation; the editor camera still follows the player while playing.

Roadmap (see the end of `README.md`): game layer (enemies, triggers,
scripting), endless worlds in the games (Stunt Yard etc.), projectiles and a top-down mode (shooter test), more sounds
(sliding and scraping, flowing water, engines), an agent command channel for the open editor, level-design guard rails.

### Cave shape and texture update

See `docs/CAVE-GENERATION.md`. Natural tile interiors are warped and rounded
with doorway collars kept exact; built features retain their original cells.
Only 38% of eligible bricks use authored landmarks; other bricks get seeded
layouts, with fewer hub rooms and more narrow branches. `rock` now uses an
original procedural texture, and generated stone patches use `shale`.
`tools/make_cave_textures.py` now writes only legacy experiments to `shots/`;
the active textures are generated artwork (see materials/textures/CREDITS.md).

Natural built-in caves now mostly use `organic_grid`: six plan families on
16 blocks per unit (meanders, caverns, branching trees, pillar loops,
chamber chains, fractured shafts). One fifth keep the older rooms.
`draw_cave_blocks` handles both grid resolutions. New plans must keep the
protected outer rim and write only the six declared doorway collars;
`cave_shape_tests` checks all door masks and plan variety.

Huge caverns are a separate map-coordinate pass (`large_caverns` and
`carve_large_caverns`) across multiple bricks. Candidate lists include
whole-brick padding: keep this or partial-region generation may choose
different authored tiles. Their conservative bounds select natural-only
bricks. `Settings::caverns` and `dead_ends` are exposed as editor sliders
and scene/endless keys `caverns` / `deadends` (0..1). `add_blind_branches`
rejects a passage that reconnects away from its mouth. Owned cells remain
excluded from all cave passes.

Cavern interiors: `LargeCavern::contains` defines the raw outline;
`cavern_rocks` samples it for actual roof/floor/wall attachments and plans
stalactites, floor pillars and ledges. Keep the open central travel band
and map-coordinate evaluation. All features are real anchored ground.
The 28 active texture surfaces are original generated artwork; prompts are
in `materials/textures/generated-prompts.json`, preview in
`docs/material-textures.png`, packaging in `tools/import_material_textures.py`.
Do not regenerate the old tiny swatches over the active set. Copper Creek
only writes its old wood fallback if the shared texture is missing.

Geological texture joins: `Material::texture_blend` (0..8 cells, default 0),
parsed and exposed in the material inspector. Rock/shale/stone/sandstone opt
in at 5. `render.rs` feathers only between opted-in solid scenery, stopping
at air/liquid and excluding paint/owned cells; heat view bypasses it.
`tests/texture_blending.rs` checks locality, unchanged material IDs, crisp
boundaries/objects, gaps and pixel-identical cropped output. No simulation
layer or material mixing is involved.

Cave atmosphere: `Settings::crystals` / `torches` (0..1, also scene/endless
keys). `cave_fixtures` creates editable painted crystal cells and wood/torch
cells around map-space caverns. Generator UI includes fixture densities and
ambient brightness; the latter uses normal undoable `Sim::lighting`.
`Material::light_radius` / `light_strength` / `light_flicker` are generic
material lamps, editable in the material inspector. `light::material_lamps`
groups connected patches, chooses a stable exposed source, and the renderer
adds them to normal shadow-casting lamps. No new World layer or persistent
light entity is needed; digging, save/load and endless cell streaming carry
the source automatically. Torch tips are cool decorative emitters. Docs:
`docs/CAVE-GENERATION.md`, `materials/README.md`; tests in lighting/levelgen.

### DEEPWARD / cave explorer prototype

`Play/Cave-Explorer.cmd` builds `src/bin/cave-explorer.rs`; its mission/save
rules are in `src/bin/cave-explorer/game.rs`. Scene: `scenes/cave-expedition.txt`;
tuning: `assets/cave-explorer/game.txt`; controls: `assets/input/cave-explorer.input`.
Docs: `docs/DEEPWARD.md`. Mine real cyan/violet cells and return to camp.
Native title/map/pause/save checks and `--verify` cover a complete control-
driven expedition; keep it a prototype, not a claim of finished balance.

Reusable engine features: `ActorSpec::jetpack: Option<Jetpack>` (rewindable
fuel), `Controls::thrust`, personal `headlamp`, scene player keys `jetpack`,
`flight_speed`, `jet_capacity`, `jet_fuel`, `headlamp`; `Sim::dig_circle`
removes diggable terrain, wakes it and returns per-material counts, without
removing owned cells/liquids. Tests: `tests/explorer_movement.rs`.
The editor reads thrust from editor.input; Space flies in an active jetpack
scene, P/controller Start pauses. Non-explorer Space behavior is preserved.
`Input::pressed` retains non-repeat key/mouse press events even when released
before the next frame; held movement remains state-based. Test in input.rs.

Explorer look: original Blender character derived from the editable hero rig,
with helmet/visor, pack and drill; `tools/make_explorer_in_blender.py` builds
66 clean PNG frames plus a local ignored .blend. The animation graph supplies
fly/drill alongside normal locomotion. Strip PNG metadata after Blender
rendering (the script does this) and never commit local .blend source paths.
Keep saves under ignored `saves/cave-explorer/`; verify mode must not alter a
player's save. Save files contain terrain, samples, fuel/position and surveyed
map; health/breath restore through current scene defaults.

Editor mining: `player_tools` defaults on for player scenes. `explorer_play`
gates the canvas's early player-input branch, bypassing selection/painting/
notes while playing; the toolbar checkbox restores live editing. Named
editor.input actions drill/aim_x/aim_y feed a canvas-scoped drill latch and
world-space aim. `step_once` snapshots before drilling, uses `Mining::strike`
at 10 Hz, then records the result for rewind; Reset restores original terrain.
E's turn shortcut is suppressed in player Play mode. The UI test checks real
mouse mining without a selection box, Reset and paused selection.

Shared collection missions: `src/mission.rs` defines generic per-material
`Goal`s and a return-to-home `Mission`. `Sim` and `Snapshot` carry optional
mission state, `Sim::step` checks alive return-to-camp completion, and shift
moves the camp. Scene commands: `mission X Y RADIUS`, `sample MAT TARGET COUNT`,
`mission_complete yes`. History diffs and Timeline moments carry mission
state alongside terrain. Editor and DEEPWARD credit actual removals through `Sim::collect_mined`;
tracked mission quotas reflect carried inventory after crafting/ammo use.
The editor HUD shows quotas, signal and completion.
Toolbar **Crystal mission** adds/removes the mission undoably; no standalone
save is touched in editor tests. The cave examples have scene-defined goals.
Tests: `tests/mission.rs` (save/reset/rewind/undo/shift) and the editor's actual
mouse-mining/return/reset UI test. Scene-defined goals take precedence over
the standalone tuning defaults. Avoid separate mission rule implementations.

### Surface drilling / mining trial

Player drills use `src/mining.rs`: `Mining::strike` at 10 Hz, `Drill`
(radius/reach/power/grade), first-surface ray and at most two exposed layers
per strike. Hard facings, owned cells and liquids block excavation. Material
`mining_hardness` is separate from physics toughness; unspecified values
derive from toughness. Starter grade 3, dirt 0.1, rock 0.7, basalt 2.2, bedrock 8.
Sparse fractures store actual work and material identity; they are pruned
when cells change, bounded, shifted and carried in Sim/Snapshot, History,
Timeline and scene `fracture X Y MATERIAL WORK` lines. Timeline memory counts
fractures. Renderer captures fractures/last impact through `light(sim)` for
progressive veins and cosmetic chips; no extra mineable cells. Material
sound cues come from strikes. Both editor and DEEPWARD use this rule through `Sim::use_tool` and
award only its removed-cell counts. `dig_circle` remains instant for other
tools; do not put it back in the player drill. The editor HUD probes the
actual surface and shows hardness/work. All values are editable through the
material inspector. Test scene: `scenes/mining-trial.txt`, launcher
`Play/Mining-Trial.cmd`, guide `docs/MINING-TRIAL.md`. Tests in
`tests/mining.rs` cover resistance, shielding, persistence, rendering and
explorer-sized route clearance; editor tests cover real mouse excavation
and sample mission completion.

### Cave storage, workshops and tool loadout

`src/supplies.rs` owns exact u64 material counters, owned/equipped tools,
workshop markers, catalog parsing, atomic recipes, and the shared storage UI.
Sim/Snapshot carry Supplies; History and Timeline carry it too (including
transactions made with the clock paused via `record_now`), and shift moves
workshops. Scene lines: `storage on`, `stock MAT N`, `owned_tool ID`,
`equipped_tool ID`, `workshop KIND X Y RADIUS`. Once tracking starts,
mission sample quotas mirror current stock until completion. Old untracked
saves seed known crystal counts on their first mining award; ordinary mined
materials from old saves cannot be reconstructed.

Player tools call `Sim::use_tool`: drills credit exact actual removals once,
sprayers spend one ammo cell only after `spawn_flyer` succeeds. No harvesting
credit for fire/acid destruction. Tool defs: `assets/crafting/cave-tools.txt`;
recipes: `assets/crafting/cave-recipes.txt` (editor reloads, standalone reads
at startup). Crafting checks all costs, ownership and a living player near
the required workshop before changing any counter. No duplicate tool charge.
Editor B/left-stick click opens the shared panel and pauses simulation and
player input; F/LB cycles tools; D-pad chooses recipes, Y crafts. Suppress
B/F editor shortcuts in player Play. Reset restores inventory and terrain.

Trial has mechanic/chemistry stations, finite timber/iron ore, a steel cache
and burnable roots. Natural ore slots now use `iron_ore` instead of worked
iron. Acid, hot flame jets and iron ore are normal material files. Acid's
finite reactions spend it to water, and bedrock has no acid reaction. The
generic `hurts_on_touch` material key applies the strongest body contact
rate to characters each tick, independently of heat; exposed in inspector.
Tests: `tests/supplies.rs` covers ledger conservation, craft transactions,
save/reset/undo/rewind/shift, actual burning/steel dissolution and contact
harm; the editor UI test mines, opens storage, crafts, pauses and resets.
Guide: `docs/CAVE-WORKSHOPS.md`. This remains a progression prototype.

### Cave crawlers and elemental variants

`src/creatures.rs` supplies data-defined skitter/shellback brains and native
art. Species: `assets/creatures/cave.creatures` (editor reloads on file change).
`ActorSpec::creature` carries species, home/target, mood, timers, element,
burst quota and emitted flag. All bodies use existing flesh: temperature,
contact harm, drowning and physics health are shared with the player. Material
affinity raises safe temperature limits to the element's starting temperature
and ignores that material's contact damage; AI avoidance follows the same
properties. Every motion goes through independent actor controls.

`ActorSpec::surface` / `SurfaceWalker` in actor/surface.rs follows nearby
terrain contours via bounded nearest-solid probes and iterative separation.
It handles inside/outside corners and rejects narrow slits. Kinematic adhesion
is lost when support is removed, restoring normal dynamic gravity. Brains
compare both contour directions to reach a player on another surface. This
is connected-terrain traversal, not flying or a global pathfinder. Native
art and stamped body cells use the same rotation about the collider centre;
rendered health/warning labels stay readable for wall/ceiling poses.

Creature scene syntax: `creature KIND X FEET [element MAT] [burst N]` plus
optional health/dead, home/target, normal/attached, mood/timers and erupted.
Scene load orders players first; player_status/player_count exclude creatures
from controls, crafting, HUD and player triggers. Sim calls brains before
physics and once-only death bursts after physics. A payload creates finite
actual flying cells (1–2048 authored, bounded by world capacity), never
overwrites terrain, and uses normal material reactions afterwards. Erupted
flags prevent duplicate emissions after save/load or rewind. Dead creatures
fall, do not respawn, and later stop blocking/fade. Snapshot, Timeline, undo
and scene save carry AI, health, poses and death flags; shifting moves homes
and targets. Timeline memory includes actor records.

Mining Trial protects initial supplies, guards richer resources and a cache.
DEEPWARD has an authored guarded bonus gallery. `Play/Elemental-Creatures.cmd`
opens a supplied combat gallery with fire/acid/oil/lava/water variants.
Tests: creatures.rs (bites, occlusion, dodge, retreat, actual crafted-weapon
kills, persistence, ceiling pursuit), surface_walk.rs (four surfaces, convex
corners, support removal, gaps, saved pose/crop equality), elemental_creatures.rs
(finite bursts, affinities, acid scenery erosion, oil ignition, no save/rewind
duplication), and editor mouse firing/reaction/Reset. Docs: CAVE-CREATURES.md.
No automatic endless-region creature population is included yet.

### Cave kit polish, rig links, rope and gases

Cave scenes now use `look cave-diver` and `rope` rather than starter jetpacks.
`look.link` is a generic character alias: source folder, graph override, fallback.
The installed imported rig/art/sockets stay local; public checkouts use explorer.
`Look::hands` optionally loads clip/frame/image-pixel pairs from `hand.sockets`;
`tools/export_character_sockets.py` exports actual rig wrists. Shared
`tool_pose` drives tools/emission with obstruction checks. Cave-specific graph
aiming preserves locomotion and the animation lab graph.

`physics/actor/grapple.rs` stores Rope in ActorSpec (snapshot/rewind), saved via
player rope settings/path and shifted with the actor. Elastic velocity constraint
uses normal terrain collisions, preserves release momentum and bounded nearby
corner wrapping/unwrapping. Q/right mouse/R3 toggles; Space/S or D-pad reels.
`cave_ui` shares the game HUD and exact exposed-pixel mining preview between
editor and standalone. Rope-equipped editor Play hides panels; P restores them.
Keep mining preview candidates shared with the actual strike.

Gas collector is data-defined `vacuum REACH RADIUS CAPACITY`; exact gas counters
in Supplies are separate from mined stock. Air/occluded gas are excluded.
Cargo shows tank counters; V/RT vents finite real cells; chemistry recipes can
spend gas to refine fuel. Save command `gas_stock MATERIAL N`. Counts preserve
material identity, not heat/lifetime. Guide: docs/CAVE-KIT.md; test scene/launcher
Cave-Kit-Trial; tests/cave_kit.rs plus editor click-through tests.

Overhead rubble crack-site jitter is spatial/frame-seeded, independent of runtime
material IDs. Material IDs still separate fragment groups. This prevents adding
a cave material from changing other games' rubble geometry and combat routes.

Cave controls simplified after user feedback: hold right mouse/Q/R3 to attach
and maintain the rope; release to detach. `Physics::hold_grapple` shares this
input rule; `wind_rope` changes length from the mouse wheel while attached.
Wheel otherwise zooms. Space jumps for rope loadouts (rope_jump input), with W
still supported. Big mining ring removed: tiny cross, exposed-pixel tint and
progress text only during work/blocked surfaces. Keep editor and standalone
controls and hints consistent; do not restore the old toggle/Space-to-reel UI.
