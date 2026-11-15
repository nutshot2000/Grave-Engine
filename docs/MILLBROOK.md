# Millbrook waterwheel

Open **Play/Millbrook.cmd** (or `scenes/millbrook-waterwheel.txt` in the editor)
and press **Play**. The mill pond drains through the sluice into the flume,
pours onto the top of the wheel, and the **weight of the water in the
buckets** turns it. It spills into the stone-lined tailrace and runs down to
the lower pond. Nothing is animated: the wheel is one physical wooden body on
a pin, and the water is the engine's ordinary water.

- The pond is finite: the wheel spins up to about 13 turns a minute, then slows
  as the pond empties (about 3 turns in the first 20 seconds).
- Burn it, blast it or block the flume and it behaves like wood and water.

## How it's made

- **The wheel:** `assets/workshop/millbrook_waterwheel.asset`, built from the
  recipe `assets/recipes/millbrook_waterwheel.json` by `tools/make_waterwheel.py`
  (with `tools/asset_builder.py`): rim, eight spokes, sixteen buckets with
  lips, iron hub, brass bolts, painted wood. One `hinge wheel ground` pin.
- **The scene:** `tools/make_millbrook.py` writes the hill, pond, sluice gate,
  flume, tailrace, mill house and paint. The stone pier and the flume's side
  boards are in the plane behind: in front they would dam the water.

## Engine features it needed

- **Liquid weighs on objects.** Water resting on an object presses down on it
  where it rests (the wheel's buckets, a tray on a seesaw, rain in a boat).
  Objects already floating feel it through buoyancy instead. Powders don't
  press yet (sand thrown onto a bike would weigh it down; needs tuning first).
- **A stream beside an object isn't a pool.** An object only counts as in
  water when there's water on both sides, or water on one side and a wall on
  the other. Before, water pouring past the wheel made it "float" (heavy drag,
  no weight in the buckets).
- **Objects placed inside scenery are reported.** They're lifted clear (the
  parts of one asset together), and the editor/headless run says so when it's
  more than 2 cells, naming the object.

Tests: `tests/waterwheel.rs`, `tests/scene_overlap.rs`.
