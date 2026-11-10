"""Author the long Stunt Yard course using normal engine scene commands."""
from pathlib import Path
r=Path(__file__).resolve().parent.parent
lines=['# Stunt Yard: Inferno Run, 360 metres of physical material obstacles.','size 3600 240','seed 42','fill air','rect stone 0 210 3600 30']
def rect(m,x,y,w,h):lines.append(f'rect {m} {x} {y} {w} {h}')
# Six launch / catch ramp pairs. Each is real terrain, not a scripted jump.
for n,start in enumerate([180,750,1320,1890,2460,3030]):
 height=36 if n==0 else 24
 for x in range(start,start+80,2):
  y=210-int((x-start)*height/80);rect('stone',x,y,2,240-y)
 for x in range(start+105,start+165,2):
  y=min(210,194+int((x-start-105)*.30));rect('stone',x,y,2,240-y)
 if n in [0,2,4]:
  rect('wood',start+84,204,15,6);rect('oil',start+85,201,12,3);lines.append(f'heat rect {start+84} 201 15 9 700')
 else:
  rect('air',start+82,210,20,16);rect('lava' if n==3 else 'water',start+84,220,16,6)
# Shallow water, loose gravel and sand provide different tyre surfaces.
for x in [510,1650,2790]:
 rect('air',x,210,55,2);rect('water',x,210,45,2)
for x in [1120,2240]:rect('gravel',x,208,45,2)
rect('sand',3370,208,50,2)
# First prop remains before the bike to preserve the asset manifest ordering.
lines+=['body rect wood 680 192 1 18']
lines+=(r/'assets/stunt-bike/physical-bike.txt').read_text(encoding='utf-8').splitlines()
# Destructible film-set obstacles. Supported by real ground.
for x in [1220,1800,2370,2940,3440]:
 lines += [f'body rect wood {x} 193 1 17','depth 0.01']
# Course furniture below/above the riding line; finish gantry has clearance.
for x in [420,990,1560,2130,2700,3270]:
 rect('brick',x,216,30,24)
rect('iron',3500,146,3,64);rect('iron',3570,146,3,64);rect('iron',3500,146,73,3)
# Keep the exit clear: the gantry posts are presentation marks below track level.
# Horizontal world is side-on: remove posts from riding band, retain top flags.
rect('air',3500,170,3,40);rect('air',3570,170,3,40)
(r/'scenes/stunt-yard.txt').write_text('\n'.join(lines)+'\n',encoding='utf-8')
