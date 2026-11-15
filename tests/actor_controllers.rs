//! Independent brains must survive snapshots and never inherit player input.
use grave_engine::{Materials,Sim,physics::{ActorSpec,Controls}};
#[test]
fn independent_input_damage_and_checkpoint_survive_rewind(){
 let m=Materials::load_dir(concat!(env!("CARGO_MANIFEST_DIR"),"/materials"));
 let mut s=Sim::empty(300,120,&m);let stone=m.id("stone").unwrap();
 for y in 110..120{for x in 0..300{s.world.set(&m,x,y,stone);}}
 let mut a=ActorSpec::standing_at(m.id("flesh").unwrap(),60.,110.);a.respawn=false;
 s.physics.add_actor(&mut s.world,&m,a.clone());a.x=180.;a.spawn=(180.,102.);
 s.physics.add_actor(&mut s.world,&m,a);
 s.physics.set_controls(Controls{walk:1.,..Default::default()});
 assert!(s.physics.set_actor_controls(1,Some(Controls{walk:-1.,..Default::default()})));
 for _ in 0..40{s.step(&m);}
 let status=s.physics.actor_status();assert!(status[0].x>65.);assert!(status[1].x<175.);
 let snap=s.snapshot();assert_eq!(snap.actors[1].control_override.unwrap().walk,-1.);
 assert!(!s.physics.damage_actor(0,f32::NAN,"test"));
 assert!(s.physics.damage_actor(1,200.,"test"));s.step(&m);
 for _ in 0..150{s.step(&m);}assert!(s.physics.actor_status()[1].dead);
 assert!(!s.physics.heal_actor(1,20.));assert!(!s.physics.set_actor_controls(99,None));
 s=Sim::restore(&snap,&m);assert!(!s.physics.actor_status()[1].dead);
 let before=s.physics.actor_status()[1].x;for _ in 0..10{s.step(&m);}assert!(s.physics.actor_status()[1].x<before);
}
