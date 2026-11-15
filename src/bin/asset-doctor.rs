//! Read-only asset review: native material/geometry checks, a simulated copy,
//! a screenshot, and optional full-turn verification of a named moving part.
use grave_engine::{Materials,Sim,asset::Asset,render::Renderer};
use std::{path::{Path,PathBuf},collections::BTreeMap};

fn run()->Result<(),String>{
    let mut args=std::env::args().skip(1);
    let Some(file)=args.next()else{return Err("Use asset-doctor FILE.asset [--frames 600] [--shot shots/review.png] [--motion crank --min-turns 1]".into())};
    if file=="--help" || file=="-h" {println!("Use asset-doctor FILE.asset [--frames 600] [--shot shots/review.png] [--motion crank --min-turns 1]\nRuns a fresh asset copy and never saves over the source. Inspect the screenshot after the physics checks.");return Ok(());}
    let mut frames=0usize;let mut shot=None;let mut motion=None;let mut min_turns=0.5f32;
    while let Some(arg)=args.next(){
        let value=args.next().ok_or_else(||format!("{arg} needs a value"))?;
        match arg.as_str(){
            "--frames"=>frames=value.parse().map_err(|_|"frames needs a number")?,
            "--shot"=>shot=Some(PathBuf::from(value)),
            "--motion"=>motion=Some(value),
            "--min-turns"=>min_turns=value.parse().map_err(|_|"min-turns needs a number")?,
            _=>return Err(format!("unknown option {arg}")),
        }
    }
    if frames>36000 || !min_turns.is_finite() || min_turns<0. {return Err("Use at most 36000 frames and a finite nonnegative turn count".into());}
    let mats=Materials::load_dir("materials");
    let asset=Asset::load_file(Path::new(&file),&mats)?;
    let mut issues=asset.validate(&mats);
    println!("ASSET {}: {} named parts, {} welds, {} moving joints",asset.name,asset.parts.len(),asset.welds.len(),asset.joints.len());
    for issue in &issues {println!("CHECK: {issue}");}
    let floor=asset.h+50;let width=(asset.w+100).max(240);
    let mut sim=Sim::empty(width,floor+30,&mats);
    let stone=mats.id("stone").ok_or("stone material missing")?;
    for y in floor..sim.world.h{for x in 0..width{sim.world.set(&mats,x,y,stone);}}
    let ids=asset.instantiate(&mut sim,&mats,width as f32/2.,floor as f32-1.)?;
    let before=sim.physics.body_count();
    let mut angles:BTreeMap<String,(u64,f32,f32)>=ids.iter().filter_map(|(name,id)|Some((name.clone(),(*id,sim.physics.pose_of(*id)?.2,0.)))).collect();
    if let Some(name)=&motion {if !angles.contains_key(name){return Err(format!("no part named {name}"));}if frames<180{return Err("Motion review needs at least 180 frames".into());}}
    for _ in 0..frames{
        sim.step(&mats);
        for (_, (id,last,total))in &mut angles{
            if let Some((_,_,angle))=sim.physics.pose_of(*id){let delta=angle-*last;*total+=delta.sin().atan2(delta.cos());*last=angle;}
        }
    }
    for(name,(id,_,angle))in &angles{
        match sim.physics.pose_of(*id){
            Some((x,y,_))=>println!("PART {name}: at {x:.1},{y:.1}; net turns {:.3}",angle.abs()/std::f32::consts::TAU),
            None=>issues.push(format!("{name} did not remain intact during the motion review")),
        }
    }
    if let Some(name)=motion{
        let turns=angles[&name].2.abs()/std::f32::consts::TAU;
        if turns<min_turns{issues.push(format!("{name} turned {turns:.3} times; expected at least {min_turns}. Check full-sweep clearance and motor strength."));}
    }
    println!("SIMULATION: {frames} frames; {before} bodies initially, {} now",sim.physics.body_count());
    if let Some(path)=shot{
        if let Some(dir)=path.parent(){std::fs::create_dir_all(dir).map_err(|e|e.to_string())?;}
        let mut renderer=Renderer::default();renderer.light(&sim);
        grave_engine::headless::shot_with_actors(&sim.world,&mats,&mut renderer,&[],None,&path,4,false)?;
        println!("SCREENSHOT: {} (inspect this; passing physics does not prove visual quality)",path.display());
    }
    if issues.is_empty(){println!("PASS: source asset unchanged");Ok(())}else{Err(issues.join("\n"))}
}
fn main(){if let Err(e)=run(){eprintln!("ASSET REVIEW: {e}");std::process::exit(3);}}
