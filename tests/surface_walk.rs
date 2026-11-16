//! Surface locomotion driven through real actor physics, independent of AI choices.
use grave_engine::{Materials, Sim, physics::Controls, scene};
fn setup(text: &str) -> (Materials, Sim) {
    let m = Materials::load_dir("materials");
    let (s, p) = scene::load(text, &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    s.physics.actor_status().first().unwrap();
    (m, s)
}
const ROOM: &str = "size 180 140\nfill bedrock\nrect air 20 20 120 80\nanchor 0 0 180 140\ncreature skitter 50 100\n";
fn step(s: &mut Sim, m: &Materials) {
    s.world.step(m);
    s.physics.update(&mut s.world, m);
}
#[test]
fn a_crawler_rounds_the_room_on_floor_wall_ceiling_and_opposite_wall() {
    let (m, mut s) = setup(ROOM);
    s.physics.set_actor_controls(
        0,
        Some(Controls {
            walk: 1.,
            crouch: true,
            ..Default::default()
        }),
    );
    let mut faces = [false; 4];
    for _ in 0..1100 {
        step(&mut s, &m);
        let a = s.physics.actor_status().remove(0);
        assert_eq!(
            a.health, 36.,
            "normal surface traversal must not injure a crawler"
        );
        let n = a.surface.unwrap().normal;
        if n.1 < -0.85 {
            faces[0] = true;
        }
        if n.0 < -0.85 {
            faces[1] = true;
        }
        if n.1 > 0.85 {
            faces[2] = true;
        }
        if n.0 > 0.85 {
            faces[3] = true;
        }
        let c = (a.x, a.y + (16. - a.body_height) * 0.5);
        assert!(
            c.0 >= 20. && c.0 <= 140. && c.1 >= 20. && c.1 <= 100.,
            "must stay in the cave: {c:?}"
        );
    }
    assert!(
        faces.iter().all(|v| *v),
        "visited faces: {faces:?}, actor {:?}",
        s.physics.actor_status()[0]
    );
    let mut r = grave_engine::render::Renderer::default();
    r.light(&s);
    grave_engine::headless::shot_with_actors(
        &s.world,
        &m,
        &mut r,
        &s.physics.actor_status(),
        None,
        std::path::Path::new("shots/surface-walk-room.png"),
        4,
        false,
    )
    .unwrap();
}
#[test]
fn convex_ledge_corners_are_followed_instead_of_turning_into_flight() {
    let (m, mut s) = setup(
        "size 180 140\nfill air\nrect bedrock 0 100 180 40\nrect bedrock 65 65 20 35\nanchor 0 0 180 140\ncreature skitter 40 100\n",
    );
    s.physics.set_actor_controls(
        0,
        Some(Controls {
            walk: 1.,
            crouch: true,
            ..Default::default()
        }),
    );
    let mut climbed = false;
    let mut crossed = false;
    for _ in 0..550 {
        step(&mut s, &m);
        let a = s.physics.actor_status().remove(0);
        climbed |= a.y < 65.;
        crossed |= a.x > 95.;
    }
    assert!(
        climbed && crossed,
        "must climb and descend a connected pillar, climbed={climbed} crossed={crossed}, {:?}",
        s.physics.actor_status()[0]
    );
}
#[test]
fn removing_ceiling_support_detaches_the_crawler_and_it_falls() {
    let (m, mut s) = setup(ROOM);
    s.physics.set_actor_controls(
        0,
        Some(Controls {
            walk: 1.,
            crouch: true,
            ..Default::default()
        }),
    );
    let mut ceiling = false;
    for _ in 0..650 {
        step(&mut s, &m);
        let a = s.physics.actor_status().remove(0);
        if a.surface.unwrap().normal.1 > 0.9 && a.x > 60. && a.x < 110. {
            ceiling = true;
            break;
        }
    }
    assert!(ceiling);
    s.physics.set_actor_controls(
        0,
        Some(Controls {
            crouch: true,
            ..Default::default()
        }),
    );
    let before = s.physics.actor_status()[0].y;
    for y in 0..26 {
        for x in 35..130 {
            let i = s.world.idx(x, y);
            s.world.make_air(i);
        }
    }
    let mut detached = false;
    for _ in 0..50 {
        step(&mut s, &m);
        detached |= !s.physics.actor_status()[0].surface.unwrap().attached;
    }
    assert!(detached);
    assert!(
        s.physics.actor_status()[0].y > before + 15.,
        "unsupported crawlers must fall rather than cling to empty air"
    );
}
#[test]
fn surface_pose_is_saved_and_rewindable_and_cropped_art_matches_the_full_picture() {
    let (m, mut s) = setup(ROOM);
    s.physics.set_actor_controls(
        0,
        Some(Controls {
            walk: 1.,
            crouch: true,
            ..Default::default()
        }),
    );
    for _ in 0..380 {
        step(&mut s, &m);
    }
    let a = s.physics.actor_status().remove(0);
    let pose = a.surface.unwrap();
    assert!(pose.attached);
    let snap = s.snapshot();
    let text = scene::save(&snap, &m);
    let (again, p) = scene::load(&text, &m, None).into_sim(&m);
    assert!(p.is_empty(), "{p:?}");
    let saved = again.physics.actor_status()[0].surface.unwrap();
    assert!(saved.attached);
    assert!(
        (saved.normal.0 - pose.normal.0).abs() < 0.001
            && (saved.normal.1 - pose.normal.1).abs() < 0.001
    );
    let mut r = grave_engine::render::Renderer::default();
    r.light(&s);
    let all = r.draw(&s.world, &m, Default::default()).to_vec();
    let win = (20, 15, 125, 90);
    let part = r
        .draw_window(&s.world, &m, Default::default(), win)
        .to_vec();
    for y in 0..win.3 {
        for x in 0..win.2 {
            assert_eq!(
                &part[(y * win.2 + x) * 4..(y * win.2 + x) * 4 + 4],
                &all[((y + win.1) * s.world.w + x + win.0) * 4
                    ..((y + win.1) * s.world.w + x + win.0) * 4 + 4]
            );
        }
    }
    s = Sim::restore(&snap, &m);
    assert_eq!(s.physics.actor_status()[0].surface, Some(pose));
}

#[test]
fn a_crawler_cannot_squeeze_through_a_slit_smaller_than_its_body() {
    let (m, mut s) = setup(&(ROOM.to_string() + "rect bedrock 80 25 10 75\nanchor 80 25 10 75\n"));
    s.physics.set_actor_controls(
        0,
        Some(Controls {
            walk: 1.,
            crouch: true,
            ..Default::default()
        }),
    );
    for _ in 0..1000 {
        step(&mut s, &m);
        assert!(
            s.physics.actor_status()[0].x < 80.,
            "adhesion must not phase the creature through a narrow slit"
        );
    }
}
