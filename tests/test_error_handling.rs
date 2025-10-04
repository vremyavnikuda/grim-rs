use grim_rs::error::Error;

#[test]
fn test_buffer_creation_error_with_context() {
    let err = Error::BufferCreation("failed to create temporary file: Permission denied".to_string());
    assert!(err.to_string().contains("Buffer creation failed"));
    assert!(err.to_string().contains("Permission denied"));
}

#[test]
fn test_io_with_context_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let err = Error::IoWithContext {
        operation: "creating output file '/tmp/screenshot.png'".to_string(),
        source: io_err,
    };
    assert!(err.to_string().contains("IO error during"));
    assert!(err.to_string().contains("creating output file"));
    assert!(err.to_string().contains("file not found"));
}

#[test]
fn test_transform_not_supported_error() {
    let err = Error::TransformNotSupported("unsupported transform type".to_string());
    assert!(err.to_string().contains("Output transform not supported"));
    assert!(err.to_string().contains("unsupported transform type"));
}

#[test]
fn test_invert_failed_error() {
    let err = Error::InvertFailed("failed to apply vertical flip".to_string());
    assert!(err.to_string().contains("Failed to apply Y-invert transformation"));
    assert!(err.to_string().contains("vertical flip"));
}

#[test]
fn test_scaling_failed_error() {
    let err = Error::ScalingFailed("failed to create image buffer for scaling 1920x1080 -> 960x540".to_string());
    assert!(err.to_string().contains("Image scaling failed"));
    assert!(err.to_string().contains("1920x1080"));
    assert!(err.to_string().contains("960x540"));
}

#[test]
fn test_existing_error_types_still_work() {
    let err = Error::NoOutputs;
    assert_eq!(err.to_string(), "No outputs available");
    
    let err = Error::OutputNotFound("HDMI-1".to_string());
    assert_eq!(err.to_string(), "Output not found: HDMI-1");
    
    let err = Error::InvalidRegion("region too small".to_string());
    assert_eq!(err.to_string(), "Invalid capture region: region too small");
}
