"""A visibly timber-clad alternative to the original brick-front layered shop."""
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
source=(ROOT/'assets/workshop/layered_shop.asset').read_text(encoding='utf-8')
source=source.replace('asset layered_shop 128 112','asset timber_shop 128 112')
source=source.replace('brick_facade','timber_facade')
lines=[]
front=False
grid=False
for line in source.splitlines():
    if line.startswith('part '):
        front=line=='part timber_facade'
    if front and line=='grid':grid=True
    elif front and line=='end':grid=False
    elif front and grid:
        line=line.replace('B','F')  # Existing legend: B brick, F wood.
    elif front and line.startswith('paint '):
        _,x,y,_=line.split()
        x,y=int(x),int(y)
        # Cream horizontal weatherboards, with darker recessed seams.
        color='938c73' if y%5==0 else 'dbcea7' if y%5==1 else 'c5b58d'
        if x in (8,119):color='8b8169'
        line=f'paint {x} {y} #{color}'
    lines.append(line)
lines[0]='# Timber-clad shop exterior, real timber furnishings, masonry rear wall.'
(ROOT/'assets/workshop/timber_shop.asset').write_text('\n'.join(lines)+'\n',encoding='utf-8')
night=(ROOT/'scenes/layered-shop-night.txt').read_text(encoding='utf-8')
night=night.replace('layered_shop.asset','timber_shop.asset').replace('shop.brick_facade','shop.timber_facade')
night=night.replace('break 8000','break 1800')
(ROOT/'scenes/timber-shop-night.txt').write_text(night,encoding='utf-8')
