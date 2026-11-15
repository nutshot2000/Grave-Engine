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
Regenerate the two original seamless rock textures with
`tools/make_cave_textures.py` (Python with Pillow). No source photographs or
third-party texture packs are used by this script.

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
