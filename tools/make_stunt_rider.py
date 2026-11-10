import os
os.chdir(r'C:\Users\batti\Desktop\Grave-Engine')
import bpy, math
from mathutils import Vector
bpy.ops.wm.read_factory_settings(use_empty=True)
def mat(n,c):
 m=bpy.data.materials.new(n);m.diffuse_color=(*c,1);m.use_nodes=True;m.node_tree.nodes.get('Principled BSDF').inputs['Base Color'].default_value=(*c,1);return m
red=mat('Crimson suit',(0.65,0.025,0.04));dark=mat('Boots and visor',(0.015,0.025,0.04));white=mat('Helmet',(0.9,0.9,0.8))
def ball(n,p,s,m):
 bpy.ops.mesh.primitive_uv_sphere_add(segments=16,ring_count=8,location=p);o=bpy.context.object;o.name=n;o.scale=s;o.data.materials.append(m)
def limb(a,b,r,m):
 a,b=Vector(a),Vector(b);d=b-a;bpy.ops.mesh.primitive_cylinder_add(vertices=12,radius=r,depth=d.length,location=(a+b)/2);o=bpy.context.object;o.rotation_euler=d.to_track_quat('Z','Y').to_euler();o.data.materials.append(m)
ball('Torso',(0,0,1.1),(.22,.16,.38),red)
ball('Helmet',(.14,0,1.66),(.25,.22,.25),white)
ball('Visor',(.33,-.08,1.69),(.12,.19,.10),dark)
for y in [-.14,.14]:
 limb((0,y,.92),(.4,y,.62),.11,red);limb((.4,y,.62),(.12,y,.30),.09,dark);ball('Boot',(.25,y,.25),(.22,.11,.09),dark)
 limb((.08,y,1.35),(.43,y,1.15),.085,red);limb((.43,y,1.15),(1.12,y,1.13),.075,white)
bpy.ops.object.camera_add(location=(0,-7,1));cam=bpy.context.object;cam.rotation_euler=(Vector((.2,0,1))-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.type='ORTHO';cam.data.ortho_scale=2.25;bpy.context.scene.camera=cam
bpy.ops.object.light_add(type='AREA',location=(-2,-4,5));bpy.context.object.data.energy=500;bpy.context.object.data.shape='DISK';bpy.context.object.data.size=5
s=bpy.context.scene;s.render.engine='CYCLES';s.cycles.samples=16;s.render.resolution_x=192;s.render.resolution_y=192;s.render.resolution_percentage=100;s.render.film_transparent=True;s.render.filepath=r'C:\Users\batti\Desktop\Grave-Engine\assets\stunt-rider.png';bpy.ops.wm.save_as_mainfile(filepath=r'C:\Users\batti\Desktop\Grave-Engine\assets\stunt-rider.blend');bpy.ops.render.render(write_still=True)



