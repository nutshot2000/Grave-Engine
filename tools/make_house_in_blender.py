"""Build a detailed timber-and-iron house in Blender, as a cutaway (the front
taken off, like a doll's house) so the game sees its rooms from the side.

Every part uses a Blender material named after a Grave Engine material
(wood, iron, brass, brick, stone), so importing it picks the right one.

Run inside Blender (the scene "Grave House" is rebuilt, nothing else is
touched), or in the background to export the model:

    blender -b --factory-startup -P tools/make_house_in_blender.py -- assets/house/timber-house.glb

Then `object assets/house/timber-house.glb X Y static` in a scene (see
scenes/house.txt). One metre = 10 cells.
"""
import math
import sys

import bpy

SCENE = "Grave House"

# name: (colour, metallic, roughness)
MATERIALS = {
    "wood": ((0.45, 0.27, 0.13), 0.0, 0.8),
    "iron": ((0.42, 0.44, 0.47), 1.0, 0.45),
    "brass": ((0.80, 0.62, 0.25), 1.0, 0.3),
    "brick": ((0.55, 0.22, 0.15), 0.0, 0.9),
    "stone": ((0.45, 0.45, 0.44), 0.0, 0.95),
}


def material(name):
    m = bpy.data.materials.get(name) or bpy.data.materials.new(name)
    colour, metal, rough = MATERIALS[name]
    m.diffuse_color = (*colour, 1.0)
    m.metallic = metal
    m.roughness = rough
    m.use_nodes = True
    bsdf = m.node_tree.nodes.get("Principled BSDF")
    if bsdf:
        bsdf.inputs["Base Color"].default_value = (*colour, 1.0)
        bsdf.inputs["Metallic"].default_value = metal
        bsdf.inputs["Roughness"].default_value = rough
    return m


def scene(into=None):
    """The scene to build in: `into` (cleared), or "Grave House" (made or cleared)."""
    sc = into or bpy.data.scenes.get(SCENE) or bpy.data.scenes.new(SCENE)
    for o in list(sc.objects):
        bpy.data.objects.remove(o, do_unlink=True)
    return sc


class House:
    def __init__(self, sc):
        self.sc = sc
        self.coll = sc.collection
        self.n = 0

    def box(self, mat, x0, z0, x1, z1, y0=-0.2, y1=0.2, name=None):
        """An upright box from (x0, z0) to (x1, z1) in metres."""
        return self.bar(mat, (x0 + x1) / 2, (z0 + z1) / 2, abs(x1 - x0), abs(z1 - z0), 0.0, y0, y1, name)

    def bar(self, mat, cx, cz, length, thick, angle_deg, y0=-0.2, y1=0.2, name=None):
        """A box centred at (cx, cz), `length` along its axis, turned by
        `angle_deg` (counter-clockwise seen from the front)."""
        self.n += 1
        mesh = bpy.data.meshes.new(f"{mat}_{self.n}")
        hx, hz, hy = length / 2, thick / 2, (y1 - y0) / 2
        v = [(sx * hx, sy * hy, sz * hz) for sx in (-1, 1) for sy in (-1, 1) for sz in (-1, 1)]
        f = [(0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (2, 3, 7, 6), (0, 2, 6, 4), (1, 5, 7, 3)]
        mesh.from_pydata(v, [], f)
        mesh.materials.append(material(mat))
        o = bpy.data.objects.new(name or f"{mat}_{self.n}", mesh)
        o.location = (cx, (y0 + y1) / 2, cz)
        o.rotation_euler = (0.0, -math.radians(angle_deg), 0.0)
        self.coll.objects.link(o)
        return o

    def beam(self, mat, a, b, thick, **kw):
        """A bar from point a to point b (x, z)."""
        (ax, az), (bx, bz) = a, b
        length = math.hypot(bx - ax, bz - az)
        angle = math.degrees(math.atan2(bz - az, bx - ax))
        return self.bar(mat, (ax + bx) / 2, (az + bz) / 2, length, thick, angle, **kw)


def build(into=None):
    sc = scene(into)
    h = House(sc)
    W = 16.0  # house width (m)
    # Front parts sit nearer the viewer (more negative y) so they show in front.
    front = dict(y0=-0.5, y1=-0.25)

    # --- foundation and cellar floor
    h.box("stone", -0.4, 0.0, W + 0.4, 0.6)
    for x in (2.0, 6.0, 10.0, 14.0):
        h.box("stone", x - 0.3, 0.6, x + 0.3, 0.75)  # pads under the posts

    # --- ground floor (z 0.75 .. 4.0)
    h.box("wood", 0.0, 0.75, W, 0.95)  # floorboards
    # left wall: brick, window opening 1.6 .. 2.8
    h.box("brick", 0.0, 0.95, 0.5, 1.6)
    h.box("brick", 0.0, 2.8, 0.5, 4.0)
    h.box("wood", -0.05, 1.5, 0.55, 1.6)  # sill
    h.box("wood", -0.05, 2.8, 0.55, 2.9)  # lintel
    # right wall: timber frame with a door opening 0.95 .. 3.1
    h.box("wood", W - 0.4, 0.95, W, 0.95 + 0.3)  # threshold step
    h.box("wood", W - 0.4, 3.1, W, 4.0)  # above the door
    h.box("brass", W - 0.42, 1.4, W - 0.32, 1.6)  # hinges
    h.box("brass", W - 0.42, 2.5, W - 0.32, 2.7)
    # iron column holding up the middle of the floor above
    h.box("iron", 7.85, 0.95, 8.15, 3.75)
    h.box("iron", 7.6, 3.6, 8.4, 3.75)  # cap plate
    # kitchen: iron stove with a flue up the chimney
    h.box("iron", 1.0, 0.95, 2.4, 2.0, **front)
    h.box("iron", 0.9, 2.0, 2.5, 2.15, **front)  # hot plate
    h.box("brass", 1.15, 1.2, 1.5, 1.35, **front)  # door handle
    h.box("iron", 1.55, 2.15, 1.75, 3.75, **front)  # flue
    # table and two stools
    h.box("wood", 3.3, 2.0, 5.7, 2.15, **front)
    for x in (3.5, 5.4):
        h.box("wood", x, 0.95, x + 0.15, 2.0, **front)
    for x in (2.8, 6.0):
        h.box("wood", x - 0.25, 1.45, x + 0.25, 1.55, **front)
        h.box("wood", x - 0.05, 0.95, x + 0.05, 1.45, **front)
    # brass oil lamp hanging over the table on an iron chain
    h.box("iron", 4.47, 3.0, 4.53, 3.75, **front)
    h.box("brass", 4.3, 2.75, 4.7, 3.0, **front)
    # crates by the stairs
    h.box("wood", 9.0, 0.95, 10.0, 1.95, **front)
    h.box("wood", 9.2, 1.95, 9.9, 2.6, **front)
    # stairs up to the first floor: wooden treads on an iron stringer
    steps = 10
    x0, z0, run, rise = 10.4, 0.95, 0.42, (4.0 - 0.95) / steps
    for i in range(steps):
        h.box("wood", x0 + i * run, z0 + i * rise, x0 + (i + 1) * run + 0.08, z0 + (i + 1) * rise, **front)
    h.beam("iron", (x0, z0), (x0 + steps * run, 4.0), 0.18, y0=-0.55, y1=-0.45)

    # --- first floor (z 4.0 .. 7.4) on iron I-beams
    h.box("iron", 0.0, 3.75, W, 3.85)  # I-beam bottom flange
    h.box("iron", 0.0, 3.85, W, 3.92)
    h.box("wood", 0.0, 3.92, x0 + steps * run - 4.3, 4.12)  # floor, with the stairwell open
    h.box("wood", x0 + steps * run, 3.92, W, 4.12)
    # balcony sticking out on the right, iron railing
    h.box("wood", W, 3.92, W + 2.4, 4.12)
    h.box("iron", W, 3.75, W + 2.4, 3.92)
    for x in (W + 0.6, W + 1.2, W + 1.8, W + 2.3):
        h.box("iron", x - 0.06, 4.05, x + 0.06, 5.25, **front)
    h.box("iron", W - 0.3, 5.1, W + 2.4, 5.25, **front)
    h.beam("iron", (W, 2.6), (W + 2.2, 3.75), 0.15)  # brace under it
    for x in (W + 1.0, W + 2.25):  # iron posts holding it up, on stone pads
        h.box("iron", x - 0.1, 0.6, x + 0.1, 3.75)
    h.box("stone", W + 0.4, 0.0, W + 2.6, 0.6)
    # walls: brick left (window), timber right
    h.box("brick", 0.0, 4.12, 0.5, 5.0)
    h.box("brick", 0.0, 6.4, 0.5, 7.4)
    h.box("wood", -0.05, 4.9, 0.55, 5.0)
    h.box("wood", W - 0.3, 4.12, W, 7.3)  # timber end wall up to the roof beam
    # timber posts, studs and a cross brace (the middle wall)
    h.box("wood", 8.0, 4.12, 8.3, 7.3)
    h.beam("wood", (8.15, 4.3), (11.0, 7.1), 0.15, y0=-0.1, y1=0.1)
    # bedroom: bed with an iron frame, wooden chest, bookshelf
    h.box("iron", 1.0, 4.12, 1.1, 5.4, **front)  # headboard post
    h.box("iron", 3.6, 4.12, 3.7, 5.0, **front)
    h.box("iron", 1.0, 4.6, 3.7, 4.7, **front)
    h.box("wood", 1.1, 4.7, 3.6, 4.95, **front)  # mattress board
    h.box("brass", 0.95, 5.4, 1.15, 5.55, **front)  # knob
    h.box("wood", 4.4, 4.12, 5.6, 4.9, **front)  # chest
    h.box("iron", 4.4, 4.5, 5.6, 4.56, **front)  # its iron band
    for z in (4.12, 5.0, 5.9, 6.8):  # bookshelf
        h.box("wood", 12.6, z, 14.2, z + 0.1, **front)
    h.box("wood", 12.5, 4.12, 12.6, 6.9, **front)
    h.box("wood", 14.2, 4.12, 14.3, 6.9, **front)

    # --- attic floor and roof truss
    h.box("iron", -0.3, 7.25, W + 0.3, 7.4)  # tie: iron beam
    h.box("wood", -0.3, 7.4, W + 0.3, 7.6)
    ridge = (W / 2, 11.6)
    eaves_l, eaves_r = (-0.8, 7.1), (W + 0.8, 7.1)
    h.beam("wood", eaves_l, ridge, 0.3)  # rafters
    h.beam("wood", eaves_r, ridge, 0.3)
    for x0, x1 in ((-1.0, -0.2), (W + 0.2, W + 1.0)):  # eave boards tie the rafter tips in
        h.box("wood", x0, 6.8, x1, 7.6)
    # iron roof sheets on top of the rafters
    def sheet(a, b):
        dx, dz = b[0] - a[0], b[1] - a[1]
        l = math.hypot(dx, dz)
        nx, nz = -dz / l * 0.22, dx / l * 0.22
        if nz < 0:
            nx, nz = -nx, -nz
        h.beam("iron", (a[0] + nx, a[1] + nz), (b[0] + nx, b[1] + nz), 0.1)
    sheet(eaves_l, ridge)
    sheet(eaves_r, ridge)
    h.box("iron", ridge[0] - 0.3, ridge[1] + 0.1, ridge[0] + 0.3, ridge[1] + 0.35)  # ridge cap
    h.box("wood", W / 2 - 0.15, 7.6, W / 2 + 0.15, ridge[1] - 0.2)  # king post
    h.beam("wood", (W / 2, 7.8), (3.0, 9.3), 0.2)  # struts
    h.beam("wood", (W / 2, 7.8), (W - 3.0, 9.3), 0.2)
    h.box("brass", W / 2 - 0.25, 9.0, W / 2 + 0.25, 9.5, **front)  # bell in the attic
    h.box("iron", W / 2 - 0.03, 9.5, W / 2 + 0.03, 10.0, **front)
    # attic clutter: a trunk and a barrel
    h.box("wood", 10.0, 7.6, 11.2, 8.3, **front)
    h.box("brass", 10.5, 8.0, 10.7, 8.15, **front)
    h.box("wood", 4.3, 7.6, 5.1, 8.6, **front)
    for z in (7.8, 8.4):
        h.box("iron", 4.25, z, 5.15, z + 0.07, **front)

    # --- chimney: brick from the stove through the roof, brass cap
    h.box("brick", 1.3, 4.12, 2.0, 7.25, y0=0.25, y1=0.5)  # behind the bedroom
    h.box("brick", 1.3, 7.6, 2.0, 10.6)
    h.box("brass", 1.15, 10.6, 2.15, 10.75)
    return sc


if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    if args:
        # Background: build into the empty startup scene and export it.
        build(bpy.context.scene)
        bpy.ops.export_scene.gltf(filepath=args[0], export_format="GLB")
        print("exported", args[0])
    else:
        build()
