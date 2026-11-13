"""Build the large scrolling Red Mesa operation from native reusable props."""
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'scenes/red-mesa-map';OUT.mkdir(exist_ok=True)
W,D,H=1536,1152,88
lines=[f'size {W} {D} {H}','spawn 160 992 32']
ops=['base 160 992 5']
def box(name,mat,x,y,z,w,d,h,c):return f'box {name} {mat} {x} {y} {z} {w} {d} {h} #{c}\n'
def plot(name,tx,ty,ground=2):
 lines.append(f'region red-mesa-map/{name}.flight {tx} {ty} {ground}')
 s=f'size 64 64 {H}\nspawn 8 8 70\n'+box('ground','stone',0,0,0,64,64,ground+1,'bea476')
 s+=box('yard','concrete',9,12,ground+1,46,43,1,'8e8b75')
 return [s,tx*64,ty*64,ground+2,name]
def prop(p,file,x,y):p[0]+=f'prop ../../assets/overhead/{file}.volume {x} {y} {p[3]}\n'
def target(p,id,role,name,x,y,z,w,d,h,hp,arm):ops.append(f'target {id} {role} {name} {p[1]+x} {p[2]+y} {p[3]+z} {w} {d} {h} {hp} {arm}')
def gun(p,id,kind,x=46,y=4):
 prop(p,'mesa-'+kind,x,y);target(p,id,kind,('SAM_BATTERY' if kind=='sam' else 'AA_CANNON'),x-1,y-1,1,12,12,12,110 if kind=='sam' else 70,12 if kind=='sam' else 4)
def save(p):
 p[0]+=box('crate','wood',5,25,p[3],5,5,5,'987d4d')
 p[0]+=box('cactus','wood',58,46,p[3]-1,2,2,13,'6c844b')
 (OUT/(p[4]+'.flight')).write_text(p[0])
def barrel(p,id,x,y):prop(p,'oil-barrel',x,y);ops.append(f'barrel {id} {p[1]+x} {p[2]+y} {p[3]}')
def cache(p,id,kind,x,y):p[0]+=box('supply_'+kind,'wood',x,y,p[3],5,5,5,'b59956');ops.append(f'cache {id} {kind} {p[1]+x+2} {p[2]+y+2} {p[3]+4}')
def rescue(p,id,name,x,y,n):
 ops.append(f'rescue {id} {name} {p[1]+x} {p[2]+y} {p[3]+1} {n}')
 p[0]+=box('rescue_awning','cloth',x-5,y-8,p[3]+11,11,7,1,'aa976d')
 for dx in [-5,5]:p[0]+=box('awning_post','wood',x+dx,y-8,p[3],1,7,11,'88704a')
# Safe landing base in the south-west.
p=plot('base',2,15);p[0]+=box('helipad','concrete',17,17,3,30,30,1,'5b6f67')
for x in [23,38]:p[0]+=box('pad_mark','iron',x,23,4,2,17,1,'e0d5a5')
p[0]+=box('pad_mark','iron',25,30,4,13,2,1,'e0d5a5');barrel(p,80,50,29);cache(p,60,'ammo',10,45);save(p)
p=plot('west-radar',4,10,10);prop(p,'mesa-radar',22,18);target(p,10,'radar','WEST_RADAR',21,17,34,16,14,17,130,4);gun(p,21,'aa');rescue(p,52,'ENGINEER',15,52,1);cache(p,61,'fuel',5,6);save(p)
p=plot('east-radar',18,3,18);prop(p,'mesa-radar',24,20);target(p,11,'radar','EAST_RADAR',23,19,34,16,14,17,150,6);gun(p,23,'sam',4,5);cache(p,62,'ammo',51,48);save(p)
p=plot('power',10,9);prop(p,'mesa-power',17,22);target(p,12,'power','POWER_STATION',16,21,3,32,24,21,180,10);gun(p,22,'aa',5,4);barrel(p,81,50,40);cache(p,63,'armour',7,52);save(p)
p=plot('command',19,13,10);prop(p,'mesa-command',14,22);target(p,13,'command','COMMAND_CENTRE',13,21,3,38,28,29,230,15);gun(p,24,'sam',5,4);gun(p,25,'aa',48,4);save(p)
p=plot('ammo-dump',7,5);prop(p,'mesa-armoury',17,22);target(p,14,'armoury','AMMUNITION_DUMP',16,21,3,32,26,24,130,5);gun(p,26,'aa',46,4);cache(p,64,'ammo',4,49);save(p)
p=plot('prison',20,15);prop(p,'mesa-prison',19,20);gun(p,27,'sam',5,5);rescue(p,50,'CAPTIVES',14,53,3);cache(p,65,'fuel',54,47);save(p)
p=plot('workshop',9,14);prop(p,'mesa-workshop',17,22);gun(p,28,'aa',5,5);barrel(p,82,53,48);cache(p,66,'armour',5,52);save(p)
p=plot('crash-site',12,6);rescue(p,51,'DOWNED_CREW',30,36,2);gun(p,29,'aa',48,4);p[0]+=box('wreck','aluminium',20,18,4,17,6,5,'667c61')+box('wreck_fuel','oil',24,19,5,6,3,2,'4a3a1c');barrel(p,83,6,46);save(p)
for name,tx,ty,id,kind in [('west-pass',4,13,30,'aa'),('central-pass',8,10,31,'sam'),('north-pass',13,4,32,'sam'),('east-pass',17,10,33,'aa'),('command-road',18,14,34,'aa')]:
 p=plot(name,tx,ty);gun(p,id,kind,27,25)
 for x in [9,45]:p[0]+=box('road_barricade','brick',x,12,3,2,42,6,'a7916e')
 save(p)
# Two additional service outposts shorten supply routes across the large map.
for name,tx,ty in [('north-lz',11,3),('east-lz',20,9)]:
 p=plot(name,tx,ty);p[0]+=box('forward_pad','concrete',18,18,3,28,28,1,'5b6f67')
 for x in [24,38]:p[0]+=box('H','iron',x,25,4,2,15,1,'ded4a3')
 p[0]+=box('H','iron',26,31,4,12,2,1,'ded4a3');ops.append(f'service {tx*64+32} {ty*64+32} 5');cache(p,67+tx,'fuel',7,43);save(p)
# Roads are physical low passes, with branching routes through the stepped ridges.
for a,b in [((160,992),(288,864)),((288,864),(288,672)),((288,672),(480,352)),((288,672),(544,672)),((544,672),(672,608)),((544,672),(608,928)),((480,352),(736,224)),((736,224),(864,288)),((864,288),(1184,224)),((672,608),(800,416)),((800,416),(864,288)),((672,608),(1120,672)),((1120,672),(1312,608)),((1120,672),(1248,864)),((1248,864),(1312,992)),((1248,864),(1184,928)),((1312,608),(1184,224))]:lines.append(f'road {a[0]} {a[1]} {b[0]} {b[1]} 11')
(ROOT/'scenes/red-mesa.map').write_text('\n'.join(lines)+'\n')
(ROOT/'assets/missions/red-mesa.ops').write_text('\n'.join(ops)+'\n')
print('Built scrolling map',W,'x',D,'with',len(list(OUT.glob('*.flight'))),'persistent material regions')
