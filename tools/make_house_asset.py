"""Build an original, reusable two-storey cutaway within the 128-cell/24-part limits."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
W, H = 128, 120
parts = {}
welds = []


def cell(part, x, y, material="wood", color=None):
    if not (0 <= x < W and 0 <= y < H):
        return
    for cells in parts.values():
        cells.pop((x, y), None)
    parts.setdefault(part, {})[x, y] = material, color


def rect(part, x, y, w, h, material="wood", color=None):
    for yy in range(y, y + h):
        for xx in range(x, x + w):
            cell(part, xx, yy, material, color)


def line(part, a, b, radius=0, material="wood", color=None):
    steps = max(abs(b[0]-a[0]), abs(b[1]-a[1]), 1)
    for i in range(steps+1):
        x = round(a[0]+(b[0]-a[0])*i/steps)
        y = round(a[1]+(b[1]-a[1])*i/steps)
        rect(part, x-radius, y-radius, radius*2+1, radius*2+1, material, color)


def join(a, b, x, y, force=1200):
    welds.append(f"weld {a} {b} {x} {y} {force}")


def window(part, glass, x, y, post_x):
    # Window frames connect physically to the adjacent exterior post.
    rect(part, min(post_x, x), y, abs(post_x-x)+21, 2, color="d9b879")
    rect(part, x, y, 20, 18, color="d9b879")
    # The glazing is one continuous sheet. Painted glazing bars remain cosmetic.
    rect(glass, x+3, y+3, 14, 12, "glass", "77b7c8")
    for yy in range(y+3, y+15):
        cell(glass, x+9, yy, "glass", "ddd1ac")
    for xx in range(x+3, x+17):
        cell(glass, xx, y+8, "glass", "ddd1ac")
    join(part, glass, x+10, y+8, 90)


rect("foundation", 8, 112, 112, 5, "stone")
rect("ground_left_wall", 10, 76, 4, 36, color="7c5030")
rect("ground_left_wall", 10, 102, 4, 10, "brick")
window("ground_left_wall", "living_window", 17, 80, 13)
rect("ground_right_wall", 114, 76, 4, 36, color="7c5030")
rect("ground_right_wall", 114, 102, 4, 10, "brick")
window("ground_right_wall", "kitchen_window", 91, 80, 94)
rect("ground_partition", 62, 76, 4, 17, color="b99a6b")
rect("ground_partition", 62, 93, 2, 19, color="876239")
rect("first_floor_left", 10, 72, 54, 4, color="b2814e")
rect("first_floor_right", 64, 72, 54, 4, color="b2814e")
rect("upper_left_wall", 10, 38, 4, 34, color="7c5030")
window("upper_left_wall", "bedroom_window", 17, 42, 13)
rect("upper_right_wall", 114, 38, 4, 34, color="7c5030")
window("upper_right_wall", "study_window", 91, 42, 94)
rect("upper_partition", 62, 38, 4, 17, color="b99a6b")
rect("upper_partition", 62, 55, 2, 17, color="876239")
rect("attic_floor", 10, 35, 108, 3, color="97683c")

# Pitched timber rafters with individual clay-tile clusters on their upper edge.
for part, start, end in [("roof_left", (6, 34), (64, 7)),
                          ("roof_right", (65, 7), (122, 34))]:
    line(part, start, end, 1, color="68452c")
    for x in range(start[0], end[0]+1):
        y = round(start[1]+(end[1]-start[1])*(x-start[0])/(end[0]-start[0]))
        if x % 6 < 5:
            cell(part, x, y-2, "brick", "734638" if x % 6 else "a06448")
    # Roof ridge/rafters stay separate from the masonry chimney.
rect("chimney", 95, 10, 7, 25, "brick")
rect("chimney", 93, 9, 11, 3, "brick", "633b32")
# Timber flashing routes the right roof around the chimney opening.
line("roof_right", (94, 22), (94, 35), color="68452c")
line("roof_right", (94, 35), (102, 35), color="68452c")
line("roof_right", (102, 35), (102, 25), color="68452c")

# Closed, breakable doors. These are weld-mounted, not interactive hinges.
for name, y, h in [("ground_door", 94, 18), ("upper_door", 55, 17)]:
    rect(name, 64, y, 4, h, color="496a64")
    rect(name, 65, y+2, 2, h-4, color="6d9180")
    cell(name, 67, y+h//2, "brass", "d5b15b")

# A stair stringer carries its treads, so it is one connected timber part.
line("stairs", (70, 109), (88, 78), 1, color="8f6037")
for i in range(9):
    x, y = 70+i*2, 109-i*4
    rect("stairs", x-1, y, 6, 2, color="c09a63")

rect("bed", 18, 67, 23, 3, color="81512e")
rect("bed", 18, 61, 2, 11, color="a2784b")
rect("bed", 38, 67, 3, 5, color="81512e")
rect("bed", 20, 64, 18, 3, "cloth", "6d91ab")
rect("bed", 21, 62, 5, 2, "cloth", "e1d9b8")
rect("desk", 92, 65, 17, 2, color="c39354")
rect("desk", 94, 67, 2, 5, color="70492b")
rect("desk", 105, 67, 2, 5, color="70492b")
rect("sofa", 18, 105, 25, 5, color="70472d")
rect("sofa", 19, 101, 3, 6, "cloth", "a86743")
rect("sofa", 39, 101, 3, 6, "cloth", "a86743")
rect("sofa", 22, 104, 17, 3, "cloth", "c08052")
rect("sofa", 20, 110, 2, 2, color="70472d")
rect("sofa", 39, 110, 2, 2, color="70472d")
rect("kitchen", 94, 103, 17, 9, color="638577")
rect("kitchen", 93, 101, 19, 2, "iron", "adb5ad")
for x in (98, 106):
    rect("kitchen", x, 106, 2, 1, "brass", "d9bb7e")

# Load paths: roof -> attic -> upper walls -> floors -> ground posts -> foundation.
for wall, x in [("ground_left_wall", 12), ("ground_partition", 63), ("ground_right_wall", 116)]:
    join("foundation", wall, x, 111, 1800)
join("ground_left_wall", "first_floor_left", 12, 75, 1500)
join("ground_partition", "first_floor_left", 63, 75, 1500)
join("ground_partition", "first_floor_right", 64, 75, 1500)
join("ground_right_wall", "first_floor_right", 116, 75, 1500)
join("first_floor_left", "upper_left_wall", 12, 72, 1300)
join("first_floor_right", "upper_right_wall", 116, 72, 1300)
join("first_floor_left", "upper_partition", 63, 72, 1300)
for wall, x in [("upper_left_wall", 12), ("upper_partition", 63), ("upper_right_wall", 116)]:
    join(wall, "attic_floor", x, 38, 1300)
join("attic_floor", "roof_left", 12, 34, 900)
join("attic_floor", "roof_right", 116, 34, 900)
join("roof_left", "roof_right", 64, 7, 700)
join("attic_floor", "chimney", 98, 35, 650)
join("ground_partition", "ground_door", 64, 101, 120)
join("upper_partition", "upper_door", 64, 62, 120)
join("first_floor_right", "stairs", 88, 75, 450)

# Export exact material grids with separate cosmetic paint.
assert len(parts) <= 24, len(parts)
legend = {m: chr(65+i) for i, m in enumerate(sorted({m for cells in parts.values() for m, _ in cells.values()}))}
out = ["# Two-storey cutaway: timber structure, clay tiles, brick footings, real glass.",
       "format 1", f"asset two_storey_house {W} {H}", "depth 0.08", "pivot 64 117"]
out += [f"legend {symbol} {material}" for material, symbol in legend.items()]
for part, cells in parts.items():
    out += [f"part {part}", "grid"]
    out += ["".join(legend[cells[x, y][0]] if (x, y) in cells else "." for x in range(W)) for y in range(H)]
    out.append("end")
    out += [f"paint {x} {y} #{color}" for (x, y), (_, color) in sorted(cells.items(), key=lambda v:(v[0][1],v[0][0])) if color]
out += welds
(ROOT / "assets/workshop/two_storey_house.asset").write_text("\n".join(out)+"\n", encoding="utf-8")
