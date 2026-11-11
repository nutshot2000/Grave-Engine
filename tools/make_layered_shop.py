"""Aligned shop facade, physical furnishings, and a separate masonry back wall."""
from pathlib import Path
import runpy

ROOT=Path(__file__).resolve().parents[1]
# Regenerate the original exterior, keeping the standalone shop asset available.
runpy.run_path(str(ROOT/'tools/make_shop_front.py'))
front=(ROOT/'assets/workshop/shop_front.asset').read_text(encoding='utf-8')
front=front.replace('asset shop_front 128 112','asset layered_shop 128 112')
parts={}
def put(part,x,y,m='wood',c='956840'):
    for cells in parts.values():cells.pop((x,y),None)
    parts.setdefault(part,{})[x,y]=(m,c)
def rect(part,x,y,w,h,m='wood',c='956840'):
    for yy in range(y,y+h):
        for xx in range(x,x+w):put(part,xx,yy,m,c)

# Structural timber inside the shop, with open air around furnishings.
rect('interior_frame',10,12,3,94,c='987649')
rect('interior_frame',115,12,3,94,c='987649')
rect('interior_frame',13,42,102,3,c='bb925c')
rect('interior_frame',13,103,102,3,c='bb925c')

def shelving(name,x,y,w,h):
    rect(name,x,y,2,h,c='885b36');rect(name,x+w-2,y,2,h,c='885b36')
    for shelf_y in range(y+7,y+h,8):
        rect(name,x,shelf_y,w,2,c='c19b64')
        for n in range(3,w-3,5):
            height=3+(n%3)
            rect(name,x+n,shelf_y-height,3,height,'cloth',['c68142','628c66','aa4d42','d4bc76'][(n//5)%4])
shelving('left_shelves',17,69,31,33)
shelving('right_shelves',80,69,31,23)
rect('counter',79,96,34,7,c='74503b')
rect('counter',78,94,36,2,c='c9a779')
rect('counter',96,89,10,5,'iron','637275')
rect('counter',97,90,5,2,'glass','83b8a2')
rect('upstairs_desk',23,34,25,2,c='c49c6c')
rect('upstairs_desk',25,36,2,6,c='805b3d')
rect('upstairs_desk',44,36,2,6,c='805b3d')
rect('upstairs_desk',29,29,12,5,'cloth','b2c2bc')
shelving('upstairs_cupboard',83,20,25,22)

def export(layer,items):
    out=[f'layer {layer}','legend Z wood','legend Y cloth','legend X iron','legend Q glass','legend J brick']
    legend={'wood':'Z','cloth':'Y','iron':'X','glass':'Q','brick':'J'}
    for name,cells in items.items():
        out += [f'part {name}','grid']
        out += [''.join(legend[cells[x,y][0]] if (x,y) in cells else '.' for x in range(128)) for y in range(112)]
        out += ['end']
        out += [f'paint {x} {y} #{c}' for (x,y),(_,c) in sorted(cells.items(),key=lambda v:(v[0][1],v[0][0]))]
    return '\n'.join(out)+'\n'

out=front+export('interior',parts)
for name,x,y in [('left_shelves',18,102),('right_shelves',109,92),('counter',110,103),('upstairs_desk',25,42),('upstairs_cupboard',84,42)]:
    out+=f'weld interior_frame {name} {x} {y} 800\n'
# Plaster-coloured brickwork belongs to the rear plane, never filling the air
# around the shelves. Flames reach the rear wall but it cannot burn like wood.
rear={}
for y in range(12,106):
    for x in range(8,120):
        color='cab18c' if y<43 else 'c6b692'
        if y in (43,44,102,103):color='847c69'
        if x in (8,9,118,119):color='a29376'
        rear[x,y]=('brick',color)
out+=export('back_wall',{'plastered_back_wall':rear})
(ROOT/'assets/workshop/layered_shop.asset').write_text(out,encoding='utf-8')
