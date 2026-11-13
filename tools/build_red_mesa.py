"""Original reusable material prefabs and the Red Mesa mission; no external art."""
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
ASSETS=ROOT/'assets/overhead'
def box(name,mat,x,y,z,w,d,h,c): return f'box {name} {mat} {x} {y} {z} {w} {d} {h} #{c}\n'
def building(name,w,d,h,color):
 s=box('foundation','concrete',0,0,0,w,d,3,'99927a')
 for x in [0,w-2]:s+=box('side_wall','weathered_wood',x,0,3,2,d,h-3,color)
 s+=box('rear_wall','weathered_wood',2,0,3,w-4,2,h-3,color)
 s+=box('front_wall','weathered_wood',2,d-2,3,w//2-6,2,h-3,color)+box('front_wall','weathered_wood',w//2+5,d-2,3,w//2-7,2,h-3,color)
 s+=box('door_header','weathered_wood',w//2-4,d-2,h-7,9,2,7,color)
 for y in range(0,d,9):s+=box('roof_tie','iron',0,y,h-1,w,1,1,'72786b')
 for x in range(0,w,4):s+=box('roof_board','weathered_wood',x,0,h,min(4,w-x),d,2,'85583f' if x%8==0 else 'a3744d')
 for x in [3,w-9]:s+=box('window','glass',x,d-2,9,6,2,7,'729c9a')
 s+=box('side_window','glass',w-2,5,9,2,min(10,d-9),7,'729c9a')
 s+=box('table','wood',5,5,3,9,4,6,'af8c56')+box('tabletop','iron',4,4,9,11,6,1,'7e8e8a')
 s+=box('stock_crate','wood',w-12,5,3,7,7,6,'a18349')
 (ASSETS/name).write_text(s)
 return s
building('mesa-power.volume',30,22,20,'b2a479')
building('mesa-command.volume',36,26,28,'aaa486')
building('mesa-prison.volume',32,20,22,'a99070')
building('mesa-armoury.volume',30,24,22,'9a8862')
building('mesa-workshop.volume',34,24,24,'a39272')
s=box('generator','iron',8,6,3,12,10,10,'687b61')+box('fuel_cell','oil',10,8,4,8,6,7,'4a3a1c')+box('fuel_vent','oil',13,10,13,2,2,1,'5c4d27')
p=ASSETS/'mesa-power.volume';p.write_text(p.read_text()+s)
p=ASSETS/'mesa-armoury.volume';p.write_text(p.read_text()+box('ammo_cases','wood',8,9,3,12,9,7,'988451')+box('ammunition','gunpowder',12,12,4,3,2,1,'3a3936'))
p=ASSETS/'mesa-command.volume';p.write_text(p.read_text()+box('radio_equipment','copper',15,10,3,5,5,9,'b1744e')+box('command_antenna','iron',29,19,30,1,1,11,'a2a89c'))
s=box('base','concrete',0,0,0,14,12,3,'a3a18b')
for x in [3,10]:s+=box('mast_support','iron',x,0,3,1,12,32,'7e8c85')
for z in range(10,34,9):s+=box('brace','iron',3,0,z,8,12,1,'8f9484')
s+=box('service_platform','wood',0,0,34,14,12,2,'8e8056')
for x in range(1,14,3):s+=box('radar_dish','aluminium',x,4,37,1,4,8,'a0afa9')
for z in [37,41,44]:s+=box('radar_dish','aluminium',0,4,z,14,4,1,'b8c0ad')
s+=box('radar_core','copper',5,4,39,4,4,4,'dd9561')+box('antenna','iron',7,5,45,1,1,5,'c0c5b2')
(ASSETS/'mesa-radar.volume').write_text(s)
for sam in [False,True]:
 s=box('gun_base','concrete',0,0,0,10,10,3,'868976')+box('turret','aluminium',2,2,3,6,6,5,'667d54')+box('gun_core','copper',4,4,6,2,2,3,'8d7450')
 if sam:
  for y in [2,6]:s+=box('missile_rail','iron',1,y,8,11,1,2,'a5ab91')+box('missile','aluminium',1,y,10,9,1,1,'c4bc92')
 else:s+=box('cannon','steel',4,4,8,10,2,1,'535e53')
 (ASSETS/('mesa-sam.volume' if sam else 'mesa-aa.volume')).write_text(s)
s=box('barrel_case','iron',0,0,0,6,6,9,'847849')+box('oil','oil',1,1,1,4,4,7,'4a3a1c')+box('barrel_band','iron',0,0,3,6,6,1,'464c44')+box('barrel_band','iron',0,0,7,6,6,1,'464c44')
(ASSETS/'oil-barrel.volume').write_text(s)
level='''# Red Mesa: complete overhead mission. Original material assets.
size 240 144 88
spawn 29 119 32
'''
level+=box('desert','stone',0,0,0,240,144,2,'bea476')
# Layered escarpments, but a clear approach route between mission areas.
for x,y,w,d,h in [(8,8,74,40,6),(14,12,62,32,9),(176,4,60,43,10),(183,8,50,34,16),(108,4,42,44,12),(114,8,30,35,22),(124,13,13,24,29)]:level+=box('mesa_strata','sandstone',x,y,2,w,d,h,'a2815c' if h%2==0 else 'b49365')
for x,y,w,d in [(15,101,45,37),(47,47,34,32),(82,87,64,27),(168,62,60,40),(171,105,58,32)]:level+=box('concrete_yard','concrete',x,y,2,w,d,1,'898b78')
for x,y,w,d in [(37,114,160,6),(53,47,6,68),(139,52,6,66),(168,43,6,74),(69,66,115,5)]:level+=box('dirt_road','stone',x,y,2,w,d,1,'ad9367')
# Helipad and contrasting perimeter stripes.
level+=box('landing_pad','concrete',16,108,2,27,25,1,'606f65')
for x in [23,34]:level+=box('H_mark','iron',x,114,3,2,13,1,'e0d5a5')
level+=box('H_mark','iron',25,119,3,9,2,1,'e0d5a5')
for x in range(16,43,4):level+=box('pad_edge','iron',x,108,3,2,1,1,'ccb75c')
props=[('mesa-radar.volume',43,19,11),('mesa-radar.volume',194,15,18),('mesa-power.volume',106,84,3),('mesa-command.volume',178,65,3),('mesa-prison.volume',192,112,3),('mesa-armoury.volume',59,49,3),('mesa-workshop.volume',85,114,3)]
for name,x,y,z in props:level+=f'prop ../assets/overhead/{name} {x} {y} {z}\n'
enemies=[(21,'aa',37,53,3),(22,'aa',78,71,3),(23,'sam',103,49,3),(24,'aa',160,97,3),(25,'sam',166,43,3),(26,'aa',218,98,3),(27,'sam',213,54,3),(28,'aa',151,115,3)]
for _,kind,x,y,z in enemies:level+=f'prop ../assets/overhead/mesa-{kind}.volume {x} {y} {z}\n'
barrels=[(80,47,110,3),(81,92,92,3),(82,174,120,3),(83,65,79,3)]
for _,x,y,z in barrels:level+=f'prop ../assets/overhead/oil-barrel.volume {x} {y} {z}\n'
# Trees, crates, salvage and fencing reward looking around without blocking paths.
for x,y in [(16,72),(96,34),(155,23),(222,120),(174,132),(82,92),(69,116),(164,73)]:
 level+=box('cactus','wood',x,y,2,2,2,13,'687f4a')+box('cactus_arm','wood',x-3,y,8,3,2,2,'758952')+box('cactus_arm','wood',x-3,y,9,1,2,4,'758952')
for x,y,z in [(51,127,3),(119,118,3),(185,105,3),(76,39,11),(226,74,3)]:level+=box('supply_crate','wood',x,y,z,5,5,5,'a88b50')
for x,y,w,d in [(170,105,60,1),(229,105,1,32),(169,110,1,15),(40,48,1,32),(40,48,36,1)]:level+=box('compound_wall','brick',x,y,3,w,d,6,'a99370')
# Rescue shelters: awnings and posts leave people visible from above.
for x,y,z in [(181,124,3),(127,55,3),(65,33,11)]:
 for px in [x,x+8]:level+=box('shelter_posts','wood',px,y,z,1,8,10,'877650')
 level+=box('awning','cloth',x,y,z+10,9,8,1,'a28b5d')
(ROOT/'scenes/red-mesa.flight').write_text(level)
ops='''# id role name minX minY minZ width depth height health armour
base 29 120 5
target 10 radar WEST_RADAR 42 18 45 16 14 17 130 4
target 11 radar EAST_RADAR 193 14 52 16 14 17 150 6
target 12 power POWER_STATION 105 83 6 32 24 21 180 10
target 13 command COMMAND_CENTRE 177 64 6 38 28 29 230 15
target 14 armoury AMMUNITION_DUMP 58 48 6 32 26 24 130 5
'''
for id,kind,x,y,z in enemies:ops+=f'target {id} {kind} '+('SAM_BATTERY' if kind=='sam' else 'AA_CANNON')+f' {x-1} {y-1} {z+1} 12 12 12 '+('110 12' if kind=='sam' else '70 4')+'\n'
for id,x,y,z in barrels:ops+=f'barrel {id} {x} {y} {z}\n'
for id,x,y,z,n,name in [(50,185,134,4,3,'CAPTIVES'),(51,131,59,4,2,'DOWNED_CREW'),(52,69,37,12,1,'ENGINEER')]:ops+=f'rescue {id} {name} {x} {y} {z} {n}\n'
for id,kind,x,y,z in [(60,'ammo',53,128,7),(61,'fuel',121,120,7),(62,'armour',183,107,7),(63,'ammo',229,74,7)]:ops+=f'cache {id} {kind} {x} {y} {z}\n'
(ROOT/'assets/missions/red-mesa-lab.ops').write_text(ops)
print('Built Red Mesa:',len(props),'buildings,',len(enemies),'defences,',len(barrels),'portable barrels')

# Keep the playable scrolling mission current when rebuilding the shared props.
if __name__ == "__main__":
 import runpy
 runpy.run_path(str(ROOT/"tools/build_red_mesa_map.py"),run_name="__main__")
