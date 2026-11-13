# Dust Hawk — Operation Red Mesa

Start **Play-Dust-Hawk.cmd**, or choose **Scenes… → Dust Hawk — Operation Red Mesa** in the editor. Press Enter at the briefing.

## Mission

Destroy two elevated radar arrays to shorten detection range, cut power to disable guided SAMs, rescue six people in three groups, destroy command and return to the landing zone. Optional goals include eight defences, an ammunition dump, four supply caches and four portable oil barrels. Cliffs and buildings obstruct shots. The map pauses action and records progress; the live guide suggests your next objective.

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

Hover slowly 0.8–3.2 m above survivor markers for the winch. Survivors count after delivery to base. Hover below 1.5 m over the pad and wait to unload, repair, refuel and rearm. The base is protected from direct defensive fire.

## Reusable implementation

- `overhead::combat`: swept shots, bounds, factions, armour, configurable ammunition, guidance, impact events and decoys.
- Material-backed targets intercept shots at actual surfaces; empty bounds do not block shots. `Combat::bind_materials` records structural integrity so material destruction can disable an installation without requiring its last scripted health point to be shot away.
- `Volume::impact` / `Physics::impact_circle`: mechanical damage without explosion heat.
- `overhead::cargo`: captured material cells, sling tracking, falling loads, rupture, spills and native thermal updates in flight.
- `overhead::atmosphere`: gases vent out of the grid and keep rising.
- `Viewport`: altitude-aware camera and optional continuous ground scenery.
- Idle 3D work is skipped without rubble; sleeping terrain retains cached colliders.
- Nine reusable volume prefabs, a material level and separate mission metadata. Regenerate with `py tools/build_red_mesa.py`.

Mission rules and original synthesised sounds live in `src/dust_hawk/`. The shared engine does not know mission or weapon names. No third-party game art/audio is included.

## Verification and limits

`cargo test --release --test dust_hawk` completes the objective route using actual projectiles, material steps, hover rescues and servicing. It relocates between encounters to bound test duration; it is not a human playthrough. `--test overhead_combat` covers AP/oil, armour/guidance, decoys, sky venting, drops, carried burning and local HE heat. The binary UI test checks quick clicks and paused input. `dust-hawk --verify` writes scene screenshots and timings.

This is fixed-angle 2.5D. Rubble moves in shared 3D, but static support and liquid flow remain section-based. The cable is kinematic, with falling cargo, rather than a physical rope. The helicopter is not a breakable material assembly. This dense mission stays within the four-million-cell bound; larger seamless maps need streaming and active regions. Reset reconstructs source; overhead save/rewind remains future work.

`dust-hawk --showcase` captures actual missile destruction and a deliberately ignited fuel test. `dust-hawk --stress` deliberately ignites all seven sites, runs 600 native ticks, samples rendering, and reports median/p95 costs. This is a worst-case benchmark, not the normal mission start.

The measured all-site stress run peaked at 483 rubble pieces: simulation median 32.8 ms / p95 39.1 ms, rendering median 18.6 ms / p95 19.5 ms at 900×560. It stayed stable, but approximately 20 fps combined in that extreme case remains a performance limitation.

Design research: the [official Desert Strike manual](https://www.sega.jp/mdmini2/en/assets/manual/pdf/EU_DesertStrike.pdf) describes radar/power objectives, rescues, supplies and weapon choice. Red Mesa uses those broad ideas with original layout, art and material-based destruction.
