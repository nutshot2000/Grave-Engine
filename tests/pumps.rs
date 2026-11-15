//! Moving machinery drives finite, state-preserving liquid transfer.
use grave_engine::{scene,Materials,Sim};
use std::path::Path;
const BASE:&str="size 140 120\nfill air\nrect stone 0 110 140 10\nrect stone 10 75 30 30\nrect air 12 77 26 26\nrect oil 12 92 25 10\nrect iron 85 50 2 60\nrect iron 118 50 2 60\nrect iron 85 108 35 2\nanchor 10 75 30 30\nanchor 85 50 35 60\nbody rect steel 45 20 20 3\ndepth 0.05\nname handle\nhinge ground handle at 46 21.5 motor 0.4 strength 100\npump oil drive handle at 64 21.5 intake 12 77 26 26 outlet 90 50 4 3 rate 4\n";
fn load(text:&str)->(Materials,Sim){let m=Materials::load_dir("materials");let(s,e)=scene::load(text,&m,None).into_sim(&m);assert!(e.is_empty(),"{e:?}");(m,s)}
fn count(s:&Sim,m:&Materials)->usize{let id=m.id("oil").unwrap();s.world.mat.iter().filter(|n|**n==id).count()+s.world.flying.iter().filter(|f|f.mat==id).count()}
fn run(s:&mut Sim,m:&Materials,n:usize){for _ in 0..n{s.step(m);}}
#[test]
fn motor_strokes_move_existing_oil_and_the_well_runs_dry(){
 let(m,mut s)=load(BASE);let initial=count(&s,&m);run(&mut s,&m,900);
 assert_eq!(count(&s,&m),initial);assert_eq!(s.pumps[0].moved,initial as u64);
 let moved=s.pumps[0].moved;run(&mut s,&m,300);assert_eq!(s.pumps[0].moved,moved);
 assert!((50..108).any(|y|(87..118).any(|x|s.world.mat[s.world.idx(x,y)]==m.id("oil").unwrap())));
}
#[test]
fn stationary_dry_blocked_and_missing_drives_do_not_create_oil(){
 for text in [BASE.replace("hinge ground handle at 46 21.5 motor 0.4 strength 100","weld ground handle at 46 21.5"),BASE.replace("rect oil 12 92 25 10","rect air 12 92 25 10"),format!("{BASE}\nrect stone 90 50 4 3\nanchor 90 50 4 3\n")] {
  let(m,mut s)=load(&text);let initial=count(&s,&m);run(&mut s,&m,600);assert_eq!(s.pumps[0].moved,0);assert_eq!(count(&s,&m),initial);
 }
 let(m,mut s)=load(BASE);let id=s.pumps[0].drive;s.physics.remove(&mut s.world,id);run(&mut s,&m,100);assert_eq!(s.pumps[0].moved,0);
}
#[test]
fn transport_preserves_temperature_burning_and_surface_payload(){
 let(m,mut s)=load(BASE);let p=&mut s.pumps[0];p.intake=(12,92,1,1);p.outlet=(90,50,1,1);p.credit=1.;
 let source=s.world.idx(12,92);let out=s.world.idx(90,50);s.world.temp[source]=170.;s.world.burn[source]=17;s.world.life[source]=33;s.world.flags[source]|=grave_engine::world::BURNING;s.world.shade[source]=123;
 p.step(&mut s.world,&m,&s.physics);
 assert_eq!(s.world.temp[out],170.);assert_eq!(s.world.burn[out],17);assert_eq!(s.world.life[out],33);assert!(s.world.is_burning(out));assert_eq!(s.world.shade[out],123);assert_eq!(s.world.mat[source],grave_engine::material::AIR);
}
#[test]
fn snapshots_and_scene_saves_keep_ports_drive_and_pumping_state(){
 let(m,mut s)=load(BASE);run(&mut s,&m,180);assert!(s.pumps[0].moved>0);
 let snap=s.snapshot();let mut restored=Sim::restore(&snap,&m);assert_eq!(restored.pumps,s.pumps);
 let text=scene::save(&snap,&m);let(mut loaded,e)=scene::load(&text,&m,None).into_sim(&m);assert!(e.is_empty(),"{e:?}");
 assert_eq!(loaded.pumps[0].moved,s.pumps[0].moved);assert_eq!(loaded.pumps[0].credit,s.pumps[0].credit);assert_eq!(loaded.pumps[0].intake,s.pumps[0].intake);
 for sim in [&mut restored,&mut loaded] {let initial=count(sim,&m);run(sim,&m,300);assert_eq!(count(sim,&m),initial);assert!(sim.pumps[0].moved>snap.pumps[0].moved);}
}
#[test]
fn invalid_or_overlapping_ports_are_reported(){
 for (from,to) in [("intake 12 77 26 26","intake 12 77 500 26"),("outlet 90 50 4 3","outlet 12 77 4 3"),("rate 4","rate NaN"),("drive handle","drive missing")] {assert!(!scene::load(&BASE.replace(from,to),&Materials::load_dir("materials"),None).problems.is_empty());}
}
#[test]
fn iron_heron_runs_multiple_cycles_and_really_fills_the_tank(){
 let m=Materials::load_dir("materials");let a=grave_engine::asset::Asset::load_file(Path::new("assets/workshop/iron_heron_pumpjack.asset"),&m).unwrap();assert!(a.validate(&m).is_empty());
 let text=std::fs::read_to_string("scenes/iron-heron-oilfield.txt").unwrap();let(mut s,e)=scene::load(&text,&m,Some(Path::new("scenes"))).into_sim(&m);assert!(e.is_empty(),"{e:?}");
 let initial=count(&s,&m);let bodies=s.physics.body_count();let mut last=s.physics.pose_of(3).unwrap().2;let mut rotation=0.;
 for _ in 0..1800{s.step(&m);let a=s.physics.pose_of(3).unwrap().2;rotation+=(a-last).sin().atan2((a-last).cos()).abs();last=a;}
 assert!(rotation>30.,"the crank must keep rotating, not jam at a linkage intersection");assert_eq!(s.physics.body_count(),bodies);assert_eq!(count(&s,&m),initial);assert!(s.pumps[0].moved>1500);
 let oil=m.id("oil").unwrap();let tank=(148..187).flat_map(|y|(283..351).map(move|x|y*420+x)).filter(|i|s.world.mat[*i]==oil).count();assert!(tank>1500);
}
