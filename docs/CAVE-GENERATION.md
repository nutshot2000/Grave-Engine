# Cave generation

Open `scenes/caves.txt` for a fixed example or `scenes/endless-caves.txt`
for a streaming map. In the editor use **Edit > Generate caves**, then
**Reroll** to compare seeds. Existing saved scene grids stay as drawn until
regenerated. Reset an endless run to replace its saved regions.

Natural passages now use rounded corners and gentle, seeded variation inside
each herringbone brick. Doorway collars keep their exact positions so the
neighbouring bricks still meet. Narrow side branches occur more often;
large hub rooms occur less often. Authored rooms from `tiles/caves` are
occasional landmarks (38% of eligible placements), mixed with fresh layouts.
Timber, masonry, ore and liquids retain their authored positions.

`rock` uses an original, subdued fractured-rock texture; generated rock
also contains patches of `shale` and sandstone. The same rock texture is
used dimmed on back walls, keeping the empty passages readable. These are
real materials, so digging, heating and breaking still work as usual.
The active material artwork is a new generated set; see
`materials/textures/CREDITS.md` and `generated-prompts.json` there.
`tools/import_material_textures.py` packages source images as compact tiles.
The old procedural cave experiment now writes only to `shots/`.

The structure still uses the engine's herringbone edge layout. It does not
guarantee that every cavity is reachable or that every route can be walked
without tools. The reference's top-down dungeon and this engine's side-view
caves also have different traversal needs. Tests cover the main connected
network, unchanged doorways, narrow-passage rounding, authored features,
and identical results when endless regions cut through partial bricks.

## Layout variety

Most built-in bricks now use a finer 16-block grid per tile unit and one
of six plans: a winding spine, broad joined caverns, a branching tree,
a loop around a rock pillar, a chain of small chambers, or fractured
shafts with offset elbows. Passage width, chamber sizes, branch counts
and internal routes differ between plans. Unlike the original layout,
they are not all organised around a chamber in each square half.

A fifth of built-in bricks retain the earlier room/shelf/pool layouts,
and the authored tile landmarks still appear, giving constructed places
among the natural caves. Required entrances connect inside each natural
plan; optional branches stop inside its protected outer rim. This does
not change the existing limits on whole-map reachability or side-view
player traversal.

Tests exercise both orientations, all 63 nonempty door combinations and
three seeds, plus 128 plans with identical entrances to guard shape and
open-space variety. Endless-region equivalence is checked after the
plans are sampled into actual world cells.

## Huge caverns and blind passages

**Generate caves** now has **Huge caverns** and **Dead ends** sliders. The
first controls how often map sectors contain a large irregular chamber,
roughly 3.4 to 5.6 tile units across before wall variation. These rooms
cross tile boundaries; smaller tunnel layouts continue around them.
Authored structures are kept out of the area around a huge chamber so
its walls do not cut through half a vault. Existing owned objects and
characters remain untouched. The sealed outer world rim is retained.

The **Dead ends** slider requests extra blind side passages in natural
plans. Candidates that join another tunnel away from their mouth are
rejected. Crowded bricks may have less room for these branches; raising
the slider requests more attempts without guaranteeing a fixed count.

Scene equivalents (also accepted by `endless`): `caverns 0.65 deadends 0.75`.
Both range from zero to one. Zero disables the corresponding extra pass;
it does not remove dead ends already present in a base or authored tile.
Cavern placement and contours use map coordinates so a large room can
cross streaming regions without seams. Reset old endless runs to replace
saved terrain; use Reroll for a different distribution of chambers.

Tests cover large open cores, owned-cell preservation, blind-branch growth,
scene controls, sealed borders, and region equivalence at negative map
coordinates and non-aligned tile sizes.

## Cavern interiors

Large chambers contain ceiling-rooted tapered stalactites, broad blunt floor
pillars, and ledges projecting from both walls. Their attachment points come
from the actual irregular chamber outline. These are ordinary anchored rock
cells: they can be heated, dug out and destroyed. An open band through the
middle prevents the formations from sealing the main chamber route. This
is a geometry safeguard, not a guarantee that every cavern is walkable.
Their plans use map coordinates, including when a formation crosses an
endless-region boundary.

The full set of 28 material textures has also been replaced. The tan brick-
looking patches were sandstone's old texture, not the cave shape generator.
Natural sandstone now has sediment layers; actual brick keeps a deliberate
masonry pattern. Preview all new surfaces in `docs/material-textures.png`.

## Soft geological contacts

Rock, shale, stone and sandstone opt into `texture_blend = 5`, a narrow
visual transition where two compatible solid materials meet. The artwork
is feathered with slight irregularity at the contact; the actual material
IDs, collision, heat and reactions are unchanged. Texture blending stops
at air or liquid and skips painted or owned cells. Built materials stay
crisp unless they explicitly opt in. The material inspector's **Texture
join** setting adjusts the width from zero to eight cells. Heat view stays
unblended, and windowed/cropped renders use the same world-space result.

## Cave atmosphere: crystals and torches

Generate caves includes **Crystals**, **Torches** and **Cave brightness**.
The first two control seeded fixture density around the large chambers.
Scene and endless lines accept `crystals 0.7 torches 0.3`; zero disables
new placements. Ambient brightness is ordinary scene lighting: for example
`lighting 0.32 #c8d6f0`. The generator's brightness control changes this as
part of its undoable operation. After generation, the **Lights** panel edits
the ambient light and its tint without regenerating the ground.

Clusters are real editable cells of `crystal_cyan` / `crystal_violet`, with
painted facets. Torches are wood stems with `torch_glow` tips. Paint, cut,
copy or erase these like any scenery; save/undo/Reset retain them. Right-click
the emitting material to edit **Light reach**, **Light brightness**, **Light
flicker** and its colour. **Glow** controls the bright core. Material colour
controls emitted light; regenerating updates the painted crystal facets.
The torch tips are cool decorative emitters, not a fire/fuel simulation.

The engine's material keys `light_radius`, `light_strength`, `light_flicker`
work on any material. Each connected patch becomes one shadow-casting lamp,
rebuilt from the cells at render time. Destroying the source removes its
light; there are no invisible leftover lamp records. Material emitters are
edited through their cells/settings rather than the free-lamp list. Ordinary
manually placed lamps still work alongside them.

Fixture plans use map coordinates and are clipped on output. Endless saves
carry the material cells and paint, so lighting returns with the region.
Tests cover connected-source grouping, erasure, scene save/load, cropped
rendering, deterministic flicker, density controls and region equivalence.

The caves now have an animated explorer loadout with jetpack and headlamp.
For the playable collect-and-return game, use `Play/Cave-Explorer.cmd`; see
[DEEPWARD](DEEPWARD.md). In editor Play mode Space flies and P pauses.
