# Detailed assets with any AI model

Use **tools/asset_builder.py** instead of asking a model to spell out thousands
of grid cells. The model describes shapes and meaningful parts in a JSON recipe;
the helper does the raster drawing, material assignment, lettering, ownership
and basic geometry checks. **asset-doctor** then runs the result in the engine.
Neither tool needs an AI account, a particular model or an image generator.

Start from an existing recipe, rename it, and change its proportions and details:

- **assets/recipes/supply_crate.json**: three joined parts, wood grain highlights,
  red paint on actual wood, black lettering, metal foot and brass fasteners.
- **assets/recipes/iron_heron.json**: the actual detailed pumpjack, including
  eight moving joints, a counterweight, cooling fins, contrasting edges,
  named control cabinet, lettering and a crank clearance check.

```text
py tools/asset_builder.py assets/recipes/iron_heron.json --strict
cargo build --release --bin asset-doctor
target/release/asset-doctor.exe assets/workshop/iron_heron_pumpjack.asset --frames 600 --motion crank --min-turns 1 --shot shots/asset-review.png
```

**Open and inspect the screenshot.** A passing test does not mean the art looks
good. Correct the silhouette, proportions, contrast and moving clearances, then
repeat the focused check. The pumpjack example completes over two full crank
turns in the ten-second review without losing any of its seven parts.

## Recipe structure

The top level contains `name`, `width`, `height`, `depth` in metres, `pivot`,
`styles`, `parts`, and optional `welds`, `joints`, `keepouts`. The grid is at most
128 by 128. Coordinates start at the top left; y increases downward. Every part
shares that grid, and its cells form an actual separate physical body.

Styles separate physical material from appearance:

```json
"styles": {
  "painted_wood": {"material": "wood", "paint": "#a83d35"},
  "wood_ink": {"material": "wood", "paint": "#211e20"},
  "frame": {"material": "steel", "paint": "#3d7373"},
  "edge": {"material": "steel", "paint": "#90a8a0"}
}
```

Red paint on wood still burns as wood. A gold-coloured steel edge stays steel;
use material `brass` when the fastener is meant to be actual brass. Paint is
optional. Material names are checked against the engine's material files.

Each part has a unique `name` and a `shapes` list. Shapes require `style`:

| `op` | Other fields | Use |
|---|---|---|
| `rect` | `at: [x,y]`, `size: [width,height]` | Panels, foundations, beams |
| `line` | `from`, `to`, optional `width` | Braces, rods, seams; use width 2–4 for load-bearing diagonals |
| `circle` | `at`, `radius` | Hubs, counterweights, bearings |
| `poly` | `points: [[x,y], ...]` | Curved/stepped silhouettes and shaped plates |
| `pixel` | `at` | Indicator lights and tiny highlights |
| `rivets` | `from`, `to`, `count` | An evenly spaced row of fasteners |
| `text` | `at`, `text`, optional `scale` | Compact 3×5 uppercase letters/digits; five letters need 19 cells at scale 1 |

A style using `air` erases the current part. It does not cut another part.
Overlapping parts are rejected unless the later part explicitly sets
`replace_overlap: true`. That transfers ownership of those pixels to the new
part; it can cut holes in the old one. Check for disconnected islands afterwards.

Connections use the same syntax as the native workshop, without the leading
`joint` or `weld` word:

```json
"joints": [
  "hinge beam base at 61 33",
  "hinge crank base at 106 76 motor 0.22 strength 800",
  "hinge base ground at 12 108"
],
"welds": ["wooden_box steel_foot 20 28 40"]
```

Use hinges for moving mechanisms, wheel joints for sprung axles, ropes for
flexible distance constraints, and welds for parts that stay rigidly together.
Two separated pins to `ground` hold a base still. Leave `drive` off a motor
that should run by itself. See [the workshop guide](ASSET-WORKSHOP.md).

Keep-outs catch obvious intrusions into a rotating sweep before simulation:

```json
"keepouts": [{
  "name": "counterweight_sweep",
  "center": [106,76], "radius": 18, "margin": 1,
  "against": ["control_cabinet"]
}]
```

These are author-specified static circles, not a full collision prediction.
Always run a moving mechanism for several complete cycles. The first Iron
Heron draft looked correct but its counterweight hit another part mid-turn.
The native motion check catches that kind of jam.

## A repeatable quality pass

1. **Silhouette first.** Use a recognisable outline with negative space. A beam
   is a beam, a brace has thickness, and a machine has room around its moving parts.
2. **Meaningful parts.** Name what can move, break or be made of another
   material. Keep a stable frame separate from the crank, linkage and rod.
3. **A small palette.** Use one dark structural outline, one body colour,
   one lighter edge, and a restrained warm accent. Keep material textures visible.
4. **Structural detail.** Add edge strips, bracing, hubs, fasteners, vents,
   guide sleeves and labels where they explain how the object works. Do not
   sprinkle random noisy pixels over every surface.
5. **Check the whole movement.** Leave room for the complete crank sweep and
   linkage stroke, including nearby cabinets and scenery. Use sensible depth:
   a side-on shape extruded a metre can become many tonnes of solid steel.
6. **Compile, simulate, look.** Fix diagnostics, inspect screenshots, and compare
   at the normal game zoom. The helper handles drawing reliably; the model still
   needs to make design decisions and judge the result visually.

## Live editor and preservation

`--live` announces the result through `live/agent.txt` and asks the running
editor to show the asset. Its existing unsaved-draft guard remains in charge.
Without `--live`, editing an asset already placed in the open scene follows the
editor's normal live-reload behaviour. Keep the editor open.

The output is written atomically, so the watcher cannot load a half-written
asset. Unknown materials, duplicate/empty parts, unintended overlaps and
out-of-grid shapes fail before writing. `--strict` also refuses geometry
warnings. Existing named parts cannot be silently omitted: removal requires
`--allow-remove-parts`. Prefer a new name/output when making an alternative.

For a private preview use `--out PATH.asset`. The doctor tests a fresh copy and
never saves over the source asset. Use `--frames 0 --shot ...` for an initial
pose, then a second shot after 600 frames for a moving machine. Keep recipe JSON
with the asset, so the next model can make small edits instead of guessing how
the grid was built.

Run helper regressions with `py -m unittest discover -s tools -p test_asset_builder.py`.
