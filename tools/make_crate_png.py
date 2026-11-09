# Writes assets/crate.png, an example image to import (plain Python, no extra
# libraries). Colours are the materials' own `color` values, so each pixel maps
# straight to wood or iron. Transparent pixels are empty space.

import os
import struct
import zlib

WOOD = (0x7A, 0x4A, 0x26, 255)
IRON = (0x8A, 0x8F, 0x99, 255)
NONE = (0, 0, 0, 0)

W = H = 24
px = [[WOOD for _ in range(W)] for _ in range(H)]
for i in range(W):
    for t in range(2):
        px[t][i] = px[H - 1 - t][i] = IRON  # top and bottom bands
        px[i][t] = px[i][W - 1 - t] = IRON  # sides
for c in [(0, 0), (0, W - 1), (H - 1, 0), (H - 1, W - 1)]:
    px[c[0]][c[1]] = NONE  # rounded corners
for i in range(3, W - 3):
    px[i][i] = IRON  # diagonal brace
    px[i][i - 1] = IRON


def write_png(path, rows):
    raw = b"".join(b"\x00" + bytes(v for p in row for v in p) for row in rows)

    def chunk(kind, data):
        c = kind + data
        return struct.pack(">I", len(data)) + c + struct.pack(">I", zlib.crc32(c) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", len(rows[0]), len(rows), 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    open(path, "wb").write(png)


out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "crate.png")
write_png(os.path.abspath(out), px)
print("wrote", os.path.abspath(out))
