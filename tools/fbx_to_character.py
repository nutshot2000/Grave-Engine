# Turns animated FBX files (from Mixamo, or any rigged character) into a
# Grave Engine character: side-on animation frames + character.txt.
#
#   blender -b --factory-startup -P tools/fbx_to_character.py -- IN_DIR OUT_DIR
#
# (or let the engine do it: `grave-engine --make-character IN_DIR`)
#
# IN_DIR holds one FBX per animation, all for the same character. File names
# say which animation is which:
#   idle, walk/run, jump, fall, swim/tread, die/death/dying
# e.g. "Idle.fbx", "Walking.fbx", "Jumping.fbx", "Falling Idle.fbx",
# "Treading Water.fbx", "Dying.fbx". Anything else keeps its own name.
#
# The character is turned to face right, centred over its hips (so walking
# animations that move forward stay in place), lit, and rendered from the
# side with a transparent background.

import math
import os
import sys

import bpy
from mathutils import Vector

FRAME_PX = 256
MAX_FRAMES = 24  # per animation (longer ones are sampled more sparsely)

argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
if len(argv) < 2:
    raise SystemExit("usage: blender -b -P fbx_to_character.py -- IN_DIR OUT_DIR")
IN_DIR, OUT_DIR = os.path.abspath(argv[0]), os.path.abspath(argv[1])

KINDS = [
    ("idle", ("idle", "breath", "stand")),
    ("walk", ("walk", "run", "jog")),
    ("jump", ("jump",)),
    ("fall", ("fall",)),
    ("swim", ("swim", "tread")),
    ("die", ("die", "death", "dying", "dead")),
]
LOOPS = {"idle", "walk", "fall", "swim"}


def kind_of(name):
    low = name.lower()
    for kind, words in KINDS:
        if any(w in low for w in words):
            return kind
    return "".join(c if c.isalnum() else "_" for c in low).strip("_") or "anim"


files = sorted(f for f in os.listdir(IN_DIR) if f.lower().endswith(".fbx"))
if not files:
    raise SystemExit(f"no .fbx files in {IN_DIR}")

bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene

# Import every file; keep the first character, and just the motion from the rest.
main_arm = None
actions = {}
for f in files:
    before = set(bpy.data.objects)
    bpy.ops.import_scene.fbx(filepath=os.path.join(IN_DIR, f), use_anim=True)
    new = [o for o in bpy.data.objects if o not in before]
    arm = next((o for o in new if o.type == "ARMATURE"), None)
    if arm is None or arm.animation_data is None or arm.animation_data.action is None:
        print(f"skipping {f}: no animated skeleton")
        for o in new:
            bpy.data.objects.remove(o, do_unlink=True)
        continue
    kind = kind_of(os.path.splitext(f)[0])
    if kind in actions:
        print(f"skipping {f}: already have a {kind} animation")
    else:
        act = arm.animation_data.action
        act.use_fake_user = True
        actions[kind] = act
    if main_arm is None:
        main_arm = arm
    else:
        for o in new:
            bpy.data.objects.remove(o, do_unlink=True)

if main_arm is None:
    raise SystemExit("none of the files had an animated skeleton")
meshes = [o for o in bpy.data.objects if o.type == "MESH"]


def bounds():
    """World bounding box of the character's meshes as they are posed now."""
    dg = bpy.context.evaluated_depsgraph_get()
    lo, hi = Vector((1e9, 1e9, 1e9)), Vector((-1e9, -1e9, -1e9))
    for m in meshes:
        ev = m.evaluated_get(dg)
        for c in ev.bound_box:
            p = ev.matrix_world @ Vector(c)
            lo = Vector(map(min, lo, p))
            hi = Vector(map(max, hi, p))
    return lo, hi


def hips():
    """The skeleton's hips/root bone (Mixamo calls it mixamorig:Hips)."""
    pb = main_arm.pose.bones
    for b in pb:
        if "hips" in b.name.lower() or "pelvis" in b.name.lower():
            return b
    return next(b for b in pb if b.parent is None)


def key_range(act):
    """First and last keyframe of an action (its own frame_range can be the
    whole scene timeline)."""
    curves = list(getattr(act, "fcurves", None) or [])
    for layer in getattr(act, "layers", []):
        for strip in layer.strips:
            for bag in getattr(strip, "channelbags", []):
                curves.extend(bag.fcurves)
    keys = [k.co.x for c in curves for k in c.keyframe_points]
    return (min(keys), max(keys)) if keys else tuple(act.frame_range)


def set_action(act, frame):
    main_arm.animation_data.action = act
    if hasattr(main_arm.animation_data, "action_slot") and act.slots:
        main_arm.animation_data.action_slot = act.slots[0]
    scene.frame_set(int(frame), subframe=frame - int(frame))


# Turn to face right (+X; the camera looks from -Y). Mixamo characters face -Y.
main_arm.rotation_euler.z += math.radians(90)
bpy.context.view_layer.update()

# Size and feet, from the first frame of idle (or whatever there is).
first = actions.get("idle") or next(iter(actions.values()))
set_action(first, key_range(first)[0])
lo, hi = bounds()
height = hi.z - lo.z
main_arm.location.z -= lo.z
bpy.context.view_layer.update()
ortho = height * 1.4
cam_z = ortho / 2 - height * 0.08

cam_data = bpy.data.cameras.new("Side")
cam_data.type = "ORTHO"
cam_data.ortho_scale = ortho
cam = bpy.data.objects.new("Side", cam_data)
scene.collection.objects.link(cam)
cam.location = (0, -10 * height, cam_z)
cam.rotation_euler = (math.radians(90), 0, 0)
cam_data.clip_end = 100 * height
scene.camera = cam


def light(name, energy, rot, color):
    d = bpy.data.lights.new(name, "SUN")
    d.energy = energy
    d.color = color
    o = bpy.data.objects.new(name, d)
    o.rotation_euler = [math.radians(a) for a in rot]
    scene.collection.objects.link(o)


light("Key", 2.4, (50, 0, 25), (1.0, 0.95, 0.88))
light("Fill", 0.6, (70, 0, -40), (0.7, 0.8, 1.0))
light("Rim", 2.5, (-60, 0, 180), (1.0, 0.85, 0.7))
world = bpy.data.worlds.new("Side world")
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
r.resolution_x = r.resolution_y = FRAME_PX
r.resolution_percentage = 100
r.film_transparent = True
r.image_settings.file_format = "PNG"
r.image_settings.color_mode = "RGBA"
try:
    scene.view_settings.view_transform = "Standard"
except TypeError:
    pass

os.makedirs(OUT_DIR, exist_ok=True)
for f in os.listdir(OUT_DIR):
    if f.endswith(".png"):
        os.remove(os.path.join(OUT_DIR, f))

base_fps = scene.render.fps / scene.render.fps_base
lines = []
for kind, act in actions.items():
    start, end = key_range(act)
    length = max(1.0, end - start)
    count = int(min(MAX_FRAMES, max(2, length // 2 + 1)))
    step = length / (count if kind in LOOPS else count - 1)
    for i in range(count):
        t = start + i * step
        set_action(act, t)
        # Keep the hips over the middle: forward motion is the engine's job.
        main_arm.location.x = 0
        main_arm.location.y = 0
        bpy.context.view_layer.update()
        h = main_arm.matrix_world @ hips().head
        main_arm.location.x -= h.x
        bpy.context.view_layer.update()
        r.filepath = os.path.join(OUT_DIR, f"{kind}_{i:02d}.png")
        bpy.ops.render.render(write_still=True)
    fps = base_fps / step
    lines.append(f"{kind} = {count} frames, {fps:.1f} fps{', loop' if kind in LOOPS else ''}")
    print(f"rendered {kind}: {count} frames")

figure = height / ortho * FRAME_PX
feet = FRAME_PX / 2 + cam_z / ortho * FRAME_PX
with open(os.path.join(OUT_DIR, "character.txt"), "w") as f:
    f.write(f"# Made from {len(files)} FBX file(s) in {IN_DIR} by tools/fbx_to_character.py\n")
    f.write(f"frame = {FRAME_PX}\n")
    f.write(f"figure = {figure:.1f}            # how tall the character stands in a picture (pixels)\n")
    f.write(f"feet = {feet:.1f}              # the row its feet are on\n")
    f.write("faces = right\n")
    for line in lines:
        f.write(line + "\n")
print("character written to", OUT_DIR, "animations:", ", ".join(actions))
