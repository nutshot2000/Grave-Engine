use grave_engine::{scene, Materials, Sim};
use grave_engine::physics::Controls;

fn setup(extra: &str) -> (Materials, Sim) {
    let m = Materials::load_dir("materials");
    let text = format!("size 340 170\nfill air\nrect stone 0 150 340 20\nplayer 40 150 moves\n{extra}");
    let (mut s,e) = scene::load(&text,&m,None).into_sim(&m);
    assert!(e.is_empty(),"{e:?}");
    run(&mut s,&m,Controls::default(),20); (m,s)
}
fn run(s: &mut Sim,m: &Materials,c: Controls,n: usize) {
    s.physics.set_controls(c); for _ in 0..n { s.step(m); }
}
#[test]
fn crouch_crawls_under_beam_and_only_stands_when_clear() {
    let (m,mut s)=setup("rect concrete 85 133 95 6\nanchor 85 133 95 6\n");
    run(&mut s,&m,Controls{walk:1.0,..Default::default()},70);
    assert!(s.physics.actor_status()[0].x<85.0,"standing capsule must hit beam");
    run(&mut s,&m,Controls{walk:1.0,crouch:true,..Default::default()},170);
    let a=&s.physics.actor_status()[0];assert!(a.x>100.0 && a.x<180.0,"{a:?}");
    assert!(a.crouched && a.body_height<=9.01 && a.health==100.0,"{a:?}");
    run(&mut s,&m,Controls::default(),30);
    let a=&s.physics.actor_status()[0];assert!(a.crouched&&a.stand_blocked&&a.body_height<=9.01,"{a:?}");
    run(&mut s,&m,Controls{walk:1.0,..Default::default()},300);
    let a=&s.physics.actor_status()[0];assert!(a.x>184.0&&!a.crouched&&a.body_height==16.0,"{a:?}");
}
#[test]
fn roll_moves_in_locked_direction_and_held_key_does_not_repeat() {
    let (m,mut s)=setup("");let x=s.physics.actor_status()[0].x;
    run(&mut s,&m,Controls{roll:true,..Default::default()},10);
    assert!(s.physics.actor_status()[0].rolling);
    run(&mut s,&m,Controls{roll:true,walk:-1.0,..Default::default()},25);
    let a=&s.physics.actor_status()[0];assert!(a.x>x+28.0&&a.facing==1.0,"{a:?}");
    assert!(a.rolling && a.grounded && (a.y+8.0-150.0).abs()<0.2,"terrain seams must not launch a roll: {a:?}");
    run(&mut s,&m,Controls{roll:true,..Default::default()},90);
    assert!(!s.physics.actor_status()[0].rolling);
    assert_eq!(s.physics.actor_status()[0].body_height,16.0);
    run(&mut s,&m,Controls::default(),1);
    run(&mut s,&m,Controls{roll:true,walk:-1.0,..Default::default()},1);
    assert!(s.physics.actor_status()[0].rolling&&s.physics.actor_status()[0].facing==-1.0);
}
#[test]
fn roll_and_crouch_survive_snapshot_with_matching_collision_height() {
    let (m,mut s)=setup("");
    run(&mut s,&m,Controls{roll:true,crouch:true,..Default::default()},20);
    let snap=s.snapshot();let mut restored=Sim::restore(&snap,&m);
    assert_eq!(s.physics.export_actors()[0].posture,restored.physics.export_actors()[0].posture);
    for sim in [&mut s,&mut restored] { run(sim,&m,Controls{crouch:true,..Default::default()},80); }
    let a=&s.physics.actor_status()[0];let b=&restored.physics.actor_status()[0];
    assert!((a.x-b.x).abs()<0.5&&a.crouched&&b.crouched,"{a:?} / {b:?}");
    let text=scene::save(&snap,&m);assert!(text.contains(" moves"));
    let loaded=scene::load(&text,&m,None);assert!(loaded.problems.is_empty());assert!(loaded.actors[0].mobility.is_some());
}
#[test]
fn airborne_roll_is_rejected_and_wall_stops_roll_without_damage() {
    let (m,mut s)=setup("rect stone 88 110 10 40\n");
    run(&mut s,&m,Controls{roll:true,..Default::default()},80);
    let a=&s.physics.actor_status()[0];assert!(a.x<88.0&&!a.rolling&&a.health==100.0,"{a:?}");
    run(&mut s,&m,Controls{jump:true,..Default::default()},5);
    run(&mut s,&m,Controls{roll:true,..Default::default()},1);
    assert!(!s.physics.actor_status()[0].rolling);
}

#[test]
fn walking_off_a_ledge_cancels_roll_instead_of_following_the_floor() {
    let m=Materials::load_dir("materials");
    let (mut s,e)=scene::load("size 240 180\nfill air\nrect stone 0 165 240 15\nrect stone 0 115 85 50\nplayer 73 115 moves\n",&m,None).into_sim(&m);
    assert!(e.is_empty());run(&mut s,&m,Controls::default(),20);
    run(&mut s,&m,Controls{roll:true,..Default::default()},35);
    let a=&s.physics.actor_status()[0];assert!(!a.rolling&&!a.grounded&&a.y+8.0>117.0,"{a:?}");
}

#[test]
fn mobility_settings_roundtrip_and_reject_invalid_heights() {
    let m=Materials::load_dir("materials");
    let text="size 80 80\nplayer 20 60 moves crouch_height 10 roll_height 7 crouch_speed 1.2 roll_speed 4.8 roll_seconds 0.9\n";
    let (mut s,e)=scene::load(text,&m,None).into_sim(&m);assert!(e.is_empty(),"{e:?}");
    let saved=scene::save(&s.snapshot(),&m);let reload=scene::load(&saved,&m,None);
    assert!(reload.problems.is_empty());assert_eq!(reload.actors[0].mobility,s.physics.export_actors()[0].mobility);
    assert!(!scene::load(&text.replace("crouch_height 10","crouch_height 3"),&m,None).problems.is_empty());
}
