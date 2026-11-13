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
