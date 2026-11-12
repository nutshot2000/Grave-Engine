# Editor Studio

The editor now has a dark workspace with resizable side panels and six focused workspaces: **Assets, Materials, Build, Events, Lights and Tests**.

## Start building

Open **Scenes... > editor-studio** for the construction-kit playground. Your original scenes and assets are unchanged. The Save field starts at `scenes/my_scene.txt` after loading a sample, so saving a variation does not replace the sample.

- **Assets:** search, filter buildings or props, drag a thumbnail into the canvas, or click a thumbnail then stamp copies. Escape cancels placement. Double-click a card to edit its named parts in the workshop. Refresh reloads assets changed outside the editor.
- **Snap:** choose the cell spacing in the top toolbar. This affects asset placement and paused scene dragging.
- **Selection:** whole-asset selection is on by default. Moving, rotating (Q/E), copying, duplicating or deleting one front part affects that asset's parts in every depth plane. Their local joints, ground attachments and mounted lamps follow. Turn off whole-asset selection to edit an individual front part. Open the asset workshop to change deeper parts.
- **Build:** choose dimensions, one to three storeys, timber or brick, and an exterior front with an interior behind it. Generate a preview, place it or save it into the asset library. The generated parts use real wood, glass, stone and brick. Doors are breakable panels; hinges can be added with Join. Each storey needs at least 24 cells below the roof, so short buildings need fewer storeys.
- **Events:** choose who enters a zone and what happens: message, heat, an explosive charge, snow, lighting or finish. Pick a target for fire or a charge, then place the zone. Try fires an existing event; removal and creation are undoable. A charge is real gunpowder, triggered by heat. Use Camera > Show trigger boxes to hide/show zones.
- **Lights:** place coloured points, spotlights, mounted neon strips or an area fill. Adjust range, brightness, cone, shadows and flicker. An area fill uses four sampled point lights; it is not a continuous area-light renderer. Lamps can attach to selected front parts. Neon uses luminous glass and is welded to its support or the ground.
- **Tests:** run a separate asset copy through Drop, Impact, Burn, Explosion or Snow. Save and reload recipes with the same conditions. Space controls the test; Reset starts a fresh copy. The source asset and your level are untouched. Snow starts at -8 C. The lab stops at the chosen frame limit.

## Included construction kit

Timber beams, steel beams, brick wall sections, a glass window with a breakable wooden frame, an open timber house and a layered exterior/interior house. The existing houses, shops, TNT boxes and explosive barrels are also in the library. All are native material assets.

## Plain-text use by AI and tools

Assets remain in `assets/workshop/*.asset`; saved recipes are in `assets/tests/*.test`. Scene bodies now preserve an `assembly NAME` across depth planes. `Building::build` and the `assemblies` module expose the same generation and whole-instance operations used by the editor. `cargo run --release --example build_editor_kit` rebuilds only the supplied kit and playground.

The event workspace creates zones and can test/remove existing ones; detailed edits to an existing zone remain available in the scene text. Whole-building mirror and resize operations are deliberately blocked: reshape individual parts in the workshop or generate a building at the desired size.
