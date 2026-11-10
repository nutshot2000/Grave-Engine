//! Stunt Yard: a small game built on Grave Engine's existing simulation.
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use grave_engine::{material::Materials, render::{Renderer, View}, sim::Sim};
use std::time::Instant;

const STAGES: [(f32,&str,u32); 9] = [(370.,"FIRE STARTER",500),(700.,"SPLASH ALLEY",250),(970.,"THE DOUBLES",500),(1510.,"INFERNO",600),(1840.,"BREAKAWAY ROW",350),(2080.,"LAVA LEAP",750),(2650.,"BURNING STRAIGHT",600),(3220.,"FINAL FLIGHT",750),(3480.,"SAND & SHATTER",400)];
const FINISH:f32=3520.;

struct Part { id:u64, name:String }

struct Game { parts:Vec<Part>, crashed:bool, sim: Sim, mats: Materials, renderer: Renderer, texture: Option<egui::TextureHandle>, rider: Option<egui::TextureHandle>, clock: f64, last: Instant, frame: u32, state: String, started: bool, best: u32, score: u32, gates: [bool;9], cam_x:f32, cam_y:f32 }
impl Game {
 fn new() -> Self {
  let mats=Materials::load_dir("materials");
  assert!(mats.problems.is_empty(), "{:?}",mats.problems);
  let text=std::fs::read_to_string("scenes/stunt-yard.txt").expect("stunt scene");
  let (sim,problems)=grave_engine::scene::load(&text,&mats,Some(std::path::Path::new("scenes"))).into_sim(&mats);
  assert!(problems.is_empty(),"{problems:?}");
  let ids=sim.physics.ids();
  let parts:Vec<Part>=std::fs::read_to_string("assets/stunt-bike/parts.tsv").expect("parts manifest").lines().enumerate().map(|(i,line)|{
   let v:Vec<_>=line.split('\t').collect();
   Part{id:ids[i+1],name:v[0].into()}
  }).collect();
  assert!(parts.iter().any(|p|p.name=="frame"),"missing bike frame");
  Self {parts,crashed:false,sim,mats,renderer:Renderer::default(),texture:None,rider:None,clock:0.,last:Instant::now(),frame:0,state:"Ready on set — press SPACE".into(),started:false,best:0,score:0,gates:[false;9],cam_x:110.,cam_y:100.}
 }
 fn update_camera(&mut self,vw:f32,vh:f32,dt:f32) {
  let b=self.sim.physics.spec_of(&self.sim.world,self.sim.follow.unwrap_or(1));
  let target_x=b.as_ref().map_or(self.cam_x,|b|b.x+45.+b.vx.clamp(-100.,150.)*0.15).clamp(vw/2.,self.sim.world.w as f32-vw/2.);
  let target_y=b.as_ref().map_or(self.cam_y,|b|b.y-vh*0.67).clamp(0.,self.sim.world.h as f32-vh);
  let blend=1.-(-dt*8.).exp();self.cam_x+=(target_x-self.cam_x)*blend;self.cam_y+=(target_y-self.cam_y)*blend;
 }
 fn retry(&mut self) {let best=self.best.max(self.score); *self=Self::new(); self.best=best; self.started=true; self.state="ACTION!".into();}
 fn crash_test(&mut self) {
  self.retry();
  let stone=self.mats.id("stone").unwrap();
  for y in 130..210 {for x in 145..151 {self.sim.world.set(&self.mats,x,y,stone);}}
  // Give the assembled bike speed; the wall and engine physics cause the damage.
  for part in &self.parts {self.sim.physics.edit(&mut self.sim.world,&self.mats,part.id,|b|b.vx=700.);}
  self.crashed=true;self.state="CRASH TEST — high-speed impact / R to reset".into();
 }
 fn tick(&mut self, throttle:f32, lean:f32) {
  if self.crashed {self.sim.physics.set_throttle(0.);self.sim.step(&self.mats);self.frame+=1;return;}

  if let Some(b)=self.sim.physics.spec_of(&self.sim.world,self.sim.follow.unwrap_or(1)) {
   // Assisted rider balance; manual lean shifts the target angle.
   let angle=b.angle.sin().atan2(b.angle.cos());
   { self.sim.physics.steer_rotation(self.sim.follow.unwrap_or(1), (-angle*60. - b.spin*15. + lean*28.).clamp(-100.,100.), 3.); }
  }
  self.sim.physics.set_throttle(throttle); self.sim.step(&self.mats); self.frame+=1;
  let wheels_attached=self.sim.physics.joints().iter().filter(|j|matches!(j.kind,grave_engine::physics::JointKind::Wheel{..})).count();
  if wheels_attached<2 {self.crashed=true;self.state="CUT! Wheel attachment failed — R for a new take".into();return;}
  let Some(b)=self.sim.physics.spec_of(&self.sim.world,self.sim.follow.unwrap_or(1)) else {self.state="CUT! Bike destroyed — R to retry".into();self.crashed=true;return};
  for (i,(x,name,points)) in STAGES.iter().enumerate() {if !self.gates[i] && b.x>*x {self.gates[i]=true;self.score+=points;self.state=format!("{name} — cleared!");}}
  if b.angle.cos()<0.0 && b.y>180. {self.state="CUT! Crashed — R to retry".into();self.crashed=true;}
  if b.x>FINISH {self.score+=3000u32.saturating_sub(self.frame/3);self.best=self.best.max(self.score);self.state="THAT'S A WRAP! Course complete — R for another take".into();self.started=false;}
  if self.frame>14400 {self.state="CUT! Take timed out — R to retry".into();self.started=false;}
 }
}
impl eframe::App for Game {
 fn ui(&mut self,ui:&mut egui::Ui,_:&mut eframe::Frame) {
  let ctx=ui.ctx().clone();
  let dt=self.last.elapsed().as_secs_f64().min(0.1);self.last=Instant::now();
  if ctx.input(|i|i.key_pressed(egui::Key::R)){self.retry();}
  if ctx.input(|i|i.key_pressed(egui::Key::Space)) && self.frame==0 {self.started=true;self.state="ACTION!".into();}
  if ctx.input(|i|i.key_pressed(egui::Key::C)){self.crash_test();}
  let throttle=ctx.input(|i| if i.key_down(egui::Key::D)||i.key_down(egui::Key::ArrowRight){1.}else if i.key_down(egui::Key::A)||i.key_down(egui::Key::ArrowLeft){-1.}else{0.});
  if self.started && ctx.input(|i|i.focused){self.clock+=dt;let mut n=0;while self.clock>=1./60. && n<6 && self.started {self.tick(throttle,ctx.input(|i|if i.key_down(egui::Key::E){1.}else if i.key_down(egui::Key::Q){-1.}else{0.}));self.clock-=1./60.;n+=1;}}else{self.clock=0.;}
  ui.heading("STUNT YARD  /  INFERNO RUN");
  ui.label("360m COURSE  •  6 JUMPS  •  FIRE / WATER / LAVA / BREAKAWAYS");
  ui.horizontal(|ui|{ui.label(format!("SCORE {:04}     BEST {:04}     TAKE {:.1}s",self.score,self.best,self.frame as f32/60.));if ui.button("New take [R]").clicked(){self.retry();}if ui.button("Crash test [C]").clicked(){self.crash_test();}});
  ui.label(egui::RichText::new(&self.state).color(Color32::from_rgb(255,190,65)).size(21.));
  ui.label("SPACE: start  •  D / →: accelerate  •  A / ←: reverse  •  Q / E: lean  •  R: retry  •  C: crash test");
  // Always draw the real material cells, for intact parts and wreckage alike.
  let mut rgba=self.renderer.draw(&self.sim.world,&self.mats,View::default()).to_vec();
  // Show the distant film-set backdrop through unlit empty air; retain fire glow.
  let air=self.mats.get(grave_engine::material::AIR).color;
  for (i,px) in rgba.chunks_exact_mut(4).enumerate() {
   if self.sim.world.mat[i]==grave_engine::material::AIR && (0..3).all(|k|px[k].abs_diff(air[k])<3) {px[3]=0;}
  }
  let image=egui::ColorImage::from_rgba_unmultiplied([self.sim.world.w,self.sim.world.h],&rgba);
  if let Some(t)=&mut self.texture {t.set(image,egui::TextureOptions::NEAREST);}else{self.texture=Some(ctx.load_texture("world",image,egui::TextureOptions::NEAREST));}
  let (rect,_)=ui.allocate_exact_size(ui.available_size(),egui::Sense::hover());
  let b=self.sim.physics.spec_of(&self.sim.world,self.sim.follow.unwrap_or(1));
  let vw=220.;let vh=(vw*rect.height()/rect.width()).min(240.);
  let world_w=self.sim.world.w as f32;let world_h=self.sim.world.h as f32;
  self.update_camera(vw,vh,dt as f32);
  let cx=self.cam_x;let y0=self.cam_y;

  let view=Rect::from_min_max(Pos2::new((cx-vw/2.)/world_w,y0/world_h),Pos2::new((cx+vw/2.)/world_w,(y0+vh)/world_h));
  ui.painter().rect_filled(rect,0.,Color32::from_rgb(15,20,31));
  let backdrop=ui.painter().with_clip_rect(rect);
  // Distant warehouse silhouettes move more slowly than the physical course.
  let scale=rect.width()/vw;
  for n in -2i32..12 {
   let wx=n as f32*75.-(cx*0.22).rem_euclid(75.);
   let x=rect.left()+wx*scale;
   let height=(22+(n*13).rem_euclid(30)) as f32*scale;
   let bottom=rect.top()+rect.height()*0.78;
   backdrop.rect_filled(Rect::from_min_size(Pos2::new(x,bottom-height),Vec2::new(60.*scale,height)),0.,Color32::from_rgb(21,27,37));
   for window in 0..5 {backdrop.rect_filled(Rect::from_min_size(Pos2::new(x+6.*scale+window as f32*10.*scale,bottom-height+8.*scale),Vec2::new(5.*scale,3.*scale)),0.,Color32::from_rgb(34,43,55));}
  }
  ui.painter().image(self.texture.as_ref().unwrap().id(),rect,view,Color32::WHITE);
  let progress=b.as_ref().map_or(0.,|b|((b.x-57.)/(FINISH-57.)).clamp(0.,1.));
  let p=ui.painter().with_clip_rect(rect);
  p.rect_filled(Rect::from_min_size(rect.min+Vec2::new(16.,16.),Vec2::new(rect.width()-32.,5.)),2.,Color32::from_rgb(45,50,60));
  p.rect_filled(Rect::from_min_size(rect.min+Vec2::new(16.,16.),Vec2::new((rect.width()-32.)*progress,5.)),2.,Color32::from_rgb(255,170,40));
  for (start,name) in [(180.,"JUMP 01 / FIRE"),(750.,"JUMP 02 / WATER"),(1320.,"JUMP 03 / FIRE"),(1890.,"JUMP 04 / LAVA"),(2460.,"JUMP 05 / FIRE"),(3030.,"JUMP 06 / FINALE")] {
   let sx=rect.left()+(start-cx+vw/2.)/vw*rect.width();
   let sy=rect.top()+(150.-y0)/vh*rect.height();
   p.text(Pos2::new(sx,sy),egui::Align2::LEFT_BOTTOM,name,egui::FontId::proportional(15.),Color32::from_rgb(240,175,60));
  }
  for (x,name,_) in STAGES {
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
 if std::env::args().any(|a|a=="--verify") {let mut g=Game::new();g.started=true;for _ in 0..10000{if !g.started || (g.crashed && g.frame>180){break;}g.tick(1.,0.);if g.frame%120==0{let b=g.sim.physics.spec_of(&g.sim.world,g.sim.follow.unwrap_or(1)).unwrap();println!("{} x={:.1} y={:.1} angle={:.1} score={}",g.frame,b.x,b.y,b.angle,g.score);}} println!("{} score={}",g.state,g.score);grave_engine::headless::shot(&g.sim.world,&g.mats,&mut g.renderer,std::path::Path::new("shots/stunt-verify.png"),2,false).unwrap();return Ok(());}
 eframe::run_native("Stunt Yard — Grave Engine",eframe::NativeOptions{viewport:egui::ViewportBuilder::default().with_inner_size(Vec2::new(1200.,760.)),..Default::default()},Box::new(|cc|{cc.egui_ctx.set_visuals(egui::Visuals::dark());Ok(Box::new(Game::new()))}))
}



#[cfg(test)]
mod tests {
 use super::*;
 #[test]
 fn full_throttle_completes_the_long_course() {
  let mut g=Game::new();g.started=true;
  for _ in 0..10000 {if !g.started {break;}g.tick(1.,0.);}
  assert!(g.state.starts_with("THAT'S A WRAP"),"{}",g.state);
  assert_eq!(g.gates,[true;9]);assert!(g.score>=1000);
  g.retry();assert_eq!(g.score,0);assert_eq!(g.frame,0);assert_eq!(g.gates,[false;9]);assert!(g.best>=1000);
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

}
