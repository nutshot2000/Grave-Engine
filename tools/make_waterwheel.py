"""Generate the Millbrook waterwheel recipe (assets/recipes/millbrook_waterwheel.json)
and build its asset with tools/asset_builder.py.

An overshot wheel: water pours from the flume onto the top of the wheel on the
right, fills the buckets on that side, and their weight turns the wheel
clockwise. Real water, real wooden buckets: nothing animates it but the
engine's physics (loose cells resting on an object weigh it down).

py tools/make_waterwheel.py [--live]
"""
import json
import math
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
W, H = 112, 112
CX, CY = 56, 56          # axle, in the grid
R_RIM_OUT, R_RIM_IN = 41, 37   # the rim ring
R_BUCKET = 50            # outer edge of the buckets
BUCKETS = 16

styles = {
    "wood_outline": {"material": "wood", "paint": "#2b1d15"},
    "wood_body": {"material": "wood", "paint": "#6e4a2f"},
    "wood_light": {"material": "wood", "paint": "#9c6d43"},
    "wood_wet": {"material": "wood", "paint": "#4f3a2c"},
    "iron_dark": {"material": "iron", "paint": "#2c3236"},
    "iron_body": {"material": "iron", "paint": "#59636a"},
    "iron_edge": {"material": "iron", "paint": "#9aa5ab"},
    "brass": {"material": "brass", "paint": "#e5ba63"},
}

def pt(r, deg):
    a = math.radians(deg)
    return [round(CX + r * math.cos(a)), round(CY + r * math.sin(a))]

def ring(r_out, r_in, style, steps=180):
    """A ring as a polygon strip: outer arc out, inner arc back (many thin polys)."""
    shapes = []
    for k in range(steps):
        a0, a1 = 360 * k / steps, 360 * (k + 1) / steps + 0.6
        shapes.append({"op": "poly", "points": [pt(r_out, a0), pt(r_out, a1), pt(r_in, a1), pt(r_in, a0)], "style": style})
    return shapes

shapes = []
# Spokes first (the rim and hub are drawn over their ends): 8 doubled timbers.
for k in range(8):
    a = 45 * k + 22.5
    shapes.append({"op": "line", "from": pt(7, a), "to": pt(R_RIM_IN, a), "width": 4, "style": "wood_outline"})
    shapes.append({"op": "line", "from": pt(7, a), "to": pt(R_RIM_IN, a), "width": 2, "style": "wood_body"})
    shapes.append({"op": "line", "from": pt(9, a - 2), "to": pt(R_RIM_IN - 2, a - 1), "width": 1, "style": "wood_light"})
# Rim: dark outline ring, body, a light inner edge.
shapes += ring(R_RIM_OUT + 1, R_RIM_IN - 1, "wood_outline")
shapes += ring(R_RIM_OUT, R_RIM_IN, "wood_body")
shapes += ring(R_RIM_IN + 1, R_RIM_IN, "wood_light")
# Iron straps round the rim with brass bolts.
for k in range(BUCKETS):
    a = 360 * k / BUCKETS + 11.25
    shapes.append({"op": "line", "from": pt(R_RIM_IN, a), "to": pt(R_RIM_OUT, a), "width": 1, "style": "iron_body"})
    shapes.append({"op": "pixel", "at": pt((R_RIM_IN + R_RIM_OUT) / 2, a), "style": "brass"})
# Buckets: a radial wall from the rim out to the bucket edge, and a lip that
# curls back over the cup. On the right-hand side going down (clockwise) the
# cups face up and hold water; at the bottom they tip it out.
for k in range(BUCKETS):
    a = 360 * k / BUCKETS
    shapes.append({"op": "line", "from": pt(R_RIM_OUT, a), "to": pt(R_BUCKET, a), "width": 3, "style": "wood_outline"})
    shapes.append({"op": "line", "from": pt(R_BUCKET, a), "to": pt(R_BUCKET - 1, a - 13), "width": 2, "style": "wood_outline"})
    shapes.append({"op": "line", "from": pt(R_RIM_OUT + 1, a - 1.5), "to": pt(R_BUCKET - 1, a - 1.5), "width": 1, "style": "wood_wet"})
# Hub: iron with a light edge and a ring of brass bolts.
shapes.append({"op": "circle", "at": [CX, CY], "radius": 8, "style": "iron_dark"})
shapes.append({"op": "circle", "at": [CX, CY], "radius": 7, "style": "iron_body"})
shapes.append({"op": "circle", "at": [CX, CY], "radius": 3, "style": "iron_edge"})
shapes.append({"op": "circle", "at": [CX, CY], "radius": 1, "style": "iron_dark"})
for k in range(8):
    shapes.append({"op": "pixel", "at": pt(5.5, 45 * k), "style": "brass"})

recipe = {
    "name": "millbrook_waterwheel",
    "width": W,
    "height": H,
    "depth": 0.12,
    "pivot": [CX, H - 1],
    "styles": styles,
    "parts": [{"name": "wheel", "shapes": shapes}],
    # The axle turns freely on a pin in the world (the stone piers are scenery).
    "joints": [f"hinge wheel ground at {CX + 0.5} {CY + 0.5}"],
}

out = ROOT / "assets/recipes/millbrook_waterwheel.json"
out.write_text(json.dumps(recipe, indent=1), encoding="utf-8")
args = [sys.executable, str(ROOT / "tools/asset_builder.py"), str(out)] + sys.argv[1:]
sys.exit(subprocess.call(args, cwd=ROOT))
