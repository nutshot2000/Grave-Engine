# Turns Kimodo motions (NVIDIA's text-to-motion model, SOMA 30-joint human
# skeleton) into a Grave Engine character: a body built to fit that
# skeleton, posed from each motion, rendered side-on.
#
#   blender -b --factory-startup -P tools/kimodo_to_character.py -- MOTIONS_DIR OUT_DIR
#
# MOTIONS_DIR has one folder per animation (idle, walk, jump, fall, swim,
# die, or anything else), each holding Kimodo's output:
#   root_positions.f32        frames x 3 floats (metres, Y up)
#   local_rotations_xyzw.f32  frames x 30 joints x 4 floats (quaternions)
# (made by kmd-generate from a sentence, e.g. "a person jumps forward").
#
# The skeleton description below is from kimodo.cpp (Apache 2.0).

import math
import os
import sys

import bpy
import numpy as np
from mathutils import Matrix, Quaternion, Vector

FRAME_PX = 256
ORTHO = 2.4
CAM_Z = 1.0
MAX_FRAMES = 48
STEP = 2  # render every 2nd motion frame (15 fps)
LOOPS = {"idle", "walk", "fall", "swim"}

argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
if len(argv) < 2:
    raise SystemExit("usage: blender -b -P kimodo_to_character.py -- MOTIONS_DIR OUT_DIR")
IN_DIR, OUT_DIR = os.path.abspath(argv[0]), os.path.abspath(argv[1])

NAMES = [
    "Hips", "Spine1", "Spine2", "Chest", "Neck1", "Neck2", "Head", "Jaw",
    "LeftEye", "RightEye", "LeftShoulder", "LeftArm", "LeftForeArm", "LeftHand",
    "LeftHandThumbEnd", "LeftHandMiddleEnd", "RightShoulder", "RightArm", "RightForeArm",
    "RightHand", "RightHandThumbEnd", "RightHandMiddleEnd", "LeftLeg", "LeftShin", "LeftFoot",
    "LeftToeBase", "RightLeg", "RightShin", "RightFoot", "RightToeBase",
]
PARENTS = [-1, 0, 1, 2, 3, 4, 5, 6, 6, 6, 3, 10, 11, 12, 13, 13, 3, 16, 17, 18, 19, 19, 0, 22, 23, 24, 0, 26, 27, 28]
OFFSETS = [
    [0.0, 0.988, 0.0], [-0.00013727, 0.0500376256, -0.00053726669], [-1.86574103e-09, 0.0712530139, -0.000298248546],
    [-5.75188398e-09, 0.0755006305, -0.00815970992], [-0.00181676517, 0.263112953, -0.00553348292],
    [-2.85102231e-08, 0.0770939664, 0.0230258546], [-4.5975437e-08, 0.0612891595, 0.0195370861],
    [2.63687901e-05, 0.0047559225, 0.0309494062], [0.0320638079, 0.0538020513, 0.0758688308],
    [-0.0322244017, 0.05361869, 0.0755823359], [0.0162165175, 0.232371641, 0.0511341324],
    [0.149198457, 2.19397873e-08, -0.0550232576], [0.287393078, 2.50268389e-09, -2.58787737e-05],
    [0.270939812, -7.06625108e-09, 2.60897248e-05], [0.122686267, -0.0322017573, 0.0483306876],
    [0.190119595, -0.00312878387, -0.000339570373], [-0.0138011824, 0.231803086, 0.0521415786],
    [-0.150371962, 1.17387901e-07, -0.0554560437], [-0.287366393, 1.87628082e-08, -2.59709359e-05],
    [-0.271336198, -1.16767401e-09, 2.61269368e-05], [-0.122642483, -0.0321145448, 0.0480403904],
    [-0.190005945, -0.00306615542, -0.0003157343], [0.10043214, -0.0843452671, 0.0259565473],
    [-1e-08, -0.432217537, -0.00802912805], [1e-08, -0.421550959, -0.0348152298],
    [0.0, -0.0505947206, 0.132315294], [-0.10047278, -0.0829525995, 0.0262031695],
    [1e-08, -0.433622059, -0.00805555828], [2e-08, -0.421173943, -0.0347839785],
    [-3.42907669e-09, -0.0507960932, 0.132841956],
]
J = {n: i for i, n in enumerate(NAMES)}
# Kimodo is Y-up and faces +Z; Blender is Z-up. C turns one into the other
# (the character then faces -Y), and TURN spins it to face +X (right).
C = Matrix(((1, 0, 0), (0, 0, -1), (0, 1, 0)))
TURN = Matrix.Rotation(math.radians(90), 3, "Z")
W = TURN @ C


def load_motion(folder):
    roots = np.fromfile(os.path.join(folder, "root_positions.f32"), dtype="<f4").reshape(-1, 3)
    quats = np.fromfile(os.path.join(folder, "local_rotations_xyzw.f32"), dtype="<f4").reshape(len(roots), 30, 4)
    return roots, quats


def pose(roots, quats, f):
    """Global position and rotation of every joint at frame f (Blender space)."""
    pos, rot = [], []
    for j in range(30):
        q = quats[f][j]
        r = Quaternion((float(q[3]), float(q[0]), float(q[1]), float(q[2]))).to_matrix()
        p = PARENTS[j]
        if p < 0:
            pos.append(Vector(roots[f]))
            rot.append(r)
        else:
            pos.append(pos[p] + rot[p] @ Vector(OFFSETS[j]))
            rot.append(rot[p] @ r)
    return [W @ p for p in pos], [W @ r @ C.transposed() for r in rot]


# --------------------------------------------------------------- the body --
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene


def material(name, rgb, rough=0.6, metal=0.0):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = (*rgb, 1)
    b.inputs["Roughness"].default_value = rough
    b.inputs["Metallic"].default_value = metal
    return m


SKIN = material("Skin", (0.72, 0.42, 0.30), 0.5)
HAIR = material("Hair", (0.16, 0.08, 0.04), 0.7)
JACKET = material("Jacket", (0.05, 0.18, 0.28), 0.55)
SCARF = material("Scarf", (0.70, 0.12, 0.08), 0.6)
PANTS = material("Pants", (0.16, 0.12, 0.09), 0.75)
BOOTS = material("Boots", (0.12, 0.07, 0.04), 0.4)
BELT = material("Belt", (0.35, 0.22, 0.10), 0.4)
BUCKLE = material("Buckle", (0.85, 0.70, 0.30), 0.25, 1.0)
EYES = material("Eyes", (0.02, 0.02, 0.03), 0.2)

joints = []
for n in NAMES:
    e = bpy.data.objects.new("joint " + n, None)
    scene.collection.objects.link(e)
    joints.append(e)


def local(v):
    """A vector in a joint's own (Kimodo) frame, in Blender axes."""
    return C @ Vector(v)


def attach(o, joint):
    o.parent = joints[J[joint]]
    o.matrix_parent_inverse = Matrix.Identity(4)


def blob(joint, at, size, mat, subsurf=1):
    """A smooth ellipsoid on a joint (at / size in the joint's frame)."""
    bpy.ops.mesh.primitive_uv_sphere_add(segments=24, ring_count=14, radius=1)
    o = bpy.context.object
    o.data.materials.append(mat)
    bpy.ops.object.shade_smooth()
    if subsurf:
        o.modifiers.new("s", "SUBSURF").levels = subsurf
    s = local(size)
    o.scale = (abs(s.x), abs(s.y), abs(s.z))
    o.location = local(at)
    attach(o, joint)
    return o


def limb(joint, child, r0, r1, mat):
    """A tapered limb from a joint to its child joint, with rounded ends."""
    v = local(OFFSETS[J[child]])
    length = v.length
    bpy.ops.mesh.primitive_cone_add(vertices=20, radius1=r0, radius2=r1, depth=length)
    o = bpy.context.object
    o.data.materials.append(mat)
    bpy.ops.object.shade_smooth()
    # The cone points along +Z: turn it along the bone.
    o.rotation_euler = Vector((0, 0, 1)).rotation_difference(v.normalized()).to_euler()
    o.location = v / 2
    attach(o, joint)
    for at, r in ((Vector((0, 0, 0)), r0), (v, r1)):
        bpy.ops.mesh.primitive_uv_sphere_add(segments=16, ring_count=10, radius=r, location=at)
        e = bpy.context.object
        e.data.materials.append(mat)
        bpy.ops.object.shade_smooth()
        attach(e, joint)


# Torso and hips (the jacket spans Spine2 -> neck).
blob("Hips", (0, -0.02, 0), (0.17, 0.12, 0.15), PANTS)
blob("Spine1", (0, 0.03, 0), (0.165, 0.08, 0.135), BELT, 0)
blob("Spine1", (0, 0.03, 0.13), (0.03, 0.035, 0.02), BUCKLE, 0)
blob("Spine2", (0, 0.12, 0.0), (0.17, 0.17, 0.13), JACKET)
blob("Chest", (0, 0.12, 0.0), (0.2, 0.15, 0.135), JACKET)
blob("Neck1", (0, 0.01, 0.01), (0.1, 0.04, 0.09), SCARF)
limb("Neck1", "Neck2", 0.05, 0.045, SKIN)
# Head (faces +Z in the joint frame).
blob("Head", (0, 0.07, 0.01), (0.115, 0.135, 0.12), SKIN)
blob("Head", (0, 0.13, -0.015), (0.125, 0.1, 0.125), HAIR)
blob("Head", (0, 0.16, 0.06), (0.11, 0.04, 0.08), HAIR, 0)
blob("Head", (0, 0.06, 0.125), (0.02, 0.028, 0.025), SKIN, 0)
for x in (0.045, -0.045):
    blob("Head", (x, 0.085, 0.105), (0.014, 0.018, 0.012), EYES, 0)
# Arms: jacket sleeves, then bare forearms and hands.
for side in ("Left", "Right"):
    limb(side + "Arm", side + "ForeArm", 0.058, 0.048, JACKET)
    limb(side + "ForeArm", side + "Hand", 0.042, 0.034, SKIN)
    blob(side + "ForeArm", (0, 0, 0), (0.05, 0.05, 0.05), JACKET, 0)
    hand = OFFSETS[J[side + "HandMiddleEnd"]]
    blob(side + "Hand", (hand[0] * 0.45, hand[1] * 0.45, hand[2] * 0.45), (0.07, 0.03, 0.045), SKIN)
    # Legs: trousers, boots.
    limb(side + "Leg", side + "Shin", 0.078, 0.06, PANTS)
    limb(side + "Shin", side + "Foot", 0.058, 0.048, PANTS)
    toe = OFFSETS[J[side + "ToeBase"]]
    blob(side + "Foot", (toe[0] * 0.5, toe[1] * 0.5 - 0.02, toe[2] * 0.6), (0.06, 0.06, 0.13), BOOTS)
    blob(side + "Shin", (0, -0.33, 0), (0.065, 0.1, 0.065), BOOTS)

# ------------------------------------------------------- camera and light --
cam_data = bpy.data.cameras.new("Side")
cam_data.type = "ORTHO"
cam_data.ortho_scale = ORTHO
cam = bpy.data.objects.new("Side", cam_data)
scene.collection.objects.link(cam)
cam.location = (0, -10, CAM_Z)
cam.rotation_euler = (math.radians(90), 0, 0)
scene.camera = cam


def light(name, energy, rot, color):
    d = bpy.data.lights.new(name, "SUN")
    d.energy = energy
    d.color = color
    o = bpy.data.objects.new(name, d)
    o.rotation_euler = [math.radians(a) for a in rot]
    scene.collection.objects.link(o)


light("Key", 2.2, (50, 0, 25), (1.0, 0.95, 0.88))
light("Fill", 0.5, (70, 0, -40), (0.7, 0.8, 1.0))
light("Rim", 2.5, (-60, 0, 180), (1.0, 0.85, 0.7))
world = bpy.data.worlds.new("w")
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
r.film_transparent = True
r.image_settings.file_format = "PNG"
r.image_settings.color_mode = "RGBA"
try:
    scene.view_settings.view_transform = "Standard"
except TypeError:
    pass

# ------------------------------------------------------------- rendering --
motions = sorted(d for d in os.listdir(IN_DIR) if os.path.isfile(os.path.join(IN_DIR, d, "root_positions.f32")))
if not motions:
    raise SystemExit(f"no Kimodo motions in {IN_DIR}")
data = {m: load_motion(os.path.join(IN_DIR, m)) for m in motions}

# Where the floor is: the lowest foot in the first idle (or first) frame.
ref = data.get("idle") or next(iter(data.values()))
p0, _ = pose(*ref, 0)
floor = min(p0[J[n]].z for n in ("LeftToeBase", "RightToeBase", "LeftFoot", "RightFoot")) - 0.05
head_top = p0[J["Head"]].z + 0.2
height = head_top - floor

os.makedirs(OUT_DIR, exist_ok=True)
for f in os.listdir(OUT_DIR):
    if f.endswith(".png"):
        os.remove(os.path.join(OUT_DIR, f))
lines = []
for name in motions:
    roots, quats = data[name]
    n = len(roots)
    start, end = 0, n
    if name in LOOPS and n > 40:
        # Cut out one cycle that ends where it began, so the loop is seamless:
        # the frame most like the start (hips-relative pose), at least 0.6 s on.
        def shape(f):
            pos, _ = pose(roots, quats, f)
            h = pos[J["Hips"]]
            return np.array([[v.x - h.x, v.y - h.y, v.z] for v in pos])
        start = min(10, n // 4)
        a = shape(start)
        best = min(range(start + 30, n), key=lambda e: float(((shape(e) - a) ** 2).sum()))
        end = best
    elif name == "jump":
        # Games jump the moment you press: skip the slow crouch, keep the
        # push-off and the rise to the top (the last frame holds in the air).
        poses = [pose(roots, quats, f)[0] for f in range(n)]
        hips = [p[J["Hips"]].z for p in poses]
        # The top of the jump: when the lower foot is highest off the ground.
        feet = [min(p[J[k]].z for k in ("LeftToeBase", "RightToeBase", "LeftFoot", "RightFoot")) for p in poses]
        apex = max(range(n), key=lambda f: feet[f])
        crouch = min(range(apex + 1), key=lambda f: hips[f])
        # Start once the body is well on its way up out of the crouch.
        rise = hips[crouch] + 0.6 * (hips[apex] - hips[crouch])
        start = next((f for f in range(crouch, apex + 1) if hips[f] >= rise), crouch)
        end = min(n, apex + 4)
    span = end - start
    count = int(min(MAX_FRAMES, max(2, span // STEP)))
    step = span / count if name in LOOPS else (span - 1) / (count - 1)
    for i in range(count):
        f = min(n - 1, start + int(round(i * step)))
        pos, rot = pose(roots, quats, f)
        hips = pos[J["Hips"]]
        for j, e in enumerate(joints):
            # Keep the hips over the middle: moving is the engine's job.
            p = pos[j] - Vector((hips.x, hips.y, floor))
            e.matrix_world = Matrix.Translation(p) @ rot[j].to_4x4()
        bpy.context.view_layer.update()
        r.filepath = os.path.join(OUT_DIR, f"{name}_{i:02d}.png")
        bpy.ops.render.render(write_still=True)
    fps = 30.0 / step
    lines.append(f"{name} = {count} frames, {fps:.1f} fps{', loop' if name in LOOPS else ''}")
    print(f"rendered {name}: {count} frames")

with open(os.path.join(OUT_DIR, "character.txt"), "w") as fh:
    fh.write("# Animated by Kimodo (NVIDIA, text-to-motion) and rendered by tools/kimodo_to_character.py\n")
    fh.write(f"frame = {FRAME_PX}\n")
    fh.write(f"figure = {height / ORTHO * FRAME_PX:.1f}            # how tall the character stands in a picture (pixels)\n")
    fh.write(f"feet = {FRAME_PX / 2 + CAM_Z / ORTHO * FRAME_PX:.1f}              # the row its feet are on\n")
    fh.write("faces = right\n")
    for line in lines:
        fh.write(line + "\n")
    fh.write("# Prompts:\n")
    for name in motions:
        p = os.path.join(IN_DIR, name, "prompt.txt")
        if os.path.exists(p):
            fh.write(f"#   {name}: {open(p, encoding='utf-8').read().strip()}\n")
print("character written to", OUT_DIR)
