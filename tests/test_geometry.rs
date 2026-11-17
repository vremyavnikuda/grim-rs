use grim_rs::Region;

#[test]
fn test_box_parsing() {
    let box_str = "10,20 300x400";
    let parsed: Region = match box_str.parse() {
        Ok(value) => value,
        Err(err) => panic!("unexpected parse error: {err}"),
    };
    assert_eq!(parsed.x(), 10);
    assert_eq!(parsed.y(), 20);
    assert_eq!(parsed.width(), 300);
    assert_eq!(parsed.height(), 400);
}

#[test]
fn test_box_intersection() {
    let box1 = Region::new(0, 0, 100, 100);
    let box2 = Region::new(50, 50, 100, 100);
    assert!(box1.intersects(&box2));
    let intersection = match box1.intersection(&box2) {
        Some(value) => value,
        None => panic!("expected intersection, got none"),
    };
    assert_eq!(intersection.x(), 50);
    assert_eq!(intersection.y(), 50);
    assert_eq!(intersection.width(), 50);
    assert_eq!(intersection.height(), 50);
}

#[test]
fn test_geometry_parsing() {
    let geometry: Region = "100,200 800x600".parse().unwrap();
    assert_eq!(geometry.x(), 100);
    assert_eq!(geometry.y(), 200);
    assert_eq!(geometry.width(), 800);
    assert_eq!(geometry.height(), 600);
}
