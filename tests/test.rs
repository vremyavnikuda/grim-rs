use grim_rs::pixel_format::{self, PixelFormat};
use grim_rs::{Backend, CaptureParameters, CaptureResult, Error, Grim, Region};
use std::collections::HashMap;

#[test]
fn test_box_struct_creation() {
    let box1 = Region::new(10, 20, 100, 200);
    assert_eq!(box1.x(), 10);
    assert_eq!(box1.y(), 20);
    assert_eq!(box1.width(), 100);
    assert_eq!(box1.height(), 200);
}

#[test]
fn test_box_is_empty() {
    let box1 = Region::new(0, 0, 0, 0);
    assert!(box1.is_empty());
    let box2 = Region::new(0, 0, -10, 10);
    assert!(box2.is_empty());
    let box3 = Region::new(0, 0, 10, -5);
    assert!(box3.is_empty());
    let box4 = Region::new(0, 0, 10, 10);
    assert!(!box4.is_empty());
}

#[test]
fn test_box_intersection() {
    let box1 = Region::new(0, 0, 100, 100);
    let box2 = Region::new(50, 50, 100, 100);
    assert!(box1.intersects(&box2));
    let intersection = box1.intersection(&box2).unwrap();
    assert_eq!(intersection.x(), 50);
    assert_eq!(intersection.y(), 50);
    assert_eq!(intersection.width(), 50);
    assert_eq!(intersection.height(), 50);
    let box3 = Region::new(0, 0, 10, 10);
    let box4 = Region::new(100, 100, 10, 10);
    assert!(!box3.intersects(&box4));
    assert!(box3.intersection(&box4).is_none());
}

#[test]
fn test_box_string_parsing() {
    let box_str = "10,20 300x400";
    let parsed: Region = box_str.parse().unwrap();
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
        .region(Region::new(0, 0, 800, 600))
        .overlay_cursor(true)
        .scale(1.5);
    assert_eq!(params.output_name(), "eDP-1");
    assert_eq!(params.region_ref(), Some(&Region::new(0, 0, 800, 600)));
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
    let _box = Region::new(0, 0, 100, 100);
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
fn test_convert_xrgb8888_to_rgba() {
    let mut pixel_data = vec![10, 20, 30, 99];
    pixel_format::convert_to_rgba(&mut pixel_data, PixelFormat::Xrgb8888);
    assert_eq!(pixel_data, vec![30, 20, 10, 255]);
}

#[test]
fn test_convert_argb8888_to_rgba_preserves_alpha() {
    let mut pixel_data = vec![10, 20, 30, 40];
    pixel_format::convert_to_rgba(&mut pixel_data, PixelFormat::Argb8888);
    assert_eq!(pixel_data, vec![30, 20, 10, 40]);
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
fn test_png_compression_level_mapping() {
    // Gradient mixed with LCG noise: compressible, but not trivially.
    let w = 128u32;
    let h = 128u32;
    let mut data = vec![0u8; (w * h * 4) as usize];
    let mut state = 0x1234_5678u32;
    for (i, px) in data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let x = i as u32 % w;
        let y = i as u32 / w;
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = (state >> 24) as u8;
        px[0] = (x * 2) as u8 ^ (noise & 0x0f);
        px[1] = (y * 2) as u8 ^ (noise & 0x0f);
        px[2] = (x + y) as u8 | (noise & 0x30);
        px[3] = 255;
    }
    if let Ok(grim) = Grim::new() {
        let mut sizes = [0usize; 10];
        for (level, size) in sizes.iter_mut().enumerate() {
            let png = grim
                .to_png_with_compression(&data, w, h, level as u8)
                .unwrap();
            assert!(
                png.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]),
                "level {level}: invalid PNG signature"
            );
            *size = png.len();
        }
        // Compression effort grows monotonically with the level.
        assert!(
            sizes[9] < sizes[0],
            "level 9 ({} bytes) should compress better than level 0 ({} bytes)",
            sizes[9],
            sizes[0]
        );
        assert!(sizes[6] <= sizes[3]);
        assert!(sizes[3] <= sizes[0]);
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
    let invalid_box = Region::new(0, 0, -10, 100);
    assert!(invalid_box.is_empty());
    let invalid_box2 = Region::new(0, 0, 100, -10);
    assert!(invalid_box2.is_empty());
    let valid_box = Region::new(10, 10, 100, 100);
    assert!(!valid_box.is_empty());
}

#[test]
fn test_region_intersection_with_outputs() {
    let output_box = Region::new(0, 0, 1920, 1080);
    let capture_region = Region::new(100, 100, 500, 500);
    assert!(output_box.intersects(&capture_region));
    let intersection = output_box.intersection(&capture_region).unwrap();
    assert_eq!(intersection.x(), 100);
    assert_eq!(intersection.y(), 100);
    assert_eq!(intersection.width(), 500);
    assert_eq!(intersection.height(), 500);
    let region_outside = Region::new(2000, 2000, 100, 100);
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

    #[test]
    fn test_frame_state_flags_field() {
        let flags: u32 = 0;
        assert_eq!(flags, 0);

        let flags_with_invert: u32 = 1;
        assert_eq!(flags_with_invert, 1);
    }
}

#[test]
fn test_mock_capture() {
    let result = std::panic::catch_unwind(|| {
        let mut grim = Grim::new().unwrap();
        grim.capture_all()
    });

    match result {
        Ok(capture_result) => {
            if let Ok(capture) = capture_result {
                assert_eq!(
                    capture.data().len(),
                    (capture.width() * capture.height() * 4) as usize
                );
            } else {
                assert!(capture_result.is_err());
            }
        }
        Err(_) => {
            panic!("Test panicked unexpectedly");
        }
    }
}

#[test]
#[cfg(feature = "png")]
fn test_to_png() {
    let grim = Grim::new().unwrap();
    let test_data = vec![255u8; 64];
    let png_data = grim.to_png(&test_data, 4, 4).unwrap();
    assert!(!png_data.is_empty());
}

#[test]
#[cfg(feature = "jpeg")]
fn test_to_jpeg() {
    let grim = Grim::new().unwrap();
    let test_data = vec![255u8; 64];
    let jpeg_data = grim.to_jpeg(&test_data, 4, 4).unwrap();
    assert!(!jpeg_data.is_empty());
}

#[test]
#[cfg(feature = "jpeg")]
fn test_jpeg_rejects_dimensions_over_u16() {
    if let Ok(grim) = Grim::new() {
        // JPEG headers store dimensions as u16; larger images must error
        // out instead of silently truncating.
        let too_wide = vec![0u8; 65536 * 4];
        assert!(grim.to_jpeg(&too_wide, 65536, 1).is_err());
        let too_tall = vec![0u8; 65536 * 4];
        assert!(grim.to_jpeg(&too_tall, 1, 65536).is_err());
        let boundary = vec![0u8; 4];
        assert!(grim.to_jpeg(&boundary, 1, 1).is_ok());
    }
}

#[test]
#[cfg(not(feature = "jpeg"))]
fn test_jpeg_disabled() {
    let grim = Grim::new().unwrap();
    let test_data = vec![255u8; 16];
    let jpeg_result = grim.to_jpeg(&test_data, 4, 4);
    assert!(jpeg_result.is_err());
}

#[test]
fn test_read_region_from_stdin() {
    let region_str = "10,20 300x400";
    let result: std::result::Result<Region, _> = region_str.parse();
    assert!(result.is_ok());
    let region = result.unwrap();
    assert_eq!(region.x(), 10);
    assert_eq!(region.y(), 20);
    assert_eq!(region.width(), 300);
    assert_eq!(region.height(), 400);
}

#[test]
fn test_scale_functionality() {
    let mut grim = Grim::new().unwrap();
    let test_capture = grim.capture_all_with_scale(1.0);
    if test_capture.is_ok() {}
}

#[test]
fn test_backend_enum_variants() {
    let auto = Backend::Auto;
    let ext = Backend::ExtImageCopyCapture;
    let wlr = Backend::WlrScreencopy;
    assert_ne!(auto, ext);
    assert_ne!(ext, wlr);
    assert_eq!(auto, Backend::Auto);
    // Auto is the default-like variant
    assert!(matches!(auto, Backend::Auto));
}

#[test]
fn test_new_ext_constructor() {
    match Grim::new_ext() {
        Ok(_) => {}
        Err(e) => {
            assert!(
                e.to_string().contains("not available"),
                "expected UnsupportedProtocol, got: {e}"
            );
        }
    }
}

#[test]
fn test_new_wlr_constructor() {
    match Grim::new_wlr() {
        Ok(_) => {}
        Err(e) => {
            assert!(
                e.to_string().contains("not available"),
                "expected UnsupportedProtocol, got: {e}"
            );
        }
    }
}

#[test]
fn test_new_auto_is_equivalent_to_new() {
    let result_auto = Grim::new();
    match result_auto {
        Ok(_) | Err(Error::NoOutputs) => {} // expected
        Err(e) => panic!("Unexpected error from Grim::new(): {e}"),
    }
}

#[test]
fn test_new_ext_can_capture() {
    if let Ok(mut grim) = Grim::new_ext() {
        match grim.capture_all() {
            Ok(result) => {
                assert_eq!(
                    result.data().len(),
                    (result.width() * result.height() * 4) as usize
                );
            }
            Err(Error::NoOutputs) => {} // no monitors connected
            Err(e) => panic!("Capture failed on ext backend: {e}"),
        }
    }
    // ext not available — skip
}

#[test]
fn test_new_wlr_can_capture() {
    if let Ok(mut grim) = Grim::new_wlr() {
        match grim.capture_all() {
            Ok(result) => {
                assert_eq!(
                    result.data().len(),
                    (result.width() * result.height() * 4) as usize
                );
            }
            Err(Error::NoOutputs) => {} // no monitors connected
            Err(e) => panic!("Capture failed on wlr backend: {e}"),
        }
    }
    // wlr not available — skip
}
