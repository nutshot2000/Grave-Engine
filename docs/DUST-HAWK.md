# Dust Hawk — Operation Red Mesa

Start **Play-Dust-Hawk.cmd**, or choose **Scenes… → Dust Hawk — Operation Red Mesa** in the editor. Press Enter at the briefing.

## Mission

The scrolling operation covers **1,536 × 1,152 cells**, over 50 times the ground area of the first town. It has 16 separate installations/outposts joined by branching roads and stepped desert ridges. The camera follows the helicopter continuously; the level no longer fits on one screen.

Destroy two elevated radar arrays, cut power to disable guided SAMs, rescue six people in three groups, destroy command and return to a landing zone. There are 19 target installations/defences, an optional ammunition dump, supply caches and four portable oil barrels. Two forward service pads shorten return trips. Cliffs and buildings obstruct shots. The map pauses action and records progress; the live guide suggests your next objective.

| Control | Action |
|---|---|
| WASD / arrows | Fly in screen directions |
| E / Q | Climb / descend |
| Mouse / left button | Aim / fire selected weapon |
| 1 / 2 / 3 | AP cannon / HE rockets / guided AT |
| Space / right button | Heavy weapon; rockets when cannon is selected |
| G | Winch nearby barrel / release load |
| F | Flares |
| M | Tactical map |
| Wheel | Zoom |
| P / Escape | Pause |
| R | Restart |

## Tactics

AP makes precise mechanical impacts without blanket heating. It can light exposed volatile oil. HE damages a local area; it is not an incendiary fireball, although local heat and secondary reactions can start fires. AT penetrates armour: hold the reticle over a target until the lock turns green. Flares break enemy guidance; keep moving because rounds continue ballistically.

Effective ranges differ: cannon 78 cells, rockets 100, guided AT 135. The reticle warns when the aim point is out of range. You need to move into the defended area or find another approach rather than firing across the entire map from the base.

Hover above a gold-marked oil barrel and press G. Carrying it reduces acceleration and speed. Climb, move it to your chosen position, then release it. A hard drop ruptures the case and deposits real oil. Shoot the exposed spill to ignite it. Carried contents retain heat and burn timers.

Hover slowly 0.8–3.2 m above survivor markers for the winch. Survivors count after delivery to a service pad. Hover below 1.5 m over a pad and wait to unload, repair, refuel and rearm. Landing zones are protected from direct defensive fire. Green map markers include the forward pads.

## Reusable implementation

- `overhead::combat`: swept shots, bounds, factions, armour, configurable ammunition, guidance, impact events and decoys.
- Material-backed targets intercept shots at actual surfaces; empty bounds do not block shots. `Combat::bind_materials` records structural integrity so material destruction can disable an installation without requiring its last scripted health point to be shot away.
- `Volume::impact` / `Physics::impact_circle`: mechanical damage without explosion heat.
- `overhead::cargo`: captured material cells, sling tracking, falling loads, rupture, spills and native thermal updates in flight.
- `overhead::atmosphere`: gases vent out of the grid and keep rising.
- `Viewport`: altitude-aware camera and optional continuous ground scenery.
- `overhead::space::Space`: common material access for flight, surface picking, ammunition and cargo. Existing bounded volumes still work through the same interface.
- `overhead::map::WorldMap`: sparse native material patches over collision-backed terrain. Empty travel space does not allocate a giant cell grid. Material state and rubble persist when you leave.
- Near patches run at 60 Hz. Distant changed patches run at up to 5 Hz, with a deterministic round-robin quota of one remote patch per tick. Distant fire and physics therefore advance more slowly under load; this is deliberate simulation detail reduction, not full-rate global simulation.
- Rendering culls unseen patches, smoke and rubble, draws terrain coarsely, and limits bloom to the lit area. Paused/unfocused frames reuse the existing image.
- Idle 3D work is skipped without rubble; sleeping terrain retains cached colliders.
- Nine reusable volume prefabs, regional scenes and separate mission metadata. `py tools/build_red_mesa_map.py` regenerates the map; `py tools/build_red_mesa.py` rebuilds shared prefabs and then the map.

Mission rules and original synthesised sounds live in `src/dust_hawk/`. The shared engine does not know mission or weapon names. No third-party game art/audio is included.

## Verification and limits

`cargo test --release --test dust_hawk` completes the objective route using real projectiles, material steps, rescues and servicing. It relocates between encounters to bound duration; it is not a human playthrough. `--test overhead_map` checks long flight, sparse allocation, persistent damage, bounded/fair distant work, drops into open terrain and projectile/blast crossings at patch edges. `--test overhead_combat` covers ammunition, guidance, decoys, fire and cargo. Binary tests check quick clicks, paused input and the actual follow camera. `dust-hawk --verify` also flies continuously over 1,100 cells and writes route screenshots.

This is fixed-angle 2.5D. Rubble moves in 3D within each patch's physical space; cross-patch rigid contacts and continuous fluid/fire transfer need further work. Static support and liquid flow remain section-based. The cable is kinematic. The helicopter is not a breakable material assembly. Sparse regions remove the whole-map dense-grid limit, but generated damage patches remain resident; compressed streaming and a residency cap are future work. Reset reconstructs source; overhead save/rewind is not implemented.

`dust-hawk --showcase` captures missile destruction and a fuel test. `dust-hawk --stress` heats all target sites, runs 3,600 map ticks, samples rendering and reports median/p95 costs. It exercises distant-work scheduling as well as local full-rate fire. It is a stress benchmark, not normal startup.

The earlier 20 fps dense-town result is historical and does not describe this map. Updated measurements are recorded in the overhead feedback file.

Design research: the [official Desert Strike manual](https://www.sega.jp/mdmini2/en/assets/manual/pdf/EU_DesertStrike.pdf) describes radar/power objectives, rescues, supplies and weapon choice. Red Mesa uses those broad ideas with original layout, art and material-based destruction.
