//! # grim-rs
//!
//! A pure Rust implementation of the grim screenshot utility for Wayland.
//!
//! This library provides a simple interface for taking screenshots on Wayland
//! compositors that support the `wlr-screencopy` protocol.
//!
//! ## Features
//!
//! - Capture entire screen (all outputs)
//! - Capture specific output by name
//! - Capture specific region
//! - Capture multiple outputs with different parameters
//! - Save screenshots as PNG or JPEG
//! - Get screenshot data as PNG or JPEG bytes
//!
//! ## Example
//!
//! ```no_run
//! use grim_rs::Grim;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut grim = Grim::new()?;
//! let result = grim.capture_all()?;
//! grim.save_png(&result.data, result.width, result.height, "screenshot.png")?;
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod geometry;

mod wayland_capture;

pub use error::{Error, Result};
pub use geometry::Box;

use wayland_capture::WaylandCapture as PlatformCapture;

/// Result of a screenshot capture operation.
///
/// Contains the raw image data and dimensions of the captured area.
#[derive(Debug, Clone)]
pub struct CaptureResult {
    /// Raw RGBA image data.
    ///
    /// Each pixel is represented by 4 bytes in RGBA format (Red, Green, Blue, Alpha).
    pub data: Vec<u8>,
    /// Width of the captured image in pixels.
    pub width: u32,
    /// Height of the captured image in pixels.
    pub height: u32,
}

/// Information about a display output.
#[derive(Debug, Clone)]
pub struct Output {
    /// Name of the output (e.g., "eDP-1", "HDMI-A-1").
    pub name: String,
    /// Geometry of the output (position and size).
    pub geometry: Box,
    /// Scale factor of the output (e.g., 1 for normal DPI, 2 for HiDPI).
    pub scale: i32,
}

/// Parameters for capturing a specific output.
///
/// Allows specifying different capture parameters for each output when
/// capturing multiple outputs simultaneously.
#[derive(Debug, Clone)]
pub struct CaptureParameters {
    /// Name of the output to capture.
    ///
    /// Must match one of the names returned by [`Grim::get_outputs`].
    pub output_name: String,
    /// Optional region within the output to capture.
    ///
    /// If `None`, the entire output will be captured.
    /// If `Some(region)`, only the specified region will be captured.
    /// The region must be within the bounds of the output.
    pub region: Option<Box>,
    /// Whether to include the cursor in the capture.
    ///
    /// If `true`, the cursor will be included in the screenshot.
    /// If `false`, the cursor will be excluded from the screenshot.
    pub overlay_cursor: bool,
}

/// Result of capturing multiple outputs.
///
/// Contains a map of output names to their respective capture results.
#[derive(Debug, Clone)]
pub struct MultiOutputCaptureResult {
    /// Map of output names to their capture results.
    ///
    /// The keys are output names, and the values are the corresponding
    /// capture results for each output.
    pub outputs: std::collections::HashMap<String, CaptureResult>,
}

/// Main interface for taking screenshots.
///
/// Provides methods for capturing screenshots of the entire screen,
/// specific outputs, regions, or multiple outputs with different parameters.
pub struct Grim {
    platform_capture: PlatformCapture,
}

impl Grim {
    /// Create a new Grim instance.
    ///
    /// Establishes a connection to the Wayland compositor and initializes
    /// the necessary protocols for screen capture.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Cannot connect to the Wayland compositor
    /// - Required Wayland protocols are not available
    /// - Other initialization errors occur
    ///
    /// # Example
    ///
    /// ```no_run
    /// use grim_rs::Grim;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let grim = Grim::new()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new() -> Result<Self> {
        let platform_capture = PlatformCapture::new()?;
        Ok(Self { platform_capture })
    }

    /// Get information about available display outputs.
    ///
    /// Returns a list of all connected display outputs with their names,
    /// geometries, and scale factors.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No outputs are available
    /// - Failed to retrieve output information
    ///
    /// # Example
    ///
    /// ```no_run
    /// use grim_rs::Grim;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut grim = Grim::new()?;
    /// let outputs = grim.get_outputs()?;
    ///
    /// for output in outputs {
    ///     println!("Output: {} ({}x{})", output.name, output.geometry.width, output.geometry.height);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_outputs(&mut self) -> Result<Vec<Output>> {
        self.platform_capture.get_outputs()
    }

    /// Capture the entire screen (all outputs).
    ///
    /// Captures a screenshot that includes all connected display outputs,
    /// arranged according to their physical positions.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No outputs are available
    /// - Failed to capture the screen
    /// - Buffer creation failed
    ///
    /// # Example
    ///
    /// ```no_run
    /// use grim_rs::Grim;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// println!("Captured screen: {}x{}", result.width, result.height);
    /// # Ok(())
    /// # }
    /// ```
    pub fn capture_all(&mut self) -> Result<CaptureResult> {
        self.platform_capture.capture_all()
    }

    /// Capture a specific output by name.
    ///
    /// Captures a screenshot of the specified display output.
    ///
    /// # Arguments
    ///
    /// * `output_name` - Name of the output to capture (e.g., "eDP-1", "HDMI-A-1")
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The specified output is not found
    /// - Failed to capture the output
    /// - Buffer creation failed
    ///
    /// # Example
    ///
    /// ```no_run
    /// use grim_rs::Grim;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_output("eDP-1")?;
    /// println!("Captured output: {}x{}", result.width, result.height);
    /// # Ok(())
    /// # }
    /// ```
    pub fn capture_output(&mut self, output_name: &str) -> Result<CaptureResult> {
        self.platform_capture.capture_output(output_name)
    }

    /// Capture a specific region.
    ///
    /// Captures a screenshot of the specified rectangular region.
    ///
    /// # Arguments
    ///
    /// * `region` - The region to capture, specified as a [`Box`]
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No outputs are available
    /// - Failed to capture the region
    /// - Buffer creation failed
    ///
    /// # Example
    ///
    /// ```no_run
    /// use grim_rs::{Grim, Box};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut grim = Grim::new()?;
    /// let region = Box::new(100, 100, 800, 600); // x=100, y=100, width=800, height=600
    /// let result = grim.capture_region(region)?;
    /// println!("Captured region: {}x{}", result.width, result.height);
    /// # Ok(())
    /// # }
    /// ```
    pub fn capture_region(&mut self, region: Box) -> Result<CaptureResult> {
        self.platform_capture.capture_region(region)
    }

    /// Capture multiple outputs with different parameters.
    ///
    /// Captures screenshots of multiple outputs simultaneously, each with
    /// potentially different parameters (region, cursor inclusion, etc.).
    ///
    /// # Arguments
    ///
    /// * `parameters` - Vector of [`CaptureParameters`] specifying what to capture
    ///                  from each output
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any specified output is not found
    /// - Any specified region is outside the bounds of its output
    /// - Failed to capture any of the outputs
    /// - Buffer creation failed
    ///
    /// # Example
    ///
    /// ```no_run
    /// use grim_rs::{Grim, CaptureParameters, Box};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut grim = Grim::new()?;
    ///
    /// // Get available outputs
    /// let outputs = grim.get_outputs()?;
    ///
    /// // Prepare capture parameters for multiple outputs
    /// let mut parameters = vec![
    ///     CaptureParameters {
    ///         output_name: outputs[0].name.clone(),
    ///         region: None, // Capture entire output
    ///         overlay_cursor: true, // Include cursor
    ///     }
    /// ];
    ///
    /// // If we have a second output, capture a region of it
    /// if outputs.len() > 1 {
    ///     let region = Box::new(0, 0, 400, 300);
    ///     parameters.push(CaptureParameters {
    ///         output_name: outputs[1].name.clone(),
    ///         region: Some(region), // Capture specific region
    ///         overlay_cursor: false, // Exclude cursor
    ///     });
    /// }
    ///
    /// // Capture all specified outputs
    /// let results = grim.capture_outputs(parameters)?;
    /// println!("Captured {} outputs", results.outputs.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn capture_outputs(&mut self, parameters: Vec<CaptureParameters>) -> Result<MultiOutputCaptureResult> {
        self.platform_capture.capture_outputs(parameters)
    }

    /// Save captured data as PNG.
    ///
    /// Saves the captured image data to a PNG file.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw RGBA image data from a capture result
    /// * `width` - Width of the image in pixels
    /// * `height` - Height of the image in pixels
    /// * `path` - Path where to save the PNG file
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to create or write to the file
    /// - Image processing failed
    ///
    /// # Example
    ///
    /// ```no_run
    /// use grim_rs::Grim;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// grim.save_png(&result.data, result.width, result.height, "screenshot.png")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn save_png<P: AsRef<std::path::Path>>(&self, data: &[u8], width: u32, height: u32, path: P) -> Result<()> {
        use image::{ImageBuffer, Rgba};
        
        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, data.to_vec())
            .ok_or(Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch
                )
            )))?;
        
        img.save(path)?;
        Ok(())
    }

    /// Save captured data as JPEG.
    ///
    /// Saves the captured image data to a JPEG file.
    ///
    /// This function is only available when the `jpeg` feature is enabled.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw RGBA image data from a capture result
    /// * `width` - Width of the image in pixels
    /// * `height` - Height of the image in pixels
    /// * `path` - Path where to save the JPEG file
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to create or write to the file
    /// - Image processing failed
    /// - JPEG support is not enabled (when feature is disabled)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use grim_rs::Grim;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// grim.save_jpeg(&result.data, result.width, result.height, "screenshot.jpg")?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "jpeg")]
    pub fn save_jpeg<P: AsRef<std::path::Path>>(&self, data: &[u8], width: u32, height: u32, path: P) -> Result<()> {
        use image::{ImageBuffer, Rgba, Rgb, buffer::ConvertBuffer};
        
        let rgba_img = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, data.to_vec())
            .ok_or(Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch
                )
            )))?;
        
        // Convert RGBA to RGB for JPEG
        let rgb_img: ImageBuffer<Rgb<u8>, Vec<u8>> = rgba_img.convert();
        
        rgb_img.save_with_format(path, image::ImageFormat::Jpeg)?;
        Ok(())
    }
    
    /// Save captured data as JPEG (stub when feature is disabled).
    ///
    /// This stub is used when the `jpeg` feature is disabled.
    ///
    /// # Errors
    ///
    /// Always returns an error indicating that JPEG support is not enabled.
    #[cfg(not(feature = "jpeg"))]
    pub fn save_jpeg<P: AsRef<std::path::Path>>(&self, _data: &[u8], _width: u32, _height: u32, _path: P) -> Result<()> {
        Err(Error::ImageProcessing(image::ImageError::Unsupported(
            image::error::UnsupportedError::from_format_and_kind(
                image::error::ImageFormatHint::Name("JPEG".to_string()),
                image::error::UnsupportedErrorKind::Format(image::ImageFormat::Jpeg.into())
            )
        )))
    }

    /// Get image data as JPEG bytes.
    ///
    /// Converts the captured image data to JPEG format and returns the bytes.
    ///
    /// This function is only available when the `jpeg` feature is enabled.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw RGBA image data from a capture result
    /// * `width` - Width of the image in pixels
    /// * `height` - Height of the image in pixels
    ///
    /// # Returns
    ///
    /// Returns the JPEG-encoded image data as a vector of bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Image processing failed
    /// - JPEG support is not enabled (when feature is disabled)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use grim_rs::Grim;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// let jpeg_bytes = grim.to_jpeg(&result.data, result.width, result.height)?;
    /// println!("JPEG data size: {} bytes", jpeg_bytes.len());
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "jpeg")]
    pub fn to_jpeg(&self, data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
        use image::{ImageBuffer, Rgba, Rgb, buffer::ConvertBuffer};
        
        let rgba_img = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, data.to_vec())
            .ok_or(Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch
                )
            )))?;
        
        // Convert RGBA to RGB for JPEG
        let rgb_img: ImageBuffer<Rgb<u8>, Vec<u8>> = rgba_img.convert();
        
        let mut jpeg_data = Vec::new();
        rgb_img.write_to(&mut std::io::Cursor::new(&mut jpeg_data), image::ImageFormat::Jpeg)?;
        Ok(jpeg_data)
    }
    
    /// Get image data as JPEG bytes (stub when feature is disabled).
    ///
    /// This stub is used when the `jpeg` feature is disabled.
    ///
    /// # Errors
    ///
    /// Always returns an error indicating that JPEG support is not enabled.
    #[cfg(not(feature = "jpeg"))]
    pub fn to_jpeg(&self, _data: &[u8], _width: u32, _height: u32) -> Result<Vec<u8>> {
        Err(Error::ImageProcessing(image::ImageError::Unsupported(
            image::error::UnsupportedError::from_format_and_kind(
                image::error::ImageFormatHint::Name("JPEG".to_string()),
                image::error::UnsupportedErrorKind::Format(image::ImageFormat::Jpeg.into())
            )
        )))
    }

    /// Get image data as PNG bytes.
    ///
    /// Converts the captured image data to PNG format and returns the bytes.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw RGBA image data from a capture result
    /// * `width` - Width of the image in pixels
    /// * `height` - Height of the image in pixels
    ///
    /// # Returns
    ///
    /// Returns the PNG-encoded image data as a vector of bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Image processing failed
    ///
    /// # Example
    ///
    /// ```no_run
    /// use grim_rs::Grim;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// let png_bytes = grim.to_png(&result.data, result.width, result.height)?;
    /// println!("PNG data size: {} bytes", png_bytes.len());
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_png(&self, data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
        use image::{ImageBuffer, Rgba};
        
        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, data.to_vec())
            .ok_or(Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch
                )
            )))?;
        
        let mut png_data = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png_data), image::ImageFormat::Png)?;
        Ok(png_data)
    }
}

impl Default for Grim {
    fn default() -> Self {
        Self::new().expect("Failed to initialize Grim")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geometry_parsing() {
        let geometry: Box = "100,200 800x600".parse().unwrap();
        assert_eq!(geometry.x, 100);
        assert_eq!(geometry.y, 200);
        assert_eq!(geometry.width, 800);
        assert_eq!(geometry.height, 600);
    }

    #[test]
    fn test_mock_capture() {
        let mut grim = Grim::new().unwrap();
        let result = grim.capture_all().unwrap();
        assert_eq!(result.width, 1920);
        assert_eq!(result.height, 1080);
        assert_eq!(result.data.len(), (1920 * 1080 * 4) as usize);
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
    #[cfg(not(feature = "jpeg"))]
    fn test_jpeg_disabled() {
        let grim = Grim::new().unwrap();
        let test_data = vec![255u8; 16];
        let jpeg_result = grim.to_jpeg(&test_data, 4, 4);
        assert!(jpeg_result.is_err());
    }
}