"""Legacy procedural cave texture experiment (not the active artwork): broad strata, fractured faces and quiet grain.
Run from any folder; no source photographs or third-party textures are used.
"""
from pathlib import Path
import math
import random
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / 'materials' / 'textures'

def draw(name, tint, seed):
    rng = random.Random(seed)
    size = 128
    sites = [(rng.randrange(size), rng.randrange(size), rng.uniform(-6, 6)) for _ in range(65)]
    image = Image.new('RGB', (size, size))
    for y in range(size):
        for x in range(size):
            distances = sorted((min(abs(x-sx), size-abs(x-sx))**2 + 1.7*min(abs(y-sy), size-abs(y-sy))**2, shade) for sx,sy,shade in sites)
            d0, shade = distances[0]
            d1 = distances[1][0]
            fracture = max(0, 1 - (math.sqrt(d1)-math.sqrt(d0))/0.9)
            strata = math.sin(y*math.tau/32 + 0.5*math.sin(x*math.tau/128))
            broad = math.sin(x*math.tau/128 + math.sin(y*math.tau/128))*3
            v = shade + strata*2.5 + broad - fracture*10 + rng.uniform(-1.8,1.8)
            image.putpixel((x,y), tuple(max(0,min(255,round(c+v))) for c in tint))
    image.save(OUT / name)

if __name__ == '__main__':
    draw('cave_rock.png', (112,110,103), 411)
    draw('cave_shale.png', (99,103,106), 912)
