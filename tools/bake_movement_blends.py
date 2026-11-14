"""Render phase-matched skeletal blend banks for the local 2D character.

Blender -b -P tools/bake_movement_blends.py -- --look assets/characters/mixamo-fbx-local
Rendered banks stay local; the rig and original clips are preserved.
"""
import argparse,math,sys
from pathlib import Path
import bpy

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--look',type=Path,required=True)
p.add_argument('--seconds',type=float,default=.14)
p.add_argument('--graph',type=Path)
p.add_argument('--reuse-existing',action='store_true')
p.add_argument('--shard',type=int,default=0)
p.add_argument('--shards',type=int,default=1)
p.add_argument('--no-install',action='store_true')
a=p.parse_args(sys.argv[sys.argv.index('--')+1:])
if not .04<=a.seconds<=1.5:raise SystemExit('Use .04..1.5 seconds.')
if not 1<=a.shards<=16 or not 0<=a.shard<a.shards:raise SystemExit('Use 1..16 shards and an index from 0 to shards-1.')
if a.shards>1 and not a.no_install:raise SystemExit('Sharded workers need --no-install; install only after every worker finishes.')
folder=a.look.resolve()
bpy.ops.wm.open_mainfile(filepath=str(folder/'mixamo-fbx-animations.blend'))
scene=bpy.context.scene
arm=next(o for o in bpy.data.objects if o.type=='ARMATURE')
clips={}
for line in (folder/'character.txt').read_text().splitlines():
 if ' frames,'not in line:continue
 name,value=line.split('=',1);parts=value.split(',');clips[name.strip()]=(int(parts[0].strip().split()[0]),float(parts[1].strip().split()[0]),'loop'in value)

def pose(clip,seconds):
 action=bpy.data.actions['Imported_'+clip]
 arm.animation_data.action=action
 arm.animation_data.action_slot=action.slots[0]
 count,fps,loop=clips[clip]
 index=min(count-1,math.floor((seconds%(count/fps)if loop else max(0.,seconds))*fps+1e-5))
 # Sample exactly the same source pose as the rendered frame being replaced.
 frame=action.frame_range[0]+index*(30./fps)
 scene.frame_set(math.floor(frame),subframe=frame-math.floor(frame))
 bpy.context.view_layer.update()
 return arm.location.copy(),{b.name:(b.location.copy(),b.rotation_quaternion.copy(),b.scale.copy())for b in arm.pose.bones}

pairs=[]
for start in ['idle','walk','run']:
 for end in ['idle','walk','run','jump','ual_jump_loop']:
  if start!=end:pairs.append((start,end,1.))
pairs += [('jump','ual_jump_loop',1.),('ual_jump_loop','ual_jump_land',2.2)]
pairs += [('ual_jump_land',end,1.)for end in ['idle','walk','run','jump']]
if a.graph:
 states={}
 rules=[]
 for line in a.graph.read_text(encoding='utf-8').splitlines():
  w=line.split('#')[0].split()
  if w and w[0]=='state':states[w[1]]=(w[3],float(w[6]))
  if w and w[0]=='blend':rules.append((w[1],w[2],float(w[3])))
 pairs=[];seen={}
 for start,end,seconds in rules:
  source=states[start][0];target,speed=states[end]
  if source not in clips or target not in clips:raise SystemExit(f'Graph needs missing clip {source} or {target}')
  if not .04<=seconds<=1.5:raise SystemExit('Graph blend times must be .04..1.5 seconds.')
  key=(source,target)
  if key in seen:
   if seen[key]!=(speed,seconds):raise SystemExit('Aliases sharing a clip pair must use the same blend time and target speed.')
   continue
  seen[key]=(speed,seconds);pairs.append((source,target,speed,seconds))
else:pairs=[(*pair,a.seconds)for pair in pairs]
fps=50
scene.render.resolution_x=scene.render.resolution_y=128
scene.render.resolution_percentage=100
scene.render.use_stamp=False
for pair_index,(source,target,speed,seconds) in enumerate(pairs):
 if pair_index%a.shards!=a.shard:continue
 steps=round(seconds*fps)+1
 out=folder/'blends'/(source+'--'+target)
 out.mkdir(parents=True,exist_ok=True)
 count,source_fps,_=clips[source]
 metadata=f'from {source}\nto {target}\nseconds {seconds}\nsteps {steps}\nsource_frames {count}\nsource_fps {source_fps}\ntarget_speed {speed}\nscale 2\n'
 if a.reuse_existing and (out/'blend.txt').is_file() and dict(line.split(' ',1) for line in (out/'blend.txt').read_text(encoding='utf-8').splitlines())==dict(line.split(' ',1) for line in metadata.splitlines()) and all((out/f'{phase:03d}_{step:02d}.png').is_file() for phase in range(count) for step in range(steps)):
  print('BANK REUSED',source,target,flush=True);continue
 for phase in range(count):
  for step in range(steps):
   t=seconds*step/(steps-1)
   u=step/(steps-1);weight=u*u*(3-2*u)
   location0,bones0=pose(source,phase/source_fps+t)
   location1,bones1=pose(target,t*speed)
   arm.animation_data.action=None
   arm.location=location0.lerp(location1,weight)
   for b in arm.pose.bones:
    p0,q0,s0=bones0[b.name];p1,q1,s1=bones1[b.name]
    b.location=p0.lerp(p1,weight)
    b.rotation_quaternion=q0.slerp(q1,weight)
    b.scale=s0.lerp(s1,weight)
   bpy.context.view_layer.update()
   scene.render.filepath=str(out/f'{phase:03d}_{step:02d}.png')
   bpy.ops.render.render(write_still=True)
 (out/'blend.txt').write_text(f'from {source}\nto {target}\nseconds {seconds}\nsteps {steps}\nsource_frames {count}\nsource_fps {source_fps}\ntarget_speed {speed}\nscale 2\n',encoding='utf-8')
 print('BANK READY',source,target,count*steps,flush=True)
print('ALL MOVEMENT BANKS READY',flush=True)
if a.graph and not a.no_install:(folder/'animation.graph').write_text(a.graph.read_text(encoding='utf-8'),encoding='utf-8')
