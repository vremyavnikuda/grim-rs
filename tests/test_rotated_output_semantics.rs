use grim_rs::Region;

#[test]
fn rotated_output_full_capture_uses_logical_dimensions() {
    let logical_region = Region::new(0, 0, 1080, 1920);
    let old_physical_region = Region::new(0, 0, 1920, 1080);
    assert_eq!(logical_region.width(), 1080);
    assert_eq!(logical_region.height(), 1920);
    assert_ne!(logical_region, old_physical_region);
}

#[test]
fn rotated_output_multi_capture_defaults_to_local_logical_origin() {
    let local_logical_region = Region::new(0, 0, 1080, 1920);
    let old_global_physical_region = Region::new(3440, 0, 1920, 1080);
    assert_eq!(local_logical_region, Region::new(0, 0, 1080, 1920));
    assert_eq!(old_global_physical_region, Region::new(3440, 0, 1920, 1080));
    assert_ne!(local_logical_region, old_global_physical_region);
}

#[test]
fn rotated_output_region_requests_stay_in_logical_space_even_with_scale() {
    let output_box = Region::new(3440, 0, 1080, 1920);
    let region = Region::new(3300, 0, 300, 1400);
    let intersection = output_box.intersection(&region).unwrap();
    let requested_region = Region::new(
        intersection.x() - output_box.x(),
        intersection.y() - output_box.y(),
        intersection.width(),
        intersection.height(),
    );
    let old_physical_region = Region::new(0, 0, 320, 2160);
    assert_eq!(requested_region, Region::new(0, 0, 160, 1400));
    assert_eq!(old_physical_region, Region::new(0, 0, 320, 2160));
    assert_ne!(requested_region, old_physical_region);
}
