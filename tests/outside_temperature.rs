//! Scene climate must cool the ground as well as incoming snow, and persist.
use grave_engine::{Materials,Sim,scene,edit::History};
fn m()->Materials {Materials::load_dir("materials")}
#[test]
fn cold_snow_accumulates_while_a_warm_scene_melts_it() {
    let m=m();let base="size 160 110\nfill air\nrect stone 0 100 160 10\nrect stone 45 58 70 4\nanchor 45 58 70 4\nweather snow 0.9\n";
    let (mut cold,p)=scene::load(&format!("{base}outside_temperature -8\n"),&m,None).into_sim(&m);assert!(p.is_empty(),"{p:?}");
    let (mut warm,p)=scene::load(&format!("{base}outside_temperature 20\n"),&m,None).into_sim(&m);assert!(p.is_empty());
    assert_eq!(cold.world.temp[cold.world.idx(5,102)],-8.);
    for _ in 0..900 {cold.step(&m);warm.step(&m);}
    let snow=m.id("snow").unwrap();
    let c=cold.world.mat.iter().filter(|v|**v==snow).count();let w=warm.world.mat.iter().filter(|v|**v==snow).count();
    assert!(c>150,"cold scene should accumulate snow: {c}");assert!(c>w*2,"warm scene should melt more: cold={c},warm={w}");
    let mut r=grave_engine::render::Renderer::default();r.light(&cold);grave_engine::headless::shot(&cold.world,&m,&mut r,std::path::Path::new("shots/outside-cold-snow.png"),4,false).unwrap();
}
#[test]
fn climate_applies_to_all_layers_and_preserves_hot_sources_and_explicit_heat() {
    let m=m();let (mut s,p)=scene::load("size 60 50\nfill air\nrect stone 0 45 60 5\nbody rect wood 20 20 5 5\nheat rect 20 20 5 5 700\noutside_temperature -8\n",&m,None).into_sim(&m);assert!(p.is_empty(),"{p:?}");
    assert_eq!(s.world.temp[s.world.idx(20,20)],700.);
    s.ensure_layer(1,"inside",&m);s.layers[0].world.set(&m,10,10,m.id("lava").unwrap());
    let lava=s.layers[0].world.temp[s.world.idx(10,10)];
    let mut h=History::new();h.touch(&mut s,"outside temperature");s.set_outside_temperature(-12.,&m,true).unwrap();h.commit(&mut s);
    assert_eq!(s.world.ambient,-12.);assert_eq!(s.layers[0].world.ambient,-12.);assert_eq!(s.layers[0].world.temp[s.world.idx(10,10)],lava);
    h.undo(&mut s,&m);assert_eq!(s.world.ambient,-8.);h.redo(&mut s,&m);assert_eq!(s.world.ambient,-12.);
    let (copy,p)=scene::load(&scene::save(&s.snapshot(),&m),&m,None).into_sim(&m);assert!(p.is_empty(),"{p:?}");assert_eq!(copy.world.ambient,-12.);assert_eq!(copy.world.temp[copy.world.idx(5,47)],-12.);
    for bad in ["NaN","inf","-101","101"] {let (_,p)=scene::load(&format!("outside_temperature {bad}"),&m,None).into_sim(&m);assert!(!p.is_empty());}
}
#[test]
fn changing_the_target_does_not_instantly_freeze_an_existing_warm_floor() {
    let m=m();let mut s=Sim::empty(20,20,&m);s.world.set(&m,5,5,m.id("stone").unwrap());
    s.set_outside_temperature(-8.,&m,false).unwrap();assert_eq!(s.world.temp[s.world.idx(5,5)],20.);
    s.set_outside_temperature(-8.,&m,true).unwrap();assert_eq!(s.world.temp[s.world.idx(5,5)],-8.);
}

#[test]
fn climate_rewinds_with_the_recorded_weather_simulation() {
    let m=m();let mut s=Sim::empty(20,20,&m);let mut t=grave_engine::edit::Timeline::new();t.every=1;t.record(&mut s);
    s.set_outside_temperature(-8.,&m,true).unwrap();t.record(&mut s);assert!(t.restore(0,&mut s,&m));assert_eq!(s.world.ambient,20.);
}
