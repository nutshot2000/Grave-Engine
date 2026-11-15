"""Regenerate Iron Heron from its model-neutral recipe and paint its test scene."""
from pathlib import Path
import json
from asset_builder import build,FONT
ROOT=Path(__file__).resolve().parents[1]
recipe=json.loads((ROOT/'assets/recipes/iron_heron.json').read_text(encoding='utf-8'))
text,checks=build(recipe)
if checks:raise SystemExit('\n'.join(checks))
output=ROOT/'assets/workshop/iron_heron_pumpjack.asset'
temp=output.with_suffix('.asset.tmp');temp.write_text(text,encoding='utf-8');temp.replace(output)
dark='#25383b';teal='#3d7373';gold='#e5ba63';glyph=dict(FONT)
# Keep the delivery pipe, tank and well screen visibly metallic in the test scene.
scene=ROOT/'scenes/iron-heron-oilfield.txt'
if scene.is_file():
    marker='# Generated surface paint: underlying materials stay unchanged.'
    source=scene.read_text(encoding='utf-8').split(marker)[0].rstrip()
    fixed={}
    for raw in source.splitlines():
        w=raw.split()
        if len(w)==6 and w[0]=='rect':
            mat=w[1];x,y,ww,hh=map(int,w[2:])
            for yy in range(y,y+hh):
                for xx in range(x,x+ww):fixed[xx,yy]=mat
    paint=[]
    for (x,y),mat in sorted(fixed.items(),key=lambda p:(p[0][1],p[0][0])):
        if mat!='iron':continue
        color=dark
        if y in [137,143,192,200,205]or x in [107,113,273,278]:color=teal
        if x in [280,352]and 148<=y<187:color=teal
        if 280<=x<354 and y>=187 and y<190:color=gold if (x+y)%10<5 else dark
        if x in [281,352]and y in [152,160,168,176,184]:color=gold
        if 102<=x<120 and 264<=y<276:color=gold
        paint.append(f'paint {x} {y} {color}')
    glyph.update({'I':['111','010','010','010','111'],'L':['100','100','100','100','111']})
    for i,c in enumerate('OIL'):
        for y,row in enumerate(glyph[c]):
            for x,on in enumerate(row):
                if on=='1':paint.append(f'paint {256+i*4+x} {176+y} {gold}')
    scene.write_text(source+'\n\n'+marker+'\n'+'\n'.join(paint)+'\n',encoding='utf-8')
