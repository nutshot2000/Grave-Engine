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
