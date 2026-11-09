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

## Materials are text files

Each file in [`materials/`](materials/) is one material. Here's wood:

```text
state = solid
density = 700
color = #7a4a26
conducts_heat = 0.15
catches_fire_at = 260
burns_for = 160
burns_into = ash
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

## For AI agents: headless mode

Run a scene without a window and get a screenshot plus a report:

```bash
cargo run --release -- --run scenes/woodfire.txt --frames 600 --shot shots/out.png
```

Options: `--every N` saves a picture every N frames, `--heat` saves the heat view,
`--scale S` sets the picture size. The report lists the temperature range and how many
cells of each material exist (and how many are burning). Problems in material or scene
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
2. Editor: inspector, select, move, rotate, scale, unlimited undo, rewind after Play
3. A command channel so agents can drive the open editor
4. Rigid, breakable objects; import images and Blender models
5. Game layer: player, triggers, scripting, export
