//! Mission route verification uses real ammunition and hover/service rules.
#[allow(dead_code)]
#[path = "../src/dust_hawk/mission.rs"]
mod mission;
use grave_engine::{
    Materials,
    overhead::{FlightInput, Point},
};
use mission::{Mission, Phase};
fn wait(g: &mut Mission, m: &Materials, n: usize) {
    for _ in 0..n {
        g.step(FlightInput::default(), false, false, m);
        assert_ne!(g.phase, Phase::Lost, "{} armour {}", g.toast, g.health());
    }
}
fn relocate(g: &mut Mission, p: Point) {
    g.craft.at = p;
    g.craft.velocity = Point::default();
    g.combat.shots.retain(|s| s.faction == 1);
}
fn service(g: &mut Mission, m: &Materials) {
    let p = g.base;
    relocate(g, Point::new(p.x, p.y, 9.));
    wait(g, m, 200);
}
#[test]
fn red_mesa_completes_through_real_combat_rescues_and_lz_service() {
    let m = Materials::load_dir("materials");
    let mut g = Mission::new(&m);
    g.start();
    // Teleport only between encounters to keep this a bounded regression, then
    // run actual projectiles, material destruction, enemy fire and mission ticks.
    for id in [10, 11, 12, 13] {
        let t = g.combat.target(id).unwrap().bounds.centre();
        relocate(
            &mut g,
            Point::new((t.x - 20.).max(8.), t.y, (t.z + 22.).min(77.)),
        );
        g.selected = 2;
        g.aim = t;
        g.lock_target = Some(id);
        g.lock = 1.;
        for attempt in 0..4 {
            g.cooldown[2] = 0.;
            g.fire(false);
            wait(&mut g, &m, 35);
            if !g.alive(id) {
                break;
            }
            assert!(
                attempt < 3,
                "target {id} still has {} armour",
                g.combat.target(id).unwrap().health
            );
        }
        assert!(!g.alive(id));
        service(&mut g, &m);
    }
    assert_eq!(g.remaining_radars(), 0);
    for i in 0..g.rescues.len() {
        let p = g.rescues[i].at;
        relocate(&mut g, Point::new(p.x, p.y, p.z + 20.));
        wait(&mut g, &m, 95);
        assert!(
            g.rescues[i].collected,
            "rescue {} failed at {:?}",
            g.rescues[i].id, g.craft.at
        );
        service(&mut g, &m);
    }
    assert_eq!(g.rescued, 6);
    assert_eq!(g.phase, Phase::Won);
    assert!(g.score > 2000);
    println!(
        "Mission won through actual weapons and hover rescues; score {}, time {:.1}s",
        g.score, g.time
    );
}
#[test]
fn winch_drop_then_cannon_lights_the_native_oil_spill() {
    use grave_engine::overhead::cargo::CargoState;
    use grave_engine::world::BURNING;
    let m = Materials::load_dir("materials");
    let mut g = Mission::new(&m);
    g.start();
    relocate(&mut g, Point::new(50., 113., 31.));
    g.hook(&m);
    assert_eq!(g.held, Some(80));
    assert!(g.barrels[0].taken);
    let oil = m.id("oil").unwrap();
    assert!(g.cargo[0].cells.iter().any(|(_, c)| c.mat == oil));
    g.cargo[0].at = Point::new(115., 78., 43.);
    g.hook(&m);
    assert_eq!(g.held, None);
    for _ in 0..100 {
        g.cargo[0].step(&mut g.world, &m, None, 1. / 60.);
        if g.cargo[0].state == CargoState::Deployed {
            break;
        }
    }
    assert_eq!(g.cargo[0].state, CargoState::Deployed);
    let mut spill = None;
    for y in 70..90 {
        for z in 2..14 {
            for x in 105..130 {
                let p = Point::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                if g.world.material_at(p) == oil {
                    spill = Some(p);
                }
            }
        }
    }
    let p = spill.expect("barrel didn't leave native oil");
    g.combat
        .shots
        .push(mission::CANNON.launch(Point::new(p.x, p.y, p.z + 7.), p, 1, None));
    for _ in 0..5 {
        g.combat.step(&mut g.world, &m, 1. / 60.);
    }
    g.world.step(&m);
    assert!(
        g.world
            .sections
            .iter()
            .any(|s| s.world.flags.iter().any(|f| f & BURNING != 0)),
        "cannon failed to ignite the dropped oil"
    );
}
#[test]
fn untouched_installations_stay_intact_without_player_fire(){
 let m=Materials::load_dir("materials");let mut g=Mission::new(&m);g.start();
 for _ in 0..300 {g.step(FlightInput::default(),false,false,&m);}
 for u in &g.units{let t=g.combat.target(u.id).unwrap();assert!(t.health>t.max_health*0.85,"{} fell apart before combat: {}/{}",u.name,t.health,t.max_health);}
}
