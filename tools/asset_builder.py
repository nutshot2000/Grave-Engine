"""Build detailed engine assets from JSON recipes; no image generator required.

py tools/asset_builder.py assets/recipes/iron_heron.json [--out FILE] [--live]
See docs/AI-ASSET-BUILDER.md for the recipe format and the review workflow.
"""
import argparse
import json
import math
import re
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
FONT={
 'A':['010','101','111','101','101'],'B':['110','101','110','101','110'],
 'C':['111','100','100','100','111'],'D':['110','101','101','101','110'],
 'E':['111','100','110','100','111'],'F':['111','100','110','100','100'],
 'G':['111','100','101','101','111'],'H':['101','101','111','101','101'],
 'I':['111','010','010','010','111'],'J':['001','001','001','101','111'],
 'K':['101','101','110','101','101'],'L':['100','100','100','100','111'],
 'M':['101','111','111','101','101'],'N':['101','111','111','111','101'],
 'O':['111','101','101','101','111'],'P':['110','101','110','100','100'],
 'Q':['111','101','101','111','001'],'R':['110','101','110','101','101'],
 'S':['111','100','111','001','111'],'T':['111','010','010','010','010'],
 'U':['101','101','101','101','111'],'V':['101','101','101','101','010'],
 'W':['101','101','111','111','101'],'X':['101','101','010','101','101'],
 'Y':['101','101','010','010','010'],'Z':['111','001','010','100','111'],
 '0':['111','101','101','101','111'],'1':['010','110','010','010','111'],
 '2':['110','001','010','100','111'],'3':['110','001','010','001','110'],
 '4':['101','101','111','001','001'],'5':['111','100','110','001','110'],
 '6':['011','100','111','101','111'],'7':['111','001','010','010','010'],
 '8':['111','101','111','101','111'],'9':['111','101','111','001','110'],
 '-':['000','000','111','000','000'],' ':['000']*5,
}

def name(value):
    if not isinstance(value,str) or not re.fullmatch(r'[A-Za-z0-9_-]{1,48}',value):
        raise ValueError(f'Invalid name: {value!r}; use 1..48 letters, digits, _ or -')
    return value

def integer(n,low,high,label):
    if isinstance(n,bool) or not isinstance(n,int) or not low<=n<=high:
        raise ValueError(f'{label} needs an integer from {low} to {high}')
    return n

def unique_object(pairs):
    out={}
    for key,value in pairs:
        if key in out:raise ValueError(f'Duplicate JSON key {key!r}')
        out[key]=value
    return out

def build(recipe):
    """Return native asset text and diagnostics. Does not write or mutate input."""
    asset_name=name(recipe['name'])
    w=integer(recipe['width'],1,128,'width');h=integer(recipe['height'],1,128,'height')
    depth=recipe.get('depth',.1)
    if not isinstance(depth,(int,float)) or not math.isfinite(depth) or not 0<depth<=10:raise ValueError('depth needs 0..10 metres, excluding zero')
    pivot=recipe.get('pivot',[w/2,h-1])
    if len(pivot)!=2 or any(not isinstance(v,(int,float)) or not math.isfinite(v)for v in pivot) or not 0<=pivot[0]<=w or not 0<=pivot[1]<=h:raise ValueError('pivot must be inside the grid')
    materials={p.stem for p in (ROOT/'materials').glob('*.txt')}
    styles={}
    for key,s in recipe['styles'].items():
        name(key);mat=s['material'];color=s.get('paint')
        if mat!='air' and mat not in materials:raise ValueError(f'Style {key}: unknown material {mat}')
        if color is not None and not re.fullmatch(r'#[0-9A-Fa-f]{6}',color):raise ValueError(f'Style {key}: paint needs #RRGGBB')
        styles[key]=(mat,color)
    parts={};owner={};warnings=[]
    if not 1<=len(recipe['parts'])<=256:raise ValueError('Use 1..256 named parts')
    for spec in recipe['parts']:
        part_name=name(spec['name'])
        if part_name in parts:raise ValueError(f'Duplicate part {part_name}')
        cells={};parts[part_name]=cells
        def put(x,y,value):
            if not 0<=x<w or not 0<=y<h:raise ValueError(f'{part_name}: drawing goes outside the grid at {x},{y}')
            key=(x,y);mat,_=value
            if mat=='air':
                cells.pop(key,None)
                if owner.get(key)==part_name:owner.pop(key)
                return
            old=owner.get(key)
            if old is not None and old!=part_name:
                if not spec.get('replace_overlap',False):raise ValueError(f'{part_name} overlaps {old} at {x},{y}; move it, or explicitly set replace_overlap')
                parts[old].pop(key,None)
            owner[key]=part_name;cells[key]=value
        def point(value):
            if len(value)!=2:raise ValueError('Points need [x,y]')
            return integer(value[0],0,w-1,'x'),integer(value[1],0,h-1,'y')
        def rect(x,y,ww,hh,value):
            integer(ww,1,w,'rectangle width');integer(hh,1,h,'rectangle height')
            for yy in range(y,y+hh):
                for xx in range(x,x+ww):put(xx,yy,value)
        def line(a,b,value,width=1):
            integer(width,1,16,'line width');n=max(abs(b[0]-a[0]),abs(b[1]-a[1]),1)
            for i in range(n+1):rect(round(a[0]+(b[0]-a[0])*i/n)-width//2,round(a[1]+(b[1]-a[1])*i/n)-width//2,width,width,value)
        for op in spec['shapes']:
            kind=op['op'];value=styles[op['style']]
            if kind=='rect':rect(*point(op['at']),*op['size'],value)
            elif kind=='pixel':put(*point(op['at']),value)
            elif kind=='line':line(point(op['from']),point(op['to']),value,op.get('width',1))
            elif kind=='circle':
                x,y=point(op['at']);r=integer(op['radius'],0,128,'radius')
                for yy in range(y-r,y+r+1):
                    for xx in range(x-r,x+r+1):
                        if (xx-x)**2+(yy-y)**2<=r*r:put(xx,yy,value)
            elif kind=='poly':
                pts=[point(p)for p in op['points']]
                if len(pts)<3:raise ValueError('Polygon needs at least three points')
                for y in range(min(p[1]for p in pts),max(p[1]for p in pts)+1):
                    for x in range(min(p[0]for p in pts),max(p[0]for p in pts)+1):
                        inside=False
                        for (ax,ay),(bx,by)in zip(pts,pts[1:]+pts[:1]):
                            if (ay>y)!=(by>y) and x<(bx-ax)*(y-ay)/(by-ay)+ax:inside=not inside
                        if inside:put(x,y,value)
            elif kind=='rivets':
                a=point(op['from']);b=point(op['to']);count=integer(op['count'],1,128,'rivet count')
                for i in range(count):
                    t=i/max(1,count-1);put(round(a[0]+(b[0]-a[0])*t),round(a[1]+(b[1]-a[1])*t),value)
            elif kind=='text':
                x,y=point(op['at']);scale=integer(op.get('scale',1),1,8,'text scale')
                for i,c in enumerate(op['text'].upper()):
                    if c not in FONT:raise ValueError(f'Unsupported lettering character {c!r}')
                    for yy,row in enumerate(FONT[c]):
                        for xx,on in enumerate(row):
                            if on=='1':rect(x+(i*4+xx)*scale,y+yy*scale,scale,scale,value)
            else:raise ValueError(f'Unknown shape {kind}')
    for part_name,cells in parts.items():
        if not cells:raise ValueError(f'{part_name} is empty or was completely overwritten')
        unseen=set(cells);islands=0
        while unseen:
            islands+=1;todo=[unseen.pop()]
            while todo:
                x,y=todo.pop()
                for n in [(x-1,y),(x+1,y),(x,y-1),(x,y+1)]:
                    if n in unseen:unseen.remove(n);todo.append(n)
        if islands>1:warnings.append(f'{part_name}: {islands} disconnected islands; thicken diagonals or make separate joined parts')
    for check in recipe.get('keepouts',[]):
        cx,cy=check['center'];radius=check['radius']+check.get('margin',0)
        if not math.isfinite(radius)or radius<=0:raise ValueError('Keepout radius must be positive')
        for part_name in check['against']:
            if part_name not in parts:raise ValueError(f'Unknown keepout part {part_name}')
            if any((x-cx)**2+(y-cy)**2<radius**2 for x,y in parts[part_name]):warnings.append(f'{part_name} enters the swept keepout {check.get("name","circle")}')
    out=['# Built from a reusable AI asset recipe. Materials and surface paint are separate.','format 1',f'asset {asset_name} {w} {h}',f'depth {depth}',f'pivot {pivot[0]} {pivot[1]}']
    for part_name,cells in parts.items():
        out.append('part '+part_name)
        for y in range(h):
            x=0
            while x<w:
                value=cells.get((x,y))
                if value is None:x+=1;continue
                end=x+1
                while end<w and cells.get((end,y),(None,None))[0]==value[0]:end+=1
                out.append(f'rect {value[0]} {x} {y} {end-x} 1');x=end
        out.extend(f'paint {x} {y} {color}'for (x,y),(mat,color)in sorted(cells.items(),key=lambda p:(p[0][1],p[0][0]))if color)
    for kind,lines in [('weld',recipe.get('welds',[])),('joint',recipe.get('joints',[]))]:
        for text in lines:
            if '\n'in text or '\r'in text:raise ValueError('Connections must be single lines')
            words=text.split();start=1 if kind=='joint'else 0
            if len(words)<start+2 or words[start]not in parts or words[start+1]not in {*parts,'ground'}:raise ValueError(f'Unknown part in {kind} {text}')
            if kind=='joint'and words[0]not in ['hinge','wheel','rope']:raise ValueError('Use hinge, wheel or rope joints')
            out.append(kind+' '+text)
    if len(parts)>1 and not recipe.get('welds')and not recipe.get('joints'):warnings.append('Multiple parts have no connections')
    return '\n'.join(out)+'\n',warnings

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('recipe',type=Path);p.add_argument('--out',type=Path)
    p.add_argument('--live',action='store_true',help='Announce and show the result in the open editor')
    p.add_argument('--strict',action='store_true',help='Do not write a result with geometry warnings')
    p.add_argument('--allow-remove-parts',action='store_true')
    a=p.parse_args();r=json.loads(a.recipe.read_text(encoding='utf-8'),object_pairs_hook=unique_object);text,warnings=build(r)
    out=(a.out or ROOT/'assets/workshop'/(r['name']+'.asset')).resolve()
    for warning in warnings:print('CHECK:',warning)
    if a.strict and warnings:raise SystemExit('Resolve the checks before publishing this asset.')
    if out.is_file() and not a.allow_remove_parts:
        existing={l.split()[1]for l in out.read_text(encoding='utf-8').splitlines()if l.startswith('part ')}
        removed=existing-{part['name']for part in r['parts']}
        if removed:raise SystemExit(f'Recipe would remove existing named parts {sorted(removed)}; preserve them or explicitly allow removal.')
    out.parent.mkdir(parents=True,exist_ok=True)
    temp=out.with_suffix(out.suffix+'.tmp');temp.write_text(text,encoding='utf-8');temp.replace(out)
    print(f'BUILT {out.name}: {len(r["parts"])} named parts; {len(r.get("joints",[]))} moving joints; {len(warnings)} checks')
    if a.live:
        relative=out.relative_to(ROOT).as_posix()
        (ROOT/'live').mkdir(exist_ok=True)
        (ROOT/'live/agent.txt').write_text(f'say Built {r["name"]}: named material parts, surface detail and checked connections\nopen {relative}\n',encoding='utf-8')

if __name__=='__main__':
    try:main()
    except (ValueError,KeyError,TypeError)as e:raise SystemExit(f'Recipe error: {e}')
