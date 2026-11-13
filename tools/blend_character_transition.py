"""Bake a real pose-to-pose transition into a local rendered character look.

Blender -b -P tools/blend_character_transition.py -- --blend FILE --out LOOK
  --from-action Imported_climb --to-action Imported_idle --name climb_finish
  --seconds 0.4 [--graph FILE]
Uses skeletal interpolation, not a dissolve between two sprite pictures.
"""
import argparse
import math
import sys
from pathlib import Path

import bpy
from mathutils import Vector

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--blend',type=Path,required=True)
p.add_argument('--out',type=Path,required=True)
p.add_argument('--from-action',required=True)
p.add_argument('--to-action',required=True)
p.add_argument('--name',required=True)
p.add_argument('--seconds',type=float,default=0.4)
p.add_argument('--fps',type=int,default=60)
p.add_argument('--graph',type=Path)
a=p.parse_args(sys.argv[sys.argv.index('--')+1:])
if not (0.1<=a.seconds<=3 and 12<=a.fps<=60):
    raise SystemExit('Use 0.1..3 seconds and 12..60 fps.')
if not a.name or not all(c.isalnum() or c=='_' for c in a.name):
    raise SystemExit('Clip names use letters, digits and underscores.')
if 'Imported_'+a.name in {a.from_action,a.to_action}:
    raise SystemExit('The transition must have a separate action name.')
bpy.ops.wm.open_mainfile(filepath=str(a.blend.resolve()))
scene=bpy.context.scene
arm=next(o for o in bpy.data.objects if o.type=='ARMATURE')
meshes=[o for o in bpy.data.objects if o.type=='MESH' and not o.hide_render]
source=bpy.data.actions[a.from_action]
destination=bpy.data.actions[a.to_action]

def bounds_bottom():
    dg=bpy.context.evaluated_depsgraph_get()
    return min((m.evaluated_get(dg).matrix_world@Vector(c)).z for m in meshes for c in m.evaluated_get(dg).bound_box)

def capture(action,frame):
    arm.animation_data.action=action
    if action.slots:arm.animation_data.action_slot=action.slots[0]
    scene.frame_set(math.floor(frame),subframe=frame-math.floor(frame))
    bpy.context.view_layer.update()
    return arm.location.copy(),{b.name:(b.location.copy(),b.rotation_quaternion.copy(),b.scale.copy())for b in arm.pose.bones}

start,first=capture(source,float(source.frame_range[1]))
end,last=capture(destination,float(destination.frame_range[0]))
arm.animation_data.action=None
count=round(a.seconds*a.fps)+1
poses=[]
a.out.mkdir(parents=True,exist_ok=True)
for i in range(count):
    u=i/(count-1)
    t=u*u*(3-2*u)
    arm.location=start.lerp(end,t)
    for b in arm.pose.bones:
        p0,q0,s0=first[b.name]
        p1,q1,s1=last[b.name]
        b.rotation_mode='QUATERNION'
        b.location=p0.lerp(p1,t)
        b.rotation_quaternion=q0.slerp(q1,t)
        b.scale=s0.lerp(s1,t)
    bpy.context.view_layer.update()
    # Maintain contact while weight transfers from hands to planted feet.
    arm.location.z-=bounds_bottom()
    bpy.context.view_layer.update()
    poses.append((arm.location.copy(),{b.name:(b.location.copy(),b.rotation_quaternion.copy(),b.scale.copy())for b in arm.pose.bones}))
    scene.render.use_stamp=False
    scene.render.filepath=str((a.out/f'{a.name}_{i:02d}.png').resolve())
    bpy.ops.render.render(write_still=True)

name='Imported_'+a.name
old=bpy.data.actions.get(name)
if old:bpy.data.actions.remove(old)
action=bpy.data.actions.new(name)
action.use_fake_user=True
arm.animation_data.action=action
previous={}
for i,(location,bones) in enumerate(poses):
    arm.location=location
    arm.keyframe_insert(data_path='location',frame=i+1)
    for b in arm.pose.bones:
        b.location,b.rotation_quaternion,b.scale=bones[b.name]
        if b.name in previous and previous[b.name].dot(b.rotation_quaternion)<0:b.rotation_quaternion.negate()
        previous[b.name]=b.rotation_quaternion.copy()
        for channel in ['location','rotation_quaternion','scale']:
            b.keyframe_insert(data_path=channel,frame=i+1)

manifest=a.out/'character.txt'
lines=[l for l in manifest.read_text(encoding='utf-8').splitlines() if not l.startswith(a.name+' =')]
lines.append(f'{a.name} = {count} frames, {a.fps} fps')
manifest.write_text('\n'.join(lines)+'\n',encoding='utf-8')
if a.graph:
    available={l.split(' =')[0]for l in lines if ' frames,' in l}
    graph=a.graph.read_text(encoding='utf-8').splitlines()
    states={l.split()[1]for l in graph if l.startswith('state ') and l.split()[3] in available}
    kept=[]
    for line in graph:
        w=line.split()
        if w and w[0]in{'state','event'} and w[1]not in states:continue
        if w and w[0]=='transition' and (w[2]not in states or(w[1]!='*'and w[1]not in states)):continue
        if 'run'not in available:line=line.replace(' & speed < 2.5','')
        kept.append(line)
    used=set(' '.join(l for l in kept if l.startswith('transition ')).split())
    kept=[l for l in kept if not l.startswith('trigger ')or l.split()[1]in used]
    (a.out/'animation.graph').write_text('\n'.join(kept)+'\n',encoding='utf-8')
arm.animation_data.action=destination
if destination.slots:arm.animation_data.action_slot=destination.slots[0]
scene.frame_set(round(destination.frame_range[0]))
bpy.ops.wm.save_as_mainfile(filepath=str(a.blend.resolve()))
print('TRANSITION READY',a.name,count,'frames',flush=True)
