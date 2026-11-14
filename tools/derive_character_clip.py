"""Derive a timed pose range without changing its original animation.

Blender -b -P tools/derive_character_clip.py -- --look LOOK --source ual_roll
  --name roll_low --first 5 --last 16 --seconds .76
Frame indices refer to the source character manifest, not Blender's timeline.
"""
import argparse,math,sys
from pathlib import Path
import bpy

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--look',type=Path,required=True)
p.add_argument('--source',required=True)
p.add_argument('--name',required=True)
p.add_argument('--first',type=int,required=True)
p.add_argument('--last',type=int,required=True)
p.add_argument('--seconds',type=float,required=True)
a=p.parse_args(sys.argv[sys.argv.index('--')+1:])
if a.name==a.source or not a.name or not all(c.isalnum()or c=='_'for c in a.name):raise SystemExit('Use a distinct alphanumeric clip name.')
if not .1<=a.seconds<=5:raise SystemExit('Use .1..5 seconds.')
folder=a.look.resolve();manifest=folder/'character.txt'
lines=manifest.read_text(encoding='utf-8').splitlines()
value=next(l.split('=',1)[1]for l in lines if l.split('=')[0].strip()==a.source)
parts=value.split(',');count=int(parts[0].split()[0]);fps=float(parts[1].split()[0])
if not 0<=a.first<a.last<count:raise SystemExit('Frame range is outside the source clip.')
bpy.ops.wm.open_mainfile(filepath=str(folder/'mixamo-fbx-animations.blend'))
scene=bpy.context.scene;arm=next(o for o in bpy.data.objects if o.type=='ARMATURE')
source=bpy.data.actions['Imported_'+a.source]
arm.animation_data.action=source;arm.animation_data.action_slot=source.slots[0]
out_count=round(a.seconds*32);out_fps=out_count/a.seconds
poses=[]
scene.render.resolution_x=scene.render.resolution_y=256
scene.render.resolution_percentage=100;scene.render.use_stamp=False
for i in range(out_count):
    frame=source.frame_range[0]+(a.first+(a.last-a.first)*i/(out_count-1))*30/fps
    scene.frame_set(math.floor(frame),subframe=frame-math.floor(frame))
    bpy.context.view_layer.update()
    poses.append((arm.location.copy(),{b.name:(b.location.copy(),b.rotation_quaternion.copy(),b.scale.copy())for b in arm.pose.bones}))
    scene.render.filepath=str(folder/f'{a.name}_{i:02d}.png')
    bpy.ops.render.render(write_still=True)
old=bpy.data.actions.get('Imported_'+a.name)
if old:bpy.data.actions.remove(old)
action=bpy.data.actions.new('Imported_'+a.name);action.use_fake_user=True
arm.animation_data.action=action
previous={}
for i,(location,bones)in enumerate(poses):
    frame=1+i*30/out_fps
    arm.location=location;arm.keyframe_insert(data_path='location',frame=frame)
    for b in arm.pose.bones:
        b.location,b.rotation_quaternion,b.scale=bones[b.name]
        if b.name in previous and previous[b.name].dot(b.rotation_quaternion)<0:b.rotation_quaternion.negate()
        previous[b.name]=b.rotation_quaternion.copy()
        for channel in ['location','rotation_quaternion','scale']:b.keyframe_insert(data_path=channel,frame=frame)
lines=[l for l in lines if l.split('=')[0].strip()!=a.name]
lines.append(f'{a.name} = {out_count} frames, {out_fps:.4f} fps')
manifest.write_text('\n'.join(lines)+'\n',encoding='utf-8')
bpy.ops.wm.save_as_mainfile(filepath=str(folder/'mixamo-fbx-animations.blend'))
print('DERIVED CLIP READY',a.name,out_count,flush=True)
