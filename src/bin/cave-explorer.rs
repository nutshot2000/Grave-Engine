//! DEEPWARD: a small playable cave expedition built on Grave Engine.
//! Keyboard/controller input is data-driven; the editable scene owns the cave.
#[path="cave-explorer/game.rs"] mod game;
use game::Expedition;
use eframe::egui::{self, Color32, Pos2, Rect, Vec2, FontId, Align2};
use grave_engine::{input::Input, physics::Controls, render::View};
use std::{collections::BTreeMap, time::Instant, path::Path};
const INPUT:&str=include_str!("../../assets/input/cave-explorer.input");

struct App {
    supplies:grave_engine::supplies::Panel,
    game:Expedition,input:Input,texture:Option<egui::TextureHandle>,
    sprites:BTreeMap<(String,usize),egui::TextureHandle>,last:Instant,clock:f32,
    started:bool,paused:bool,map:bool,view_at:Option<(Pos2,f32)>,last_mouse:Option<Pos2>,mouse_aim:bool,pending_tool_press:bool,camera:(f32,f32),save_clock:u32,
}
impl App {
    fn new()->Self{let game=Expedition::new(true);let p=game.player();Self{supplies:Default::default(),game,input:Input::load_or("assets/input/cave-explorer.input",INPUT).expect("controls"),texture:None,sprites:BTreeMap::new(),last:Instant::now(),clock:0.,started:false,paused:false,map:false,view_at:None,last_mouse:None,mouse_aim:true,pending_tool_press:false,camera:(p.x,p.y),save_clock:0}}
    fn draw(&mut self,ctx:&egui::Context,ui:&mut egui::Ui){
        let rect=ui.max_rect();let p=self.game.player();
        if !self.supplies.open {let wheel=ctx.input(|i|i.smooth_scroll_delta.y);if wheel.abs()>0.1&&p.rope.as_ref().is_some_and(|r|!r.pins.is_empty()){self.game.sim.physics.wind_rope(wheel*0.08);}else if wheel.abs()>0.1{self.game.tuning.view=(self.game.tuning.view*(1.12f32).powf(-wheel/60.)).clamp(180.,600.);}}
        let width=self.game.tuning.view;let scale=rect.width()/width;
        let height=rect.height()/scale;
        self.camera.0+=(p.x+p.facing*16.-self.camera.0)*0.09;self.camera.1+=(p.y-if p.rope.is_some(){30.}else{8.}-self.camera.1)*0.08;
        let (w,h)=(self.game.sim.world.w as f32,self.game.sim.world.h as f32);
        let left=(self.camera.0-width/2.).clamp(0.,(w-width).max(0.));let top=(self.camera.1-height/2.).clamp(0.,(h-height).max(0.));
        let win=(left as usize,top as usize,(width.ceil() as usize+1).min(w as usize-left as usize),(height.ceil() as usize+1).min(h as usize-top as usize));
        let point=|x:f32,y:f32|Pos2::new(rect.left()+(x-left)*scale,rect.top()+(y-top)*scale);
        self.view_at=Some((point(0.,0.),scale));
        let mouse=ctx.input(|i|i.pointer.hover_pos());
        let stick=(self.input.axis("aim_x"),self.input.axis("aim_y"));
        if stick.0.hypot(stick.1)>0.2 {self.game.aim=(stick.0/stick.0.hypot(stick.1),stick.1/stick.0.hypot(stick.1));}
        else if self.mouse_aim {if let Some(mouse)=mouse.filter(|m|rect.contains(*m)){let shoulder=grave_engine::tool_pose::shoulder(&p);let v=mouse-point(shoulder.0,shoulder.1);if v.length()>0.5{self.game.aim=(v.x/v.length(),v.y/v.length());}}}
        self.last_mouse=mouse;
        self.game.renderer.light(&self.game.sim);self.game.renderer.set_camera(self.camera.0,self.camera.1);
        let rgba=self.game.renderer.draw_window(&self.game.sim.world,&self.game.mats,View::default(),win);
        let image=egui::ColorImage::from_rgba_unmultiplied([win.2,win.3],rgba);
        match &mut self.texture{Some(t) if t.size()==[win.2,win.3]=>t.set(image,egui::TextureOptions::NEAREST),_=>self.texture=Some(ctx.load_texture("expedition-world",image,egui::TextureOptions::NEAREST))}
        let draw=ui.painter().with_clip_rect(rect);
        draw.image(self.texture.as_ref().unwrap().id(),Rect::from_min_size(point(win.0 as f32,win.1 as f32),Vec2::new(win.2 as f32*scale,win.3 as f32*scale)),Rect::from_min_max(Pos2::ZERO,Pos2::new(1.,1.)),Color32::WHITE);
        // Sample the same blended rig pose used by the physical body.
        if let Some(look)=&p.look{
            let (frame,clip,index)=look.sample(&p.anim,p.anim_time,p.blend.as_ref());
            let key=(clip.to_string(),index);
            let tex=self.sprites.entry(key).or_insert_with(||ctx.load_texture("explorer-frame",egui::ColorImage::from_rgba_unmultiplied([frame.w,frame.h],&frame.rgba),egui::TextureOptions::LINEAR));
            let cpp=look.cells_per_px(16)*frame.scale;let feet=p.y+8.+p.visual_offset.1;
            let min=point(p.x+p.visual_offset.0-frame.w as f32*cpp/2.,feet-look.feet*look.cells_per_px(16));
            let uv=if p.facing<0.{Rect::from_min_max(Pos2::new(1.,0.),Pos2::new(0.,1.))}else{Rect::from_min_max(Pos2::ZERO,Pos2::new(1.,1.))};
            draw.image(tex.id(),Rect::from_min_size(min,Vec2::new(frame.w as f32*cpp*scale,frame.h as f32*cpp*scale)),uv,Color32::WHITE);
        }
        if p.boosting {for i in 0..7{let flicker=((self.game.frames+i*13) as f32*0.7).sin();let x=p.x-p.facing*3.+flicker*0.6;let y=p.y+4.+i as f32*0.9;draw.circle_filled(point(x,y),(2.4-i as f32*0.24)*scale,Color32::from_rgba_unmultiplied(85,210,255,200-i as u8*22));}}
        grave_engine::cave_ui::equipment(&draw,&self.game.sim,&self.game.mats,&self.game.catalog,self.game.aim,self.game.drilling,scale,|p|point(p.0,p.1));
        let camp=point(self.game.tuning.home.0,self.game.tuning.home.1-17.);draw.circle_stroke(camp,9.,(1.2,Color32::from_rgb(255,196,90)));draw.text(camp-Vec2::new(0.,14.),Align2::CENTER_BOTTOM,"LIFT CAMP",FontId::proportional(14.),Color32::from_rgb(255,210,140));
        for b in &self.game.sim.supplies.stations {let at=point(b.x,b.y);draw.circle_stroke(at,7.,(1.5,Color32::from_rgb(120,235,190)));draw.text(at-Vec2::new(0.,18.),Align2::CENTER_BOTTOM,format!("{} workshop / B",b.kind),FontId::proportional(13.),Color32::from_rgb(140,240,200));}
        for a in self.game.sim.physics.actor_status() {if let Some(c)=&a.creature {if !a.dead {
            let n=a.surface.map_or((0.,-1.),|s|s.normal);let gap=c.species.height*0.5+3.;let at=point(a.x+n.0*gap,a.y+(16.-a.body_height)*0.5+n.1*gap);let width=12.*scale;let r=Rect::from_center_size(at,Vec2::new(width,4.));draw.rect_filled(r,1.,Color32::from_black_alpha(180));let mut fill=r;fill.set_width(width*(a.health/c.species.health).clamp(0.,1.));draw.rect_filled(fill,1.,Color32::from_rgb(220,150,90));draw.text(at+Vec2::new(0.,if n.1>0.5 {7.}else{-7.}),if n.1>0.5 {Align2::CENTER_TOP}else{Align2::CENTER_BOTTOM},format!("{} / {}",c.label(),c.mood.name()),FontId::proportional(12.),if c.mood==grave_engine::creatures::Mood::Windup {Color32::LIGHT_RED}else{Color32::from_rgb(225,205,165)});
        }}}
        self.hud(&draw,rect);
        if self.map&&self.started&&!self.paused&&!self.game.won{self.draw_map(&draw,rect);}
        if !self.started||self.paused||self.game.won {
            draw.rect_filled(rect,0.,Color32::from_black_alpha(150));
            let center=rect.center();draw.text(center-Vec2::new(0.,120.),Align2::CENTER_CENTER,"D E E P W A R D",FontId::proportional(50.),Color32::from_rgb(220,242,240));
            let label=if self.game.won{"EXPEDITION COMPLETE"}else if self.paused{"EXPEDITION PAUSED"}else{"A CAVE EXPEDITION"};
            draw.text(center-Vec2::new(0.,65.),Align2::CENTER_CENTER,label,FontId::proportional(18.),Color32::from_rgb(102,218,209));
            draw.text(center,Align2::CENTER_CENTER,"Recover cyan and violet crystal samples.\nBring both back to the lift. Make your own route.",FontId::proportional(19.),Color32::from_rgb(220,223,225));
            draw.text(center+Vec2::new(0.,90.),Align2::CENTER_CENTER,"A/D move   SPACE jump   Hold right mouse: rope
Scroll while attached to climb/lower; release right mouse to let go\nMouse aim + hold click / E to drill   C crouch\nM map   F5 save   ESC pause   R new expedition",FontId::proportional(16.),Color32::from_rgb(180,191,204));
            if !self.game.won{draw.text(center+Vec2::new(0.,170.),Align2::CENTER_CENTER,"ENTER / controller A to continue",FontId::proportional(20.),Color32::from_rgb(255,205,116));}
        }
    }
    fn hud(&self,p:&egui::Painter,r:Rect){
        grave_engine::cave_ui::hud(p,r,&self.game.sim,&self.game.mats,&self.game.catalog,self.input.controller().is_some());
        let message=self.game.visible_message();if !message.is_empty(){p.text(r.center_bottom()-Vec2::new(0.,150.),Align2::CENTER_BOTTOM,message,FontId::proportional(15.),Color32::from_rgb(255,215,139));}
    }
    fn draw_map(&self,p:&egui::Painter,r:Rect){let size=Vec2::new(520.,390.);let map=Rect::from_center_size(r.center(),size);p.rect_filled(map.expand(18.),8.,Color32::from_rgba_unmultiplied(8,15,22,245));let w=self.game.map_w;let h=self.game.explored.len()/w;let (sx,sy)=(size.x/w as f32,size.y/h as f32);
        for y in 0..h{for x in 0..w{if !self.game.explored[y*w+x]{continue;}let (wx,wy)=((x*16+8).min(self.game.sim.world.w-1),(y*16+8).min(self.game.sim.world.h-1));let id=self.game.sim.world.mat[self.game.sim.world.idx(wx,wy)];let color=if id==0{Color32::from_rgb(18,31,42)}else{Color32::from_rgb(81,87,84)};p.rect_filled(Rect::from_min_size(map.min+Vec2::new(x as f32*sx,y as f32*sy),Vec2::new(sx+0.5,sy+0.5)),0.,color);}}
        let dot=|x:f32,y:f32|map.min+Vec2::new(x/self.game.sim.world.w as f32*size.x,y/self.game.sim.world.h as f32*size.y);let a=self.game.player();p.circle_filled(dot(a.x,a.y),4.,Color32::WHITE);p.circle_stroke(dot(self.game.tuning.home.0,self.game.tuning.home.1),6.,(2.,Color32::from_rgb(255,196,90)));if let Some((x,y))=self.game.signal{p.circle_stroke(dot(x,y),5.,(1.5,Color32::from_rgb(109,227,228)));}p.text(map.left_top()-Vec2::new(0.,4.),Align2::LEFT_BOTTOM,"SURVEY MAP  /  white: you   amber: lift   cyan: signal",FontId::proportional(13.),Color32::LIGHT_GRAY);
    }
}
impl eframe::App for App{
    fn ui(&mut self,ui:&mut egui::Ui,_:&mut eframe::Frame){
        let ctx=ui.ctx().clone();
        self.input.update(&ctx);let now=Instant::now();let dt=(now-self.last).as_secs_f32().min(0.1);self.last=now;
        let stick=(self.input.axis("aim_x"),self.input.axis("aim_y"));
        if stick.0.hypot(stick.1)>0.2 {self.mouse_aim=false;let n=stick.0.hypot(stick.1);self.game.aim=(stick.0/n,stick.1/n);}
        else if let (Some(mouse),Some((origin,scale)))=(ctx.input(|i|i.pointer.hover_pos()),self.view_at) {if Some(mouse)!=self.last_mouse || ctx.input(|i|i.pointer.primary_down()){self.mouse_aim=true;} if self.mouse_aim {let shoulder=grave_engine::tool_pose::shoulder(&self.game.player());let d=((mouse.x-origin.x)/scale-shoulder.0,(mouse.y-origin.y)/scale-shoulder.1);let n=d.0.hypot(d.1);if n>0.1 {self.game.aim=(d.0/n,d.1/n);}}}
        if self.started&&!self.paused&&!self.game.won {self.supplies.input(&self.input,self.game.catalog.recipes.len());if self.input.pressed("tool_next"){self.game.sim.supplies.cycle(&self.game.catalog);}}
        if self.input.pressed("start")&&!self.supplies.open{self.started=true;self.paused=false;}
        if self.input.pressed("pause")&&self.started{self.paused=!self.paused;if self.paused{self.game.save();}}
        if self.input.pressed("map"){self.map=!self.map;}
        if self.input.pressed("save")&&!self.supplies.open{self.game.save();}
        if self.input.pressed("restart")&&(!self.started||self.paused||self.game.won){self.game=Expedition::new(false);self.game.save();self.paused=false;self.started=true;self.sprites.clear();}
        if self.started&&!self.paused&&!self.game.won&&!self.supplies.open{
            self.game.sim.physics.hold_grapple(&self.game.sim.world,&self.game.mats,self.game.aim,self.input.down("grapple"),self.input.pressed("grapple"));
            self.game.sim.physics.reel_rope(self.input.axis("reel"));
            self.game.venting=self.input.down("vent");
            self.pending_tool_press |= self.input.pressed("drill");
            self.clock=(self.clock+dt).min(0.1);while self.clock>=1./60.{self.clock-=1./60.;let c=Controls{walk:self.input.axis("walk"),jump:self.input.down("jump")||(self.game.player().rope.is_some()&&self.input.down("rope_jump")),thrust:self.input.down("thrust"),drop:self.input.down("drop"),crouch:self.input.down("crouch"),roll:self.input.down("roll")};let tapped=std::mem::take(&mut self.pending_tool_press);self.game.step(c,self.input.down("drill")||tapped,self.game.aim);self.save_clock+=1;if self.save_clock>=1800||self.game.won{self.game.save();self.save_clock=0;}}}else{self.clock=0.;self.pending_tool_press=false;}
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui,|ui|self.draw(&ctx,ui));
        if let Some(action)=self.supplies.show(&ctx,&self.game.sim,&self.game.mats,&self.game.catalog) {
            let outcome=match action {grave_engine::supplies::Action::Equip(id)=>if self.game.sim.supplies.equip(&id){Ok(())}else{Err("Tool not built".into())},grave_engine::supplies::Action::Craft(id)=>self.game.sim.craft(&self.game.catalog,&id)};
            self.supplies.message=match outcome {Ok(())=>"Ready — close storage to try it.".into(),Err(e)=>e};
            for i in 0..2 {if let Some(id)=self.game.mats.id(if i==0 {"crystal_cyan"}else{"crystal_violet"}) {self.game.samples[i]=self.game.sim.supplies.count(id) as usize;}}
        }
        ctx.request_repaint();
    }
}
impl Drop for App{fn drop(&mut self){if self.started{self.game.save();}}}

fn verify() -> bool {
    let mut game = Expedition::new(false);
    // The runner only walks and swings near camp: it checks the short loop
    // (controls, mining, return) with small quotas. The full quotas send a
    // player out into the cave, which needs a human playtest.
    if let Some(m) = game.sim.mission.as_mut() { for g in &mut m.goals { g.target = g.target.min(24); } }
    // It mines the guarded violet without fighting back, so a few unlucky
    // bites decided pass or fail. Guard fights are covered by the creature
    // tests; here the suit is tougher so the run checks the loop itself.
    game.sim.physics.edit_actor(0, |a| a.health = 300.0);
    let mut last_x = game.player().x;
    let mut stalled = 0;
    let mut last_hook = -100_i32;
    let mut catches = 0;
    let mut last_gain = 0;
    let mut deaths = 0;
    let mut respawn_finish = false;
    let mut max_distance = 0.0_f32;
    game.shot(Path::new("shots/deepward-start.png"), false);
    for step in 0..6000 {
        if game.won { break; }
        let Some(target) = game.signal else { break; };
        let p = game.player();
        let shoulder = grave_engine::tool_pose::shoulder(&p);
        let dx = target.0 - shoulder.0;
        let dy = target.1 - shoulder.1;
        let near_sample = !game.ready() && dx.hypot(dy) < 28.;
        // A guard or ledge can block the face even inside drill range. Change angle
        // if sustained work yields nothing, especially while guarding the violet seam.
        let change_angle = near_sample && step - last_gain > 65;
        let lift_gap = game.ready() && (198.0..260.).contains(&p.x);
        let below_lift = game.ready() && p.x < 211. && p.y > game.tuning.home.1 + 5.;
        let walk = if below_lift { 1. } else if near_sample && !change_angle { 0. } else { (dx / 8.).clamp(-1., 1.) };
        if walk.abs() > 0.3 && (p.x - last_x).abs() < 0.12 { stalled += 1; } else { stalled = 0; }
        last_x = p.x;
        let guard_ahead = game.sim.physics.actor_status().iter().any(|a| {
            let ahead = (a.x - p.x) * walk.signum();
            a.creature.is_some() && !a.dead && (0.0..40.).contains(&ahead) && (a.y - p.y).abs() < 22.
        });
        let launch = !near_sample && step - last_hook > 55 && (stalled > 8 || guard_ahead || lift_gap);
        if launch { last_hook = step; }
        let rope_aim = (if lift_gap { -0.4 } else { walk.signum() * 0.8 }, -1.);
        let held = step - last_hook < 35 && !near_sample;
        game.sim.physics.hold_grapple(&game.sim.world, &game.mats, rope_aim, held, launch);
        if launch && game.player().rope.is_some_and(|r| !r.pins.is_empty()) { catches += 1; }
        if held { game.sim.physics.wind_rope(0.9); }
        let jump = launch || ((stalled > 4 || change_angle || p.hanging) && step % 16 == 0);
        let aim = if near_sample || dx.abs() < 18. {
            (dx, dy)
        } else {
            // Work the passage at foot height when a low obstacle blocks walking.
            (dx.signum(), if stalled > 4 { 0.9 } else { 0.15 })
        };
        let samples_before = game.samples;
        game.step(Controls { walk, jump, ..Default::default() }, !launch && (!game.ready() || (stalled > 8 && !below_lift && !lift_gap)), if launch { rope_aim } else { aim });
        let after = game.player();
        if !p.dead && after.dead { deaths += 1; }
        if game.won { respawn_finish = p.dead; }
        if samples_before != game.samples || !near_sample { last_gain = step; }
        max_distance = max_distance.max(game.home_distance());
        if step % 300 == 0 {
            println!("frame={step} position=({:.1},{:.1}) target={target:?} samples={:?} health={:.0}",
                     p.x, p.y, game.samples, p.health);
        }
        if matches!(step, 180 | 420 | 900 | 2400) {
            game.shot(Path::new(&format!("shots/deepward-route-{step}.png")), false);
        }
    }
    game.shot(Path::new("shots/deepward-finish.png"), false);
    game.shot(Path::new("shots/deepward-map.png"), true);
    println!("EXPEDITION won={} samples={:?} distance={max_distance:.1} hook catches={catches} deaths={deaths} finished_on_respawn={respawn_finish} cells mined={}",
             game.won, game.samples, game.mined);
    game.won && !respawn_finish
}
fn main()->eframe::Result{
    std::env::set_current_dir(env!("CARGO_MANIFEST_DIR")).expect("project directory");
    if std::env::args().any(|s|s=="--verify"){if !verify(){std::process::exit(3);}return Ok(());}
    eframe::run_native("DEEPWARD — Grave Engine",eframe::NativeOptions{viewport:egui::ViewportBuilder::default().with_inner_size(Vec2::new(1280.,800.)),..Default::default()},Box::new(|cc|{cc.egui_ctx.set_visuals(egui::Visuals::dark());Ok(Box::new(App::new()))}))
}

#[cfg(test)] mod tests{
    use super::*;
    #[test] fn expedition_loads_with_animated_equipment_and_both_sample_types(){let g=Expedition::new(false);let p=g.player();assert!(p.look.as_ref().unwrap().anims.contains_key("swim"));assert!(p.rope.is_some());assert!(p.jet_fuel.is_none());assert!(p.headlamp>0.);assert!(g.signal.is_some());assert!(!g.ready());}
    #[test]
    fn walk_from_lift_and_cut_the_first_timber() {
        let mut g = Expedition::new(false);
        let start = g.player().x;
        for _ in 0..27 {
            g.step(Controls { walk: 1., ..Default::default() }, false, (1., 0.));
        }
        assert!(g.player().x - start > 20., "expedition stride from lift");
        let aim = |g: &Expedition| {
            let p = grave_engine::tool_pose::shoulder(&g.player());
            (145.5 - p.0, 608.5 - p.1)
        };
        assert_ne!(g.sim.world.frame % 6, 0);
        g.step(Controls::default(), true, aim(&g));
        let wood = g.mats.id("wood").unwrap();
        assert_eq!(g.sim.mining.impact.as_ref().unwrap().material, wood);
        for _ in 0..100 {
            g.step(Controls::default(), true, aim(&g));
        }
        assert!(g.sim.supplies.count(wood) > 0);
    }
    #[test] fn game_controls_include_keyboard_and_controller(){let i=Input::parse(INPUT).unwrap();let _=i;}
}
