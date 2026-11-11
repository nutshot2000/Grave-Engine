"""Build Mountain Run: a long trials ride from a valley at dusk, over a snowy
summit, down through a burning forest at night, through a mine and a quarry
to a floodlit finish. Writes scenes/mountain-run.txt.

The ground is one long profile of points; each stretch is a `slope` line
(filled solid underneath) in its own material, with an optional cover
(gravel, snow). Everything else (bridges, cabins, glass, mud, oil, lamps,
triggers) is placed on top. Run: python tools/make_mountain_run.py
"""
from pathlib import Path

root = Path(__file__).resolve().parent.parent
W, H = 5600, 760
L = []
add = L.append

add('# Mountain Run: a long trials ride into the night. Made by tools/make_mountain_run.py.')
add(f'size {W} {H}')
add('seed 11')
add('fill air')
add('wind 1.5')
add('ambient 0                      # a cold evening: snow and ice stay frozen, water stays liquid')

# ---------------------------------------------------------------- the sky --
add('# Backdrop: a dusk sky, far snowy peaks, nearer ridges, forest')
add('sky #1c2850 #e3875a stars')
add('layer clouds #f0b090 depth 0.04 y 120 height 30')
add('layer mountains #4d5480 depth 0.07 y 430 height 170 snow seed 4')
add('layer mountains #353b5e depth 0.18 y 560 height 120 seed 9')
add('layer forest #1e2c28 depth 0.45 y 720 height 70 seed 3')

# ------------------------------------------------------------- lighting --
add('# Lighting: a low evening sun; triggers bring the night on as you ride')
add('lighting 0.7 #ffcfa8')
add('sun 55 0.9 #ffb074')

# --------------------------------------------------------- the ground --
# (x, y, material, cover) — each point starts a stretch to the next point.
P = []
def pt(x, y, mat='stone', cover=None):
    P.append((x, y, mat, cover))

# A. Valley village (flat), a kicker over a fence
pt(0, 700); pt(290, 700); pt(335, 686); pt(338, 700, 'stone'); pt(380, 692); pt(430, 700)
pt(520, 700, 'rock', ('gravel', 3))
# B. First climb: slopes and ledges
pt(600, 688, 'rock'); pt(660, 664, 'rock', ('gravel', 3)); pt(700, 664, 'rock'); pt(703, 658, 'rock')
pt(760, 646, 'rock'); pt(820, 646, 'rock', ('gravel', 4)); pt(900, 616, 'rock'); pt(960, 612, 'rock')
pt(963, 606, 'rock'); pt(1040, 590, 'rock'); pt(1110, 560, 'rock'); pt(1180, 548, 'rock')
pt(1183, 543, 'rock'); pt(1260, 536, 'rock'); pt(1340, 520, 'rock'); pt(1420, 512, 'rock')
# C. Second climb: steeper, a crevasse with a wooden bridge
pt(1500, 504, 'rock'); pt(1580, 470, 'rock'); pt(1640, 458, 'rock'); pt(1700, 430, 'rock')
pt(1780, 424, 'rock')
CREV = (1782, 1838)  # the crevasse (bridged)
pt(1840, 424, 'rock'); pt(1900, 410, 'rock'); pt(1960, 384, 'rock'); pt(2020, 376, 'rock')
pt(2023, 370, 'rock'); pt(2100, 352, 'rock'); pt(2200, 340, 'rock', ('snow', 4))
# D. Snowy summit: snow, an ice patch, a frozen lake
pt(2300, 330, 'rock', ('snow', 3)); pt(2420, 332, 'rock', ('snow', 3))
LAKE = (2440, 2580)
# (a shallow dip with sloping banks: break through the ice and you can ride out)
pt(2440, 334, 'rock'); pt(2580, 334, 'rock'); pt(2600, 334, 'rock'); pt(2700, 314, 'rock')
pt(2740, 298, 'rock'); pt(2760, 294, 'rock'); pt(2775, 294, 'rock'); pt(2790, 297, 'rock'); pt(2805, 302, 'rock'); pt(2865, 322, 'rock'); pt(2876, 325, 'rock'); pt(2885, 326, 'rock'); pt(2893, 325, 'rock'); pt(2899, 323, 'rock'); pt(2905, 320, 'rock')
# E. The gorge: drop off the peak for speed, kick off at 2905, land on the
# far side going down
pt(2906, 700, 'rock'); pt(2995, 700, 'rock')
pt(3005, 404, 'rock'); pt(3200, 462, 'rock'); pt(3260, 476, 'rock')
# F. Forest descent (night): bumps, logs, a cabin, a trestle bridge
pt(3340, 492, 'dirt'); pt(3420, 520, 'dirt'); pt(3470, 522, 'dirt'); pt(3560, 556, 'dirt')
RAVINE = (3600, 3780)
pt(3600, 560, 'dirt'); pt(3780, 560, 'dirt'); pt(3860, 594, 'dirt'); pt(3930, 630, 'dirt'); pt(3970, 640, 'dirt')
# G. The mine: flat floor, a mountain overhead with a tunnel through it
pt(4000, 640, 'rock'); pt(4290, 640, 'rock')
PUDDLE = (4290, 4380)
pt(4310, 647, 'rock'); pt(4360, 647, 'rock'); pt(4380, 640, 'rock'); pt(4700, 640, 'rock')
# H. Quarry: concrete, glass, mud, a jump over burning oil
BOG = (4840, 4930)
pt(4840, 638, 'concrete'); pt(4855, 650, 'concrete'); pt(4915, 650, 'concrete'); pt(4930, 638, 'concrete')
pt(4960, 636, 'concrete'); pt(5120, 636, 'concrete'); pt(5140, 632, 'concrete'); pt(5170, 618, 'concrete')
PIT = (5172, 5222)
pt(5173, 700, 'concrete'); pt(5222, 700, 'concrete'); pt(5224, 650, 'concrete'); pt(5340, 660, 'concrete')
# I. Finish arena
pt(5400, 660, 'concrete'); pt(5600, 660, 'concrete')

def ground_y(x):
    for (x1, y1, *_), (x2, y2, *_) in zip(P, P[1:]):
        if x1 <= x <= x2 and x2 > x1:
            return y1 + (y2 - y1) * (x - x1) / (x2 - x1)
    return P[-1][1]

add('# The ground: one long profile, filled solid underneath')
for (x1, y1, mat, cover), (x2, y2, *_) in zip(P, P[1:]):
    if x2 <= x1:
        continue
    gap = (CREV[0] < x1 < CREV[1]) or (RAVINE[0] < x1 < RAVINE[1]) or (PIT[0] < x1 < PIT[1]) or (x1 == 2905)
    line = f'slope {mat} {x1} {y1} {x2} {y2}'
    if cover:
        line += f' cover {cover[0]} {cover[1]}'
    add(line)

# Holes cut into the ground: the crevasse, the ravine, the lake, puddle, bog, pit
add('# Crevasse under the first bridge, with a stream at the bottom')
add(f'rect air {CREV[0]} 300 {CREV[1]-CREV[0]} 330'); add(f'rect water {CREV[0]} 620 {CREV[1]-CREV[0]} 20')
add(f'rect wood {CREV[0]-6} 424 {CREV[1]-CREV[0]+12} 4    # the bridge planks')
add('# The frozen lake: water under a skin of ice (land hard and it cracks)')
add(f'rect ice {LAKE[0]} 334 {LAKE[1]-LAKE[0]} 2    # a frozen pond: glassy and slippery')
add('# The gorge below the big jump')
add('rect water 2910 690 85 10')
add('# Forest ravine and the trestle bridge over it')
add(f'rect air {RAVINE[0]} 470 {RAVINE[1]-RAVINE[0]} 200'); add(f'rect water {RAVINE[0]} 670 {RAVINE[1]-RAVINE[0]} 30')
add(f'rect wood {RAVINE[0]-8} 560 {RAVINE[1]-RAVINE[0]+16} 4   # deck')
for x in range(RAVINE[0] + 10, RAVINE[1] - 4, 30):
    add(f'rect wood {x} 564 4 106')                      # posts down to the water
    add(f'line wood {x} 600 {x+28} 566 2')              # cross braces
add('# Mine puddle, quarry mud bog, the oil pit')
for x in range(PUDDLE[0], PUDDLE[1]):
    g = int(ground_y(x))
    if g > 641:
        add(f'rect water {x} 641 1 {g-641}')
for x in range(BOG[0], BOG[1]):
    g = int(ground_y(x))
    if g > 637:
        add(f'rect mud {x} 637 1 {g-637}')
add(f'rect oil {PIT[0]} 684 {PIT[1]-PIT[0]} 16')

# ------------------------------------------------------------ scenery --
# (Everything on the riding line is solid in a side-on world, so lamps are
# small fixtures pinned in the air above head height: `anchor` stops them
# falling.)
def lamp(x, y, reach, color, extra='', strength=1.4, mat='iron'):
    add(f'rect {mat} {x-2} {y-2} 5 2'); add(f'anchor {x-2} {y-2} 5 2')
    add(f'light {x} {y+2} {reach} strength {strength} color {color} {extra}'.rstrip())

add('# Valley village: a fence to jump, street lamps')
add('rect wood 352 688 3 12'); add('rect wood 352 688 22 3'); add('rect wood 371 688 3 12')
for x in [140, 250, 460]:
    lamp(x, 650, 70, '#ffd8a0')
add('# A fallen log on the first climb, and rocks to ride over')
add('body rect wood 780 638 18 4'); add('depth 0.4')
add('circle rock 1300 532 5       # a half-buried boulder to bump over')
add('# Summit lanterns: flickering fire light hung above the trail')
for x in [2260, 2560, 2840]:
    lamp(x, int(ground_y(x)) - 46, 80, '#ff9a4a', 'flicker 0.35', 1.7, 'brass')
add('# Forest: a woodcutter hut down in the ravine, under the trestle bridge (it catches fire), fallen logs')
add('rect stone 3606 650 60 20'); add('rect wood 3612 628 44 22'); add('poly wood 3608 628 3634 614 3660 628'); add('rect cloth 3620 636 10 8')
add('body rect wood 3330 480 26 5'); add('depth 0.4')
add('body rect wood 3890 596 24 5'); add('depth 0.4')
add('# The mine: a mountain over the tunnel, lamps, a beam, gunpowder')
add('hill rock 4050 4720 200 590 to 580')
for x in range(4045, 4735):   # the tunnel itself, cut through (open air above the floor)
    g = int(ground_y(x))
    add(f'rect air {x} 584 1 {g-584}')
add('rect rock 4060 580 640 4')
for x in range(4100, 4690, 90):
    add(f'rect iron {x} 584 2 4')
    add(f'light {x+1} 590 80 strength 1.6 color #ffcf8a flicker 0.2 cone 90 70')
add('rect wood 4060 580 640 2    # ceiling boards')
add('rect air 4396 568 22 4'); add('rect gunpowder 4400 572 14 8   # charges in a niche above the tunnel')
add('# Quarry: floodlights, glass panes to smash, concrete blocks')
for x in [4770, 4810]:   # free-standing panes: knocked flying, they shatter
    add(f'body rect glass {x} 622 2 16'); add('depth 0.3')
for x, d in [(4740, 50), (5130, 110), (5330, 60)]:
    lamp(x, int(ground_y(x)) - 70, 170, '#e8f0ff', f'cone {d} 30', 2.2, 'steel')
add('# Finish arena: floodlights, a banner overhead, fireworks past the line')
for x in [5420, 5500]:
    lamp(x, 600, 150, '#ffe0b0', 'cone 90 45', 2.0, 'steel')
add('rect brass 5460 610 30 4'); add('anchor 5460 610 30 4     # the finish banner')
for x in [5530, 5580]:
    add(f'rect concrete {x} 600 8 60'); add(f'rect gunpowder {x+2} 596 4 4')

add('# Everything starts at the cold evening air temperature')
add(f'heat rect 0 0 {W} {H} 0')

# --------------------------------------------------------------- bike --
bike = (root / 'assets/stunt-bike/physical-bike.txt').read_text(encoding='utf-8').splitlines()
DY = 700 - 210  # the bike file sits on ground at y 210
DX = 0
out = []
for line in bike:
    w = line.split()
    if len(w) == 4 and w[0] == 'body':
        line = f'body {float(w[1]) + DX:.3f} {float(w[2]) + DY:.3f} {w[3]}'
    elif w[:2] == ['upright', 'frame']:
        line = 'upright frame strength 70 air 0.15 heavy 1.6   # trials handling: heavier, lean to rotate in the air'
    elif w and w[0] == 'wheel':
        line = line.replace('spring 4 damping 1', 'spring 7 damping 2.5').replace('strength 3', 'strength 6')
    out.append(line)
add('# The bike (assets/stunt-bike/physical-bike.txt), moved to the start, trials handling')
L.extend(out)
add('# Its headlight and tail light ride along with it')
add(f'light {57.5+13} {196.7+DY-3} 120 strength 1.8 color #fff0d0 cone 0 22 on frame')
add(f'light {57.5-12} {196.7+DY-1} 14 strength 1.2 color #ff3020 on frame')

# ----------------------------------------------------------- triggers --
def trig(x, actions, w=40):
    add(f'trigger {x} 0 {w} {H} by frame : {actions}')

def checkpoint(x, label, points):
    trig(x, f'checkpoint {x+20} {ground_y(x+20)-14:.0f} ; score {points} ; message {label}')

add('# Messages, checkpoints, light changes and events along the way')
trig(260, 'message JUMP THE FENCE')
checkpoint(520, 'THE CLIMB — lean forward on the steep bits', 300)
checkpoint(1500, 'STEEPER — mind the crevasse bridge', 400)
trig(1700, 'sun 68 0.7 #ff8a50 8 ; lighting 0.5 8')
checkpoint(2200, 'THE SUMMIT — snow and ice, easy on the gas', 500)
trig(2420, 'message FROZEN POND — no grip, carry your speed')
trig(2600, 'sun 74 0.35 #c86a5a 10 ; lighting 0.3 10')
trig(2895, 'message SEND IT! ; slowmo 0.35 1.4 ; zoom 1.25 1.6', 20)
checkpoint(3200, 'NIGHT FALLS — the forest is burning', 800)
trig(3200, 'sun 60 0.18 #8090c0 6 ; lighting 0.16 6')
trig(3380, 'heat rect 3612 640 44 10 700 ; message FIRE UNDER THE BRIDGE — keep your speed!', 20)
checkpoint(4000, 'THE MINE — lights out', 600)
trig(4030, 'lighting 0.04 1.5')
trig(4590, 'heat rect 4400 572 14 8 900 ; shake 5 ; message THE ROOF IS COMING DOWN — GO GO GO!', 20)
checkpoint(4700, 'THE QUARRY — smash on through', 600)
trig(4700, 'lighting 0.14 2')
trig(4990, 'heat rect 5176 684 44 4 400 ; message OIL FIRE — FULL RUN-UP!', 20)
checkpoint(5300, 'HOME STRAIGHT', 400)
trig(5470, 'finish ; heat rect 5532 596 4 4 900 ; heat rect 5582 596 4 4 900 ; zoom 1.2 2', 20)

(root / 'scenes/mountain-run.txt').write_text('\n'.join(L) + '\n', encoding='utf-8')
print(f'{len(L)} lines')
