"""Build a warmly lit night street around the layered shop asset."""
from pathlib import Path
root=Path(__file__).resolve().parents[1]
out=['# Night street: layered shop, street lamps, warm sign lights and interior lighting.',
'size 320 210','seed 23','fill air','sky #080f25 #283b51 stars',
'layer city #18273c depth 0.35 y 179 height 45 windows',
'lighting 0.36 #9aaad4',
'rect stone 0 179 320 31']
for y in range(179,210):
 for x in range(320):
  c='596574' if y<184 else 'a3a99d' if y==184 else '26313f'
  if y in (180,181) and x%24==0:c='343e4b'
  if 201<=y<=202 and x%40<20:c='a9a58c'
  out.append(f'paint {x} {y} #{c}')
out+=['anchor 0 179 320 31']
for x in (62,256):
 out += [f'rect iron {x} 96 3 83',f'rect iron {x-2} 173 7 6',
         f'rect iron {x} 96 13 2',f'rect brass {x+7} 94 9 2',f'rect lamp_glass {x+9} 96 5 3',
         f'anchor {x-2} 94 20 85',
         f'light {x+11} 100 125 strength 2.4 color #ffcc8b cone 90 85']
 for yy in range(96,179):
  for xx in range(x,x+3):out.append(f'paint {xx} {yy} #3e4953')
 for yy in range(96,99):
  for xx in range(x+9,x+14):out.append(f'paint {xx} {yy} #fff0b3')
# Street furniture, pinned so it does not drop during initial setup.
out+=['rect iron 25 173 2 6','rect iron 47 173 2 6','rect wood 23 169 28 4',
      'rect wood 23 165 28 2','rect wood 23 166 2 5','rect wood 49 166 2 5','anchor 23 165 28 14',
      'rect iron 281 164 11 15','rect iron 280 162 13 2','anchor 280 162 13 17',
      'rect brick 104 177 4 2','rect brick 212 177 4 2','anchor 104 177 4 2','anchor 212 177 4 2',
      'asset ../assets/workshop/layered_shop.asset 160 179 as shop',
      'weld ground shop.brick_facade at 105 176 break 8000',
      'weld ground shop.brick_facade at 213 176 break 8000',
      'light 132 110 60 strength 1.0 color #ffdc9e unshadowed on shop.shop_sign',
      'light 188 110 60 strength 1.0 color #ffdc9e unshadowed on shop.shop_sign',
      'plane 1 interior','size 320 210','fill air','rect stone 0 179 320 31',
      'lighting 0.38 #ecd7b2',
      'light 130 143 39 strength 0.85 color #ffd293 unshadowed',
      'light 192 143 39 strength 0.85 color #ffd293 unshadowed',
      'light 160 103 49 strength 0.65 color #ffe3b7 unshadowed','endplane',
      'plane 2 back_wall','size 320 210','fill air','rect stone 0 179 320 31',
      'lighting 0.48 #d9cbb1','endplane']
(root/'scenes/layered-shop-night.txt').write_text('\n'.join(out)+'\n',encoding='utf-8')
