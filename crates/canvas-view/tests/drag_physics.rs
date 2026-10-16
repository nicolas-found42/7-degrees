use canvas_view::physics::DragPhysics;

#[test]
fn dragging_moves_connected_neighbors_then_everyone_returns_without_pins() {
    let home = vec![(0.0, 0.0), (100.0, 0.0), (200.0, 0.0), (900.0, 0.0)];
    let adjacency = vec![vec![1], vec![0, 2], vec![1], vec![]];
    let mut points = home.clone();
    let mut physics = DragPhysics::new(&home);
    physics.begin(0, &adjacency);
    physics.drag_to((120.0, 60.0), &mut points);
    for _ in 0..30 {
        physics.step(1.0 / 60.0, &mut points);
    }
    assert_eq!(points[0], (120.0, 60.0));
    assert!(
        points[1].0 > 105.0 && points[1].0 < 140.0,
        "neighbor follows gently: {:?}",
        points[1]
    );
    assert_eq!(
        points[2], home[2],
        "motion does not propagate across the map"
    );
    assert_eq!(points[3], home[3], "unconnected players stay put");
    physics.release();
    for _ in 0..240 {
        physics.step(1.0 / 60.0, &mut points);
    }
    assert!(!physics.is_active());
    assert_eq!(
        points, home,
        "all affected players settle back without a persistent pin"
    );
}
