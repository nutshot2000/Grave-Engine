# Writes the example material textures in materials/textures/ (plain Python,
# no extra libraries). Each is a small tiling picture; Grave Engine repeats it
# across everything made of that material. Swap in your own pictures any time.

import os
import random
import struct
import zlib

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "materials", "textures")
os.makedirs(OUT, exist_ok=True)


def write_png(name, rows):
    raw = b"".join(b"\x00" + bytes(v for p in row for v in (*p, 255)) for row in rows)

    def chunk(kind, data):
        c = kind + data
        return struct.pack(">I", len(data)) + c + struct.pack(">I", zlib.crc32(c) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", len(rows[0]), len(rows), 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    open(os.path.join(OUT, name), "wb").write(png)


def clamp(v):
    return max(0, min(255, int(v)))


def shade(c, k):
    return tuple(clamp(x * k) for x in c)


rnd = random.Random(7)

# Wood: planks 6 cells tall with long grain streaks and a dark seam.
W, H = 48, 24
base = (122, 74, 38)
rows = []
grain = [[rnd.uniform(0.9, 1.1) for _ in range(W)] for _ in range(H)]
for y in range(H):
    row = []
    stripe = 1.0 + 0.08 * ((y * 7) % 3 - 1)
    for x in range(W):
        g = (grain[y][x] + grain[y][(x + 1) % W] + grain[y][(x + 2) % W]) / 3  # streaky along x
        k = stripe * g
        if y % 6 == 5:
            k *= 0.7  # seam between planks
        if (x + (y // 6) * 17) % 24 == 0:
            k *= 0.75  # plank ends, staggered
        row.append(shade(base, k))
    rows.append(row)
write_png("wood.png", rows)

# Stone: mottled with a few darker cracks.
W = H = 32
base = (107, 102, 112)
noise = [[rnd.uniform(0.85, 1.12) for _ in range(W)] for _ in range(H)]
rows = []
for y in range(H):
    row = []
    for x in range(W):
        n = sum(noise[(y + dy) % H][(x + dx) % W] for dy in (-1, 0, 1) for dx in (-1, 0, 1)) / 9
        k = n
        if (x * 3 + y * 5) % 37 == 0 or (x * 7 - y * 2) % 41 == 0:
            k *= 0.8
        row.append(shade(base, k))
    rows.append(row)
write_png("stone.png", rows)

# Iron: brushed metal streaks with a faint rivet line.
W, H = 32, 16
base = (138, 143, 153)
rows = []
streak = [rnd.uniform(0.94, 1.06) for _ in range(H)]
for y in range(H):
    row = []
    for x in range(W):
        k = streak[y] * rnd.uniform(0.97, 1.03)
        if y == 2 and x % 8 == 3:
            k *= 1.18  # rivet
        row.append(shade(base, k))
    rows.append(row)
write_png("iron.png", rows)
print("wrote textures to", os.path.abspath(OUT))
