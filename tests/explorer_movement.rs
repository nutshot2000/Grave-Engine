//! Reusable cave traversal: flight, passive descent, light and real excavation.
use grave_engine::{Materials, Sim, scene, physics::Controls};
fn setup(text:&str)->(Materials,Sim){let m=Materials::load_dir("materials");let(s,p)=scene::load(text,&m,None).into_sim(&m);assert!(p.is_empty(),"{p:?}");(m,s)}

#[test]
fn jetpack_reaches_above_jump_height_then_recharges_and_saves(){
    let(m,mut s)=setup("size 240 300\nfill air\nrect stone 0 280 240 20\nplayer 80 280 jetpack headlamp 110\n");
    for _ in 0..25{s.step(&m);}let start=s.physics.actor_status()[0].y;
    s.physics.set_controls(Controls{thrust:true,walk:0.4,..Default::default()});for _ in 0..120{s.step(&m);}
    let p=s.physics.actor_status()[0].clone();assert!(p.y<start-80.,"flight should cross a cavern, not just jump");assert!(p.health>99.);let fuel=p.jet_fuel.unwrap().0;
    s.physics.set_controls(Controls::default());for _ in 0..120{s.step(&m);}
    let p=s.physics.actor_status()[0].clone();assert!(p.jet_fuel.unwrap().0>fuel);assert!(p.health>99.);assert!(!s.lamps().is_empty());
    let snap=s.snapshot();let text=scene::save(&snap,&m);assert!(text.contains("jetpack")&&text.contains("headlamp 110"));let(again,problems)=scene::load(&text,&m,None).into_sim(&m);assert!(problems.is_empty());assert!(again.physics.actor_status()[0].jet_fuel.is_some());
    s=Sim::restore(&snap,&m);assert_eq!(s.physics.actor_status()[0].jet_fuel,p.jet_fuel);
}

#[test]
fn excavation_reports_real_material_and_preserves_owned_cells_and_liquids(){
    let(m,mut s)=setup("size 120 80\nfill air\nrect rock 40 20 30 40\nanchor 40 20 30 40\nrect water 80 20 10 10\nplayer 20 70\n");
    let rock=m.id("rock").unwrap();let removed=s.dig_circle(&m,50.,40.,9.,8000.);assert!(*removed.get(&rock).unwrap()>100);assert_eq!(s.world.mat[s.world.idx(50,40)],0);
    let water=m.id("water").unwrap();s.dig_circle(&m,85.,25.,9.,8000.);assert_eq!(s.world.mat[s.world.idx(85,25)],water);
    let i=s.world.idx(65,40);s.world.flags[i]|=grave_engine::world::OWNED;s.dig_circle(&m,65.,40.,9.,8000.);assert_eq!(s.world.mat[i],rock);
    let snap=s.snapshot();for _ in 0..20{s.step(&m);}s=Sim::restore(&snap,&m);assert_eq!(s.world.mat[s.world.idx(50,40)],0);
}
