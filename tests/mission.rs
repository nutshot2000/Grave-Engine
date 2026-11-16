//! Shared collection rules and persistent editor test-run state.
use grave_engine::{Materials,Sim,scene,edit::{History,Timeline}};
fn setup()->(Materials,Sim){let m=Materials::load_dir("materials");let(s,p)=scene::load("size 180 120\nfill air\nrect rock 0 100 180 20\nrect crystal_cyan 80 72 4 10\nrect crystal_violet 120 72 4 10\nplayer 30 100\nmission 30 92 24\nsample crystal_cyan 8 0\nsample crystal_violet 8 0\n",&m,None).into_sim(&m);assert!(p.is_empty(),"{p:?}");(m,s)}
#[test]
fn collection_requires_actual_mining_and_both_samples_before_return(){
 let(m,mut s)=setup();s.step(&m);assert!(!s.mission.as_ref().unwrap().complete);
 for x in [82.,122.] {let removed=s.dig_circle(&m,x,77.,8.5,8000.);s.mission.as_mut().unwrap().collect(&removed);}
 assert!(s.mission.as_ref().unwrap().ready());assert!(!s.mission.as_ref().unwrap().complete);
 s.step(&m);assert!(s.mission.as_ref().unwrap().complete);
 let before=s.mission.clone();let removed=s.dig_circle(&m,82.,77.,8.5,8000.);s.mission.as_mut().unwrap().collect(&removed);assert_eq!(s.mission,before);
}
#[test]
fn progress_follows_save_reset_rewind_and_undo(){
 let(m,mut s)=setup();let start=s.snapshot();let mut t=Timeline::new();t.every=1;t.record(&mut s);
 let removed=s.dig_circle(&m,82.,77.,8.5,8000.);s.mission.as_mut().unwrap().collect(&removed);s.step(&m);t.record(&mut s);
 let collected=s.mission.clone();assert!(collected.as_ref().unwrap().goals[0].collected>=8);
 let text=scene::save(&s.snapshot(),&m);let(saved,p)=scene::load(&text,&m,None).into_sim(&m);assert!(p.is_empty());assert_eq!(saved.mission,collected);
 assert!(t.restore(0,&mut s,&m));assert_eq!(s.mission,start.mission);assert!(t.restore(1,&mut s,&m));assert_eq!(s.mission,collected);
 s=Sim::restore(&start,&m);assert_eq!(s.mission.as_ref().unwrap().goals[0].collected,0);
 let mut history=History::new();history.touch(&mut s,"mission target");s.mission.as_mut().unwrap().goals[0].target=32;history.commit(&mut s);history.undo(&mut s,&m);assert_eq!(s.mission,start.mission);
 s.shift(-32,16);assert_eq!(s.mission.as_ref().unwrap().home,(-2.,108.));
}
