"""Render independently simulated motorcycle assemblies and material-cell masks."""
import bpy, math, json
from pathlib import Path
root=Path(r'C:/Users/batti/Desktop/Grave-Engine/assets/stunt-bike')
bpy.ops.wm.open_mainfile(filepath=str(root/'Ignition-450.blend'))
s=bpy.context.scene;s.cycles.samples=16;s.render.film_transparent=True
cam=s.camera;cam.location=(0,-6,.7);cam.rotation_euler=(math.pi/2,0,0);cam.data.ortho_scale=3.2
names=['frame','engine','fork','tank','tail','mudguard','exhaust','seat','rear-wheel','front-wheel']
def group(o):
 n=o.name.lower()
 if o.get('part')=='Rear wheel':return 'rear-wheel'
 if o.get('part')=='Front wheel':return 'front-wheel'
 if any(k in n for k in ['number board','fork','stanchion','triple clamp','handlebar','bar grip','lever']):return 'fork'
 if any(k in n for k in ['fuel tank','radiator','coolant']):return 'tank'
 if any(k in n for k in ['tail fairing','side number','race number']):return 'tail'
 if 'mudguard' in n:return 'mudguard'
 if any(k in n for k in ['exhaust','muffler']):return 'exhaust'
 if 'saddle' in n:return 'seat'
 if any(k in n for k in ['crankcase','engine cover','cover bolt','cooling fin']):return 'engine'
 return 'frame'
objects=[o for o in s.objects if o.type in {'MESH','CURVE','FONT'}]
colors={'iron':(1,0,0,1),'rubber':(0,1,0,1),'brass':(0,0,1,1)}
flat={}
for name,c in colors.items():
 m=bpy.data.materials.new('mask '+name);m.use_nodes=True;nodes=m.node_tree.nodes;nodes.clear();e=nodes.new('ShaderNodeEmission');e.inputs[0].default_value=c;o=nodes.new('ShaderNodeOutputMaterial');m.node_tree.links.new(e.outputs[0],o.inputs[0]);flat[name]=m
masks={}
for name in names:
 for o in objects:o.hide_render=group(o)!=name
 s.render.resolution_x=1024;s.render.resolution_y=512;s.view_settings.view_transform='AgX';s.render.filepath=str(root/(name+'.png'));bpy.ops.render.render(write_still=True)
 originals=[]
 for o in objects:
  if o.hide_render:continue
  originals.append((o,list(o.data.materials)))
  for i,m in enumerate(o.data.materials):o.data.materials[i]=flat.get(m.name.split(' |')[0],flat['iron'])
 s.render.resolution_x=116;s.render.resolution_y=58;s.view_settings.view_transform='Standard';s.render.filepath=str(root/(name+'-mask.png'));bpy.ops.render.render(write_still=True)
 img=bpy.data.images.load(s.render.filepath,check_existing=False);pix=list(img.pixels);grid=[]
 for y in range(29):
  for x in range(58):
   samples=[]
   for dy in range(2):
    for dx in range(2):
     i=((57-(y*2+dy))*116+x*2+dx)*4
     if pix[i+3]>.4:samples.append(pix[i:i+3])
   if not samples:grid.append(None)
   else:
    rgb=[sum(p[k] for p in samples) for k in range(3)];grid.append(['iron','rubber','brass'][max(range(3),key=lambda k:rgb[k])])
 masks[name]=grid
 for o,materials in originals:
  for i,m in enumerate(materials):o.data.materials[i]=m
# Assign overlaps to the outermost assembly so no two bodies own one cell.
# Crop each body around its real bounds. The side camera maps 58 cells over 3.2 m.
parts=[];lines=['legend I iron','legend R rubber','legend B brass','legend . air']
for name in names:
 grid=masks[name];used=[i for i,m in enumerate(grid) if m]
 if not used:continue
 x0=min(i%58 for i in used);x1=max(i%58 for i in used)+1;y0=min(i//58 for i in used);y1=max(i//58 for i in used)+1
 w=x1-x0;h=y1-y0;cx=63-29+x0+w/2;cy=195.2-14.5+y0+h/2
 lines+=['body %.3f %.3f 0'%(cx,cy)]+[''.join({'iron':'I','rubber':'R','brass':'B',None:'.'}[grid[y*58+x]] for x in range(x0,x1)) for y in range(y0,y1)]+['end','name '+name,'depth 0.02']
 parts.append(dict(name=name,x=cx,y=cy,left=34-cx,top=180.7-cy,width=58,height=29,cells=len(used),materials={m:grid.count(m) for m in colors}))
limits={'engine':5000,'fork':2000,'tank':500,'tail':500,'mudguard':500,'exhaust':500,'seat':500}
for name in names[1:-2]:lines+=['weld frame %s break %s'%(name,limits[name])]
for name in names[-2:]:lines+=['wheel frame %s spring 4 damping 1 travel 2 motor 10 strength 2 drive break 2000'%name]
lines+=['follow frame zoom 3']
(root/'physical-bike.txt').write_text('\n'.join(lines)+'\n',encoding='utf-8')
(root/'parts.json').write_text(json.dumps(parts,indent=2),encoding='utf-8')
# Simple tab-separated visual metadata needs no extra runtime dependency.
(root/'parts.tsv').write_text('\n'.join('%s\t%s\t%s\t%s\t%s\t%s'%(p['name'],p['left'],p['top'],p['width'],p['height'],p['cells']) for p in parts),encoding='utf-8')
print('EXPORTED',[(p['name'],p['cells']) for p in parts])
