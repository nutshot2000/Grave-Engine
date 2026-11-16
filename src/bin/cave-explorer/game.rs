//! DEEPWARD expedition rules. Movement, mining, materials and lighting live
//! in the engine; this module owns the collect-and-return mission and saves.
use grave_engine::{Materials, Sim, physics::{ActorStatus, Controls}, render::Renderer};
use std::{path::Path, collections::BTreeMap};

pub const SAVE: &str = "saves/cave-explorer/run.txt";
pub struct Tuning { pub scene: String, pub home: (f32, f32), pub targets: [usize; 2], pub view: f32 }
impl Tuning {
    fn load() -> Self {
        let mut c = Self { scene: "scenes/cave-expedition.txt".into(), home: (96.,606.), targets: [24,24], view: 230. };
        let text = std::fs::read_to_string("assets/cave-explorer/game.txt").unwrap_or_default();
        for line in text.lines() {
            let w: Vec<_> = line.split('#').next().unwrap_or("").split_whitespace().collect();
            let n = |i| w.get(i).and_then(|s: &&str| s.parse::<f32>().ok()).filter(|v| v.is_finite());
            match w.first().copied() {
                Some("scene") => if let Some(s) = w.get(1) { c.scene = s.to_string(); },
                Some("home") => if let (Some(x),Some(y)) = (n(1),n(2)) { c.home=(x,y); },
                Some("target_cyan") => c.targets[0] = n(1).unwrap_or(24.).clamp(1.,500.) as usize,
                Some("target_violet") => c.targets[1] = n(1).unwrap_or(24.).clamp(1.,500.) as usize,
                Some("view_width") => c.view = n(1).unwrap_or(230.).clamp(150.,450.),
                _ => {}
            }
        }
        c
    }
}

pub struct Expedition {
    pub sim: Sim, pub mats: Materials, pub renderer: Renderer, pub tuning: Tuning,
    pub samples: [usize; 2], pub won: bool, pub frames: u32, pub toast: String,
    pub catalog:grave_engine::supplies::Catalog,
    pub aim: (f32,f32), pub drilling: bool, pub venting:bool, pub mined: usize,
    pub signal: Option<(f32,f32)>, pub explored: Vec<bool>, pub map_w: usize,
    ids: [u16;2], toast_until: u32,
}
impl Expedition {
    pub fn new(resume: bool) -> Self {
        let mut tuning = Tuning::load(); let mats = Materials::load_dir("materials");
        assert!(mats.problems.is_empty(), "{:?}",mats.problems);
        let base = std::fs::read_to_string(&tuning.scene).expect("expedition scene");
        let saved = resume.then(||std::fs::read_to_string(SAVE).ok()).flatten();
        let parse = |text: &str| grave_engine::scene::load(text,&mats,Some(Path::new("scenes"))).into_sim(&mats);
        let (mut sim, problems) = parse(saved.as_deref().unwrap_or(&base));
        let valid = problems.is_empty() && sim.physics.player_count() == 1;
        if !valid { (sim, _) = parse(&base); }
        let mut samples = [0,0];
        if valid { if let Some(text) = &saved {
            if let Some(line) = text.lines().find(|l|l.starts_with("# deepward ")) {
                let v: Vec<_> = line.split_whitespace().skip(2).filter_map(|s|s.parse::<usize>().ok()).collect();
                if v.len()==2 { samples=[v[0],v[1]]; }
            }
        } }
        sim.physics.edit_actor(0, |a| a.spawn=tuning.home);
        let ids=[mats.id("crystal_cyan").unwrap(),mats.id("crystal_violet").unwrap()];
        let mut mission=sim.mission.take().unwrap_or_else(||grave_engine::mission::Mission::new(tuning.home,24.));
        if mission.goals.is_empty(){for i in 0..2{mission.goals.push(grave_engine::mission::Goal{material:ids[i],target:tuning.targets[i],collected:samples[i]});}}
        for i in 0..2 {samples[i]=mission.goals.iter().find(|g|g.material==ids[i]).map_or(0,|g|g.collected);}
        tuning.home=mission.home;
        for i in 0..2 {if let Some(g)=mission.goals.iter().find(|g|g.material==ids[i]) {tuning.targets[i]=g.target;}}
        sim.physics.edit_actor(0,|a|a.spawn=tuning.home);
        sim.mission=Some(mission);
        let map_w=sim.world.w.div_ceil(16);let map_h=sim.world.h.div_ceil(16);
        let catalog=grave_engine::supplies::Catalog::load(&mats);
        let mut g=Self {catalog,sim,mats,renderer:Renderer::default(),tuning,samples,won:false,frames:0,toast:"Recover cyan and violet samples, then return to the lift.".into(),aim:(1.,0.),drilling:false,venting:false,mined:0,signal:None,explored:vec![false;map_w*map_h],map_w,ids,toast_until:900};
        if valid { if let Some(text)=&saved {
            if let Some(hex)=text.lines().find_map(|l|l.strip_prefix("# surveyed ")) {
                for (i,c) in hex.chars().enumerate() {if let Some(bits)=c.to_digit(16) {for bit in 0..4 {let n=i*4+bit;if n<g.explored.len(){g.explored[n]=bits&(1<<bit)!=0;}}}}
            }
        } }
        g.refresh_signal();g.reveal();g
    }
    pub fn player(&self) -> ActorStatus { self.sim.physics.actor_status().remove(0) }
    pub fn ready(&self)->bool {self.sim.mission.as_ref().is_some_and(|m|m.ready())}
    pub fn home_distance(&self)->f32 {let p=self.player();((p.x-self.tuning.home.0).powi(2)+(p.y-self.tuning.home.1).powi(2)).sqrt()}
    pub fn message(&mut self,s:impl Into<String>){self.toast=s.into();self.toast_until=self.frames+300;}
    pub fn visible_message(&self)->&str {if self.frames<self.toast_until {&self.toast}else{""}}
    pub fn refresh_signal(&mut self) {
        let p=self.player();self.signal=self.sim.mission.as_ref().and_then(|m|m.signal(&self.sim.world,(p.x,p.y)));
    }
    fn reveal(&mut self){let p=self.player();let (cx,cy)=(p.x as i32/16,p.y as i32/16);for y in cy-6..=cy+6 {for x in cx-6..=cx+6 {if x>=0&&y>=0&&(x as usize)<self.map_w&&(y as usize)<self.explored.len()/self.map_w&&(x-cx).pow(2)+(y-cy).pow(2)<40 {self.explored[y as usize*self.map_w+x as usize]=true;}}}}
    pub fn step(&mut self, controls:Controls, drill:bool, aim:(f32,f32)) {
        if self.won{return;}let p=self.player();self.frames+=1;
        if aim.0.hypot(aim.1)>0.1 {let d=aim.0.hypot(aim.1);self.aim=(aim.0/d,aim.1/d);}
        self.drilling=drill&&!p.dead;
        self.sim.aim_tool(self.aim,!p.dead,self.drilling);
        if self.venting&&!p.dead{self.sim.vent_gas(&self.mats,&self.catalog,self.aim);}
        if self.drilling {
            let removed=self.sim.use_player_tool(&self.mats,&self.catalog,self.aim);
            self.award(removed);
        }
        if self.drilling&&controls.walk.abs()<0.1&&self.aim.0.abs()>0.1{let facing=self.aim.0.signum();self.sim.physics.edit_actor(0,|a|a.facing=facing);}
        self.sim.physics.set_controls(controls);self.sim.step(&self.mats);
        self.sim.world.sounds.clear();self.reveal();
        if self.frames%45==0{self.refresh_signal();}
        if self.sim.mission.as_ref().is_some_and(|m|m.complete) {self.won=true;self.message("EXPEDITION COMPLETE — samples secured at the lift.");}
    }
    fn award(&mut self, removed:BTreeMap<u16,usize>){
        self.mined+=removed.values().sum::<usize>();
        for i in 0..2 {let found=removed.get(&self.ids[i]).copied().unwrap_or(0);if found>0 {self.samples[i]=self.sim.supplies.count(self.ids[i]) as usize;self.message(format!("{} sample +{}",if i==0 {"Cyan"}else{"Violet"},found));}}
        if self.ready(){self.message("Samples secured. Follow the signal home to the lift.");self.signal=Some(self.tuning.home);}
    }
    pub fn save_to(&mut self,path:&Path)->Result<(),String>{
        let surveyed:String=self.explored.chunks(4).map(|chunk|{let n=chunk.iter().enumerate().fold(0u32,|n,(i,b)|n|((*b as u32)<<i));char::from_digit(n,16).unwrap()}).collect();
        let text=format!("# deepward {} {}\n# surveyed {}\n{}",self.samples[0],self.samples[1],surveyed,grave_engine::scene::save(&self.sim.snapshot(),&self.mats));
        if let Some(p)=path.parent(){std::fs::create_dir_all(p).map_err(|e|e.to_string())?;}
        let temporary=path.with_extension("tmp");std::fs::write(&temporary,text).map_err(|e|e.to_string())?;
        std::fs::rename(temporary,path).map_err(|e|e.to_string())
    }
    pub fn save(&mut self){match self.save_to(Path::new(SAVE)){Ok(())=>self.message("Expedition saved."),Err(e)=>self.message(format!("Save failed: {e}"))}}
    pub fn shot(&mut self,path:&Path,wide:bool){let p=self.player();self.renderer.light(&self.sim);let win=if wide{None}else{Some(((p.x as usize).saturating_sub(115).min(self.sim.world.w-230),(p.y as usize).saturating_sub(75).min(self.sim.world.h-150),230,150))};grave_engine::headless::shot_with_actors(&self.sim.world,&self.mats,&mut self.renderer,&self.sim.physics.actor_status(),win,path,if wide{1}else{4},false).unwrap();}
}
