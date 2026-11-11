"""An exterior high-street shop elevation, drawn as actual material cells."""
from pathlib import Path
from make_explosive_props import Prop

ROOT=Path(__file__).resolve().parents[1]
p=Prop("shop_front",128,112)
def box(part,mat,x,y,w,h,c):p.rect(part,mat,x,y,w,h,c)
def weld(a,b,x,y,f=1200):p.welds.append(f"weld {a} {b} {x} {y} {f}")
font={
"G":[14,17,16,23,17,17,15],"R":[30,17,17,30,20,18,17],
"A":[14,17,17,31,17,17,17],"V":[17,17,17,17,17,10,4],
"E":[31,16,16,30,16,16,31],"S":[15,16,16,14,1,1,30],
"T":[31,4,4,4,4,4,4],"O":[14,17,17,17,17,17,14],
"P":[30,17,17,30,16,16,16],"N":[17,25,25,21,19,19,17]}
def text(x,y,words,c):
    for n,ch in enumerate(words):
        for yy,row in enumerate(font.get(ch,[0]*7)):
            for xx in range(5):
                if row&(1<<(4-xx)):p.tint(x+n*6+xx,y+yy,c)
def reflections(part,x,y,w,h):
    box(part,"glass",x,y,w,h,"629eac")
    for yy in range(h):
        for xx in range(w):
            c="50828e" if yy>h-5 else "74abb7"
            if xx in (w//2,w//2+1):c="ddd5bb"
            if 0<xx+yy-w//2<3 or w//2+8<xx+yy<w//2+10:c="b7d9d8"
            if yy==h//2:c="e3d9bb"
            p.tint(x+xx,y+yy,c)

# Opaque brick exterior. Every opening is replaced by its own frame/glazing.
box("brick_facade","brick",8,10,112,95,"ad6c50")
box("top_cornice","stone",6,7,116,3,"d5c6a3")
box("top_cornice","stone",8,10,112,2,"8e8877")
for frame,pane,x in [("upper_left_frame","upper_left_glass",21),("upper_right_frame","upper_right_glass",83)]:
    box(frame,"wood",x,16,25,23,"ddd3b4")
    reflections(pane,x+3,19,19,17)
    weld("brick_facade",frame,x+12,16)
    weld(frame,pane,x+12,27,130)
    box(frame,"wood",x-1,38,27,2,"a39a7d")

box("timber_shopfront","wood",12,55,104,50,"315c50")
box("timber_shopfront","wood",12,101,104,4,"203e39")
for x in (12,49,75,113):box("timber_shopfront","wood",x,65,3,37,"517567")
for x in (17,80):
    box("timber_shopfront","wood",x-2,68,35,29,"b6a476")
for name,x in [("left_display_glass",17),("right_display_glass",80)]:
    reflections(name,x,70,31,25)
    weld("timber_shopfront",name,x+15,69,130)
    # Shop-window shelf reflections, still a continuous real glass sheet.
    for yy in (88,93):
        for xx in range(x+2,x+29):p.tint(xx,yy,"c5c5a7")
    for xx,c in [(x+5,"a7aa74"),(x+11,"ddba80"),(x+23,"9fbda5")]:
        for yy in range(89,93):
            for xxx in range(xx,xx+3):p.tint(xxx,yy,c)

box("entrance_door","wood",55,68,18,36,"24483f")
box("entrance_door","wood",57,91,14,10,"486a55")
box("door_glass","glass",58,71,12,18,"7badb3")
box("door_glass","glass",58,80,12,9,"446f74")
tiny={"O":[7,5,5,5,7],"P":[7,5,7,4,4],"E":[7,4,6,4,7],"N":[5,7,7,7,5]}
for n,ch in enumerate("OPEN"):
    for yy,row in enumerate(tiny[ch]):
        for xx in range(3):
            if row&(1<<(2-xx)):p.tint(56+n*4+xx,74+yy,"e4dbb6")
box("door_handle","brass",70,88,1,4,"e7c779")
weld("timber_shopfront","entrance_door",55,84,180)
weld("entrance_door","door_glass",58,79,100)
weld("entrance_door","door_handle",70,90,80)
box("doorstep","stone",52,104,24,3,"a2a59c")
weld("brick_facade","doorstep",64,104,2400)

box("shop_sign","wood",12,42,104,13,"24483f")
for y in (42,54):
    for x in range(12,116):p.tint(x,y,"be9f5e")
text(28,45,"GRAVE STORES","ead698")
weld("brick_facade","shop_sign",18,48,900)
weld("brick_facade","shop_sign",110,48,900)

# Fabric awning, striped cream and burgundy, with a scalloped valance.
box("awning_support","wood",12,55,104,2,"315c50")
for cells in p.parts.values():
    for y in range(62,66):
        for x in range(12,116):cells.pop((x,y),None)
box("striped_awning","cloth",12,57,104,5,"dfcfaa")
for y in range(57,64):
    for x in range(12,116):
        if y<62 or (x-12)%8 in (1,2,3,4,5,6):
            p.cell("striped_awning","cloth",x,y,"ab4941" if ((x-12)//8)%2 else "e4d3aa")
for x in range(12,116):p.tint(x,57,"92795c")
weld("awning_support","striped_awning",18,57,400)
weld("awning_support","striped_awning",110,57,400)
weld("brick_facade","awning_support",12,55,1000)
weld("brick_facade","awning_support",115,55,1000)
weld("brick_facade","timber_shopfront",12,68,1800)
weld("brick_facade","timber_shopfront",115,68,1800)

# Low timber kick panels with painted raised trim.
for x in (16,80):
    box("timber_shopfront","wood",x,98,32,2,"63806a")

assert len(p.parts)<=24
legend={m:chr(65+i) for i,m in enumerate(sorted({m for cells in p.parts.values() for m,_ in cells.values()}))}
out=["# Exterior elevation: opaque brick facade, glazed display windows and a fabric awning.",
     "format 1","asset shop_front 128 112","depth 0.06","pivot 64 107"]
out += [f"legend {ch} {m}" for m,ch in legend.items()]
for name,cells in p.parts.items():
    out += [f"part {name}","grid"]
    out += ["".join(legend[cells[x,y][0]] if (x,y) in cells else "." for x in range(p.w)) for y in range(p.h)]
    out += ["end"]
    out += [f"paint {x} {y} #{c}" for (x,y),(_,c) in sorted(cells.items(),key=lambda v:(v[0][1],v[0][0]))]
out += p.welds
(ROOT/'assets/workshop/shop_front.asset').write_text('\n'.join(out)+'\n',encoding='utf-8')
