"""Draw the Grave Engine icon: a pixel-art tombstone with a teal cross,
a flame on top (the engine's fire) and a strip of earth. 32x32 pixels,
saved as assets/icon.png (scaled up, sharp) and assets/icon.ico (Windows).

Run: py tools/make_icon.py
"""
from PIL import Image

N = 32
img = Image.new("RGBA", (N, N), (0, 0, 0, 0))
px = img.load()


def put(x, y, c):
    if 0 <= x < N and 0 <= y < N:
        px[x, y] = c + (255,) if len(c) == 3 else c


STONE = (128, 134, 146)
STONE_LIGHT = (168, 174, 186)
STONE_DARK = (78, 82, 94)
OUTLINE = (30, 30, 38)
TEAL = (105, 221, 204)
TEAL_DARK = (40, 140, 128)
EARTH = (92, 64, 44)
EARTH_DARK = (60, 40, 28)
GRASS = (88, 150, 70)

# Tombstone: x 7..24, top rounded, from y 9 to 27.
left, right, top, bottom = 7, 24, 9, 27
cx = (left + right) / 2
r = (right - left) / 2
inside = set()
for y in range(top, bottom + 1):
    for x in range(left, right + 1):
        if y < top + r:
            if (x + 0.5 - cx - 0.5) ** 2 + (y - (top + r)) ** 2 > r * r:
                continue
        inside.add((x, y))
for (x, y) in inside:
    edge = any((x + dx, y + dy) not in inside for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))
    if edge:
        put(x, y, OUTLINE)
    else:
        # Light from the top left.
        shade = STONE_LIGHT if x < left + 4 or y < top + 3 else STONE
        if x > right - 3:
            shade = STONE_DARK
        put(x, y, shade)
# A small crack.
for (x, y) in [(20, 22), (21, 23), (20, 24), (21, 25)]:
    put(x, y, STONE_DARK)
# Glowing teal cross.
for y in range(13, 24):
    put(15, y, TEAL)
    put(16, y, TEAL)
for x in range(12, 20):
    put(x, 16, TEAL)
    put(x, 17, TEAL)
for (x, y) in [(14, 13), (17, 13), (11, 16), (20, 16), (11, 17), (20, 17), (14, 23), (17, 23)]:
    put(x, y, TEAL_DARK)

# Flame on top of the stone.
flame = {
    3: [(15, 16)],
    4: [(15, 16)],
    5: [(14, 17)],
    6: [(14, 17)],
    7: [(13, 18)],
    8: [(13, 18)],
}
for y, spans in flame.items():
    for (a, b) in spans:
        for x in range(a, b + 1):
            core = abs(x - 15.5) < (1.2 if y > 5 else 0.6)
            put(x, y, (255, 236, 140) if core else (255, 140, 40) if y > 4 else (255, 190, 70))
for (x, y) in [(12, 8), (19, 8), (12, 7), (19, 7)]:
    put(x, y, (200, 70, 30))

# Earth and grass.
for y in range(27, 31):
    for x in range(3, 29):
        put(x, y, EARTH if (x + y) % 3 else EARTH_DARK)
for x in range(3, 29):
    put(x, 27, GRASS if x % 2 else (70, 124, 56))
for x in range(left, right + 1):
    put(x, 27, OUTLINE)

big = img.resize((256, 256), Image.NEAREST)
big.save("assets/icon.png")
img.resize((256, 256), Image.NEAREST).save(
    "assets/icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
)
print("saved assets/icon.png and assets/icon.ico")
