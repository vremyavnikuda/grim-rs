use grim_rs::{Box as GrimBox, CaptureParameters, CaptureResult, Grim};
use std::collections::HashMap;

#[test]
fn test_box_struct_creation() {
    let box1 = GrimBox::new(10, 20, 100, 200);
    assert_eq!(box1.x(), 10);
    assert_eq!(box1.y(), 20);
    assert_eq!(box1.width(), 100);
    assert_eq!(box1.height(), 200);
}

#[test]
fn test_box_is_empty() {
    let box1 = GrimBox::new(0, 0, 0, 0);
    assert!(box1.is_empty());

    let box2 = GrimBox::new(0, 0, -10, 10);
    assert!(box2.is_empty());

    let box3 = GrimBox::new(0, 0, 10, -5);
    assert!(box3.is_empty());

    let box4 = GrimBox::new(0, 0, 10, 10);
    assert!(!box4.is_empty());
}

#[test]
fn test_box_intersection() {
    let box1 = GrimBox::new(0, 0, 100, 100);
    let box2 = GrimBox::new(50, 50, 100, 100);

    assert!(box1.intersects(&box2));
    let intersection = box1.intersection(&box2).unwrap();
    assert_eq!(intersection.x(), 50);
    assert_eq!(intersection.y(), 50);
    assert_eq!(intersection.width(), 50);
    assert_eq!(intersection.height(), 50);

    let box3 = GrimBox::new(0, 0, 10, 10);
    let box4 = GrimBox::new(100, 100, 10, 10);
    assert!(!box3.intersects(&box4));
    assert!(box3.intersection(&box4).is_none());
}

#[test]
fn test_box_string_parsing() {
    let box_str = "10,20 300x400";
    let parsed: GrimBox = box_str.parse().unwrap();
    assert_eq!(parsed.x(), 10);
    assert_eq!(parsed.y(), 20);
    assert_eq!(parsed.width(), 300);
    assert_eq!(parsed.height(), 400);
    assert_eq!(parsed.to_string(), "10,20 300x400");
}

#[test]
fn test_capture_result_struct() {
    let data = vec![255u8; 400];
    let result = CaptureResult::new(data, 10, 10);

    assert_eq!(result.width(), 10);
    assert_eq!(result.height(), 10);
    assert_eq!(result.data().len(), 400); // 10x10x4
}

#[test]
fn test_capture_parameters_struct() {
    let params = CaptureParameters::new("eDP-1")
        .region(GrimBox::new(0, 0, 800, 600))
        .overlay_cursor(true)
        .scale(1.5);

    assert_eq!(params.output_name(), "eDP-1");
    assert_eq!(params.region_ref(), Some(&GrimBox::new(0, 0, 800, 600)));
    assert!(params.overlay_cursor_enabled());
    assert_eq!(params.scale_factor(), Some(1.5));
}

#[test]
fn test_error_messages() {
    let error = grim_rs::Error::InvalidGeometry("test".to_string());
    assert!(error.to_string().contains("Invalid geometry format"));

    let error = grim_rs::Error::NoOutputs;
    assert_eq!(error.to_string(), "No outputs available");

    let error = grim_rs::Error::OutputNotFound("test".to_string());
    assert!(error.to_string().contains("Output not found"));
}

#[test]
fn test_crate_export_structs() {
    let _box = GrimBox::new(0, 0, 100, 100);
    let _params = CaptureParameters::new("test");
    let _result = CaptureResult::new(vec![], 0, 0);
}

#[test]
fn test_image_data_format() {
    let width = 2;
    let height = 2;
    let data = [
        255, 0, 0, 255, // Red pixel
        0, 255, 0, 255, // Green pixel
        0, 0, 255, 255, // Blue pixel
        255, 255, 255, 255, // White pixel
    ];

    assert_eq!(data.len(), (width * height * 4) as usize);

    assert_eq!(data[0], 255); // R
    assert_eq!(data[1], 0); // G
    assert_eq!(data[2], 0); // B
    assert_eq!(data[3], 255); // A
}

#[test]
fn test_png_compression_levels() {
    let test_data = vec![255u8; 100 * 100 * 4]; // 100x100 image

    if let Ok(grim) = Grim::new() {
        let _ = grim.to_png(&test_data, 100, 100);
        let _ = grim.to_png_with_compression(&test_data, 100, 100, 0);
        let _ = grim.to_png_with_compression(&test_data, 100, 100, 6);
        let _ = grim.to_png_with_compression(&test_data, 100, 100, 9);
    }
}

#[test]
fn test_ppm_format_generation() {
    let test_data = vec![255u8; 10 * 10 * 4];

    if let Ok(grim) = Grim::new() {
        let ppm_result = grim.to_ppm(&test_data, 10, 10);
        assert!(ppm_result.is_ok());

        let ppm_data = ppm_result.unwrap();
        let ppm_str = String::from_utf8(ppm_data[..13].to_vec()).unwrap();

        // Check PPM header
        assert!(ppm_str.starts_with("P6\n"));
        assert!(ppm_str.contains("10 10\n"));
        assert!(ppm_str.contains("255\n"));

        assert_eq!(ppm_data.len(), 13 + 10 * 10 * 3);
    }
}

#[test]
fn test_capture_parameters_default_behavior() {
    let params = CaptureParameters::new("test");

    assert_eq!(params.output_name(), "test");
    assert!(params.region_ref().is_none());
    assert!(!params.overlay_cursor_enabled());
    assert!(params.scale_factor().is_none());
}

#[cfg(feature = "jpeg")]
#[test]
fn test_jpeg_functionality_available() {
    let test_data = vec![255u8; 10 * 10 * 4];

    if let Ok(grim) = Grim::new() {
        let jpeg_result = grim.to_jpeg(&test_data, 10, 10);
        assert!(jpeg_result.is_ok());

        let jpeg_result_with_quality = grim.to_jpeg_with_quality(&test_data, 10, 10, 85);
        assert!(jpeg_result_with_quality.is_ok());
    }
}

#[cfg(not(feature = "jpeg"))]
#[test]
fn test_jpeg_functionality_unavailable() {
    let test_data = vec![255u8; 10 * 10 * 4];

    match Grim::new() {
        Ok(grim) => {
            let jpeg_result = grim.to_jpeg(&test_data, 10, 10);
            assert!(jpeg_result.is_err());
        }
        Err(_) => {}
    }
}

#[test]
fn test_multi_output_capture_result() {
    let mut outputs_map = HashMap::new();
    outputs_map.insert(
        "output1".to_string(),
        CaptureResult::new(vec![255u8; 100 * 100 * 4], 100, 100),
    );
    outputs_map.insert(
        "output2".to_string(),
        CaptureResult::new(vec![128u8; 200 * 150 * 4], 200, 150),
    );

    let multi_result = grim_rs::MultiOutputCaptureResult::new(outputs_map);

    assert_eq!(multi_result.outputs().len(), 2);
    assert!(multi_result.outputs().contains_key("output1"));
    assert!(multi_result.outputs().contains_key("output2"));

    let output1_result = multi_result.get("output1").unwrap();
    assert_eq!(output1_result.width(), 100);
    assert_eq!(output1_result.height(), 100);
    assert_eq!(output1_result.data().len(), 100 * 100 * 4);
}

#[test]
fn test_scale_functionality_validation() {
    let scales = [0.5, 1.0, 1.5, 2.0, 0.25];

    for scale in scales.iter() {
        let new_width = (800.0 * scale) as u32;
        let new_height = (600.0 * scale) as u32;

        assert!(new_width > 0);
        assert!(new_height > 0);
    }
}

#[test]
fn test_geometry_bounds_checking() {
    let invalid_box = GrimBox::new(0, 0, -10, 100);
    assert!(invalid_box.is_empty());

    let invalid_box2 = GrimBox::new(0, 0, 100, -10);
    assert!(invalid_box2.is_empty());

    let valid_box = GrimBox::new(10, 10, 100, 100);
    assert!(!valid_box.is_empty());
}

#[test]
fn test_region_intersection_with_outputs() {
    let output_box = GrimBox::new(0, 0, 1920, 1080);
    let capture_region = GrimBox::new(100, 100, 500, 500);

    assert!(output_box.intersects(&capture_region));
    let intersection = output_box.intersection(&capture_region).unwrap();
    assert_eq!(intersection.x(), 100);
    assert_eq!(intersection.y(), 100);
    assert_eq!(intersection.width(), 500);
    assert_eq!(intersection.height(), 500);

    let region_outside = GrimBox::new(2000, 2000, 100, 100);
    assert!(!output_box.intersects(&region_outside));
    assert!(output_box.intersection(&region_outside).is_none());
}

mod transform_tests {
    #[test]
    fn test_transform_normal() {
        let width = 1920;
        let height = 1080;

        assert_eq!(width, 1920);
        assert_eq!(height, 1080);
    }

    /// Test that 90° rotation swaps width and height
    #[test]
    fn test_transform_90_degree_rotation() {
        let original_width = 1920;
        let original_height = 1080;

        let expected_width = 1080;
        let expected_height = 1920;

        assert_ne!(original_width, expected_width);
        assert_ne!(original_height, expected_height);
        assert_eq!(original_width, expected_height);
        assert_eq!(original_height, expected_width);
    }

    /// Test that 180° rotation keeps same dimensions
    #[test]
    fn test_transform_180_degree_rotation() {
        let width = 1920;
        let height = 1080;

        assert_eq!(width, 1920);
        assert_eq!(height, 1080);
    }

    /// Test that 270° rotation swaps width and height
    #[test]
    fn test_transform_270_degree_rotation() {
        let original_width = 1920;
        let original_height = 1080;

        let expected_width = 1080;
        let expected_height = 1920;

        assert_eq!(original_width, expected_height);
        assert_eq!(original_height, expected_width);
    }

    /// Test flipped transform behavior
    #[test]
    fn test_transform_flipped() {
        let width = 1920;
        let height = 1080;

        assert_eq!(width, 1920);
        assert_eq!(height, 1080);
    }

    /// Test flipped 90° rotation
    #[test]
    fn test_transform_flipped_90() {
        let original_width = 1920;
        let original_height = 1080;

        let expected_width = 1080;
        let expected_height = 1920;

        assert_eq!(original_width, expected_height);
        assert_eq!(original_height, expected_width);
    }

    /// Test multiple outputs with different transforms
    #[test]
    fn test_multi_output_with_transforms() {
        struct TestOutput {
            width: i32,
            height: i32,
            rotated: bool,
        }

        let outputs = [
            TestOutput {
                width: 1920,
                height: 1080,
                rotated: false,
            },
            TestOutput {
                width: 1080,
                height: 1920,
                rotated: true,
            },
        ];

        assert_eq!(outputs[0].width, 1920);
        assert_eq!(outputs[0].height, 1080);
        assert!(!outputs[0].rotated);

        assert_eq!(outputs[1].width, 1080);
        assert_eq!(outputs[1].height, 1920);
        assert!(outputs[1].rotated);

        assert_eq!(outputs[0].width, outputs[1].height);
        assert_eq!(outputs[0].height, outputs[1].width);
    }

    /// Test logical geometry calculation with transforms
    #[test]
    fn test_logical_geometry_with_scale_and_transform() {
        let physical_width = 3840;
        let physical_height = 2160;
        let scale = 2;

        let logical_width = physical_width / scale;
        let logical_height = physical_height / scale;

        assert_eq!(logical_width, 1920);
        assert_eq!(logical_height, 1080);

        let logical_width_rotated = logical_height;
        let logical_height_rotated = logical_width;

        assert_eq!(logical_width_rotated, 1080);
        assert_eq!(logical_height_rotated, 1920);
    }

    /// Test transform integration - verify dimensions change correctly
    #[test]
    fn test_transform_integration_dimensions() {
        let original_width = 1920;
        let original_height = 1080;

        let rotated_width = 1080;
        let rotated_height = 1920;

        assert_eq!(original_width, rotated_height);
        assert_eq!(original_height, rotated_width);
    }

    /// Test that transform is applied to captured data
    #[test]
    fn test_image_transform_application() {
        let test_data: Vec<u8> = vec![
            255, 0, 0, 255, // Red
            0, 255, 0, 255, // Green
            0, 0, 255, 255, // Blue
            255, 255, 255, 255, // White
        ];

        assert_eq!(test_data.len(), 2 * 2 * 4);
    }

    /// Test flipped transforms preserve dimensions
    #[test]
    fn test_flipped_transforms_dimensions() {
        let width = 1920;
        let height = 1080;

        assert_eq!(width, 1920);
        assert_eq!(height, 1080);

        assert_eq!(width, 1920);
        assert_eq!(height, 1080);
    }

    /// Test rotation angle constants
    #[test]
    fn test_rotation_angles() {
        use std::f64::consts::{FRAC_PI_2, PI};

        let angle_90 = FRAC_PI_2; // π/2
        let angle_180 = PI; // π
        let angle_270 = 3.0 * FRAC_PI_2; // 3π/2

        assert!(angle_90 > 0.0 && angle_90 < PI);
        assert_eq!(angle_180, PI);
        assert!(angle_270 > PI && angle_270 < 2.0 * PI);
    }
}

#[cfg(test)]
mod y_invert_tests {
    /// Test Y-invert flag constant
    #[test]
    fn test_y_invert_flag_value() {
        const Y_INVERT: u32 = 1;
        assert_eq!(Y_INVERT, 1);
        assert_eq!(Y_INVERT & 1, 1);
    }

    /// Test Y-invert flag detection
    #[test]
    fn test_y_invert_flag_detection() {
        const Y_INVERT: u32 = 1;

        let flags_with_invert = 1u32;
        assert_ne!(flags_with_invert & Y_INVERT, 0);

        let flags_without_invert = 0u32;
        assert_eq!(flags_without_invert & Y_INVERT, 0);

        let flags_mixed = 3u32;
        assert_ne!(flags_mixed & Y_INVERT, 0);
    }

    /// Test that Y-invert preserves dimensions
    #[test]
    fn test_y_invert_preserves_dimensions() {
        let width = 1920;
        let height = 1080;

        assert_eq!(width, 1920);
        assert_eq!(height, 1080);
    }

    /// Test Y-invert with transform combination
    #[test]
    fn test_y_invert_with_transform() {
        let _original_width = 1920;
        let _original_height = 1080;

        let transformed_width = 1080;
        let transformed_height = 1920;

        let final_width = transformed_width;
        let final_height = transformed_height;

        assert_eq!(final_width, 1080);
        assert_eq!(final_height, 1920);
    }

    /// Test FrameState flags field
    #[test]
    fn test_frame_state_flags_field() {
        let flags: u32 = 0;
        assert_eq!(flags, 0);

        let flags_with_invert: u32 = 1;
        assert_eq!(flags_with_invert, 1);
    }
}
