# Hand-made level tiles

Tiles for `generate ... tiles tiles/rooms` (see `src/levelgen.rs`). Wherever a
tile's edges fit a brick of the herringbone layout, it's used instead of a
built-in cave tile (chosen at random among those that fit).

- `shape h` = 2 wide x 1 tall, `shape v` = 1 wide x 2 tall.
- `edges` = six 0/1 values: is there a passage across that edge piece (at its
  middle)? Wide tiles: top-left, top-right, right, bottom-right, bottom-left,
  left. Tall tiles: top, right-upper, right-lower, bottom, left-lower, left-upper.
- `legend C material` names the characters; `.` is empty.
- Any drawing size works (it's stretched to the tile), but keep the 2:1 shape.
  Leave each open edge's middle clear so passages line up with neighbours.
