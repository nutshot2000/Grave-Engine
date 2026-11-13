//! Real ledge traversal, obstruction, release and snapshot regression tests.
use grave_engine::{scene, Materials, Sim};
use grave_engine::physics::Controls;

const BASE: &str = "size 240 170\nfill air\nrect stone 0 150 240 20\nrect concrete 90 126 55 24\nplayer 65 150 climb 1.2 reach 6\n";
fn setup(text: &str) -> (Materials, Sim) {
    let m = Materials::load_dir("materials");
    let (s,e) = scene::load(text,&m,None).into_sim(&m);
    assert!(e.is_empty(),"{e:?}"); (m,s)
}
fn run(s: &mut Sim, m: &Materials, n: usize, walk: f32, up: bool, drop: bool) {
    s.physics.set_controls(Controls { walk, jump: up, drop });
    for _ in 0..n { s.step(m); }
}
fn grab(s: &mut Sim, m: &Materials) {
    for _ in 0..100 {
        run(s,m,1,1.0,true,false);
        if s.physics.actor_status()[0].hanging { return; }
    }
    panic!("did not grab: {:?}",s.physics.actor_status()[0]);
}

#[test]
fn grab_hold_press_again_and_stand_on_top() {
    let (m,mut s) = setup(BASE);
    grab(&mut s,&m);
    let first = s.physics.actor_status()[0].clone();
    assert_eq!(first.animation_state,"hang");
    run(&mut s,&m,30,1.0,true,false);
    let held = &s.physics.actor_status()[0];
    assert!(held.hanging && !held.climbing, "held up must not auto-climb");
    assert!((held.y-first.y).abs()<0.1);
    run(&mut s,&m,1,0.0,false,false);
    run(&mut s,&m,1,0.0,true,false);
    assert!(s.physics.actor_status()[0].climbing);
    assert_eq!(s.physics.actor_status()[0].animation_state,"climb");
    run(&mut s,&m,100,0.0,false,false);
    let a=&s.physics.actor_status()[0];
    assert!(!a.hanging && !a.climbing);
    assert!(a.grounded && a.x>92.0 && (a.y+8.0-126.0).abs()<1.0,"{a:?}");
    assert_eq!(a.health,100.0,"climbing must not count as a hard impact");
}

#[test]
fn drop_releases_and_does_not_immediately_grab_again() {
    let (m,mut s)=setup(BASE); grab(&mut s,&m);
    run(&mut s,&m,1,0.0,true,true);
    assert!(!s.physics.actor_status()[0].hanging);
    assert!(s.physics.export_actors()[0].grab_cooldown>0);
    run(&mut s,&m,15,0.0,true,false);
    assert!(!s.physics.actor_status()[0].hanging);
}

#[test]
fn blocked_headroom_and_unreachable_walls_do_not_allow_climbs() {
    for extra in ["rect stone 82 102 60 12\n", "rect concrete 90 65 55 85\n"] {
        let (m,mut s)=setup(&(BASE.to_owned()+extra));
        run(&mut s,&m,120,1.0,true,false);
        assert!(s.physics.export_actors()[0].ledge.is_none());
        assert!(s.physics.actor_status()[0].x<91.0);
    }
}

#[test]
fn destroyed_support_and_new_obstructions_release_a_hanging_actor() {
    for remove in [true,false] {
        let (m,mut s)=setup(BASE); grab(&mut s,&m);
        let h=s.physics.export_actors()[0].ledge.unwrap();
        if remove { s.world.make_air(s.world.idx(h.edge.0 as usize,h.edge.1 as usize)); }
        else {
            let id=m.id("stone").unwrap();
            for y in 105..116 { for x in 83..102 { s.world.set(&m,x,y,id); } }
            run(&mut s,&m,1,0.0,false,false);
            run(&mut s,&m,1,0.0,true,false);
        }
        run(&mut s,&m,3,0.0,false,false);
        // A new overhead obstruction keeps the actor hanging, instead of
        // teleporting through it; missing support always drops the actor.
        if remove { assert!(!s.physics.actor_status()[0].hanging); }
        else { assert!(!s.physics.actor_status()[0].climbing); }
    }
}

#[test]
fn traversal_restores_mid_climb_and_scene_keeps_settings() {
    let (m,mut s)=setup(BASE); grab(&mut s,&m);
    run(&mut s,&m,1,0.0,false,false); run(&mut s,&m,1,0.0,true,false);
    run(&mut s,&m,20,0.0,false,false);
    let snap=s.snapshot();
    let mut restored=Sim::restore(&snap,&m);
    assert_eq!(s.physics.export_actors()[0].ledge,restored.physics.export_actors()[0].ledge);
    run(&mut s,&m,100,0.0,false,false); run(&mut restored,&m,100,0.0,false,false);
    let a=&s.physics.actor_status()[0];let b=&restored.physics.actor_status()[0];
    assert!((a.x-b.x).abs()<0.01 && (a.y-b.y).abs()<0.01);
    let text=scene::save(&snap,&m);
    let (loaded,e)=scene::load(&text,&m,None).into_sim(&m);
    assert!(e.is_empty(),"{e:?}");
    assert_eq!(loaded.physics.export_actors()[0].climb,snap.actors[0].climb);
}

#[test]
fn traversal_is_opt_in_and_works_facing_left() {
    let (m,mut s)=setup(&BASE.replace(" climb 1.2 reach 6",""));
    run(&mut s,&m,100,1.0,true,false);
    assert!(!s.physics.actor_status()[0].hanging);
    let (m,mut s)=setup(&BASE.replace("player 65 150", "player 165 150"));
    for _ in 0..100 {
        run(&mut s,&m,1,-1.0,true,false);
        if s.physics.actor_status()[0].hanging { break; }
    }
    assert!(s.physics.actor_status()[0].hanging);
    run(&mut s,&m,1,0.0,false,false);run(&mut s,&m,1,0.0,true,false);
    run(&mut s,&m,100,0.0,false,false);
    let a=&s.physics.actor_status()[0];
    assert!(a.grounded && a.x<144.0 && (a.y+8.0-126.0).abs()<1.0,"{a:?}");
}

#[test]
fn all_four_roofs_in_the_playable_course_are_reachable() {
    let (m,mut s)=setup(include_str!("../scenes/climbing-lab.txt"));
    for roof in 0..4 {
        if roof>0 {
            let launch=[0.0,150.0,248.0,338.0][roof];
            for _ in 0..250 {
                if s.physics.actor_status()[0].x>=launch { break; }
                run(&mut s,&m,1,1.0,false,false);
            }
        }
        grab(&mut s,&m);
        run(&mut s,&m,1,0.0,false,false);
        run(&mut s,&m,1,0.0,true,false);
        run(&mut s,&m,150,0.0,false,false);
        let p=&s.physics.actor_status()[0];
        assert!(p.grounded && p.health==100.0,"roof {}: {p:?}",roof+1);
        assert!((p.y+8.0-[212.0,188.0,162.0,138.0][roof]).abs()<1.0);
    }
}

#[test]
fn the_crouched_climb_picture_contacts_the_roof_at_the_end() {
    use grave_engine::animation::{Animator,Graph};
    use grave_engine::look::{Look,Frame,Anim};
    use grave_engine::physics::{ActorSpec,ClimbSettings,LedgeHold};
    use std::sync::Arc;
    let graph=Arc::new(Graph::parse("initial climb\nstate climb clip climb once speed 1\n").unwrap());
    let frame=|| {
        let mut rgba=vec![0;32*32*4];
        for y in 2..=20 { for x in 15..=17 { rgba[(y*32+x)*4+3]=255; } }
        Frame{w:32,h:32,rgba,mask:vec![false;320]}
    };
    let look=Arc::new(Look{graph:graph.clone(),name:"crouched-fixture".into(),dir:Default::default(),figure:16.0,feet:30.0,faces_right:true,anims:[("hang".into(),Anim{frames:vec![frame()],fps:1.0,looping:true}),("climb".into(),Anim{frames:vec![frame()],fps:1.0,looping:false})].into()});
    let mut a=ActorSpec::standing_at(0,0.0,0.0);
    a.look=Some(look);a.animation=Some(Animator::new(graph).unwrap());
    a.climb=Some(ClimbSettings{reach:6.0,seconds:2.0});
    for side in [-1.0,1.0] {
        a.facing=side;
        for elapsed in [1.6,1.8,1.99] {
            a.ledge=Some(LedgeHold{edge:(90,126),side,climbing:true,elapsed});
            let offset=a.visual_offset();
            let visible_bottom=125.75+(21.0-30.0)+offset.1;
            assert!((visible_bottom-126.0).abs()<0.001,"end pose floats: {visible_bottom}");
        }
    }
}

#[test]
fn successful_pullup_can_play_a_finish_clip_while_dropping_skips_it() {
    use grave_engine::animation::Graph;
    use std::sync::Arc;
    let graph=Arc::new(Graph::parse("initial idle
param hanging 0
param climbing 0
trigger climb_done
trigger release
state idle clip idle loop speed 1
state hang clip idle loop speed 1
state climb clip jump once speed 1
state finish clip idle once speed 1 duration 0.4
transition * hang priority 98 after 0 when hanging > 0
transition * climb priority 98 after 0 when climbing > 0
transition climb finish priority 99 after 0 when climb_done > 0
transition hang idle priority 97 after 0 when hanging == 0 & climbing == 0
transition climb idle priority 97 after 0 when hanging == 0 & climbing == 0
transition finish idle priority 0 after 0 when finished > 0
").unwrap());
    let (m,mut s)=setup(BASE);
    s.physics.set_actor_graph(0,graph.clone()).unwrap();grab(&mut s,&m);
    run(&mut s,&m,1,0.0,false,false);run(&mut s,&m,1,0.0,true,false);
    for _ in 0..100 {
        if !s.physics.actor_status()[0].climbing { break; }
        run(&mut s,&m,1,0.0,false,false);
    }
    assert_eq!(s.physics.actor_status()[0].animation_state,"finish");
    run(&mut s,&m,10,0.0,false,false);
    assert_eq!(s.physics.actor_status()[0].animation_state,"finish");
    run(&mut s,&m,20,0.0,false,false);
    assert_eq!(s.physics.actor_status()[0].animation_state,"idle");
    let (m,mut s)=setup(BASE);
    s.physics.set_actor_graph(0,graph).unwrap();grab(&mut s,&m);
    run(&mut s,&m,1,0.0,false,true);
    assert_eq!(s.physics.actor_status()[0].animation_state,"idle");
}
