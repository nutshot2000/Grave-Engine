"""Retarget fresh Kimodo SOMA motions onto a local Mixamo FBX and render a look.
Run in Blender: -b --factory-startup -P tools/kimodo_to_mixamo.py -- MODEL MOTIONS OUT [--preview]
The source FBX stays untouched. Rendered model frames are local project assets.
"""
import ast,math,sys
from pathlib import Path
import bpy
import numpy as np
from mathutils import Matrix,Quaternion,Vector
ROOT=Path(__file__).resolve().parents[1]
args=sys.argv[sys.argv.index('--')+1:]
MODEL,MOTIONS,OUT=map(lambda s:Path(s).resolve(),args[:3])
PREVIEW='--preview' in args
SAVE_BLEND='--save-blend' in args
# Reuse the existing SOMA skeleton math without running its rendering pipeline.
tree=ast.parse((ROOT/'tools/kimodo_to_character.py').read_text(encoding='utf-8'))
keep={'NAMES','PARENTS','OFFSETS','J','C','TURN','W'}
nodes=[n for n in tree.body if isinstance(n,ast.Assign)and any(isinstance(t,ast.Name)and t.id in keep for t in n.targets) or isinstance(n,ast.FunctionDef)and n.name in {'load_motion','pose'}]
ns={'np':np,'Matrix':Matrix,'Quaternion':Quaternion,'Vector':Vector,'math':math,'os':__import__('os')}
exec(compile(ast.Module(body=nodes,type_ignores=[]),'soma_motion','exec'),ns)
pose,load_motion,J,TURN=ns['pose'],ns['load_motion'],ns['J'],ns['TURN']
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.fbx(filepath=str(MODEL),use_anim=False)
a=next(o for o in bpy.data.objects if o.type=='ARMATURE')
a.animation_data_clear()
for pb in a.pose.bones:pb.rotation_mode="QUATERNION"
a.matrix_world=Matrix.Rotation(math.pi/2,4,'Z')@a.matrix_world
meshes=[o for o in bpy.data.objects if o.type=='MESH']
def bounds():
 dg=bpy.context.evaluated_depsgraph_get();pts=[o.evaluated_get(dg).matrix_world@Vector(p)for o in meshes for p in o.evaluated_get(dg).bound_box]
 return Vector(tuple(min(p[k]for p in pts)for k in range(3))),Vector(tuple(max(p[k]for p in pts)for k in range(3)))
bpy.context.view_layer.update();lo,hi=bounds();a.location.z-=lo.z;bpy.context.view_layer.update();lo,hi=bounds();height=hi.z-lo.z
arm_world=a.matrix_world.copy();arm_inv=arm_world.inverted();scale=arm_world.to_scale()
rest={b.name:arm_world@b.matrix_local for b in a.data.bones}
rest_rot={n:m.to_quaternion().to_matrix()for n,m in rest.items()}
map_names={'Hips':'Hips','Spine':'Spine1','Spine1':'Spine2','Spine2':'Chest','Neck':'Neck1','Head':'Head','LeftShoulder':'LeftShoulder','LeftArm':'LeftArm','LeftForeArm':'LeftForeArm','LeftHand':'LeftHand','RightShoulder':'RightShoulder','RightArm':'RightArm','RightForeArm':'RightForeArm','RightHand':'RightHand','LeftUpLeg':'LeftLeg','LeftLeg':'LeftShin','LeftFoot':'LeftFoot','LeftToeBase':'LeftToeBase','RightUpLeg':'RightLeg','RightLeg':'RightShin','RightFoot':'RightFoot','RightToeBase':'RightToeBase'}
root=next(b for b in a.data.bones if b.parent is None)
ratio=rest[root.name].translation.z/0.988

def apply_motion(data,frame,kind):
 pos,rot=pose(*data,frame);world_mats={};pose_mats={}
 def bone_pose(pb):
  name=pb.name;short=name.split(':')[-1];r=rest[name];rr=rest_rot[name]
  if short in map_names:
   delta=rot[J[map_names[short]]]@TURN.transposed();rotation=delta@rr
  elif pb.parent:
   pn=pb.parent.name;rotation=world_mats[pn].to_quaternion().to_matrix()@rest_rot[pn].transposed()@rr
  else:rotation=rr
  if pb.parent:
   pn=pb.parent.name;pr=world_mats[pn];location=pr.translation+pr.to_quaternion().to_matrix()@rest_rot[pn].transposed()@(r.translation-rest[pn].translation)
  else:
   dz=(pos[J['Hips']].z-0.988)*ratio
   if kind in {'jump','fall'}:dz=min(dz,0.) if kind=='jump' else 0.
   location=Vector((0.,0.,r.translation.z+dz))
  matrix=Matrix.LocRotScale(location,rotation.to_quaternion(),scale)
  world_mats[name]=matrix;target=arm_inv@matrix;pose_mats[name]=target
  kw={'parent_matrix':pose_mats[pb.parent.name],'parent_matrix_local':pb.parent.bone.matrix_local}if pb.parent else{}
  pb.matrix_basis=pb.bone.convert_local_to_pose(target,pb.bone.matrix_local,invert=True,**kw)
  for child in pb.children:bone_pose(child)
 a.location=arm_world.translation
 for pb in a.pose.bones:
  if pb.parent is None:bone_pose(pb)
 bpy.context.view_layer.update()
 lo,_=bounds()
 # Planted-foot clips should neither float nor sink after proportion changes.
 if kind in {'idle','walk','die'} or lo.z<0:
  a.location.z-=lo.z;bpy.context.view_layer.update()

scene=bpy.context.scene
ortho=height*1.55;camz=ortho/2-height*0.09
cd=bpy.data.cameras.new('Animation side camera');cd.type='ORTHO';cd.ortho_scale=ortho
cam=bpy.data.objects.new('Animation side camera',cd);scene.collection.objects.link(cam);cam.location=(0,-10,camz);cam.rotation_euler=(math.pi/2,0,0);scene.camera=cam
for name,energy,rot,color in [('Key',2.4,(50,0,25),(1.,.95,.88)),('Fill',.8,(70,0,-40),(.7,.8,1.)),('Rim',1.6,(-60,0,180),(1.,.85,.7))]:
 d=bpy.data.lights.new(name,'SUN');d.energy=energy;d.color=color;o=bpy.data.objects.new(name,d);scene.collection.objects.link(o);o.rotation_euler=[math.radians(v)for v in rot]
world=bpy.data.worlds.new('Animation world');world.use_nodes=True;world.node_tree.nodes['Background'].inputs[0].default_value=(.25,.27,.32,1);world.node_tree.nodes['Background'].inputs[1].default_value=.3;scene.world=world
r=scene.render;r.engine='BLENDER_EEVEE';r.resolution_x=r.resolution_y=256;r.resolution_percentage=100;r.film_transparent=True;r.image_settings.file_format='PNG';r.image_settings.color_mode='RGBA';r.use_stamp=False
if hasattr(scene,'eevee')and hasattr(scene.eevee,'taa_render_samples'):scene.eevee.taa_render_samples=16
scene.view_settings.view_transform='Standard';OUT.mkdir(parents=True,exist_ok=True)
lines=[];saved_actions=[];loops={'idle','walk','swim','fall'}
for kind in ['idle','walk','jump','fall','swim','die']:
 folder=MOTIONS/kind
 if not(folder/'root_positions.f32').exists():continue
 data=load_motion(str(folder));n=len(data[0]);start,end=0,n
 if a.animation_data:a.animation_data.action=None
 captured=[]
 if kind=='walk' and n>60:
  # Pick the nearest matching complete stride after the warm-up frames.
  start=10;first=pose(*data,start)[0];origin=first[0];base=np.array([tuple(p-origin)for p in first]);end=min(range(start+30,n),key=lambda f:float(((np.array([tuple(p-pose(*data,f)[0][0])for p in pose(*data,f)[0]])-base)**2).sum()))
 if kind=='fall':start,end=15,min(n,50)
 if kind=='jump':
  poses=[pose(*data,f)[0]for f in range(n)];feet=[min(p[J[k]].z for k in ['LeftFoot','RightFoot','LeftToeBase','RightToeBase'])for p in poses];apex=max(range(n),key=lambda f:feet[f]);hips=[p[0].z for p in poses];crouch=min(range(apex+1),key=lambda f:hips[f]);rise=hips[crouch]+.55*(hips[apex]-hips[crouch]);start=next((f for f in range(crouch,apex+1)if hips[f]>=rise),crouch);end=min(n,apex+3)
 count=3 if PREVIEW else min(120,max(8,end-start));step=(end-start)/count if kind in loops else (end-start-1)/max(1,count-1);fps=30./max(step,.1)
 for i in range(count):
  f=min(n-1,start+round(i*step));apply_motion(data,f,kind)
  if SAVE_BLEND:captured.append((1.+i*step,a.location.copy(),{pb.name:pb.matrix_basis.copy()for pb in a.pose.bones}))
  r.filepath=str(OUT/f'{kind}_{i:02d}.png');bpy.ops.render.render(write_still=True)
 if SAVE_BLEND:
  a.animation_data_create();action=bpy.data.actions.new('Kimodo_'+kind);action.use_fake_user=True;a.animation_data.action=action
  for t,location,matrices in captured:
   a.location=location;a.keyframe_insert(data_path='location',frame=t)
   for pb in a.pose.bones:
    pb.matrix_basis=matrices[pb.name]
    for channel in ['location','rotation_quaternion','scale']:pb.keyframe_insert(data_path=channel,frame=t)
  saved_actions.append(action)
 lines.append(f'{kind} = {count} frames, {fps:.2f} fps'+(', loop'if kind in loops else''));print('RENDERED',kind,count,flush=True)
(OUT/'character.txt').write_text('# Local Mixamo model, retargeted from newly generated NVIDIA Kimodo motions.\nframe = 256\n'+f'figure = {height/ortho*256:.2f}\nfeet = {128+camz/ortho*256:.2f}\nfaces = right\n'+'\n'.join(lines)+'\n',encoding='utf-8')
if SAVE_BLEND and saved_actions:
 a.animation_data.action=saved_actions[0]
 if hasattr(a.animation_data,'action_slot') and saved_actions[0].slots:a.animation_data.action_slot=saved_actions[0].slots[0]
 scene.frame_start=1;scene.frame_end=90;scene.render.fps=30;scene.frame_set(1)
 for im in bpy.data.images:
  if im.packed_file:im.filepath='//textures/'+im.name+'.png'
 bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'kimodo-mixamo-lab.blend'))
print('READY',OUT,flush=True)
