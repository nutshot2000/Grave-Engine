# What the overhead flight test adds, and what should come next

Implemented: shared 3D rigid rubble with XYZ blast scatter, tumbling, terrain/rubble contacts, settling, material state and continued burning; height-aware flying and swept projectiles; isometric view and surface picking; shared material fire/destruction through vertical sections; volume explosions; named reusable building/prop files; configurable aircraft controls; editor entry and standalone test.

Priorities from this test:

1. **Full 3D structural support and secondary fracture.** Moving fragments between sections is implemented. Static support loss is still detected per vertical section, and hard 3D impacts do not yet fracture terrain or split an existing fragment again. Extend those rules to shared 3D neighbors next; retain the existing material values.
2. **Overhead authoring in the editor.** Add footprint/height brushes, roof/interior visibility controls, named volume-part selection and prefab previews. Keep physical materials separate from paint. Current files/API are reusable, but this first version is text-authored.
3. **Save, reset and rewind for overhead worlds.** The lab reconstructs its source scene; it does not yet expose the normal editor's timeline/history for a whole volume.
4. **Active simulation regions.** Sleep/cull whole distant sections and draw chunks; benchmark long firefights before growing the map. Avoid a separate simplified fire system for distant buildings.
5. **Physical aircraft assemblies.** Reuse material parts, damaged engines, breakable rotors and altitude-dependent crash damage. Current fuselage motion/collision works, but the visual helicopter is not a destructible rigid assembly.
6. **Altitude-aware effects and sound.** Add coherent smoke volumes, rotor audio, blast occlusion and debris crossing the ground plane. The first isometric renderer draws actual material cells, so gas can look sparse from this angle.

Rescue objectives, enemy AI, ammunition management and missions should come after these foundations. Keep them separate from projection, height collision and material simulation so other games can use the same engine work.

## Red Mesa findings — November 2026

The flight foundation now supports a complete mission with radar/power dependencies, armour and ammunition, AA/SAM fire, countermeasures, rescues, supplies, base service and victory/failure. Mission rules remain separate from shared combat/cargo/atmosphere code. See [Dust Hawk](DUST-HAWK.md).

Resolved in this build:

- Removed the extra 1,050°C overhead heat across every blast radius, which made ordinary explosions act as incendiaries. Native local heat remains; AP is mechanical and can ignite exposed volatile liquid.
- Added open upper atmosphere so gases rise beyond the grid instead of flattening against a ceiling.
- Added material-preserving sling loads and spills, including thermal updates while carried.
- Added reusable swept combat, armour, guidance, decoys and impact events.
- Added mission map/progress, objective guide, altitude/speed camera and quiet rotor/weapon audio.
- Skip idle 3D work and retain colliders for unchanged sleeping terrain.

Next priorities:

1. **Active regions and streaming.** The initially awake 3-million-cell mission costs roughly 25–30 ms per simulation tick here, before rendering. Profile sustained fire/rubble before increasing map size. Keep native simulation in distant regions rather than substituting arbitrary fire timers.
2. **Full 3D support, impacts and fluids.** Static support and oil flow remain section-based. Secondary impact fracture and XY fluid spreading would improve destruction and dropped fuel tactics.
3. **Physical aircraft and sling ropes.** Add breakable rotors/engines, whole-load contacts and rope tension. The current controller uses a swept fuselage; cargo uses a shortened kinematic cable and falling sweep.
4. **Volume authoring and persistence.** Workshop footprint/height/interior tools, previews, snapshots, save/undo and reproducible regression scenes.
5. **AI navigation and mission schema.** Current defences are stationary. Moving patrols, obstruction-aware firing and civilian reactions would add life. Validate mission metadata and add generic objective hooks before a campaign.
6. **Spatial sound and effect occlusion.** Add stereo direction, distance, wall occlusion, rotor wash and surface-specific contacts. Open-sky plumes are visual after leaving the physical grid.

A passing automated mission route verifies rules; it does not prove balance or long-session performance. The engine limits above remain relevant.

Measured all-seven-site stress run: 600 native ticks after warm-up, all sites deliberately heated together. Simulation median **32.8 ms**, p95 **39.1 ms**, worst **81.0 ms**; sampled 900×560 rendering median **18.6 ms**, p95 **19.5 ms**; peak **483** shared fragments. It completed without failure, but combined work is around 20 fps in this extreme case. Active regions and cached/chunked rendering are the next performance priority. This benchmark excludes UI and audio; normal mission play ignites fewer sites at once.

Final combat polish adds finite ammunition ranges and a range warning, material-backed target integrity, and quick-click buffering. A structure's empty bounds no longer block shots; burning/removing its actual material can disable it. Cargo now preserves burning flags and remaining fuel on landing as well as in flight.
