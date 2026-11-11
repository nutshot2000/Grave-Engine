//! Stunt Yard: a small game built on Grave Engine's existing simulation.
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use grave_engine::{material::Materials, render::{Renderer, View}, sim::Sim};
use std::time::Instant;

/// A course: its scene file, what it's called, and (for Wreck Run, made
/// before levels could say it themselves) its stages, signs and finish line.
/// Newer levels put all that in the scene: triggers with `message`, `score`,
/// `checkpoint` and `finish`.
#[derive(Clone,Copy)]
struct Level { file:&'static str, title:&'static str, blurb:&'static str, stages:&'static [(f32,&'static str,u32)], signs:&'static [(f32,&'static str)], finish:Option<f32>, timeout:u32 }
const LEVELS:[Level;2]=[
 Level{file:"scenes/stunt-yard.txt",title:"WRECK RUN",blurb:"360m MATERIAL PLAYGROUND  •  DUNES / MUD / DEMOLITION / ICE / LAVA",stages:&STAGES,signs:&SIGNS,finish:Some(FINISH),timeout:14400},
 Level{file:"scenes/mountain-run.txt",title:"MOUNTAIN RUN",blurb:"A LONG RIDE INTO THE NIGHT  •  CLIMBS / SNOW / FOREST FIRE / MINE / QUARRY",stages:&[],signs:&[],finish:None,timeout:60*60*12},
];
const STAGES: [(f32,&str,u32); 9] = [(410.,"BURNING SCRAPYARD",500),(1000.,"DUNE CLIMB",750),(1340.,"THE BOG",500),(1660.,"DEMOLITION LEAP",800),(1960.,"TIMBER CROSSING",500),(2400.,"ICE DESCENT",500),(2850.,"LAVA ESCAPE",1000),(3200.,"LAST DUNE",750),(3480.,"EXPLOSIVE FINALE",1000)];
const SIGNS: [(f32,&str);9] = [(170.,"JUMP THE BURNING WRECK"),(440.,"SAND HILLS / KEEP MOMENTUM"),(850.,"BRAKE DOWNHILL / LOW GANTRY"),(1150.,"DEEP MUD / POWER THROUGH"),(1400.,"JUMP / CHARGES BEHIND YOU"),(1750.,"TIMBER BRIDGE"),(2190.,"ICE / EASY ON THE BRAKES"),(2550.,"LAVA GAP / FULL RUN-UP"),(3210.,"FINAL DEMOLITION")];
const FINISH:f32=3520.;

struct Part { id:u64, name:String }

// Fuses, checkpoints and their messages are engine triggers in the scene
// (`trigger ... by frame : ...`); the game reacts to `sim.events`.
struct Game { ai_wait:u32, level:usize, checkpoint:f32, surface:String, parts:Vec<Part>, crashed:bool, upside_down:u32, sim: Sim, mats: Materials, renderer: Renderer, texture: Option<egui::TextureHandle>, rider: Option<egui::TextureHandle>, clock: f64, last: Instant, frame: u32, state: String, started: bool, best: u32, score: u32, gates: Vec<bool>, cam_x:f32, cam_y:f32, cam_width:f32 }
impl Game {
 /// Wreck Run.
 fn new() -> Self {Self::with_level(0)}
 fn with_level(level:usize) -> Self {
  let lv=LEVELS[level];
  let mats=Materials::load_dir("materials");
  assert!(mats.problems.is_empty(), "{:?}",mats.problems);
  let text=std::fs::read_to_string(lv.file).expect("level scene");
  let (sim,problems)=grave_engine::scene::load(&text,&mats,Some(std::path::Path::new("scenes"))).into_sim(&mats);
  assert!(problems.is_empty(),"{problems:?}");
  // The bike's parts, in the order of the parts list, start with its frame
  // (the object the camera follows): wherever the level put the bike.
  let ids=sim.physics.ids();
  let first=ids.iter().position(|&id|Some(id)==sim.follow).expect("the level follows the bike's frame");
  let parts:Vec<Part>=std::fs::read_to_string("assets/stunt-bike/parts.tsv").expect("parts manifest").lines().enumerate().map(|(i,line)|{
   let v:Vec<_>=line.split('\t').collect();
   Part{id:ids[first+i],name:v[0].into()}
  }).collect();
  assert!(parts.iter().any(|p|p.name=="frame"),"missing bike frame");
  let start=sim.physics.spec_of(&sim.world,sim.follow.unwrap_or(1)).map_or((110.,100.),|b|(b.x,b.y-80.));
  Self {ai_wait:0,level,checkpoint:0.,surface:String::new(),parts,crashed:false,upside_down:0,sim,mats,renderer:Renderer::default(),texture:None,rider:None,clock:0.,last:Instant::now(),frame:0,state:"Ready on set — press SPACE  (1 / 2: pick a course)".into(),started:false,best:0,score:0,gates:vec![false;lv.stages.len()],cam_x:start.0,cam_y:start.1,cam_width:220.}
 }
 fn lv(&self)->Level {LEVELS[self.level]}
 /// Where the finish line is (for the progress bar): the level's, or its
 /// `finish` trigger's, or the far end of the world.
 fn finish_x(&self)->f32 {
  self.lv().finish.or_else(||self.sim.triggers.iter().find(|t|t.actions.contains(&grave_engine::trigger::Action::Finish)).map(|t|t.x)).unwrap_or(self.sim.world.w as f32)
 }
 fn update_zoom(&mut self,dt:f32)->f32 {
  let speed=self.sim.physics.spec_of(&self.sim.world,self.sim.follow.unwrap_or(1)).map_or(0.,|b|b.vx.abs());
  let t=((speed-20.)/120.).clamp(0.,1.);
  // Zoom effects (from triggers: `zoom 1.3 1`) bring the camera closer.
  let target=(220.+120.*t*t*(3.-2.*t))/self.sim.camera.zoom.map_or(1.,|z|z.value);
  let rate=if target>self.cam_width {2.5}else{1.5};
  self.cam_width+=(target-self.cam_width)*(1.-(-dt.max(0.)*rate).exp());
  self.cam_width
 }
 fn update_camera(&mut self,vw:f32,vh:f32,dt:f32) {
  let b=self.sim.physics.spec_of(&self.sim.world,self.sim.follow.unwrap_or(1));
  // A focus effect (`focus it`, `focus X Y`) looks there instead.
  let focus=self.sim.camera.focus_point(&self.sim);
  let target_x=match focus {Some((x,_))=>x,None=>b.as_ref().map_or(self.cam_x,|b|b.x+45.+b.vx.clamp(-100.,150.)*0.15)}.clamp(vw/2.,self.sim.world.w as f32-vw/2.);
  // (Looks further down when you're dropping, further up when climbing.)
  let target_y=match focus {Some((_,y))=>y-vh*0.5,None=>b.as_ref().map_or(self.cam_y,|b|b.y-vh*0.6+b.vy.clamp(-90.,140.)*0.3)}.clamp(0.,self.sim.world.h as f32-vh);
  let blend=1.-(-dt*8.).exp();self.cam_x+=(target_x-self.cam_x)*blend;self.cam_y+=(target_y-self.cam_y)*(1.-(-dt*5.).exp());
  self.cam_x=self.cam_x.clamp(vw/2.,self.sim.world.w as f32-vw/2.);
  self.cam_y=self.cam_y.clamp(0.,self.sim.world.h as f32-vh);
 }
 fn retry(&mut self) {let best=self.best.max(self.score); *self=Self::with_level(self.level); self.best=best; self.started=true; self.state="ACTION!".into();}
 fn retry_checkpoint(&mut self) {
  let checkpoint=self.checkpoint;let checkpoint_y=self.sim.checkpoint.map(|c|c.1);let score=self.score;let gates=self.gates.clone();self.retry();
  if checkpoint>0. {
   let b=self.sim.physics.spec_of(&self.sim.world,self.sim.follow.unwrap()).unwrap();
   // On hilly levels, back on the ground at the checkpoint (the first solid
   // ground from the top there); Wreck Run is flat, so it just slides along.
   // (Best: where the level's checkpoint trigger says to come back to.)
   let marked=self.sim.triggers.iter().flat_map(|t|t.actions.iter()).find_map(|a|match a {grave_engine::trigger::Action::Checkpoint(Some((cx,cy))) if (cx-checkpoint).abs()<1. => Some(*cy),_=>None});
   let dy=if let (true,Some(y))=(self.lv().stages.is_empty(),marked) {y-b.y} else if self.lv().stages.is_empty() {
    let x=(checkpoint as usize).min(self.sim.world.w-1);
    let ground=(0..self.sim.world.h).find(|&y|{let i=self.sim.world.idx(x,y);!self.sim.world.is_owned(i) && matches!(self.mats.get(self.sim.world.mat[i]).state,grave_engine::material::State::Solid|grave_engine::material::State::Powder)});
    ground.map_or(0.,|g|g as f32-16.-b.y)
   } else {0.};
   let _=checkpoint_y;
   for p in &self.parts {self.sim.physics.edit(&mut self.sim.world,&self.mats,p.id,|bb|{bb.x+=checkpoint-b.x;bb.y+=dy;});}
   self.sim.checkpoint=checkpoint_y.map(|y|(checkpoint,y));
   self.checkpoint=checkpoint;self.score=score;self.gates=gates;self.cam_x=checkpoint+40.;
   // Fuses and checkpoints behind the restart point stay done.
   self.sim.skip_triggers_before(checkpoint);
   self.state="CHECKPOINT — back on the bike".into();
  }
 }
 fn guided_controls(&self)->(f32,bool) {
  let b=self.sim.physics.spec_of(&self.sim.world,self.sim.follow.unwrap()).unwrap();
  let slow=(b.x>875. && b.x<1125.) || (b.x>2310. && b.x<2370.);
  (1.,slow && b.vx>38.)
 }
 /// An automatic rider for any course (for testing levels): full gas;
 /// on the ground, lean against wheelies and nose-dives and brake when
 /// going too fast downhill; in the air, lean to land slightly nose down.
 fn ai_controls(&self)->(f32,f32,bool) {
  // (`--wait=N`: sit still for N frames first, to check a level isn't a one-timing trick.)
  if self.frame<self.ai_wait {return (0.,0.,false);}
  let Some(b)=self.sim.physics.spec_of(&self.sim.world,self.sim.follow.unwrap_or(1)) else {return (0.,0.,false)};
  let grounded=self.sim.physics.wheel_contact(&self.sim.world,&self.mats,self.sim.follow.unwrap_or(1)).grounded;
  if grounded {
   let lean=if b.angle< -0.25 {1.} else if b.angle>0.45 {-0.6} else {0.};
   let brake=b.angle>0.2 && b.vx>115.;
   (if brake {0.} else {1.},lean,brake)
  } else {
   let target=0.15;
   let lean=((target-b.angle)*2.5-b.spin*0.5).clamp(-1.,1.);
   (0.6,lean,false)
  }
 }
 fn crash_test(&mut self) {
  self.retry();
  let stone=self.mats.id("stone").unwrap();
  for y in 130..210 {for x in 145..151 {self.sim.world.set(&self.mats,x,y,stone);}}
  // Give the assembled bike speed; the wall and engine physics cause the damage.
  for part in &self.parts {self.sim.physics.edit(&mut self.sim.world,&self.mats,part.id,|b|b.vx=700.);}
  self.crashed=true;self.state="CRASH TEST — high-speed impact / R to reset".into();
 }
 // Surfaces, brakes, rolling resistance and balance are engine vehicle
 // controls (physics/vehicle.rs + material `rolling_resistance` + the
 // scene's `upright frame`): the game only passes on the keys.
 fn drive(&mut self,throttle:f32,lean:f32,brake:bool) {
  let id=self.sim.follow.unwrap_or(1);
  let contact=self.sim.physics.wheel_contact(&self.sim.world,&self.mats,id);
  self.surface=match contact.surface {Some(m) if contact.resistance>0. || self.mats.get(m).name=="ice" => self.mats.get(m).name.replace('_'," ").to_uppercase(),_=>String::new()};
  self.sim.physics.set_brake(if brake && !self.crashed {1.}else{0.});
  let was=self.crashed;
  self.tick(if brake {0.}else{throttle},lean);
  // The crash plays out in slow motion, with a jolt.
  if self.crashed && !was {self.sim.camera.slow_motion(0.3,1.5);self.sim.camera.shake(3.);}
 }
 // The helmet follows the same transform as the rider artwork. Contact with
 // solid scenery counts as a crash, even when the motorcycle fits underneath.
 fn helmet_blocked(&self,b:&grave_engine::physics::BodySpec)->bool {
  let (sn,cs)=b.angle.sin_cos();let cx=b.x-1.4*cs+17.*sn;let cy=b.y-1.4*sn-17.*cs;
  for dy in -3i32..=3 {for dx in -3i32..=3 {
   if dx*dx+dy*dy>9 {continue;}
   let (x,y)=((cx+dx as f32).round() as i32,(cy+dy as f32).round() as i32);
   if x<0||y<0||x>=self.sim.world.w as i32||y>=self.sim.world.h as i32 {continue;}
   let i=self.sim.world.idx(x as usize,y as usize);
   if self.mats.get(self.sim.world.mat[i]).state!=grave_engine::material::State::Solid {continue;}
   // Only fixed scenery counts: loose things (the bike's own parts, glass
   // shards, debris from a blast) brush past the rider's head.
   if self.sim.world.is_owned(i) {continue;}
   return true;
  }}false
 }
 // What the scene's triggers did this frame.
 fn read_events(&mut self) {
  for e in std::mem::take(&mut self.sim.events) {match e {
   grave_engine::trigger::Event::Message(m)=>self.state=m,
   grave_engine::trigger::Event::Checkpoint(x,_)=>if x>self.checkpoint {self.checkpoint=x;},
   grave_engine::trigger::Event::Score(n)=>self.score=(self.score as i64+n).max(0) as u32,
   grave_engine::trigger::Event::Finish=>self.finish(),
  }}
 }
 fn finish(&mut self) {
  if !self.started {return;}
  self.score+=(self.lv().timeout/5).saturating_sub(self.frame/3);self.best=self.best.max(self.score);
  self.state="THAT'S A WRAP! Course complete — BACKSPACE for another run".into();self.started=false;
  self.sim.camera.slow_motion(0.35,2.);
 }
 fn tick(&mut self, throttle:f32, lean:f32) {
  if self.crashed {self.sim.physics.set_throttle(0.);self.sim.step(&self.mats);self.frame+=1;return;}

  // Rider balance is the engine's `upright` assist; lean tips it.
  self.sim.physics.set_lean(lean);
  self.sim.physics.set_throttle(throttle); self.sim.step(&self.mats); self.frame+=1;
  let wheels_attached=self.sim.physics.joints().iter().filter(|j|matches!(j.kind,grave_engine::physics::JointKind::Wheel{..})).count();
  if wheels_attached<2 {self.crashed=true;self.state="CUT! Wheel attachment failed — R for a new take".into();return;}
  let Some(b)=self.sim.physics.spec_of(&self.sim.world,self.sim.follow.unwrap_or(1)) else {self.state="CUT! Bike destroyed — R to retry".into();self.crashed=true;return};
  if self.helmet_blocked(&b) {self.crashed=true;self.state="CUT! Rider hit scenery — R for checkpoint".into();return;}
  self.read_events();
  let lv=self.lv();
  for (i,(x,name,points)) in lv.stages.iter().enumerate() {if !self.gates[i] && b.x>*x {self.gates[i]=true;self.score+=points;self.state=format!("{name} — cleared!");}}
  // Stuck on its back (rolling about upside down, slowly): a crash.
  self.upside_down=if b.angle.cos()< -0.3 && b.vx.hypot(b.vy)<12. {self.upside_down+1} else {0};
  if self.upside_down>30 {self.state="CUT! Crashed — R to retry".into();self.crashed=true;}
  if lv.finish.is_some_and(|f|b.x>f) {self.finish();}
  if self.frame>lv.timeout {self.state="CUT! Take timed out — R to retry".into();self.started=false;}
 }
}
impl eframe::App for Game {
 fn ui(&mut self,ui:&mut egui::Ui,_:&mut eframe::Frame) {
  let ctx=ui.ctx().clone();
  let dt=self.last.elapsed().as_secs_f64().min(0.1);self.last=Instant::now();
  if ctx.input(|i|i.key_pressed(egui::Key::R)){self.retry_checkpoint();}
  if ctx.input(|i|i.key_pressed(egui::Key::Backspace)){self.retry();}
  if ctx.input(|i|i.key_pressed(egui::Key::Space)) && self.frame==0 {self.started=true;self.state="ACTION!".into();}
  // Pick a course before starting.
  if self.frame==0 && !self.started {
   for (k,key) in [egui::Key::Num1,egui::Key::Num2].into_iter().enumerate() {
    if ctx.input(|i|i.key_pressed(key)) && k!=self.level {let best=self.best;*self=Self::with_level(k);self.best=best;}
   }
  }
  if ctx.input(|i|i.key_pressed(egui::Key::C)){self.crash_test();}
  // Trials controls: W / ↑ throttle, S / ↓ brake (and back up when stopped),
  // A / ← lean back, D / → lean forward (Q / E too).
  let down=|k:&[egui::Key]|ctx.input(|i|k.iter().any(|k|i.key_down(*k)));
  let gas=down(&[egui::Key::W,egui::Key::ArrowUp]);
  let brake=down(&[egui::Key::S,egui::Key::ArrowDown]);
  let lean=if down(&[egui::Key::D,egui::Key::ArrowRight,egui::Key::E]){1.}else if down(&[egui::Key::A,egui::Key::ArrowLeft,egui::Key::Q]){-1.}else{0.};
  let slow=self.sim.physics.spec_of(&self.sim.world,self.sim.follow.unwrap_or(1)).is_some_and(|b|b.vx.abs()<4.);
  let (throttle,brake)=if brake && slow && !gas {(-0.6,false)} else {(if gas {1.} else {0.},brake)};
  if self.started && ctx.input(|i|i.focused){self.clock+=dt*self.sim.camera.time_scale() as f64;let mut n=0;while self.clock>=1./60. && n<6 && self.started {self.drive(throttle,lean,brake);self.clock-=1./60.;n+=1;}}else{self.clock=0.;}
  let lv=self.lv();
  ui.heading(format!("STUNT YARD  /  {}",lv.title));
  ui.label(lv.blurb);
  ui.horizontal(|ui|{ui.label(format!("SCORE {:04}     BEST {:04}     TAKE {:.1}s",self.score,self.best,self.frame as f32/60.));if ui.button("Checkpoint [R]").clicked(){self.retry_checkpoint();}if ui.button("Crash test [C]").clicked(){self.crash_test();}});
  let speed=self.sim.physics.spec_of(&self.sim.world,self.sim.follow.unwrap_or(1)).map_or(0.,|b|b.vx.abs()*0.36);
  ui.label(format!("{speed:.0} km/h   {}   |   CHECKPOINT {}m",if self.surface.is_empty(){"HARD SURFACE / AIR"}else{&self.surface},(self.checkpoint/10.) as u32));
  ui.label(egui::RichText::new(&self.state).color(Color32::from_rgb(255,190,65)).size(21.));
  ui.label("SPACE: start  •  W / ↑: throttle  •  S / ↓: brake (hold when stopped: back up)  •  A / ←: lean back  •  D / →: lean forward  •  R: checkpoint  •  BACKSPACE: restart  •  1 / 2: course  •  C: crash test");
  let (rect,_)=ui.allocate_exact_size(ui.available_size(),egui::Sense::hover());
  let b=self.sim.physics.spec_of(&self.sim.world,self.sim.follow.unwrap_or(1));
  let vw=self.update_zoom(dt as f32);let vh=(vw*rect.height()/rect.width()).min(240.);
  self.update_camera(vw,vh,dt as f32);
  let shake=self.sim.camera.shake_offset();
  let cx=self.cam_x+shake.0;let y0=self.cam_y+shake.1;
  // Weather falls around what the camera sees.
  self.sim.weather_view=Some((cx-vw/2.-30.,y0-10.,cx+vw/2.+30.,y0+vh));
  // Draw only what the camera sees (plus a margin): the real material cells,
  // for intact parts and wreckage alike.
  // (A steady size from frame to frame, so the picture buffer is reused.)
  let win=grave_engine::render::view_window(&self.sim.world,(cx,y0+vh/2.),(170.,120.),24.);
  self.renderer.light(&self.sim);
  self.renderer.set_camera(cx,y0+vh/2.);
  let mut rgba=self.renderer.draw_window(&self.sim.world,&self.mats,View::default(),win).to_vec();
  // Show the distant film-set backdrop through unlit empty air; retain fire glow.
  let air=self.mats.get(grave_engine::material::AIR).color;
  for (j,px) in rgba.chunks_exact_mut(4).enumerate() {
   let i=(win.1+j/win.2)*self.sim.world.w+win.0+j%win.2;
   if self.sim.world.mat[i]==grave_engine::material::AIR && (0..3).all(|k|px[k].abs_diff(air[k])<3) {px[3]=0;}
  }
  let image=egui::ColorImage::from_rgba_unmultiplied([win.2,win.3],&rgba);
  match &mut self.texture {Some(t) if t.size()==[win.2,win.3]=>t.set(image,egui::TextureOptions::NEAREST),_=>self.texture=Some(ctx.load_texture("world",image,egui::TextureOptions::NEAREST))}
  let (ww,wh)=(win.2 as f32,win.3 as f32);let (wx,wy)=(win.0 as f32,win.1 as f32);
  let view=Rect::from_min_max(Pos2::new((cx-vw/2.-wx)/ww,(y0-wy)/wh),Pos2::new((cx+vw/2.-wx)/ww,(y0+vh-wy)/wh));
  let desert=self.level==0 && ((430.0..1130.0).contains(&cx)||(2890.0..3200.0).contains(&cx));
  ui.painter().rect_filled(rect,0.,if desert {Color32::from_rgb(37,29,30)}else{Color32::from_rgb(15,20,31)});
  let backdrop=ui.painter().with_clip_rect(rect);
  // Distant warehouse silhouettes move more slowly than the physical course.
  let scale=rect.width()/vw;
  if desert {
   for layer in 0..2 {for n in 0..120 {
    let phase=n as f32*0.055+cx*(0.0008+layer as f32*0.0005);
    let h=(18.+layer as f32*12.+phase.sin()*12.+(phase*1.7).cos()*5.)*scale;
    let x=rect.left()+n as f32*rect.width()/120.;
    backdrop.rect_filled(Rect::from_min_size(Pos2::new(x,rect.bottom()-h),Vec2::new(rect.width()/120.+1.,h)),0.,if layer==0 {Color32::from_rgb(76,55,44)}else{Color32::from_rgb(103,75,47)});
   }}
  } else {for n in -2i32..12 {
   let wx=n as f32*75.-(cx*0.22).rem_euclid(75.);
   let x=rect.left()+wx*scale;
   let height=(22+(n*13).rem_euclid(30)) as f32*scale;
   let bottom=rect.top()+rect.height()*0.78;
   backdrop.rect_filled(Rect::from_min_size(Pos2::new(x,bottom-height),Vec2::new(60.*scale,height)),0.,Color32::from_rgb(21,27,37));
   for window in 0..5 {backdrop.rect_filled(Rect::from_min_size(Pos2::new(x+6.*scale+window as f32*10.*scale,bottom-height+8.*scale),Vec2::new(5.*scale,3.*scale)),0.,Color32::from_rgb(34,43,55));}
  }
  }
  ui.painter().image(self.texture.as_ref().unwrap().id(),rect,view,Color32::WHITE);
  let finish_x=self.finish_x();
  let progress=b.as_ref().map_or(0.,|b|((b.x-57.)/(finish_x-57.)).clamp(0.,1.));
  let p=ui.painter().with_clip_rect(rect);
  p.rect_filled(Rect::from_min_size(rect.min+Vec2::new(16.,16.),Vec2::new(rect.width()-32.,5.)),2.,Color32::from_rgb(45,50,60));
  p.rect_filled(Rect::from_min_size(rect.min+Vec2::new(16.,16.),Vec2::new((rect.width()-32.)*progress,5.)),2.,Color32::from_rgb(255,170,40));
  for &(start,name) in lv.signs {
   let sx=rect.left()+(start-cx+vw/2.)/vw*rect.width();
   let sy=rect.top()+(150.-y0)/vh*rect.height();
   p.text(Pos2::new(sx,sy),egui::Align2::LEFT_BOTTOM,name,egui::FontId::proportional(15.),Color32::from_rgb(240,175,60));
  }
  for &(x,name,_) in lv.stages {
   let sx=rect.left()+(x-cx+vw/2.)/vw*rect.width();
   if sx>rect.left() && sx<rect.right() {
    p.text(Pos2::new(sx,rect.top()+36.),egui::Align2::CENTER_TOP,name,egui::FontId::proportional(15.),Color32::from_rgb(255,195,80));
   }
  }
  let draw_sprite=|texture:egui::TextureId,x:f32,y:f32,angle:f32,left:f32,top:f32,width:f32,height:f32| {
   let mut mesh=egui::Mesh::with_texture(texture);let(sn,cs)=angle.sin_cos();
   for ((px,py),(u,v)) in [((left,top),(0.,0.)),((left+width,top),(1.,0.)),((left+width,top+height),(1.,1.)),((left,top+height),(0.,1.))] {
    let wx=x+px*cs-py*sn;let wy=y+px*sn+py*cs;
    mesh.vertices.push(egui::epaint::Vertex{pos:Pos2::new(rect.left()+(wx-cx+vw/2.)/vw*rect.width(),rect.top()+(wy-y0)/vh*rect.height()),uv:Pos2::new(u,v),color:Color32::WHITE});
   }
   mesh.indices.extend_from_slice(&[0,1,2,0,2,3]);ui.painter().with_clip_rect(rect).add(egui::Shape::mesh(mesh));
  };
  if self.rider.is_none() {
   let img=image::open("assets/stunt-rider.png").expect("rider art").to_rgba8();
   self.rider=Some(ctx.load_texture("rider",egui::ColorImage::from_rgba_unmultiplied([img.width() as usize,img.height() as usize],img.as_raw()),egui::TextureOptions::LINEAR));
  }
  if !self.crashed {if let Some(b)=b {draw_sprite(self.rider.as_ref().unwrap().id(),b.x,b.y,b.angle,-14.5,-22.5,28.,28.);}}
  ctx.request_repaint();
 }
}
fn main()->eframe::Result {
 std::env::set_current_dir(env!("CARGO_MANIFEST_DIR")).expect("project folder");
 let level=std::env::args().find_map(|a|a.strip_prefix("--level=").and_then(|s|s.parse::<usize>().ok())).unwrap_or(0).min(LEVELS.len()-1);
 if std::env::args().any(|a|a=="--verify-ai") {
  // The automatic rider rides a course; prints progress and saves pictures.
  let mut g=Game::with_level(level);g.started=true;
  g.ai_wait=std::env::args().find_map(|a|a.strip_prefix("--wait=").and_then(|s|s.parse().ok())).unwrap_or(0);
  if let Some(x)=std::env::args().find_map(|a|a.strip_prefix("--from=").and_then(|s|s.parse::<f32>().ok())){g.checkpoint=x;g.retry_checkpoint();}
  let mut best=(0.,0u32);
  for _ in 0..60*60*8 {
   if !g.started || (g.crashed && g.frame>60) {break;}
   // No progress for 20 seconds: stuck.
   if let Some(b)=g.sim.physics.spec_of(&g.sim.world,g.sim.follow.unwrap_or(1)) {if b.x>best.0+3. {best=(b.x,g.frame);} else if g.frame-best.1>1200 {println!("STUCK at x={:.0}",b.x);
    // What's around it (for fixing levels): one letter per cell.
    for y in (b.y as usize).saturating_sub(16)..(b.y as usize+16).min(g.sim.world.h) {let row:String=((b.x as usize).saturating_sub(20)..(b.x as usize+30).min(g.sim.world.w)).map(|x|{let i=g.sim.world.idx(x,y);if g.sim.world.is_owned(i){'#'}else{g.mats.get(g.sim.world.mat[i]).name.chars().next().unwrap().to_ascii_uppercase()}}).collect();println!("{y:4} {}",row.replace('A',"."));}
    break;}}
   let (t,lean,brake)=g.ai_controls();g.drive(t,lean,brake);
   if std::env::var("FLOORDBG").is_ok() {
    // Where is the mine floor (row 640) missing? Report the first time.
    let gap:Vec<usize>=(4020..4290).chain(4382..4690).filter(|&x|{let i=g.sim.world.idx(x,640);!g.sim.world.is_owned(i) && g.mats.get(g.sim.world.mat[i]).state!=grave_engine::material::State::Solid}).collect();
    if !gap.is_empty() {
     println!("FLOOR hole at frame {}: x {}..{} ({} cells)",g.frame,gap[0],gap[gap.len()-1],gap.len());
     for y in 634..=648 {let row:String=(gap[0].saturating_sub(12)..gap[0]+30).map(|x|{let i=g.sim.world.idx(x,y);if g.sim.world.is_owned(i){'#'}else{g.mats.get(g.sim.world.mat[i]).name.chars().next().unwrap().to_ascii_uppercase()}}).collect::<String>().replace('A',".");println!("{y} {row}");}
     break;
    }
   }
   let b=g.sim.physics.spec_of(&g.sim.world,g.sim.follow.unwrap_or(1)).unwrap();
   if g.frame%300==0 {
    let (x,y)=(b.x as usize,b.y as usize);g.renderer.light(&g.sim);g.renderer.set_camera(b.x,b.y);
    grave_engine::headless::shot_with_actors(&g.sim.world,&g.mats,&mut g.renderer,&[],Some((x.saturating_sub(130),y.saturating_sub(100),300,150)),std::path::Path::new(&format!("shots/ride-{}.png",g.frame)),3,false).unwrap();
   }
   let every=std::env::args().find_map(|a|a.strip_prefix("--every=").and_then(|s|s.parse::<u32>().ok())).unwrap_or(120).max(1);
   if g.frame%every==0 {let (t,lean,brake)=g.ai_controls();println!("{} x={:.0} y={:.0} angle={:.2} vx={:.0} vy={:.0} gas={t:.1} lean={lean:.1} brake={brake} score={} {}",g.frame,b.x,b.y,b.angle,b.vx,b.vy,g.score,g.state);}
  }
  if let Some(b)=g.sim.physics.spec_of(&g.sim.world,g.sim.follow.unwrap_or(1)) {
   let (x,y)=(b.x as usize,b.y as usize);g.renderer.light(&g.sim);g.renderer.set_camera(b.x,b.y);
   let _=grave_engine::headless::shot_with_actors(&g.sim.world,&g.mats,&mut g.renderer,&[],Some((x.saturating_sub(70),y.saturating_sub(60),140,90)),std::path::Path::new("shots/ride-end.png"),6,false);
  }
  println!("END frame {} {} score={}",g.frame,g.state,g.score);
  return Ok(());
 }
 if std::env::args().any(|a|a.starts_with("--verify")) {let mut g=Game::new();g.started=true;if let Some(x)=std::env::args().find_map(|a|a.strip_prefix("--from=").and_then(|s|s.parse::<f32>().ok())){g.checkpoint=x;g.retry_checkpoint();}for _ in 0..6500{if !g.started || (g.crashed && g.frame>180){break;}let(t,brake)=if std::env::args().any(|a|a=="--verify-smart"){g.guided_controls()}else{(1.,false)};g.drive(t,0.,brake);if g.frame%600==0{let x=g.sim.physics.spec_of(&g.sim.world,g.sim.follow.unwrap()).unwrap().x as usize;g.renderer.light(&g.sim);g.renderer.set_camera(x as f32+40.,180.);grave_engine::headless::shot_with_actors(&g.sim.world,&g.mats,&mut g.renderer,&[],Some((x.saturating_sub(90),100,260,160)),std::path::Path::new(&format!("shots/wreck-{}.png",g.frame)),3,false).unwrap();}if g.frame%120==0{let b=g.sim.physics.spec_of(&g.sim.world,g.sim.follow.unwrap_or(1)).unwrap();println!("{} x={:.1} y={:.1} angle={:.1} score={} flying={} surface={}",g.frame,b.x,b.y,b.angle,g.score,g.sim.world.flying.len(),g.surface);}} println!("{} score={}",g.state,g.score);grave_engine::headless::shot(&g.sim.world,&g.mats,&mut g.renderer,std::path::Path::new("shots/stunt-verify.png"),2,false).unwrap();return Ok(());}
 eframe::run_native("Stunt Yard — Grave Engine",eframe::NativeOptions{viewport:egui::ViewportBuilder::default().with_inner_size(Vec2::new(1200.,760.)),..Default::default()},Box::new(|cc|{cc.egui_ctx.set_visuals(egui::Visuals::dark());Ok(Box::new(Game::new()))}))
}



#[cfg(test)]
mod tests {
 use super::*;
 #[test]
 fn controlled_riding_completes_the_material_course() {
  let mut g=Game::new();g.started=true;
  let mut previous=0.;
  for _ in 0..6500 {if !g.started || g.crashed {break;}let(t,brake)=g.guided_controls();g.drive(t,0.,brake);
   if g.frame%240==0 {let b=g.sim.physics.spec_of(&g.sim.world,g.sim.follow.unwrap()).unwrap();println!("frame {} x {} speed {}",g.frame,b.x,b.vx);if (b.x-previous).abs()<2. {break;}previous=b.x;}
  }
  assert!(g.state.starts_with("THAT'S A WRAP"),"{}",g.state);
  assert_eq!(g.gates,[true;9]);assert!(g.score>=1000);assert!(g.sim.triggers.iter().all(|t|t.fired),"every fuse and checkpoint went off");
  g.retry();assert_eq!(g.score,0);assert_eq!(g.frame,0);assert_eq!(g.gates,[false;9]);assert!(g.best>=1000);
 }
 #[test]
 fn the_automatic_rider_finishes_mountain_run() {
  // The whole long level, ridden by the test rider: it can be done.
  let mut g=Game::with_level(1);g.started=true;
  assert_eq!(g.lv().title,"MOUNTAIN RUN");
  for _ in 0..60*60*3 {if !g.started || g.crashed {break;}let (t,lean,brake)=g.ai_controls();g.drive(t,lean,brake);}
  assert!(g.state.starts_with("THAT'S A WRAP"),"{} at x {:?}",g.state,g.sim.physics.centre(g.sim.follow.unwrap()));
  assert!(g.score>3000,"stages scored on the way: {}",g.score);
 }
 #[test]
 fn material_parts_break_loose_in_a_real_wall_crash() {
  let mut g=Game::new();let initial=g.sim.physics.joints().len();g.crash_test();
  for _ in 0..90 {g.sim.step(&g.mats);}
  assert!(g.sim.physics.joints().len()<initial,"hard wall impact must break attachments");
  let loose=g.parts.iter().filter(|p|g.sim.physics.spec_of(&g.sim.world,p.id).is_some() && g.sim.physics.joints_of(p.id).is_empty()).count();
  assert!(loose>0,"at least one intact part must survive detached");
  grave_engine::headless::shot(&g.sim.world,&g.mats,&mut g.renderer,std::path::Path::new("shots/stunt-material-crash.png"),3,false).unwrap();
 }
 #[test]
 fn a_wall_crash_bends_the_metal() {
  let mut g=Game::new();g.crash_test();
  for _ in 0..90 {g.sim.step(&g.mats);}
  for p in &g.parts {println!("{} bent {:.2}",p.name,g.sim.physics.bent(p.id));}
  assert!(g.parts.iter().any(|p|g.sim.physics.bent(p.id)>0.05),"some iron or brass part bends in a hard wall crash");
  grave_engine::headless::shot_with_actors(&g.sim.world,&g.mats,&mut g.renderer,&[],g.parts.iter().find(|p|p.name=="fork").and_then(|p|g.sim.physics.centre(p.id)).map(|(x,y)|((x as usize).saturating_sub(40),(y as usize).saturating_sub(25).min(210),80,45)),std::path::Path::new("shots/stunt-bent-crash.png"),6,false).unwrap();
 }
 #[test]
 fn tyres_have_rubber_cells_and_really_burn() {
  let mut g=Game::new();let rubber=g.mats.id("rubber").unwrap();let iron=g.mats.id("iron").unwrap();
  let wheel=g.parts.iter().find(|p|p.name=="rear-wheel").unwrap().id;
  let body=g.sim.physics.spec_of(&g.sim.world,wheel).unwrap();
  assert!(body.cells.iter().flatten().any(|c|c.mat==rubber));
  assert!(body.cells.iter().flatten().any(|c|c.mat==iron));
  g.sim.physics.edit(&mut g.sim.world,&g.mats,wheel,|b|{for c in b.cells.iter_mut().flatten(){if c.mat==rubber {c.temp=600.;}}});
  for _ in 0..5 {g.sim.step(&g.mats);}
  assert!(g.sim.world.mat.iter().enumerate().any(|(i,m)|*m==rubber && g.sim.world.flags[i] & grave_engine::world::BURNING!=0),"rubber tyre must ignite under real material rules");
 }

 #[test]
 fn camera_scrolls_beyond_the_old_world_and_clamps_at_finish() {
  let mut g=Game::new();
  for p in &g.parts {g.sim.physics.edit(&mut g.sim.world,&g.mats,p.id,|b|b.x+=1600.);}
  for _ in 0..30 {g.update_camera(220.,114.,0.1);}
  assert!(g.cam_x>1600. && g.cam_x<1800.,"camera follows beyond old 960-cell limit: {}",g.cam_x);
  for p in &g.parts {g.sim.physics.edit(&mut g.sim.world,&g.mats,p.id,|b|b.x+=1900.);}
  for _ in 0..30 {g.update_camera(220.,114.,0.1);}
  assert!(g.cam_x>3400. && g.cam_x<=g.sim.world.w as f32-110.);
 }

 #[test]
 fn loose_surfaces_slow_the_bike_and_brakes_reduce_speed() {
  let run=|material:&str,brake:bool| {
   let mut g=Game::new();let id=g.mats.id(material).unwrap();
   for y in 206..210 {for x in 0..170 {g.sim.world.set(&g.mats,x,y,id);}}
   for p in &g.parts {g.sim.physics.edit(&mut g.sim.world,&g.mats,p.id,|b|b.vx=70.);}
   for _ in 0..60 {g.drive(if brake{0.}else{1.},0.,brake);}
   g.sim.physics.spec_of(&g.sim.world,g.sim.follow.unwrap()).unwrap().x
  };
  let road=run("stone",false);let sand=run("sand",false);let mud=run("wet_sand",false);let brake=run("stone",true);
  println!("Positions after identical starts: road {road}, sand {sand}, mud {mud}, brake {brake}");
  assert!(sand<road-5. && mud<sand-3. && brake<road-20.);
 }
 #[test]
 fn passage_cues_detonate_actual_gunpowder_once() {
  let mut g=Game::new();let powder=g.mats.id("gunpowder").unwrap();
  let before=g.sim.world.mat.iter().filter(|m|**m==powder).count();
  let fuse=|g:&Game|g.sim.triggers.iter().find(|t|t.x==1535.).unwrap().fired;
  let move_to=|g:&mut Game,x:f32|{let dx=x-g.sim.physics.spec_of(&g.sim.world,g.sim.follow.unwrap()).unwrap().x;for p in &g.parts {g.sim.physics.edit(&mut g.sim.world,&g.mats,p.id,|b|{b.x+=dx;b.vx=0.;});}};
  move_to(&mut g,1525.);g.sim.step(&g.mats);assert!(!fuse(&g),"not yet");
  move_to(&mut g,1540.);g.sim.step(&g.mats);assert!(fuse(&g),"the bike reached the fuse trigger");
  g.read_events();assert_eq!(g.state,"DEMOLITION CHARGE!");
  let i=g.sim.world.idx(1508,205);
  let mut blew=false;for _ in 0..3 {g.sim.step(&g.mats);blew|=!g.sim.world.blasts.is_empty();}
  assert!(blew||g.sim.world.mat.iter().filter(|m|**m==powder).count()<before,"charge must generate engine blasts");
  assert!(g.sim.world.mat.iter().filter(|m|**m==powder).count()<before);
  let _=i;
  grave_engine::headless::shot_with_actors(&g.sim.world,&g.mats,&mut g.renderer,&[],Some((1430,100,240,160)),std::path::Path::new("shots/wreck-blast.png"),4,false).unwrap();
 }
 #[test]
 fn checkpoints_rebuild_bike_without_reawarding_gates() {
  let mut g=Game::new();g.checkpoint=1340.;g.score=1250;g.gates[0]=true;g.gates[1]=true;g.crashed=true;
  g.retry_checkpoint();
  assert!(!g.crashed);assert_eq!(g.score,1250);assert!(g.gates[1]);
  assert_eq!(g.sim.physics.spec_of(&g.sim.world,g.sim.follow.unwrap()).unwrap().x,1340.);
  let fired=|x:f32|g.sim.triggers.iter().find(|t|t.x==x).unwrap().fired;
  assert!(fired(190.) && !fired(1535.),"fuses behind the checkpoint stay done, ones ahead don't");
  for _ in 0..30 {g.drive(1.,0.,false);}assert!(!g.crashed);
 }

 #[test]
 fn popping_a_wheelie_into_the_gantry_is_not_a_clean_run() {
  // The heavy trials bike hugs the ground, so it fits under the low gantry
  // (the full-course test ride proves that); a rider who pulls the front up
  // (lean back) at it doesn't get through. From the first checkpoint (past
  // the tyre fire, whose embers make every ride a little different).
  let mut g=Game::new();g.started=true;g.checkpoint=410.;g.retry_checkpoint();
  for _ in 0..2400 {
   if g.crashed || !g.started {break;}
   let b=g.sim.physics.spec_of(&g.sim.world,g.sim.follow.unwrap()).unwrap();
   g.drive(1.,if (900.0..1060.0).contains(&b.x) {-1.} else {0.},false);
  }
  let b=g.sim.physics.spec_of(&g.sim.world,g.sim.follow.unwrap()).unwrap();
  println!("wheelie at the gantry: x {} crashed {} state {}",b.x,g.crashed,g.state);
  assert!(b.x<1130.0 && (g.crashed || b.vx.abs()<2.),"with the front up, the rider doesn't get past the gantry (x {})",b.x);
 }

 #[test]
 fn camera_zoom_eases_with_speed_and_stays_inside_world() {
  let mut g=Game::new();let id=g.sim.follow.unwrap();
  g.sim.physics.edit(&mut g.sim.world,&g.mats,id,|b|b.vx=140.);
  let first=g.update_zoom(1./60.);assert!(first>220. && first<226.);
  for _ in 0..180 {g.update_zoom(1./60.);}
  assert!(g.cam_width>339. && g.cam_width<=340.);
  let width=g.cam_width;g.update_camera(width,150.,0.016);
  assert!(g.cam_x>=width/2. && g.cam_y>=0.);
  g.sim.physics.edit(&mut g.sim.world,&g.mats,id,|b|b.vx=0.);
  let first=g.update_zoom(1./60.);assert!(first>335.);
  for _ in 0..240 {g.update_zoom(1./60.);}
  assert!(g.cam_width>=220. && g.cam_width<221.);
 }

}
