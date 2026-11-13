"""Apply animation-only Mixamo FBXs to a separate local character and render a lab look.

Blender: -b --factory-startup --python-exit-code 1 -P tools/fbx_animation_lab.py --
  --model assets/characters/stuntman_main.fbx --animations assets/animations
  --out assets/characters/mixamo-fbx-local [--preview]
Original FBXs and previous looks are preserved. Outputs derived from the rig stay local.
"""
import argparse
import json
import math
import shutil
import sys
from pathlib import Path

import bpy
from mathutils import Matrix, Vector

ROOT = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--model', type=Path, default=ROOT/'assets/characters/stuntman_main.fbx')
p.add_argument('--animations', type=Path, default=ROOT/'assets/animations')
p.add_argument('--out', type=Path, default=ROOT/'assets/characters/mixamo-fbx-local')
p.add_argument('--fallback', type=Path, default=ROOT/'assets/characters/mixamo-lab-local')
p.add_argument('--preview', action='store_true')
p.add_argument('--library', type=Path, default=ROOT/'assets/animations/ual-local/UAL1_Standard.glb')
args = p.parse_args(sys.argv[sys.argv.index('--')+1:])
out = args.out.resolve()
out.mkdir(parents=True, exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.fbx(filepath=str(args.model.resolve()), use_anim=False)
target = next(o for o in bpy.data.objects if o.type == 'ARMATURE')
target.animation_data_clear()
target.matrix_world = Matrix.Rotation(math.pi/2, 4, 'Z') @ target.matrix_world
meshes = [o for o in bpy.data.objects if o.type == 'MESH']
scene = bpy.context.scene

def bounds():
    dg = bpy.context.evaluated_depsgraph_get()
    pts = [m.evaluated_get(dg).matrix_world @ Vector(c) for m in meshes for c in m.evaluated_get(dg).bound_box]
    return (Vector(tuple(min(p[k] for p in pts) for k in range(3))),
            Vector(tuple(max(p[k] for p in pts) for k in range(3))))

bpy.context.view_layer.update()
lo, hi = bounds()
target.location.z -= lo.z
bpy.context.view_layer.update()
lo, hi = bounds()
height = hi.z-lo.z
arm_world = target.matrix_world.copy()
arm_inv = arm_world.inverted()
scale = arm_world.to_scale()
rest = {b.name: arm_world @ b.matrix_local for b in target.data.bones}
rest_rot = {n: m.to_quaternion().to_matrix() for n, m in rest.items()}
for b in target.pose.bones:
    b.rotation_mode = 'QUATERNION'

ortho = height*1.55
cam_z = ortho/2-height*.09
cd = bpy.data.cameras.new('FBX animation side camera')
cd.type = 'ORTHO'
cd.ortho_scale = ortho
cam = bpy.data.objects.new(cd.name, cd)
scene.collection.objects.link(cam)
cam.location = (0, -10, cam_z)
cam.rotation_euler = (math.pi/2, 0, 0)
scene.camera = cam
for name, energy, angles, color in [('Key',2.4,(50,0,25),(1,.95,.88)),('Fill',.8,(70,0,-40),(.7,.8,1)),('Rim',1.6,(-60,0,180),(1,.85,.7))]:
    d = bpy.data.lights.new(name, 'SUN')
    d.energy = energy
    d.color = color
    o = bpy.data.objects.new(name, d)
    scene.collection.objects.link(o)
    o.rotation_euler = [math.radians(v) for v in angles]
world = bpy.data.worlds.new('Animation world')
world.use_nodes = True
world.node_tree.nodes['Background'].inputs[0].default_value = (.25,.27,.32,1)
world.node_tree.nodes['Background'].inputs[1].default_value = .3
scene.world = world
r = scene.render
r.engine = 'BLENDER_EEVEE'
r.resolution_x = r.resolution_y = 256
r.resolution_percentage = 100
r.film_transparent = True
r.image_settings.file_format = 'PNG'
r.image_settings.color_mode = 'RGBA'
r.use_stamp = False
scene.view_settings.view_transform = 'Standard'
scene.render.fps = 30
if hasattr(scene, 'eevee') and hasattr(scene.eevee, 'taa_render_samples'):
    scene.eevee.taa_render_samples = 16

def key_range(action):
    curves = list(getattr(action, 'fcurves', None) or [])
    for layer in action.layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                curves.extend(bag.fcurves)
    keys = [key.co.x for curve in curves for key in curve.keyframe_points]
    if not keys:
        raise RuntimeError('FBX has no animation keys')
    return min(keys), max(keys)

manifest = []
saved = []
report = []
source_files = [('Jump.fbx','jump'), ('Swimming.fbx','swim'), ('Treading Water.fbx','tread'), ('Hanging Idle.fbx','hang'), ('Climbing.fbx','climb')]
groups = [(args.animations/f, [(None,k)], False) for f,k in source_files]
if args.library.is_file():
    groups.append((args.library, [], True))
ual_aliases = {'Idle_Loop':'idle','Walk_Loop':'walk','Jog_Fwd_Loop':'run','Sprint_Loop':'sprint','Death01':'die'}
ual_bones = {'Hips':'pelvis','Spine':'spine_01','Spine1':'spine_02','Spine2':'spine_03','Neck':'neck_01','Head':'Head','HeadTop_End':'Head'}
for side, suffix in [('Left','l'),('Right','r')]:
    for target_name, source_name in [('Shoulder','clavicle'),('Arm','upperarm'),('ForeArm','lowerarm'),('Hand','hand'),('UpLeg','thigh'),('Leg','calf'),('Foot','foot'),('ToeBase','ball'),('Toe_End','ball_leaf')]:
        ual_bones[side+target_name] = source_name+'_'+suffix
    for finger in ['Thumb','Index','Middle','Ring','Pinky']:
        for n in range(1,5):
            ual_bones[f'{side}Hand{finger}{n}'] = f'{finger.lower()}_{n:02d}'+('_leaf' if n==4 else '')+'_'+suffix
for path, jobs, universal in groups:
    filename = path.name
    before = set(bpy.data.objects)
    previous_actions = set(bpy.data.actions)
    if universal:
        bpy.ops.import_scene.gltf(filepath=str(path.resolve()))
    else:
        bpy.ops.import_scene.fbx(filepath=str(path.resolve()), use_anim=True)
    imported = [o for o in bpy.data.objects if o not in before]
    source = next(o for o in imported if o.type == 'ARMATURE')
    source.matrix_world = Matrix.Rotation(math.pi/2,4,'Z') @ source.matrix_world
    source.animation_data.use_nla = False
    for o in imported:
        if o.type == 'MESH':
            o.hide_render = True
    imported_actions = [a for a in bpy.data.actions if a not in previous_actions]
    if universal:
        jobs = [(a, ual_aliases.get(a.name,'ual_'+a.name.lower())) for a in imported_actions if a.name != 'A_TPose']
    else:
        jobs = [(source.animation_data.action,jobs[0][1])]
    raw_bones = {b.name.split(':')[-1]: b for b in source.data.bones}
    source_bones = {name:raw_bones[ual_bones[name]] for name in ual_bones if ual_bones[name] in raw_bones} if universal else raw_bones
    source_rest = {name: source.matrix_world @ b.matrix_local for name,b in source_bones.items()}
    root = next(b for b in target.data.bones if b.parent is None)
    root_short = root.name.split(':')[-1]
    source_root = source_bones[root_short]
    ratio = rest[root.name].translation.z / max(.01,source_rest[root_short].translation.z)
    source_rest_rot = {n:m.to_quaternion().to_matrix() for n,m in source_rest.items()}
    missing = [b.name for b in target.data.bones if b.name.split(':')[-1] not in source_bones]
    if missing:
        raise RuntimeError(f'{filename}: missing target bones {missing}')
    def pose_at(frame, clip):
        if target.animation_data:
            target.animation_data.action = None
        scene.frame_set(int(frame), subframe=frame-int(frame))
        target.location = arm_world.translation
        bpy.context.view_layer.update()
        world_mats = {}
        pose_mats = {}
        def visit(pb):
            short = pb.name.split(':')[-1]
            sm = source.matrix_world @ source.pose.bones[source_bones[short].name].matrix
            rotation = sm.to_quaternion().to_matrix() @ source_rest_rot[short].transposed() @ rest_rot[pb.name]
            if pb.parent:
                pr = world_mats[pb.parent.name]
                location = pr.translation + pr.to_quaternion().to_matrix() @ rest_rot[pb.parent.name].transposed() @ (rest[pb.name].translation-rest[pb.parent.name].translation)
            else:
                dz = (sm.translation.z-source_rest[root_short].translation.z)*ratio
                # The controller owns translation. Preserve only jump crouching;
                # water and traversal clips stay centered at the actor's hips.
                dz = min(0.0, dz) if clip in {'jump','fall','landing'} else 0.0
                location = rest[pb.name].translation + Vector((0,0,dz))
            matrix = Matrix.LocRotScale(location, rotation.to_quaternion(), scale)
            world_mats[pb.name] = matrix
            pm = arm_inv @ matrix
            pose_mats[pb.name] = pm
            kw = {'parent_matrix':pose_mats[pb.parent.name], 'parent_matrix_local':pb.parent.bone.matrix_local} if pb.parent else {}
            pb.matrix_basis = pb.bone.convert_local_to_pose(pm,pb.bone.matrix_local,invert=True,**kw)
            for child in pb.children:
                visit(child)
        for pb in target.pose.bones:
            if pb.parent is None:
                visit(pb)
        bpy.context.view_layer.update()
        if clip not in {'jump','fall','swim','tread','hang','climb','ual_jump_start','ual_jump_loop','ual_swim_fwd_loop','ual_swim_idle_loop'}:
            lo, _ = bounds()
            target.location.z -= lo.z
            bpy.context.view_layer.update()

    for action, kind in jobs:
        source.animation_data.action = action
        if action.slots:
            source.animation_data.action_slot = action.slots[0]
        action_name = action.name
        start, end = key_range(action)
        segments = [(kind,start,end,(action_name.endswith('_Loop') or action_name == 'Sword_Idle' or 'Aim_' in action_name) if universal else kind in {'swim','tread','hang'})]
        if kind == 'jump' and not universal:
            # Split the actual takeoff, descent and landing instead of looping a jump.
            frames = range(round(start),round(end)+1)
            heights = []
            for f in frames:
                scene.frame_set(f)
                heights.append((source.matrix_world @ source.pose.bones[source_root.name].matrix).translation.z)
            apex = start + max(range(len(heights)),key=lambda i:heights[i])
            landing_start = max(apex+1,end-4)
            segments = [('jump',start,apex,False),('fall',apex,landing_start,False),('landing',landing_start,end,False)]
        root_path = []
        for frame in [start, (start+end)/2, end]:
            scene.frame_set(int(frame),subframe=frame-int(frame))
            root_path.append(list((source.matrix_world @ source.pose.bones[source_root.name].matrix).translation))
        report.append({'file':filename,'action':action_name,'source_frames':[start,end],'bone_matches':len(source_bones),'root_path':root_path})
        for clip, first, last, looping in segments:
            duration = max(1/30,(last-first)/30)
            count = 3 if args.preview else max(3,min(96,round(duration*20)+(0 if looping else 1)))
            step = (last-first)/(count if looping else count-1)
            captured = []
            for i in range(count):
                f = first+i*step
                pose_at(f,clip)
                captured.append((1+i*step,target.location.copy(),{pb.name:pb.matrix_basis.copy() for pb in target.pose.bones}))
                r.filepath = str(out/f'{clip}_{i:02d}.png')
                bpy.ops.render.render(write_still=True)
            target.animation_data_create()
            baked = bpy.data.actions.new('Imported_'+clip)
            baked.use_fake_user = True
            target.animation_data.action = baked
            previous_quaternions = {}
            for t, loc, matrices in captured:
                target.location = loc
                target.keyframe_insert(data_path='location',frame=t)
                for pb in target.pose.bones:
                    pb.matrix_basis = matrices[pb.name]
                    previous = previous_quaternions.get(pb.name)
                    if previous is not None and previous.dot(pb.rotation_quaternion) < 0:
                        pb.rotation_quaternion.negate()
                    previous_quaternions[pb.name] = pb.rotation_quaternion.copy()
                    for channel in ['location','rotation_quaternion','scale']:
                        pb.keyframe_insert(data_path=channel,frame=t)
            saved.append(baked)
            fps = (count/duration) if looping else ((count-1)/duration)
            manifest.append(f'{clip} = {count} frames, {fps:.4f} fps'+(', loop' if looping else ''))
            print('RENDERED',clip,count,'source',first,last,flush=True)
    for o in imported:
        bpy.data.objects.remove(o,do_unlink=True)
    for action in imported_actions:
        action.use_fake_user = False
        if action.users == 0:
            bpy.data.actions.remove(action)

# Discard unused duplicate source meshes/materials before saving the working rig.
for collection in [bpy.data.meshes,bpy.data.armatures,bpy.data.materials,bpy.data.images]:
    for block in list(collection):
        if block.users == 0:
            collection.remove(block)

if not args.preview:
    # Preserve the previous idle/walk/death until replacements are supplied.
    fallback_lines = (args.fallback/'character.txt').read_text().splitlines() if (args.fallback/'character.txt').is_file() else []
    for kind in ['idle','walk','die']:
        if any(line.startswith(kind+' =') for line in manifest):
            continue
        line = next(l for l in fallback_lines if l.startswith(kind+' ='))
        manifest.append(line)
        for f in args.fallback.glob(kind+'_*.png'):
            shutil.copy2(f,out/f.name)
    graph = (ROOT/'assets/animations/mixamo-pack.graph').read_text()
    available = {line.split(' =')[0] for line in manifest}
    if 'ual_jump_loop' not in available:graph=graph.replace('state fall clip ual_jump_loop loop speed 1','state fall clip fall loop speed 1')
    if 'ual_jump_land' not in available:graph=graph.replace('state landing clip ual_jump_land once speed 2.2','state landing clip landing once speed 1')
    state_names = {line.split()[1] for line in graph.splitlines() if line.startswith('state ') and line.split()[3] in available}
    kept = []
    for line in graph.splitlines():
        words = line.split()
        if words and words[0] == 'blend':continue # Installed when bank baking completes.
        if words and words[0] in {'state','event'} and words[1] not in state_names:
            continue
        if words and words[0] == 'transition' and (words[2] not in state_names or (words[1] != '*' and words[1] not in state_names)):
            continue
        if 'run' not in available:
            line = line.replace(' & speed < 2.5','')
        kept.append(line)
    used = set(' '.join(line for line in kept if line.startswith('transition ')).split())
    kept = [line for line in kept if not line.startswith('trigger ') or line.split()[1] in used]
    (out/'animation.graph').write_text('\n'.join(kept)+'\n',encoding='utf-8')
    text = (ROOT/'scenes/animation-lab.txt').read_text().replace('look hero','look '+out.name)
    (ROOT/'scenes/animation-lab.local.txt').write_text(text,encoding='utf-8')
(out/'character.txt').write_text('# Local character with imported FBX / humanoid library motions.\nframe = 256\n'+f'figure = {height/ortho*256:.2f}\nfeet = {128+cam_z/ortho*256:.2f}\nfaces = right\n'+'\n'.join(manifest)+'\n',encoding='utf-8')
(out/'import-report.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
preferred = next((a for a in saved if a.name == 'Imported_idle'),saved[0])
target.animation_data.action = preferred
if preferred.slots:
    target.animation_data.action_slot = preferred.slots[0]
scene.frame_start = 1
scene.frame_end = max(round(a.frame_range[1]) for a in saved)
scene.frame_set(1)
for im in bpy.data.images:
    if im.packed_file:
        im.filepath = '//textures/'+im.name+'.png'
bpy.ops.wm.save_as_mainfile(filepath=str(out/'mixamo-fbx-animations.blend'))
print('READY',out.name,flush=True)
