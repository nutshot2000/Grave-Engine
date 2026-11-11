"""Build a material playground: hills, bogs, demolition and breakable scenery."""
from pathlib import Path
import math
r=Path(__file__).resolve().parent.parent
lines=['# Stunt Yard: Wreck Run. Each section has a different riding problem.','size 3600 260','seed 42','fill air','rect stone 0 210 3600 50']
def rect(m,x,y,w,h):
 if w>0 and h>0: lines.append(f'rect {m} {x} {y} {w} {h}')
# Engine level shapes (see the top of src/scene.rs): hills, slopes and
# covers are single scene lines, filled solid underneath.
def profile(a,b,height,material='stone',cover=None,depth=0):
 if cover=='ice':
  # Gravel on the way up, ice on the way down: one hill, two covers.
  mid=(a+b)//2
  lines.append(f'hill {material} {a} {b} {height} 210 cover gravel {depth}')
  for x in range(mid,b):
   y=210-round(height*math.sin(math.pi*(x-a)/(b-a))**2);rect('ice',x,y,1,depth)
 elif cover:lines.append(f'hill {material} {a} {b} {height} 210 cover {cover} {depth}')
 else:lines.append(f'hill {material} {a} {b} {height} 210')
def ramp(a,b,h):lines.append(f'slope stone {a} 210 {b} {210-h}')
def landing(a,b,y1,y2):lines.append(f'slope stone {a} {y1} {b} {y2}')
def pit(a,b,material,depth):
 rect('air',a,210,b-a,depth);rect(material,a,212,b-a,depth-2)
 if material in ['water','wet_sand']:lines.append(f'slope stone {b-30} {210+depth} {b} 210')
# 1. Jump an actual wreck, fuel and tyre stacks.
ramp(170,255,32)
landing(300,390,188,210)
rect('wood',266,199,24,11);rect('oil',268,196,20,3)
for x in [269,280,290]:lines.append(f'circle rubber {x} 194 5')
# 2. Broad dunes: substantial loose sand over a buried packed-earth core.
profile(440,690,38,'sandstone','sand',5)
profile(690,990,52,'sandstone','sand',6)
# Fast riders leave the second crest and hit this low overhang. Brake downhill.
# A short kicker before the low beam: roll over slowly or launch into it.
profile(970,1030,10)
# Anchored overhead beam is added after the bike below.
# 3. A real wet-sand bog and water, with uneven gravel bed.
pit(1150,1250,'wet_sand',10)
rect('water',1170,212,40,2)
# 4. Demolition leap: charges below the flight path and a destructible tower.
ramp(1370,1490,30)
landing(1550,1650,194,210)
rect('brick',1497,207,32,3);rect('gunpowder',1505,205,5,2)
rect('wood',1498,180,3,27);rect('wood',1525,180,3,27);rect('wood',1498,178,30,3)
rect('oil',1507,201,13,3)
# 5. Timber deck above a water trench, with genuinely breakable planks.
pit(1750,1950,'water',27)
for x in range(1750,1950,25):rect('wood',x,213,3,24)
rect('wood',1750,208,200,3)
# 6. Gravel rollers and a slippery ice descent.
profile(2020,2190,30,'stone','gravel',7)
profile(2190,2390,38,'stone','ice',4)
pit(2410,2470,'water',4)
# 7. A longer lava jump with a burning timber scaffold.
ramp(2520,2650,32)
pit(2650,2700,'lava',24)
landing(2700,2820,192,210)
rect('wood',2660,190,35,3);rect('wood',2660,193,3,17);rect('wood',2692,193,3,17)
# 8. Final sand climb into an explosive demolition finish.
profile(2900,3180,42,'sandstone','sand',5)
ramp(3210,3290,28)
rect('gunpowder',3310,207,5,2);rect('wood',3304,200,22,4)
rect('oil',3306,197,18,3)
landing(3340,3410,192,210)
# Keep the initial prop before the bike for stable manifest ordering.
lines+=['body rect wood 1360 193 1 17','depth 0.01']
lines+=(r/'assets/stunt-bike/physical-bike.txt').read_text(encoding='utf-8').splitlines()
lines += ['body rect iron 1040 166 90 8','depth 0.08','name low_gantry','weld low_gantry ground at 1085 166']
for x in [1680,1980,2860,3450]:lines += [f'body rect wood {x} 193 1 17','depth 0.01']
(r/'scenes/stunt-yard.txt').write_text('\n'.join(lines)+'\n',encoding='utf-8')
# Proximity cues heat existing cells; the engine does all combustion and blasts.
(r/'scenes/stunt-yard-events.txt').write_text('''# crossing-x heat-centre-x y radius temperature label
190 280 196 20 800 TYRE FIRE — CLEAR THE WRECK
1535 1508 205 8 1200 DEMOLITION CHARGE!
2530 2676 192 24 900 BURNING BRIDGE — KEEP YOUR SPEED
3340 3312 207 10 1200 FINAL CHARGE!
''',encoding='utf-8')
