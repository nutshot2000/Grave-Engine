"""Build an editable, individually materialled stunt motorcycle and game layers."""
import bpy, math, os
from mathutils import Vector
from pathlib import Path
ROOT=Path(r'C:/Users/batti/Desktop/Grave-Engine/assets/stunt-bike')
bpy.ops.wm.read_factory_settings(use_empty=True)
sc=bpy.context.scene
sc.name='GRAVE • Ignition 450 stunt motorcycle'
def material(name,color,metal=0,rough=.4):
 m=bpy.data.materials.new(name);m.diffuse_color=(*color,1)
 bs=m.node_tree.nodes.get('Principled BSDF') if m.node_tree else None
 if not bs:
  m.use_nodes=True;bs=m.node_tree.nodes.get('Principled BSDF')
 bs.inputs['Base Color'].default_value=(*color,1);bs.inputs['Metallic'].default_value=metal;bs.inputs['Roughness'].default_value=rough
 return m
rubber=material('rubber | knobby tyre',(.018,.023,.03),0,.83)
steel=material('iron | satin steel',(.27,.32,.37),.85,.24)
alloy=material('iron | brushed aluminium',(.65,.72,.78),.8,.28)
black=material('iron | graphite engine',(.035,.045,.055),.7,.32)
gold=material('brass | anodised fork',(.78,.43,.08),.8,.25)
red=material('iron | vermilion enamel',(.8,.035,.018),.45,.25)
seatmat=material('rubber | stitched seat',(.025,.032,.042),0,.72)
white=material('iron | ivory number plates',(.88,.9,.84),.15,.34)
blue=material('iron | titanium exhaust',(.20,.26,.34),.85,.22)
parts={}
def finish(o,name,m,group='Chassis'):
 o.name=name;o.data.materials.append(m);o['engine_material']=m.name.split(' |')[0];o['part']=group
 for p in o.data.polygons:p.use_smooth=True
 parts.setdefault(group,[]).append(o);return o
def box(name,p,scale,m,bevel=.03,group='Chassis'):
 bpy.ops.mesh.primitive_cube_add(size=1,location=p);o=bpy.context.object;o.scale=scale;bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
 if bevel:
  b=o.modifiers.new('Machined edge radii','BEVEL');b.width=bevel;b.segments=3;o.modifiers.new('Weighted surface normals','WEIGHTED_NORMAL')
 return finish(o,name,m,group)
def rod(name,a,b,r,m,group='Chassis',vertices=24):
 a,b=Vector(a),Vector(b);d=b-a;bpy.ops.mesh.primitive_cylinder_add(vertices=vertices,radius=r,depth=d.length,location=(a+b)/2);o=bpy.context.object;o.rotation_euler=d.to_track_quat('Z','Y').to_euler();return finish(o,name,m,group)
def torus(name,p,major,minor,m,group='Chassis'):
 bpy.ops.mesh.primitive_torus_add(major_segments=64,minor_segments=12,location=p,major_radius=major,minor_radius=minor,rotation=(math.pi/2,0,0));return finish(bpy.context.object,name,m,group)
for x,label in [(-.8,'Rear wheel'),(.8,'Front wheel')]:
 z=.38
 torus(label+' • rubber carcass',(x,0,z),.285,.095,rubber,label)
 for k in range(40):
  a=k*math.tau/40
  for row in [-1,0,1]:
   o=box(label+' • tread %02d %d'%(k,row),(x+.369*math.sin(a),row*.068,z+.369*math.cos(a)),(.055,.057,.025),rubber,.008,label);o.rotation_euler.y=a
 torus(label+' • rim lip',(x,-.095,z),.255,.018,alloy,label)
 torus(label+' • rim inner',(x,.095,z),.255,.018,alloy,label)
 rod(label+' • hub',(x,-.13,z),(x,.13,z),.055,black,label)
 for k in range(20):
  a=k*math.tau/20
  for side in [-1,1]:rod(label+' • spoke',(x,side*.10,z),(x+.25*math.sin(a),side*.078,z+.25*math.cos(a)),.0045,alloy,label,8)
 torus(label+' • brake rotor',(x,-.137,z),.14,.018,steel,label)
 for k in range(8):
  a=k*math.tau/8;rod(label+' • rotor web',(x,-.135,z),(x+.14*math.sin(a),-.135,z+.14*math.cos(a)),.009,steel,label,8)
 box(label+' • brake caliper',(x+.12,-.155,z+.06),(.08,.055,.12),gold,.018,label)
# Twin-spar tubular frame and rear swingarm.
for y in [-.13,.13]:
 for a,b in [((-.8,y,.38),(-.15,y,.47)),((-.15,y,.47),(.31,y,.96)),((.31,y,.96),(-.42,y,.85)),((-.42,y,.85),(-.15,y,.47)),((-.42,y,.85),(-.72,y,.97))]:rod('iron • frame tube',a,b,.027,red)
 rod('iron • swingarm',(-.8,y,.38),(-.15,y,.49),.042,alloy)
# Detailed finned engine, covers, bolts and skid plate.
box('iron • crankcase',(-.1,0,.56),(.37,.29,.27),black,.07)
for i in range(8):box('iron • cylinder cooling fin %02d'%i,(.02,0,.69+i*.022),(.24,.32,.012),alloy,.008)
for y in [-.17,.17]:
 rod('iron • circular engine cover',(-.14,y,.55),(-.14,y+(.025 if y>0 else -.025),.55),.115,steel)
 for k in range(6):
  a=k*math.tau/6;rod('iron • cover bolt',(-.14+.09*math.sin(a),y,.55+.09*math.cos(a)),(-.14+.09*math.sin(a),y*1.2,.55+.09*math.cos(a)),.009,alloy,vertices=6)
box('iron • skid plate',(-.06,0,.39),(.45,.32,.05),alloy,.025)
# Rear coil-over, exposed spring.
rod('iron • rear shock',(-.49,0,.52),(-.32,0,.91),.026,alloy)
pts=[]
for i in range(161):
 t=i/160;a=t*math.tau*10;pts.append((-.49+.17*t+.045*math.cos(a),.045*math.sin(a),.52+.39*t))
curve=bpy.data.curves.new('Coil spring','CURVE');curve.dimensions='3D';curve.bevel_depth=.009;curve.bevel_resolution=3;sp=curve.splines.new('POLY');sp.points.add(len(pts)-1)
for v,p in zip(sp.points,pts):v.co=(*p,1)
o=bpy.data.objects.new('brass • rear spring',curve);sc.collection.objects.link(o);o.data.materials.append(gold)
# Long-travel fork, triple clamps and handlebars.
for y in [-.14,.14]:
 rod('brass • fork upper',(.38,y,1.04),(.61,y,.64),.034,gold)
 rod('iron • fork stanchion',(.55,y,.75),(.8,y,.38),.024,alloy)
 box('iron • triple clamp',(.40,y,.98),(.12,.08,.05),black,.015)
 rod('iron • handlebar',(.36,y,1.13),(.28,y*2.3,1.18),.016,steel)
 rod('rubber • bar grip',(.28,y*2.3,1.18),(.28,y*3.0,1.18),.025,rubber)
 rod('iron • lever',(.34,y*2.1,1.18),(.39,y*3.,1.16),.008,alloy)
# Sculpted wedge panels using meshes, independently assignable.
def panel(name,coords,width,m):
 verts=[(x,y,z) for y in [-width/2,width/2] for x,z in coords];n=len(coords);faces=[tuple(range(n-1,-1,-1)),tuple(range(n,2*n))]+[(i,(i+1)%n,(i+1)%n+n,i+n) for i in range(n)]
 mesh=bpy.data.meshes.new(name);mesh.from_pydata(verts,[],faces);mesh.update();o=bpy.data.objects.new(name,mesh);sc.collection.objects.link(o);finish(o,name,m);b=o.modifiers.new('Rounded panel edges','BEVEL');b.width=.025;b.segments=3;o.modifiers.new('Panel normals','WEIGHTED_NORMAL');return o
panel('iron • fuel tank',[(-.36,.88),(-.20,1.10),(.17,1.08),(.30,.96),(.12,.83)],.31,red)
box('rubber • saddle',(-.47,0,1.015),(.55,.25,.085),seatmat,.04)
panel('iron • tail fairing',[(-.98,1.02),(-.67,1.06),(-.31,.99),(-.45,.92),(-.87,.96)],.26,red)
panel('iron • front mudguard',[(.45,.87),(1.13,.79),(1.19,.74),(.46,.81)],.22,red)
panel('iron • front number board',[(.43,1.12),(.59,1.10),(.67,.88),(.49,.89)],.22,white)
panel('iron • side number plate',[(-.71,.92),(-.33,.92),(-.39,.73),(-.68,.76)],.33,white)
# Number text on camera-facing panel.
bpy.ops.object.text_add(location=(-.66,-.174,.785),rotation=(math.pi/2,0,0));o=bpy.context.object;o.name='07 • removable race number';o.data.body='07';o.data.size=.145;o.data.extrude=.001;o.data.materials.append(black)
# Exhaust header and muffler.
for a,b in [((.13,-.10,.74),(.33,-.20,.58)),((.33,-.20,.58),(.18,-.23,.43)),((.18,-.23,.43),(-.52,-.23,.70))]:rod('iron • exhaust header',a,b,.025,steel)
rod('iron • titanium muffler',(-.47,-.23,.70),(-.86,-.23,.88),.065,blue)
rod('iron • exhaust tip',(-.86,-.23,.88),(-.90,-.23,.90),.048,black)
for y in [-.23,.23]:
 rod('iron • footpeg',(-.23,y*.5,.48),(-.23,y,.48),.024,steel)
# Chain, sprocket, radiator, hoses.
torus('iron • rear sprocket',(-.8,.145,.38),.13,.012,gold)
for z in [.30,.46]:rod('iron • chain run',(-.79,.15,z),(-.17,.15,z+.05),.011,black)
box('iron • radiator',(.26,0,.79),(.07,.29,.26),black,.012)
for i in range(9):box('iron • radiator grille',(.303,-.13+i*.032,.79),(.008,.008,.23),alloy,.002)
rod('rubber • coolant hose',(.25,-.15,.87),(.11,-.17,.72),.018,rubber)
# Organise selectable parts into collections.
for group,objects in parts.items():
 col=bpy.data.collections.new(group);sc.collection.children.link(col)
 for o in objects:
  for c in list(o.users_collection):c.objects.unlink(o)
  col.objects.link(o)
# Studio lighting and cameras.
world=bpy.data.worlds.new('Studio');world.use_nodes=True;world.node_tree.nodes['Background'].inputs[0].default_value=(.12,.16,.23,1);world.node_tree.nodes['Background'].inputs[1].default_value=.35;sc.world=world
for p,power,size in [((0,-3,5),800,4),((-3,1,3),1000,3),((3,2,4),1300,3)]:
 bpy.ops.object.light_add(type='AREA',location=p);o=bpy.context.object;o.data.energy=power;o.data.shape='DISK';o.data.size=size;o.rotation_euler=(Vector((0,0,.6))-o.location).to_track_quat('-Z','Y').to_euler()
bpy.ops.object.camera_add(location=(3.2,-5,2.5));cam=bpy.context.object;cam.name='Presentation camera';cam.rotation_euler=(Vector((0,0,.62))-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.type='ORTHO';cam.data.ortho_scale=3.15;sc.camera=cam
sc.render.engine='CYCLES';sc.cycles.samples=48;sc.render.resolution_x=1400;sc.render.resolution_y=900;sc.render.resolution_percentage=100;sc.render.film_transparent=True
sc.view_settings.view_transform='AgX'
# Save editable model with the attractive presentation camera.
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'Ignition-450.blend'))
sc.render.filepath=str(ROOT/'preview.png');bpy.ops.render.render(write_still=True)
# Game layers share a side camera: 3.2m width, 1.6m height, bottom z=-.1.
cam.location=(0,-6,.7);cam.rotation_euler=(math.pi/2,0,0);cam.data.ortho_scale=3.2;sc.render.resolution_x=1024;sc.render.resolution_y=512
for group in ['Rear wheel','Front wheel']:
 for o in parts[group]:o.hide_render=True
sc.render.filepath=str(ROOT/'chassis.png');bpy.ops.render.render(write_still=True)
for group in ['Rear wheel','Front wheel']:
 for objs in parts.values():
  for o in objs:o.hide_render=o not in parts[group]
 # Hide the spring curve and text for wheel-only render.
 for o in sc.objects:
  if o.type in {'CURVE','FONT'}:o.hide_render=True
 cam.location.x= -.8 if group=='Rear wheel' else .8;cam.location.z=.38;cam.data.ortho_scale=.84;sc.render.resolution_x=384;sc.render.resolution_y=384
 sc.render.filepath=str(ROOT/('rear-wheel.png' if group=='Rear wheel' else 'front-wheel.png'));bpy.ops.render.render(write_still=True)
print('STUNT BIKE COMPLETE:',ROOT)
