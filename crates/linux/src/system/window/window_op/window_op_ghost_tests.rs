use super::*;
use crate::system::window::window_client::facts_of;

#[test]
fn ghost_dialog_operations_never_write_to_the_main_client() {
    for global in [false, true] {
        for with_frames in [false, true] {
            for op in [
                resize(500.0, 400.0),
                WindowOp::Move { x: 30.0, y: 40.0 },
                WindowOp::Minimize,
            ] {
                let mut world = World::showing(state(10, 20, 400, 300));
                world.global = global;
                let mut ghost = window();
                ghost.title = "Question".into();
                ghost.bounds = Some(Rect {
                    x: 10.0,
                    y: 20.0,
                    width: 400.0,
                    height: 300.0,
                });
                let mut main_client = client(CLIENT, "Doc");
                main_client.bounds = Some(Extents {
                    x: 10,
                    y: 20,
                    width: 400,
                    height: 300,
                });
                world
                    .clients
                    .replace(VecDeque::from([ControlRead::Ready(vec![main_client])]));
                if with_frames {
                    let frame = facts_of(&ghost, global);
                    let mut main = frame.clone();
                    main.title = "Doc".into();
                    world.frames = Some((vec![main, frame], 1));
                }
                let error = run(&world, &ghost, &op, budget()).expect_err("ghost is not mapped");
                assert_eq!(world.writes(), 0);
                assert_eq!(world.lifts.get(), 0);
                assert_eq!(error.details.unwrap()["cause"], "window_not_mapped");
                assert_eq!(error.disposition, DeliverySemantics::not_delivered());
            }
        }
    }
}
