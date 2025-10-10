use grim_rs::geometry::Box;
use grim_rs::{CaptureParameters, CaptureResult, MultiOutputCaptureResult};
use std::collections::HashMap;

#[test]
fn box_getters_work_correctly() {
    let b = Box::new(10, 20, 300, 400);

    assert_eq!(b.x(), 10);
    assert_eq!(b.y(), 20);
    assert_eq!(b.width(), 300);
    assert_eq!(b.height(), 400);
}

#[test]
fn capture_result_accessors_work() {
    let data = vec![255u8; 100];
    let result = CaptureResult::new(data.clone(), 10, 10);

    assert_eq!(result.width(), 10);
    assert_eq!(result.height(), 10);
    assert_eq!(result.data(), &data[..]);
    assert_eq!(result.data().len(), 100);
}

#[test]
fn capture_result_into_data_consumes() {
    let data = vec![255u8; 100];
    let result = CaptureResult::new(data.clone(), 10, 10);

    let owned = result.into_data();
    assert_eq!(owned, data);
    assert_eq!(owned.len(), 100);
}

#[test]
fn capture_parameters_builder_pattern_works() {
    let region = Box::new(0, 0, 100, 100);

    let params = CaptureParameters::new("HDMI-A-1")
        .region(region)
        .overlay_cursor(true)
        .scale(2.0);

    assert_eq!(params.output_name(), "HDMI-A-1");
    assert_eq!(params.region_ref(), Some(&region));
    assert_eq!(params.overlay_cursor_enabled(), true);
    assert_eq!(params.scale_factor(), Some(2.0));
}

#[test]
fn capture_parameters_minimal_construction() {
    let params = CaptureParameters::new("eDP-1");

    assert_eq!(params.output_name(), "eDP-1");
    assert_eq!(params.region_ref(), None);
    assert_eq!(params.overlay_cursor_enabled(), false);
    assert_eq!(params.scale_factor(), None);
}

#[test]
fn capture_parameters_builder_is_chainable() {
    let region = Box::new(10, 20, 640, 480);

    let params = CaptureParameters::new("DP-1")
        .region(region)
        .overlay_cursor(true)
        .scale(1.5);

    assert_eq!(params.output_name(), "DP-1");
    assert!(params.region_ref().is_some());
    assert!(params.overlay_cursor_enabled());
    assert!(params.scale_factor().is_some());
}

#[test]
fn capture_parameters_partial_builder() {
    let params = CaptureParameters::new("HDMI-A-2").overlay_cursor(true);

    assert_eq!(params.output_name(), "HDMI-A-2");
    assert_eq!(params.region_ref(), None);
    assert_eq!(params.overlay_cursor_enabled(), true);
    assert_eq!(params.scale_factor(), None);
}

#[test]
fn multi_output_capture_result_get_works() {
    let mut outputs = HashMap::new();
    outputs.insert(
        "HDMI-A-1".to_string(),
        CaptureResult::new(vec![255; 100], 10, 10),
    );
    outputs.insert(
        "eDP-1".to_string(),
        CaptureResult::new(vec![128; 200], 10, 20),
    );

    let result = MultiOutputCaptureResult::new(outputs);

    assert!(result.get("HDMI-A-1").is_some());
    assert!(result.get("eDP-1").is_some());
    assert!(result.get("HDMI-A-2").is_none());

    let hdmi = result.get("HDMI-A-1").unwrap();
    assert_eq!(hdmi.width(), 10);
    assert_eq!(hdmi.height(), 10);
}

#[test]
fn multi_output_capture_result_outputs_returns_reference() {
    let mut outputs = HashMap::new();
    outputs.insert(
        "HDMI-A-1".to_string(),
        CaptureResult::new(vec![255; 100], 10, 10),
    );

    let result = MultiOutputCaptureResult::new(outputs);
    let outputs_ref = result.outputs();

    assert_eq!(outputs_ref.len(), 1);
    assert!(outputs_ref.contains_key("HDMI-A-1"));
}

#[test]
fn multi_output_capture_result_into_outputs_consumes() {
    let mut outputs = HashMap::new();
    outputs.insert(
        "HDMI-A-1".to_string(),
        CaptureResult::new(vec![255; 100], 10, 10),
    );
    outputs.insert(
        "eDP-1".to_string(),
        CaptureResult::new(vec![128; 200], 10, 20),
    );

    let result = MultiOutputCaptureResult::new(outputs.clone());
    let owned = result.into_outputs();

    assert_eq!(owned.len(), 2);
    assert!(owned.contains_key("HDMI-A-1"));
    assert!(owned.contains_key("eDP-1"));
}

#[test]
fn capture_result_data_returns_slice() {
    let data = vec![1, 2, 3, 4, 5];
    let result = CaptureResult::new(data.clone(), 1, 5);

    let slice = result.data();
    assert_eq!(slice.len(), 5);
    assert_eq!(slice, &[1, 2, 3, 4, 5]);
}

#[test]
fn box_encapsulation_prevents_direct_field_access() {
    let b = Box::new(100, 200, 300, 400);

    assert_eq!(b.x(), 100);
    assert_eq!(b.y(), 200);
    assert_eq!(b.width(), 300);
    assert_eq!(b.height(), 400);
}
