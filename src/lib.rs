pub mod error;
pub mod geometry;

mod wayland_capture;

pub use error::{Error, Result};
pub use geometry::Box;

use wayland_capture::WaylandCapture as PlatformCapture;

#[derive(Debug, Clone)]
pub struct CaptureResult {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct Output {
    pub name: String,
    pub geometry: Box,
    pub scale: i32,
}

/// Main interface for taking screenshots
pub struct Grim {
    platform_capture: PlatformCapture,
}

impl Grim {
    /// Create a new Grim instance
    pub fn new() -> Result<Self> {
        let platform_capture = PlatformCapture::new()?;
        Ok(Self { platform_capture })
    }

    /// Get available outputs
    pub fn get_outputs(&mut self) -> Result<Vec<Output>> {
        self.platform_capture.get_outputs()
    }

    /// Capture entire screen (all outputs)
    pub fn capture_all(&mut self) -> Result<CaptureResult> {
        self.platform_capture.capture_all()
    }

    /// Capture specific output by name
    pub fn capture_output(&mut self, output_name: &str) -> Result<CaptureResult> {
        self.platform_capture.capture_output(output_name)
    }

    /// Capture specific region
    pub fn capture_region(&mut self, region: Box) -> Result<CaptureResult> {
        self.platform_capture.capture_region(region)
    }

    /// Save captured data as PNG
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

    /// Get image data as PNG bytes
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
}