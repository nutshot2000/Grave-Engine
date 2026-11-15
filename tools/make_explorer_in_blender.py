"""Build and animate the cave explorer from the engine's editable hero rig.
Run with Blender: blender -b --factory-startup -P tools/make_explorer_in_blender.py
The separate character adds a helmet, luminous visor, utility pack and drill.
No external models or textures are used. PNG metadata is stripped afterwards.
"""
from pathlib import Path
import math
import re
import struct
import bpy

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / 'assets' / 'characters' / 'explorer'
source = (ROOT / 'tools' / 'make_hero_in_blender.py').read_text(encoding='utf-8')
source = source.replace('RENDER = True', 'RENDER = False').replace('Grave Engine hero', 'Grave Engine explorer')
source = re.sub(r'^OUT = .*$', 'OUT = '+repr(OUT_DIR.as_posix()), source, flags=re.M)
exec(compile(source, 'make_hero_in_blender.py', 'exec'), globals())
OUT_DIR.mkdir(parents=True, exist_ok=True)

# Strong silhouette: amber suit, graphite equipment and cool illuminated face.
JACKET.node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.62,0.25,0.045,1)
PANTS.node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.055,0.075,0.085,1)
SCARF.node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.03,0.3,0.32,1)
helmet = material('Explorer helmet', (0.78,0.44,0.09),0.36,0.25)
visor = material('Explorer visor', (0.035,0.34,0.4),0.2,0.25,emit=(0.025,0.28,0.33))
case = material('Explorer graphite', (0.065,0.09,0.105),0.4,0.4)
lamp = material('Explorer lamp', (0.8,0.9,0.8),0.2,0,emit=(0.7,0.9,0.8))
part('Protective helmet', sphere, helmet, neck, (0,0,1.69),(0.155,0.14,0.135))
part('Helmet visor', sphere, visor, neck, (0.105,-0.04,1.65),(0.07,0.115,0.075))
part('Headlamp housing', cyl, case, neck, (0.155,0,1.76),(0.045,0.045,0.055),rot=(0,math.pi/2,0))
part('Headlamp lens', sphere, lamp, neck, (0.19,0,1.76),(0.017,0.035,0.035))
part('Jetpack', cube, case, spine, (-0.20,0,1.22),(0.16,0.26,0.35))
for yy in [-0.13,0.13]:
 part('Pack tank', cyl, helmet, spine, (-0.20,yy,1.20),(0.055,0.055,0.29))
 part('Thruster nozzle', cyl, case, spine, (-0.20,yy,1.02),(0.07,0.07,0.065))
part('Utility glove', sphere, case, el_f, (0,-0.13,0.91),(0.055,0.05,0.07))
part('Hand drill', cube, case, el_f, (0.07,-0.13,0.9),(0.18,0.095,0.09))
part('Drill bit', cyl, METAL, el_f, (0.19,-0.13,0.9),(0.028,0.028,0.13),rot=(0,math.pi/2,0))

ANIMS['idle']=(cycle(8,idle),8,True)
ANIMS['walk']=(cycle(12,walk),16,True)
ANIMS['swim']=(cycle(8,swim),10,True)
ANIMS['die']=(die(8),12,False)
ANIMS['fly']=(cycle(8,lambda ph:dict(lean=10,hip_f=18,hip_b=-8,kn_f=-40,kn_b=-22,sh_f=35,sh_b=15,el_f=55,el_b=40,lift=0.01*S(ph))),10,True)
ANIMS['drill']=(cycle(8,lambda ph:dict(lean=10,hip_f=7,kn_f=-7,sh_f=70,el_f=65,sh_b=15,el_b=15,lift=.002*S(ph))),14,True)
scene.render.resolution_x=192;scene.render.resolution_y=192
scene.render.use_freestyle=False
try:scene.eevee.taa_render_samples=16
except AttributeError:pass
for name,(poses,fps,loop) in ANIMS.items():
 for i,p in enumerate(poses):
  pose(1,p);scene.frame_set(1)
  scene.render.filepath=str(OUT_DIR/f'{name}_{i:02}.png')
  bpy.ops.render.render(write_still=True)
  # PNG render metadata can contain local project paths. Keep only image data.
  path=OUT_DIR/f'{name}_{i:02}.png'
  data=path.read_bytes();clean=data[:8];offset=8
  while offset<len(data):
   length=struct.unpack('>I',data[offset:offset+4])[0];kind=data[offset+4:offset+8]
   if kind in (b'IHDR',b'PLTE',b'tRNS',b'IDAT',b'IEND'):clean+=data[offset:offset+length+12]
   offset+=length+12
  path.write_bytes(clean)
for j in JOINTS.values():j.animation_data_clear()
pose(1,idle(0))
scene.render.filepath='//explorer-render.png'
bpy.ops.wm.save_as_mainfile(filepath=str(OUT_DIR/'explorer.blend'))
text='# Explorer: original model, tools/make_explorer_in_blender.py\nframe = 192\nfigure = 144\nfeet = 176\nfaces = right\n'
for name,(poses,fps,loop) in ANIMS.items():text+=f'{name} = {len(poses)} frames, {fps} fps'+(', loop' if loop else '')+'\n'
(OUT_DIR/'character.txt').write_text(text,encoding='utf-8')
print('Explorer character rendered.')
