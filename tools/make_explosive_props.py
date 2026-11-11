"""Original cell art for the workshop's reusable explosive props.

Each visible cell has one physical material and an independent paint colour.
The two powder pockets are separate named bodies welded to the outer case.
"""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FONT = {"T": [31, 4, 4, 4, 4, 4, 4], "N": [17, 25, 25, 21, 19, 19, 17]}


class Prop:
    def __init__(self, name, w, h):
        self.name, self.w, self.h = name, w, h
        self.parts = {}
        self.welds = []

    def cell(self, part, material, x, y, color):
        for cells in self.parts.values():
            cells.pop((x, y), None)
        self.parts.setdefault(part, {})[x, y] = (material, color)

    def rect(self, part, material, x0, y0, w, h, color):
        for y in range(y0, y0 + h):
            for x in range(x0, x0 + w):
                self.cell(part, material, x, y, color)

    def tint(self, x, y, color):
        for cells in self.parts.values():
            if (x, y) in cells:
                material, _ = cells[x, y]
                cells[x, y] = material, color

    def letters(self, x0, y0, color):
        for i, ch in enumerate("TNT"):
            for y, bits in enumerate(FONT[ch]):
                for x in range(5):
                    if bits & (1 << (4 - x)):
                        self.tint(x0 + i * 6 + x, y0 + y, color)

    def save(self):
        legend = {"wood": "W", "iron": "I", "gunpowder": "G"}
        out = ["# Reusable explosive prop. Paint is cosmetic; powder is real.",
               "format 1", f"asset {self.name} {self.w} {self.h}",
               "depth 0.04", f"pivot {self.w / 2:g} {self.h / 2:g}"]
        out += [f"legend {symbol} {material}" for material, symbol in legend.items()]
        for part, cells in self.parts.items():
            out += [f"part {part}", "grid"]
            for y in range(self.h):
                out.append("".join(legend[cells[x, y][0]] if (x, y) in cells else "."
                                   for x in range(self.w)))
            out.append("end")
            for (x, y), (_, color) in sorted(cells.items(), key=lambda v: (v[0][1], v[0][0])):
                out.append(f"paint {x} {y} #{color}")
        out += self.welds
        (ROOT / "assets/workshop" / f"{self.name}.asset").write_text("\n".join(out) + "\n", encoding="utf-8")


def crate():
    p = Prop("tnt_box", 48, 40)
    p.rect("wooden_case", "wood", 4, 4, 40, 32, "986038")
    for part, y in [("powder_upper", 8), ("powder_lower", 27)]:
        p.rect(part, "gunpowder", 8, y, 32, 5, "986038")
        p.welds.append(f"weld wooden_case {part} 24 {y + 2} 40")
    # Four broad planks, darker seams, bevelled perimeter and restrained grain.
    for y in range(4, 36):
        for x in range(4, 44):
            c = ["a66c40", "955b32", "ad7548", "9e6439"][(x - 4) // 10]
            if x in (13, 23, 33): c = "643d25"
            elif (x * 13 + y * 7) % 37 == 0: c = "80502f"
            if x in (4, 43) or y in (4, 35): c = "513420"
            elif x == 5 or y == 5: c = "c08a55"
            p.tint(x, y, c)
    # Actual wooden label plank, painted red, with black lettering.
    for y in range(15, 25):
        for x in range(6, 42):
            p.tint(x, y, "912521" if y in (15, 24) else "c6382b")
    p.letters(15, 16, "101014")
    # Separate metal corner straps have their own material and breakable welds.
    for name, x, y in [("top_left", 6, 6), ("top_right", 40, 6),
                       ("bottom_left", 6, 31), ("bottom_right", 40, 31)]:
        p.rect(name, "iron", x, y, 2, 3, "5f6971")
        p.tint(x, y, "abb4b9")
        p.tint(x + 1, y + 1, "252a31")
        p.welds.append(f"weld wooden_case {name} {x + 1} {y + 1} 65")
    p.save()


def barrel():
    p = Prop("explosive_barrel", 36, 48)
    for y in range(4, 44):
        left = 10 if y in (4, 43) else 8 if y in (5, 6, 41, 42) else 6
        for x in range(left, 36 - left):
            p.cell("steel_shell", "iron", x, y, "a12c27")
    for part, y in [("powder_upper", 11), ("powder_lower", 33)]:
        p.rect(part, "gunpowder", 10, y, 16, 6, "bb3830")
        p.welds.append(f"weld steel_shell {part} 18 {y + 3} 55")
    for y in range(4, 44):
        for x in range(6, 30):
            c = "c24033" if x < 24 else "892825"
            if x in (7, 8): c = "e36345"
            if x in (28, 29): c = "5e2424"
            if y in (4, 5, 6, 7): c = "944039" if y < 7 else "5d2525"
            if y in (9, 19, 30, 40): c = "aeb4b6"
            if y in (10, 20, 31, 41): c = "4b535d"
            if y == 43: c = "303640"
            p.tint(x, y, c)
    # Warning panel and black stencilled TNT on the steel middle section.
    for y in range(22, 30):
        for x in range(8, 29):
            p.tint(x, y, "e6b84b")
    p.letters(10, 22, "15191f")
    for x, y in [(24, 5), (25, 5), (24, 6), (25, 6)]: p.tint(x, y, "222b32")
    for x, y in [(11, 14), (12, 14), (11, 15), (27, 35), (27, 36)]: p.tint(x, y, "d78346")
    p.save()


if __name__ == "__main__":
    crate()
    barrel()
