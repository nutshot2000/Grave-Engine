# Grave Engine

A small 2D material-simulation engine. Every pixel is a real material with real-ish
properties: sand falls, water flows, steam rises, and a **heat field** decides what
burns, melts, freezes and boils. Everything is **plain text**, so people and AI agents
can both read and change it.

![A wooden house catching fire from a burning oil puddle](docs/woodfire.png)

## Try it

```bash
cargo run --release
```

- **Left-click** paints, **right-click** erases, **mouse wheel** changes the brush size.
- **Play / Pause** (Space), **Step**, and **Reset** (R) back to how things were when you pressed Play.
- **Heat view** (H) shows temperature: blue is cold, red, yellow and white are hot.
- Tools: **Paint**, **Erase**, **Heat** and **Cool**.
- Hover over anything to see its material, temperature and state at the bottom.
- **Save** and **Load** scenes, or pick one from **Scenes...**.

## Editing

- **👆 Select** (V): click an object, or drag a box. A box picks up the objects
  inside it, or, if there are none, an area of scenery.
- **Move** by dragging. Objects are held still while you drag them, even during
  Play. **Turn** with the round handle (hold Shift for 15° steps) and **resize**
  with the square corner handle.
- **The Inspector** (right) shows what's selected in plain words: what it's made
  of, its mass, position, angle, speed and temperature. You can edit all of them.
  It also has buttons to flip, quarter-turn, resize, change material, duplicate,
  copy, delete, turn scenery into objects, or freeze objects into scenery.
- **Undo / Redo** (Ctrl+Z / Ctrl+Y) cover everything: painting, moving, deleting,
  importing, clearing, loading. Only what changed is stored, so there's no
  practical limit.
- **Keys:** Delete, Ctrl+C / X / V (pastes at the mouse), Ctrl+D duplicate,
  Q / E turn 15°, F flip, Esc deselect, Shift+click to add to the selection,
  B for the brush.

## Materials are text files

Each file in [`materials/`](materials/) is one material. Here's wood:

```text
state = solid
density = 700
color = #7a4a26
conducts_heat = 0.15
catches_fire_at = 300
burns_for = 240
burns_into = ash
burns_into_chance = 0.1
```

**Edit a file while the sandbox is running and it updates instantly.** Add a new
`.txt` file and the new material appears in the palette. Every key is explained in
[`materials/README.md`](materials/README.md).

There are no special cases like "fire burns wood". Wood burns because it got hotter
than its `catches_fire_at` while touching air. Water boils because it passed its
`boils_at`. One set of rules covers every material.

## Scenes are text files too

```text
size 512 288
fill air
rect stone 0 268 512 20        # material x y width height
circle water 100 200 20        # material x y radius
line wood 10 10 50 50 3        # material x1 y1 x2 y2 thickness
heat circle 60 238 4 600       # set the temperature (°C) somewhere
```

Saved scenes also hold a character grid (one character per cell, with a `legend`).
See the top of [`src/scene.rs`](src/scene.rs) for the full format, and
[`scenes/`](scenes/) for examples.

## Objects and physics

Painted things stay put, like scenery. **Objects** are solid things that fall, tumble,
stack, float or sink, and break apart when pieces burn, melt or get cut off. They are
still made of real material cells, so a wooden crate burns and an ice block melts.
Physics is done by [Rapier](https://rapier.rs). One cell is 10 cm.

- In the window: pick **📦 Make object** and drag a box around something you painted.
- In a scene: `body rect wood 100 50 20 10` or `body circle stone 200 40 6`.
- **Painted solids need support.** When something burns, melts or gets erased, any
  solid piece that's no longer connected to the ground (the edge of the world) breaks
  loose and falls. Burn the wall under a roof and the roof comes down. Things you paint
  floating in the air stay put until they're damaged.
- **Overhangs snap.** Each material's `strength` is how far it can stick out from a
  support: wood reaches 5 m (50 cells), iron 15 m. A beam that's too long breaks off
  at that point, so buildings need posts, like real ones.
- Things float if they're lighter than the liquid they're in (wood on water) and sink
  if they're heavier (iron).

## Forces: impacts, weight, friction and explosions

Try `scenes/forces.txt`.

- **Impacts and weight crack things.** Every hit's force comes from the physics, so
  heavier and faster objects hit harder. If it beats a material's `toughness`, that
  spot cracks: stone crumbles to gravel, wood splinters into flying pieces, and ice
  shatters. Drop an iron block on a wooden bridge and it smashes through. A gentle
  landing doesn't break anything.
- **Friction and bounce** are per material. Things slide off ice, and grip stone.
  Slopes are smooth, so blocks slide and carts roll downhill.
- **Explosions.** Gunpowder goes off when it gets hot, even buried. The blast cracks
  things, throws objects, scatters burning splinters and lights fires. Neighbouring
  gunpowder sets off in a chain.

## Importing from Blender and images

**Drag a file onto the window** to drop it in as an object (tick "Drop as fixed scenery"
to paint it in place instead). In a scene: `object ../assets/cart.glb 50 30`.

- **Blender:** export as **glTF Binary (.glb)**. Grave Engine looks at it from the front
  (numpad 1) and turns it into cells. **Name each Blender material after a Grave
  material**: `wood`, `iron`, `Stone.001` and `old_wood_planks` all work. One Blender
  metre is 10 cells (`scale 20` in a scene makes it bigger).
  [`tools/make_cart_glb.py`](tools/make_cart_glb.py) builds the example cart.
- **Images (.png):** each pixel becomes a cell, and transparent pixels are empty. Each colour
  picks the material with the closest `color`, or you can add a `crate.colors.txt`
  next to `crate.png` with lines like `#8b5a2b = wood`.

## For AI agents: headless mode

Run a scene without a window and get a screenshot plus a report:

```bash
cargo run --release -- --run scenes/woodfire.txt --frames 600 --shot shots/out.png
```

Options: `--every N` saves a picture every N frames, `--heat` saves the heat view,
`--scale S` sets the picture size. The report lists the temperature range and how many
cells of each material exist (and how many are burning), plus how many objects there are. Problems in material or scene
files are printed with file and line numbers, and the exit code is 3 if there were any.

## Tests

```bash
cargo test
```

The tests check the core rules: heat spreads, wood catches fire past its threshold
(and only when it touches air), water freezes and boils, lava plus water makes stone
and steam, and scenes save and load correctly.

## What's next

1. ~~Sandbox~~ (done)
2. Editor: ~~select, move, rotate, scale, inspector, undo~~ (done); next, rewind after Play and templates
3. A command channel so agents can drive the open editor
4. ~~Rigid, breakable objects; import images and Blender models~~ (done: next, explosions and strength)
5. Game layer: player, triggers, scripting, export
