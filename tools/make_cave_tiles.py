"""Draw the hand-made cave tile set (tiles/caves/) for the level generator
(src/levelgen.rs), check every doorway, and make a contact sheet picture.

Run:  python tools/make_cave_tiles.py

Wide tiles are 16 x 8 characters, tall ones 8 x 16: the same grid the built-in
tiles use, so doorways line up exactly. A doorway is 3 characters at the
middle of an edge piece (positions 3-5 of its 8), kept clear 2 deep; a closed
edge is solid there. Each tile is also used mirrored left to right.
"""
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "tiles" / "caves"

LEGEND = {
    "#": "ground", "b": "brick", "s": "stone", "C": "cobblestone", "w": "wood",
    "~": "water", "L": "lava", "o": "oil", "g": "glass", "r": "gravel",
    "p": "gunpowder", "$": "brass", "c": "copper", "I": "iron",
}

# name: (shape, edges, about, rows). Edges, clockwise from the top left:
#   wide (h): top-left top-right right bottom-right bottom-left left
#   tall (v): top right-upper right-lower bottom left-lower left-upper
TILES = {
    "hall": ("h", "001001", "A great hall with a cobbled floor.", [
        "################",
        "#####......#####",
        "##............##",
        "................",
        "................",
        "................",
        "CCCCCCCCCCCCCCCC",
        "################"]),
    "beams": ("h", "001001", "A hall with timber beams under the roof.", [
        "################",
        "##wwwwwwwwwwww##",
        "##.w...ww...w.##",
        "................",
        "................",
        "................",
        "################",
        "################"]),
    "mine": ("h", "001001", "A mine tunnel: timber roof, iron rails.", [
        "################",
        "################",
        "#wwwwwwwwwwwwww#",
        "...w...w...w....",
        "................",
        "................",
        "IIIIIIIIIIIIIIII",
        "################"]),
    "bridge": ("h", "001101", "A broken wooden bridge: jump the gap, or drop through it.", [
        "################",
        "################",
        "###..........###",
        "................",
        "................",
        "................",
        "####wwwwwww...##",
        "######.........#"]),
    "pool": ("h", "001001", "A flooded dip in the floor.", [
        "################",
        "################",
        "###..........###",
        "................",
        "................",
        "................",
        "###~~~~~~~~~~###",
        "######~~~~######"]),
    "lava_pit": ("h", "001001", "A lava pit to jump over.", [
        "################",
        "#####......#####",
        "###..........###",
        "................",
        "................",
        "................",
        "######...#######",
        "######LLL#######"]),
    "crossroads": ("h", "111111", "Ways in from every side.", [
        "###...#####...##",
        "###...#####...##",
        "##............##",
        "................",
        "................",
        "................",
        "###...#####...##",
        "###...#####...##"]),
    "vault": ("h", "000001", "A brick treasure vault, a dead end.", [
        "################",
        "##bbbbbbbbbbbb##",
        "##b..........b##",
        ".............b##",
        ".............b##",
        "......$...$..b##",
        "##bbbbbbbbbbbb##",
        "################"]),
    "storeroom": ("h", "001000", "A storeroom of gunpowder kegs. Mind the fire.", [
        "################",
        "################",
        "##ww..........##",
        "##w.............",
        "##.pp..w.pp.....",
        "##.pp.www.pp....",
        "##ssssssssssssss",
        "################"]),
    "waterfall": ("h", "100100", "Water spills in from above and drains away.", [
        "###...##########",
        "###...##########",
        "###...~~.....###",
        "###.~~~~......##",
        "##..~~~.......##",
        "##.............#",
        "#######........#",
        "###########...##"]),
    "crystals": ("h", "100001", "A grotto of glass crystals.", [
        "###...##########",
        "###...##########",
        "##..g...g..g..##",
        "...........g...#",
        "...............#",
        ".......gg......#",
        "#####gggggg#####",
        "################"]),
    "oil_seep": ("h", "000101", "Oil seeping from the rock. Don't bring a torch.", [
        "################",
        "################",
        "###.....########",
        "...........#####",
        "...........#####",
        "............####",
        "#####oooo#....##",
        "###########...##"]),
    "steps": ("h", "010010", "Rock steps from low left to high right.", [
        "###########...##",
        "###########...##",
        "#########.....##",
        "#######.....####",
        "#####.....######",
        "###.....########",
        "###...##########",
        "###...##########"]),
    "collapse": ("h", "001001", "A tunnel half blocked by a rockfall.", [
        "################",
        "####......######",
        "##.........#####",
        "...........rr...",
        "..........rrrr..",
        ".........rrrrr..",
        "################",
        "################"]),
    "shrine": ("h", "110000", "A stone shrine, reached from above.", [
        "###...#####...##",
        "###...#####...##",
        "##s..........s##",
        "##s..........s##",
        "##s....$$....s##",
        "##s...ssss...s##",
        "##ssssssssssss##",
        "################"]),
    "ore_den": ("h", "000001", "A dead end rich in copper.", [
        "################",
        "#####cc#########",
        "##.....c########",
        ".........#######",
        "..........c#####",
        "...........#####",
        "############c###",
        "################"]),
    "shaft": ("v", "100100", "A climbing shaft with wooden ledges.", [
        "###...##",
        "###...##",
        "#ww....#",
        "#......#",
        "#....ww#",
        "#......#",
        "#ww....#",
        "#......#",
        "#....ww#",
        "#......#",
        "#ww....#",
        "#......#",
        "#....ww#",
        "#......#",
        "###...##",
        "###...##"]),
    "well": ("v", "100000", "A deep well, water at the bottom.", [
        "###...##",
        "###...##",
        "##....##",
        "##....##",
        "##....##",
        "##....##",
        "##....##",
        "##....##",
        "#......#",
        "#......#",
        "#~~~~~~#",
        "#~~~~~~#",
        "#~~~~~~#",
        "#~~~~~~#",
        "#~~~~~~#",
        "########"]),
    "tower": ("v", "001001", "A brick tower: in at the top left, down and out at the bottom right.", [
        "########",
        "#bbbbbb#",
        "#b....b#",
        "......b#",
        "......b#",
        "......b#",
        "#bbb..b#",
        "#b....b#",
        "#bw...b#",
        "#b....b#",
        "#b....b#",
        "#b......",
        "#b......",
        "#b......",
        "#bbbbbb#",
        "########"]),
    "drop": ("v", "000101", "A drop: in from the side, fall down the shaft.", [
        "########",
        "########",
        "##.....#",
        ".......#",
        ".......#",
        ".......#",
        "####...#",
        "###....#",
        "###....#",
        "##.....#",
        "##.....#",
        "#......#",
        "#......#",
        "#......#",
        "###...##",
        "###...##"]),
    "alcove": ("v", "010000", "A treasure alcove, iron in the walls.", [
        "########",
        "########",
        "###....#",
        "##......",
        "#.......",
        "#..$....",
        "#ssssss#",
        "########",
        "###I####",
        "########",
        "#####I##",
        "########",
        "##I#####",
        "########",
        "########",
        "########"]),
    "tall_crossroads": ("v", "111111", "Ways in from every side.", [
        "###...##",
        "###...##",
        "#......#",
        "........",
        "........",
        "........",
        "##....##",
        "##....##",
        "##....##",
        "#......#",
        "#......#",
        "........",
        "........",
        "........",
        "###...##",
        "###...##"]),
    "cavern": ("v", "101010", "A tall cavern with stone teeth hanging from the roof.", [
        "###...##",
        "##.....#",
        "#......#",
        "#..s...#",
        "#..s..s#",
        "#.....s#",
        "#......#",
        "#......#",
        "#......#",
        "#......#",
        "#......#",
        "........",
        "........",
        "........",
        "#ssssss#",
        "########"]),
    "zigzag": ("v", "110110", "A zigzag climb with ways off to the sides.", [
        "###...##",
        "###...##",
        "#......#",
        "#.......",
        "#.......",
        "#ww.....",
        "#....ww#",
        "#......#",
        "#ww....#",
        "#......#",
        "#....ww#",
        ".......#",
        ".......#",
        ".......#",
        "###...##",
        "###...##"]),
}


def doorways(shape):
    """For each of the six edge pieces: the (x, y) cells of its doorway,
    2 deep, in edge order."""
    def run(x0, y0, dx, dy, nx, ny):
        return [(x0 + dx * a + nx * d, y0 + dy * a + ny * d) for a in (3, 4, 5) for d in (0, 1)]
    if shape == "h":
        return [run(0, 0, 1, 0, 0, 1), run(8, 0, 1, 0, 0, 1), run(15, 0, 0, 1, -1, 0),
                run(8, 7, 1, 0, 0, -1), run(0, 7, 1, 0, 0, -1), run(0, 0, 0, 1, 1, 0)]
    return [run(0, 0, 1, 0, 0, 1), run(7, 0, 0, 1, -1, 0), run(7, 8, 0, 1, -1, 0),
            run(0, 15, 1, 0, 0, -1), run(0, 8, 0, 1, 1, 0), run(0, 0, 0, 1, 1, 0)]


def check(name, shape, edges, rows):
    w, h = (16, 8) if shape == "h" else (8, 16)
    problems = []
    if len(rows) != h or any(len(r) != w for r in rows):
        return [f"{name}: must be {w} x {h} (got {[len(r) for r in rows]})"]
    for r in rows:
        for c in r:
            if c != "." and c not in LEGEND:
                problems.append(f"{name}: `{c}` isn't in the legend")
    for k, cells in enumerate(doorways(shape)):
        is_open = edges[k] == "1"
        border = [cells[i] for i in range(0, 6, 2)]
        for (x, y) in (cells if is_open else border):
            c = rows[y][x]
            if is_open and c != ".":
                problems.append(f"{name}: edge {k} is open but ({x},{y}) is `{c}`")
            if not is_open and c == ".":
                problems.append(f"{name}: edge {k} is closed but ({x},{y}) is empty")
    return problems


def write():
    OUT.mkdir(parents=True, exist_ok=True)
    problems = []
    for name, (shape, edges, about, rows) in TILES.items():
        problems += check(name, shape, edges, rows)
        used = sorted({c for r in rows for c in r if c != "."})
        text = [f"# {about}", "# (Made by tools/make_cave_tiles.py: edit there and run it again.)",
                f"shape {shape}", "edges " + " ".join(edges)]
        text += [f"legend {c} {LEGEND[c]}" for c in used]
        text += ["rows", *rows, "end", ""]
        (OUT / f"{name}.txt").write_text("\n".join(text), encoding="utf-8", newline="\n")
    readme = OUT / "README.md"
    readme.write_text(
        "# Cave tiles\n\nThe hand-made tile set for `generate ... tiles tiles/caves` (src/levelgen.rs),\n"
        "drawn by `tools/make_cave_tiles.py` (edit tiles there, run it again: it checks\n"
        "every doorway and redraws `contact-sheet.png`). Format: see `tiles/rooms/README.md`.\n"
        "Each tile is also used mirrored left to right.\n", encoding="utf-8", newline="\n")
    return problems


COLOURS = {
    ".": (40, 42, 52), "#": (110, 108, 104), "b": (168, 74, 52), "s": (150, 150, 150),
    "C": (128, 122, 110), "w": (150, 104, 60), "~": (50, 100, 200), "L": (240, 110, 30),
    "o": (70, 56, 30), "g": (170, 220, 230), "r": (120, 112, 100), "p": (60, 60, 60),
    "$": (220, 180, 60), "c": (200, 110, 60), "I": (150, 150, 160),
}


def sheet():
    """Contact sheet: every tile, doorways marked, like Sean Barrett's."""
    try:
        from PIL import Image, ImageDraw
    except ImportError:
        return
    px, pad = 8, 6
    wide = [t for t in TILES.items() if t[1][0] == "h"]
    tall = [t for t in TILES.items() if t[1][0] == "v"]
    cols = 4
    cw, ch = 16 * px + pad, 8 * px + pad
    rows_w = (len(wide) + cols - 1) // cols
    tw, th = 8 * px + pad, 16 * px + pad
    tcols = 8
    rows_t = (len(tall) + tcols - 1) // tcols
    W = max(cols * cw, tcols * tw) + pad
    H = rows_w * ch + rows_t * th + pad * 2
    img = Image.new("RGB", (W, H), (200, 0, 200))
    d = ImageDraw.Draw(img)

    def draw(x0, y0, rows):
        for y, r in enumerate(rows):
            for x, c in enumerate(r):
                d.rectangle([x0 + x * px, y0 + y * px, x0 + x * px + px - 1, y0 + y * px + px - 1], fill=COLOURS[c])

    for n, (name, (shape, edges, about, rows)) in enumerate(wide):
        draw(pad + (n % cols) * cw, pad + (n // cols) * ch, rows)
    y_t = pad + rows_w * ch
    for n, (name, (shape, edges, about, rows)) in enumerate(tall):
        draw(pad + (n % tcols) * tw, y_t + (n // tcols) * th, rows)
    img.save(OUT / "contact-sheet.png")


if __name__ == "__main__":
    problems = write()
    sheet()
    for p in problems:
        print("PROBLEM:", p)
    print(f"{len(TILES)} tiles written to {OUT.relative_to(ROOT)}" + (f", {len(problems)} problems" if problems else ", all doorways OK"))
