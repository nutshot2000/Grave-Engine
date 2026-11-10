# Builds "Hero", an animated adventurer, in Blender, and renders its
# animations into frames for Grave Engine (assets/characters/hero/).
#
# Run it inside Blender (Scripting tab, or through the MCP connection).
# It works in its own scene, "Grave Engine hero", so your scene is left alone.
#
# The character is rigged with simple joints (empties): hips, spine, neck,
# shoulders, elbows, hips, knees. Each animation is keyframed on those
# joints, then rendered from the side with a transparent background.
#
# Set RENDER = False to just build it (to look at or tweak in Blender).

import math
import os

import bpy

RENDER = True
FRAME_PX = 256          # each frame is FRAME_PX x FRAME_PX
ORTHO = 2.4             # metres across the frame
HEIGHT = 1.8            # the hero is 1.8 m tall, feet at z = 0
CAM_Z = 1.0             # camera centre height
OUT = os.path.join(os.path.expanduser("~"), "Desktop", "Grave-Engine", "assets", "characters", "hero")

# ------------------------------------------------------------------ scene --
old_scene = bpy.context.window.scene
scene = bpy.data.scenes.get("Grave Engine hero") or bpy.data.scenes.new("Grave Engine hero")
bpy.context.window.scene = scene
for o in list(scene.collection.objects):
    bpy.data.objects.remove(o, do_unlink=True)


def material(name, rgb, rough=0.6, metal=0.0, emit=None):
    m = bpy.data.materials.get(name) or bpy.data.materials.new(name)
    m.use_nodes = True
    b = m.node_tree.nodes.get("Principled BSDF")
    b.inputs["Base Color"].default_value = (*rgb, 1.0)
    b.inputs["Roughness"].default_value = rough
    b.inputs["Metallic"].default_value = metal
    if emit:
        b.inputs["Emission Color"].default_value = (*emit, 1.0)
        b.inputs["Emission Strength"].default_value = 1.0
    return m


SKIN = material("Hero Skin", (0.72, 0.42, 0.30), 0.5)
HAIR = material("Hero Hair", (0.16, 0.08, 0.04), 0.7)
JACKET = material("Hero Jacket", (0.05, 0.18, 0.28), 0.55)
SCARF = material("Hero Scarf", (0.70, 0.12, 0.08), 0.6)
PANTS = material("Hero Pants", (0.16, 0.12, 0.09), 0.75)
BOOTS = material("Hero Boots", (0.12, 0.07, 0.04), 0.4)
BELT = material("Hero Belt", (0.35, 0.22, 0.10), 0.4)
METAL = material("Hero Buckle", (0.85, 0.70, 0.30), 0.25, 1.0)
EYES = material("Hero Eyes", (0.02, 0.02, 0.03), 0.2)


def joint(name, loc, parent=None):
    e = bpy.data.objects.new(name, None)
    e.empty_display_size = 0.08
    scene.collection.objects.link(e)
    if parent:
        e.parent = parent
        e.location = (loc[0] - parent.matrix_world.translation.x, loc[1] - parent.matrix_world.translation.y, loc[2] - parent.matrix_world.translation.z)
    else:
        e.location = loc
    bpy.context.view_layer.update()
    return e


def part(name, mesh_fn, mat, parent, world_loc, scale=(1, 1, 1), rot=(0, 0, 0), smooth=True, subsurf=0):
    mesh_fn()
    o = bpy.context.object
    o.name = name
    o.scale = scale
    o.rotation_euler = rot
    o.data.materials.append(mat)
    if smooth:
        bpy.ops.object.shade_smooth()
    if subsurf:
        mod = o.modifiers.new("Smooth", "SUBSURF")
        mod.levels = subsurf
        mod.render_levels = subsurf
    o.parent = parent
    p = parent.matrix_world.translation
    o.location = (world_loc[0] - p.x, world_loc[1] - p.y, world_loc[2] - p.z)
    bpy.context.view_layer.update()
    return o


def sphere():
    bpy.ops.mesh.primitive_uv_sphere_add(segments=24, ring_count=14, radius=1)


def cyl():
    bpy.ops.mesh.primitive_cylinder_add(vertices=20, radius=1, depth=1)


def cube():
    bpy.ops.mesh.primitive_cube_add(size=1)


def limb(name, mat, parent, top, length, r_top, r_bot):
    """A tapered limb hanging down from `top` (a cone with rounded ends)."""
    bpy.ops.mesh.primitive_cone_add(vertices=20, radius1=r_bot, radius2=r_top, depth=length)
    o = bpy.context.object
    o.name = name
    o.data.materials.append(mat)
    bpy.ops.object.shade_smooth()
    o.parent = parent
    p = parent.matrix_world.translation
    o.location = (top[0] - p.x, top[1] - p.y, top[2] - length / 2 - p.z)
    # Round the ends with spheres.
    for z, r in ((top[2], r_top), (top[2] - length, r_bot)):
        part(name + " end", sphere, mat, parent, (top[0], top[1], z), (r, r, r))
    bpy.context.view_layer.update()
    return o


# Joints. The hero faces +X (right); the camera looks from -Y.
root = joint("Hips", (0, 0, 0.95))
spine = joint("Spine", (0, 0, 1.0), root)
neck = joint("Neck", (0, 0, 1.48), spine)
sh_b = joint("Shoulder.back", (0, 0.13, 1.42), spine)
sh_f = joint("Shoulder.front", (0, -0.13, 1.42), spine)
el_b = joint("Elbow.back", (0, 0.13, 1.15), sh_b)
el_f = joint("Elbow.front", (0, -0.13, 1.15), sh_f)
hip_b = joint("Hip.back", (0, 0.09, 0.93), root)
hip_f = joint("Hip.front", (0, -0.09, 0.93), root)
kn_b = joint("Knee.back", (0, 0.09, 0.50), hip_b)
kn_f = joint("Knee.front", (0, -0.09, 0.50), hip_f)

# Body.
# Torso: a tapered body, wider at the shoulders, with rounded shoulders.
bpy.ops.mesh.primitive_cone_add(vertices=24, radius1=0.15, radius2=0.185, depth=0.46)
torso = bpy.context.object
torso.name = "Torso"
torso.scale = (1.0, 0.78, 1.0)
torso.data.materials.append(JACKET)
bpy.ops.object.shade_smooth()
bev = torso.modifiers.new("Round", "BEVEL")
bev.width = 0.05
bev.segments = 4
torso.parent = spine
torso.location = (0, 0, 1.2 - 1.0)
part("Chest", sphere, JACKET, spine, (0.01, 0, 1.33), (0.18, 0.15, 0.12))
part("Collar", cyl, JACKET, spine, (0, 0, 1.43), (0.1, 0.09, 0.03))
for yy in (0.13, -0.13):
    part("Shoulder pad", sphere, JACKET, spine, (0, yy, 1.4), (0.07, 0.065, 0.06))
part("Jacket hem", cyl, JACKET, spine, (0, 0, 1.0), (0.16, 0.13, 0.04))
part("Belt", cyl, BELT, spine, (0, 0, 0.97), (0.165, 0.145, 0.05))
part("Buckle", cube, METAL, spine, (0.16, 0, 0.97), (0.03, 0.06, 0.05), smooth=False)
part("Pelvis", sphere, PANTS, root, (0, 0, 0.9), (0.16, 0.15, 0.12))
part("Scarf", sphere, SCARF, spine, (0.02, 0, 1.44), (0.13, 0.13, 0.06), subsurf=1)
part("Scarf tail", cube, SCARF, spine, (-0.12, 0, 1.36), (0.05, 0.08, 0.16), rot=(0, 0.5, 0))
# Head.
part("Neck", cyl, SKIN, neck, (0, 0, 1.5), (0.05, 0.05, 0.08))
part("Head", sphere, SKIN, neck, (0.01, 0, 1.63), (0.12, 0.115, 0.135), subsurf=1)
part("Nose", sphere, SKIN, neck, (0.125, 0, 1.62), (0.025, 0.02, 0.03))
part("Hair", sphere, HAIR, neck, (-0.015, 0, 1.68), (0.13, 0.125, 0.11), subsurf=1)
part("Hair fringe", sphere, HAIR, neck, (0.07, 0, 1.73), (0.08, 0.11, 0.04), rot=(0, 0.4, 0))
part("Eye", sphere, EYES, neck, (0.105, -0.045, 1.645), (0.014, 0.014, 0.02))
part("Ear", sphere, SKIN, neck, (-0.005, -0.115, 1.63), (0.025, 0.015, 0.035))
# Arms (back arm first so it's behind).
for side, sh, el in (("back", sh_b, el_b), ("front", sh_f, el_f)):
    y = sh.matrix_world.translation.y
    limb(f"Upper arm.{side}", JACKET, sh, (0, y, 1.42), 0.27, 0.055, 0.045)
    limb(f"Forearm.{side}", SKIN, el, (0, y, 1.15), 0.25, 0.042, 0.035)
    part(f"Hand.{side}", sphere, SKIN, el, (0, y, 0.88), (0.045, 0.04, 0.05))
    part(f"Cuff.{side}", cyl, JACKET, el, (0, y, 1.12), (0.05, 0.05, 0.04))
# Legs.
for side, hp, kn in (("back", hip_b, kn_b), ("front", hip_f, kn_f)):
    y = hp.matrix_world.translation.y
    limb(f"Thigh.{side}", PANTS, hp, (0, y, 0.93), 0.43, 0.075, 0.06)
    limb(f"Shin.{side}", PANTS, kn, (0, y, 0.50), 0.36, 0.058, 0.048)
    part(f"Boot.{side}", sphere, BOOTS, kn, (0.04, y, 0.07), (0.11, 0.06, 0.07), subsurf=1)
    part(f"Boot top.{side}", cyl, BOOTS, kn, (0, y, 0.17), (0.06, 0.06, 0.09))

JOINTS = dict(root=root, spine=spine, neck=neck, sh_b=sh_b, sh_f=sh_f, el_b=el_b, el_f=el_f, hip_b=hip_b, hip_f=hip_f, kn_b=kn_b, kn_f=kn_f)

# ----------------------------------------------------------- camera, light --
cam_data = bpy.data.cameras.new("Hero cam")
cam_data.type = "ORTHO"
cam_data.ortho_scale = ORTHO
cam = bpy.data.objects.new("Hero cam", cam_data)
scene.collection.objects.link(cam)
cam.location = (0, -10, CAM_Z)
cam.rotation_euler = (math.radians(90), 0, 0)
scene.camera = cam


def light(name, kind, energy, loc, rot, color=(1, 1, 1), size=None):
    d = bpy.data.lights.new(name, kind)
    d.energy = energy
    d.color = color
    if size is not None and hasattr(d, "angle"):
        d.angle = size
    o = bpy.data.objects.new(name, d)
    o.location = loc
    o.rotation_euler = rot
    scene.collection.objects.link(o)


light("Key", "SUN", 2.2, (2, -4, 5), (math.radians(50), 0, math.radians(25)), (1.0, 0.95, 0.88), 0.2)
light("Fill", "SUN", 0.5, (-3, -4, 2), (math.radians(70), 0, math.radians(-40)), (0.7, 0.8, 1.0), 0.5)
light("Rim", "SUN", 2.5, (0, 5, 3), (math.radians(-60), 0, math.radians(180)), (1.0, 0.85, 0.7), 0.1)

world = bpy.data.worlds.get("Hero world") or bpy.data.worlds.new("Hero world")
world.use_nodes = True
world.node_tree.nodes["Background"].inputs[0].default_value = (0.25, 0.27, 0.32, 1)
world.node_tree.nodes["Background"].inputs[1].default_value = 0.3
scene.world = world

r = scene.render
for engine in ("BLENDER_EEVEE", "BLENDER_EEVEE_NEXT"):
    try:
        r.engine = engine
        break
    except TypeError:
        pass
r.resolution_x = FRAME_PX
r.resolution_y = FRAME_PX
r.resolution_percentage = 100
r.film_transparent = True
r.image_settings.file_format = "PNG"
r.image_settings.color_mode = "RGBA"
try:
    scene.view_settings.view_transform = "Standard"
except TypeError:
    pass
# Ink outlines, like an illustrated game character.
try:
    r.use_freestyle = True
    r.line_thickness_mode = "ABSOLUTE"
    r.line_thickness = 1.3
    ls = scene.view_layers[0].freestyle_settings.linesets
    if len(ls):
        ls[0].linestyle.color = (0.06, 0.04, 0.05)
except Exception:
    pass

# ------------------------------------------------------------- animation --
# Each pose: joint -> rotation about Y in degrees (+ = swing forward/up for
# limbs hanging down... the hero faces +X, so a positive Y rotation tips a
# hanging limb backwards; we flip the sign so + means forward).
# Extra keys: "lift" raises/lowers the whole body (m), "lean" tips the body.


def pose(t, p):
    """Set every joint for pose dict `p` and key it at frame `t`."""
    for name, j in JOINTS.items():
        j.rotation_euler = (0, 0, 0)
    root.location.z = 0.95 + p.get("lift", 0.0)
    root.location.x = p.get("slide", 0.0)
    for key, deg in p.items():
        if key in JOINTS:
            JOINTS[key].rotation_euler = (0, -math.radians(deg), 0)
    root.rotation_euler = (0, math.radians(p.get("lean", 0.0)), 0)
    for j in JOINTS.values():
        j.keyframe_insert("rotation_euler", frame=t)
    root.keyframe_insert("location", frame=t)


def cycle(n, fn):
    """Poses for a looping animation of n frames; fn(phase 0..1) -> pose."""
    return [fn(i / n) for i in range(n)]


S = lambda ph, k=0.0: math.sin(2 * math.pi * (ph + k))


def walk(ph):
    return dict(
        hip_f=28 * S(ph), hip_b=-28 * S(ph),
        kn_f=-max(0, 45 * S(ph, 0.3)) - 5, kn_b=-max(0, 45 * S(ph, 0.8)) - 5,
        sh_f=-24 * S(ph), sh_b=24 * S(ph),
        el_f=20 + 10 * S(ph), el_b=20 - 10 * S(ph),
        lift=0.025 * abs(S(ph, 0.25)) - 0.02, lean=5, spine=3,
    )


def idle(ph):
    b = S(ph)
    return dict(spine=1.5 * b, neck=-1.5 * b, sh_f=4 + 2 * b, sh_b=-3 + 2 * b, el_f=12, el_b=10, lift=0.006 * b, kn_f=-3, kn_b=-3, hip_f=2, hip_b=1)


def swim(ph):
    return dict(
        lean=65, sh_f=150 * (0.5 + 0.5 * S(ph)) - 40, sh_b=150 * (0.5 + 0.5 * S(ph, 0.5)) - 40,
        el_f=20, el_b=20, hip_f=18 * S(ph), hip_b=-18 * S(ph), kn_f=-20 - 15 * S(ph, 0.2), kn_b=-20 - 15 * S(ph, 0.7),
        neck=-35, lift=-0.35,
    )


def fall(ph):
    return dict(sh_f=120 + 15 * S(ph), sh_b=100 - 15 * S(ph), el_f=30, el_b=30, hip_f=25 + 8 * S(ph), hip_b=-5 - 8 * S(ph), kn_f=-50, kn_b=-30, neck=10)


JUMP = [
    dict(kn_f=-30, kn_b=-30, hip_f=25, hip_b=25, lift=-0.08, sh_f=-30, sh_b=-30, el_f=20, el_b=20, lean=10),
    dict(kn_f=-50, kn_b=-50, hip_f=45, hip_b=45, lift=-0.16, sh_f=-45, sh_b=-45, el_f=25, el_b=25, lean=15),
    dict(kn_f=-10, kn_b=-5, hip_f=10, hip_b=-5, lift=0.0, sh_f=60, sh_b=40, el_f=15, el_b=15, lean=5),
    dict(kn_f=-40, kn_b=-15, hip_f=50, hip_b=-10, lift=0.02, sh_f=120, sh_b=90, el_f=25, el_b=25),
    dict(kn_f=-60, kn_b=-30, hip_f=60, hip_b=10, lift=0.02, sh_f=130, sh_b=100, el_f=30, el_b=30),
    dict(kn_f=-60, kn_b=-35, hip_f=55, hip_b=15, lift=0.02, sh_f=125, sh_b=105, el_f=35, el_b=35),
]


def die(n):
    out = []
    for i in range(n):
        k = min(1.0, i / (n * 0.7))
        e = k * k * (3 - 2 * k)
        out.append(dict(lean=-88 * e, lift=-0.82 * e, slide=-0.35 * e, sh_f=60 * e, sh_b=100 * e, el_f=10, el_b=30 * e,
                        hip_f=15 * e, hip_b=-5 * e, kn_f=-25 * e, kn_b=-10 * e, neck=20 * e))
    return out


ANIMS = {
    # name: (poses, fps, loops)
    "idle": (cycle(16, idle), 8, True),
    "walk": (cycle(16, walk), 16, True),
    "jump": (JUMP, 12, False),
    "fall": (cycle(8, fall), 10, True),
    "swim": (cycle(16, swim), 10, True),
    "die": (die(14), 12, False),
}

if RENDER:
    os.makedirs(OUT, exist_ok=True)
    for f in os.listdir(OUT):
        if f.endswith(".png"):
            os.remove(os.path.join(OUT, f))
    for name, (poses, fps, loops) in ANIMS.items():
        for i, p in enumerate(poses):
            pose(1, p)
            scene.frame_set(1)
            r.filepath = os.path.join(OUT, f"{name}_{i:02d}.png")
            bpy.ops.render.render(write_still=True)
    # Clear the keys: leave the hero standing in the idle pose.
    for j in JOINTS.values():
        j.animation_data_clear()
    pose(1, idle(0))
    figure_px = HEIGHT / ORTHO * FRAME_PX
    feet_px = FRAME_PX / 2 + CAM_Z / ORTHO * FRAME_PX
    with open(os.path.join(OUT, "character.txt"), "w") as f:
        f.write("# Hero: made and rendered by tools/make_hero_in_blender.py\n")
        f.write(f"frame = {FRAME_PX}                 # each picture is this many pixels square\n")
        f.write(f"figure = {figure_px:.1f}            # how tall the character stands in a picture (pixels)\n")
        f.write(f"feet = {feet_px:.1f}              # the row its feet are on\n")
        f.write("faces = right\n")
        for name, (poses, fps, loops) in ANIMS.items():
            f.write(f"{name} = {len(poses)} frames, {fps} fps{', loop' if loops else ''}\n")
else:
    pose(1, walk(0.1))

print("hero ready", "rendered to " + OUT if RENDER else "")
