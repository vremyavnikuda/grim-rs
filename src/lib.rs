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
//! ```rust,no_run
//! use grim_rs::Grim;
//! use chrono::Local;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut grim = Grim::new()?;
//! let result = grim.capture_all()?;
//!
//! // Generate timestamped filename (like grim-rs does by default)
//! let filename = format!("{}_grim.png", Local::now().format("%Y%m%d_%Hh%Mm%Ss"));
//! grim.save_png(result.data(), result.width(), result.height(), &filename)?;
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod geometry;
pub mod pixel_format;

mod buffer;
mod compositor;
mod platform;
mod scaling;
mod transform;

pub use error::{Error, Result};
pub use geometry::Region;

#[doc(hidden)]
pub use buffer::checked_buffer_size;
#[cfg(target_os = "windows")]
#[doc(hidden)]
pub use platform::windows::blend_cursor_rgba;
#[doc(hidden)]
pub use transform::{apply_image_transform, rotate_180, rotate_270, rotate_90, OutputTransform};

use platform::Platform;

/// Result of a screenshot capture operation.
///
/// Contains the raw image data and dimensions of the captured area.
#[derive(Debug, Clone)]
pub struct CaptureResult {
    /// Raw RGBA image data.
    ///
    /// Each pixel is represented by 4 bytes in RGBA format (Red, Green, Blue, Alpha).
    data: Vec<u8>,
    /// Width of the captured image in pixels.
    width: u32,
    /// Height of the captured image in pixels.
    height: u32,
}

impl CaptureResult {
    pub fn new(data: Vec<u8>, width: u32, height: u32) -> Self {
        Self {
            data,
            width,
            height,
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn into_data(self) -> Vec<u8> {
        self.data
    }
}

/// Information about a display output.
#[derive(Debug, Clone)]
pub struct Output {
    /// Name of the output (e.g., "eDP-1", "HDMI-A-1").
    name: String,
    /// Geometry of the output (position and size).
    geometry: Region,
    /// Scale factor of the output (e.g., 1 for normal DPI, 2 for HiDPI).
    scale: i32,
    /// Description of the output (e.g., monitor model, manufacturer info).
    description: Option<String>,
}

impl Output {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn geometry(&self) -> &Region {
        &self.geometry
    }

    pub fn scale(&self) -> i32 {
        self.scale
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

/// Parameters for capturing a specific output.
///
/// Allows specifying different capture parameters for each output when
///
/// capturing multiple outputs simultaneously.
#[derive(Debug, Clone)]
pub struct CaptureParameters {
    /// Name of the output to capture.
    ///
    /// Must match one of the names returned by [`Grim::get_outputs`].
    output_name: String,
    /// Optional region within the output to capture.
    ///
    /// If `None`, the entire output will be captured.
    ///
    /// If `Some(region)`, only the specified region will be captured.
    ///
    /// The region must be within the bounds of the output.
    region: Option<Region>,
    /// Whether to include the cursor in the capture.
    ///
    /// If `true`, the cursor will be included in the screenshot.
    ///
    /// If `false`, the cursor will be excluded from the screenshot.
    overlay_cursor: bool,
    /// Scale factor for the output image.
    ///
    /// If `None`, uses the default scale (typically the highest output scale).
    ///
    /// If `Some(scale)`, the output image will be scaled accordingly.
    scale: Option<f64>,
}

impl CaptureParameters {
    /// Creates a new CaptureParameters with the specified output name.
    ///
    /// By default, captures the entire output without cursor and with default scale.
    pub fn new(output_name: impl Into<String>) -> Self {
        Self {
            output_name: output_name.into(),
            region: None,
            overlay_cursor: false,
            scale: None,
        }
    }

    /// Sets the region to capture within the output.
    pub fn region(mut self, region: Region) -> Self {
        self.region = Some(region);
        self
    }

    /// Sets whether to include the cursor in the capture.
    pub fn overlay_cursor(mut self, overlay_cursor: bool) -> Self {
        self.overlay_cursor = overlay_cursor;
        self
    }

    /// Sets the scale factor for the output image.
    pub fn scale(mut self, scale: f64) -> Self {
        self.scale = Some(scale);
        self
    }

    /// Returns the output name.
    pub fn output_name(&self) -> &str {
        &self.output_name
    }

    /// Returns the region, if set.
    pub fn region_ref(&self) -> Option<&Region> {
        self.region.as_ref()
    }

    /// Returns whether cursor overlay is enabled.
    pub fn overlay_cursor_enabled(&self) -> bool {
        self.overlay_cursor
    }

    /// Returns the scale factor, if set.
    pub fn scale_factor(&self) -> Option<f64> {
        self.scale
    }
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
    outputs: std::collections::HashMap<String, CaptureResult>,
}

impl MultiOutputCaptureResult {
    /// Creates a new MultiOutputCaptureResult with the given outputs map.
    pub fn new(outputs: std::collections::HashMap<String, CaptureResult>) -> Self {
        Self { outputs }
    }

    /// Gets the capture result for the specified output name.
    pub fn get(&self, output_name: &str) -> Option<&CaptureResult> {
        self.outputs.get(output_name)
    }

    /// Returns a reference to the outputs map.
    pub fn outputs(&self) -> &std::collections::HashMap<String, CaptureResult> {
        &self.outputs
    }

    /// Consumes self and returns the outputs map.
    pub fn into_outputs(self) -> std::collections::HashMap<String, CaptureResult> {
        self.outputs
    }
}

/// Backend protocol preference for capture initialization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Backend {
    /// Auto-detect: prefer `ext-image-copy-capture-v1`, fall back to `wlr-screencopy`.
    Auto,
    /// Force `ext-image-copy-capture-v1` (fails if not available).
    ExtImageCopyCapture,
    /// Force `wlr-screencopy` (fails if not available).
    WlrScreencopy,
}

/// Main interface for taking screenshots.
///
/// Provides methods for capturing screenshots of the entire screen,
/// specific outputs, regions, or multiple outputs with different parameters.
pub struct Grim {
    platform: Platform,
}

impl Grim {
    /// Create a new Grim instance with auto-detected backend.
    ///
    /// Establishes a connection to the Wayland compositor and initializes
    /// the necessary protocols for screen capture. Prefers
    /// `ext-image-copy-capture-v1` when available, falling back to
    /// `wlr-screencopy`.
    ///
    /// Use [`Grim::new_ext`] or [`Grim::new_wlr`] to force a specific backend.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Cannot connect to the Wayland compositor
    /// - No capture protocol is available
    /// - Other initialization errors occur
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use grim_rs::Grim;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let grim = Grim::new()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new() -> Result<Self> {
        let inner = Platform::new()?;
        Ok(Self { platform: inner })
    }

    /// Create a new Grim instance forcing `ext-image-copy-capture-v1` backend.
    ///
    /// **Linux/Wayland only.** On Windows this method returns
    /// [`Error::UnsupportedProtocol`].
    ///
    /// Fails if the compositor does not support this protocol (e.g. KDE, GNOME,
    /// or older Sway releases). Use [`Grim::new`] for auto-detection if you need
    /// to support multiple compositors.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnsupportedProtocol`] if `ext-image-copy-capture-v1`
    /// is not available on the compositor, or if called on a non-Linux platform.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use grim_rs::Grim;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Only works on compositors with ext-image-copy-capture-v1
    /// // (Sway >= 2025, Hyprland, COSMIC)
    /// let grim = Grim::new_ext()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new_ext() -> Result<Self> {
        let inner = Platform::new_with_backend(Backend::ExtImageCopyCapture)?;
        Ok(Self { platform: inner })
    }

    /// Create a new Grim instance forcing `wlr-screencopy` backend.
    ///
    /// **Linux/Wayland only.** On Windows this method returns
    /// [`Error::UnsupportedProtocol`].
    ///
    /// Fails if the compositor does not support this protocol. Use
    /// [`Grim::new`] for auto-detection if you need to support multiple
    /// compositors.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnsupportedProtocol`] if `zwlr-screencopy-manager-v1`
    /// is not available on the compositor, or if called on a non-Linux platform.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use grim_rs::Grim;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Only works on compositors with wlr-screencopy
    /// // (older Sway, River, Wayfire, Niri)
    /// let grim = Grim::new_wlr()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new_wlr() -> Result<Self> {
        let inner = Platform::new_with_backend(Backend::WlrScreencopy)?;
        Ok(Self { platform: inner })
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
    /// ```rust,no_run
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// let outputs = grim.get_outputs()?;
    ///
    /// for output in outputs {
    ///     println!("Output: {} ({}x{})", output.name(), output.geometry().width(), output.geometry().height());
    /// }
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn get_outputs(&mut self) -> Result<Vec<Output>> {
        self.platform.get_outputs()
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
    /// ```rust,no_run
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// println!("Captured screen: {}x{}", result.width(), result.height());
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn capture_all(&mut self) -> Result<CaptureResult> {
        self.platform.capture_all()
    }

    /// Capture the entire screen (all outputs) with specified scale factor.
    ///
    /// Captures a screenshot that includes all connected display outputs,
    /// arranged according to their physical positions, with a specified scale factor.
    ///
    /// # Arguments
    ///
    /// * `scale` - Scale factor for the output image
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
    /// ```rust,no_run
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all_with_scale(1.0)?;
    /// println!("Captured screen: {}x{}", result.width(), result.height());
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn capture_all_with_scale(&mut self, scale: f64) -> Result<CaptureResult> {
        self.platform.capture_all_with_scale(scale)
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
    /// ```rust,no_run
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// // Get available outputs first
    /// let outputs = grim.get_outputs()?;
    /// if let Some(output) = outputs.first() {
    ///     let result = grim.capture_output(output.name())?;
    ///     println!("Captured output: {}x{}", result.width(), result.height());
    /// }
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn capture_output(&mut self, output_name: &str) -> Result<CaptureResult> {
        self.platform.capture_output(output_name)
    }

    /// Capture a specific output by name with specified scale factor.
    ///
    /// Captures a screenshot of the specified display output with a specified scale factor.
    ///
    /// # Arguments
    ///
    /// * `output_name` - Name of the output to capture (e.g., "eDP-1", "HDMI-A-1")
    /// * `scale` - Scale factor for the output image
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
    /// ```rust,no_run
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// // Get available outputs first
    /// let outputs = grim.get_outputs()?;
    /// if let Some(output) = outputs.first() {
    ///     let result = grim.capture_output_with_scale(output.name(), 0.5)?;
    ///     println!("Captured output at 50% scale: {}x{}", result.width(), result.height());
    /// }
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn capture_output_with_scale(
        &mut self,
        output_name: &str,
        scale: f64,
    ) -> Result<CaptureResult> {
        self.platform.capture_output_with_scale(output_name, scale)
    }

    /// Capture a specific region.
    ///
    /// Captures a screenshot of the specified rectangular region.
    ///
    /// # Arguments
    ///
    /// * `region` - The region to capture, specified as a [`Region`]
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
    /// ```rust,no_run
    /// use grim_rs::{Grim, Region};
    ///
    /// let mut grim = Grim::new()?;
    /// // x=100, y=100, width=800, height=600
    /// let region = Region::new(100, 100, 800, 600);
    /// let result = grim.capture_region(region)?;
    /// println!("Captured region: {}x{}", result.width(), result.height());
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn capture_region(&mut self, region: Region) -> Result<CaptureResult> {
        self.platform.capture_region(region)
    }

    /// Capture a specific region with specified scale factor.
    ///
    /// Captures a screenshot of the specified rectangular region with a specified scale factor.
    ///
    /// # Arguments
    ///
    /// * `region` - The region to capture, specified as a [`Region`]
    /// * `scale` - Scale factor for the output image
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
    /// ```rust,no_run
    /// use grim_rs::{Grim, Region};
    ///
    /// let mut grim = Grim::new()?;
    /// // x=100, y=100, width=800, height=600
    /// let region = Region::new(100, 100, 800, 600);
    /// let result = grim.capture_region_with_scale(region, 1.0)?;
    /// println!("Captured region: {}x{}", result.width(), result.height());
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn capture_region_with_scale(
        &mut self,
        region: Region,
        scale: f64,
    ) -> Result<CaptureResult> {
        self.platform.capture_region_with_scale(region, scale)
    }

    /// Capture multiple outputs with different parameters.
    ///
    /// Captures screenshots of multiple outputs simultaneously, each with
    /// potentially different parameters (region, cursor inclusion, etc.).
    ///
    /// # Arguments
    ///
    /// * `parameters` - Vector of [`CaptureParameters`] specifying what to capture
    ///   from each output
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
    /// ```rust,no_run
    /// use grim_rs::{Grim, CaptureParameters, Region};
    ///
    /// let mut grim = Grim::new()?;
    ///
    /// // Get available outputs
    /// let outputs = grim.get_outputs()?;
    ///
    /// // Prepare capture parameters for multiple outputs
    /// let mut parameters = vec![
    ///     CaptureParameters::new(outputs[0].name())
    ///         .overlay_cursor(true)
    /// ];
    ///
    /// // If we have a second output, capture a region of it
    /// if outputs.len() > 1 {
    ///     let region = Region::new(0, 0, 400, 300);
    ///     parameters.push(
    ///         CaptureParameters::new(outputs[1].name())
    ///             .region(region)
    ///     );
    /// }
    ///
    /// // Capture all specified outputs
    /// let results = grim.capture_outputs(parameters)?;
    /// println!("Captured {} outputs", results.outputs().len());
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn capture_outputs(
        &mut self,
        parameters: Vec<CaptureParameters>,
    ) -> Result<MultiOutputCaptureResult> {
        self.platform.capture_outputs(parameters)
    }

    /// Capture outputs with scale factor.
    ///
    /// Captures screenshots of multiple outputs simultaneously with a specific scale factor.
    ///
    /// # Arguments
    ///
    /// * `parameters` - Vector of CaptureParameters with scale factors
    /// * `default_scale` - Default scale factor to use when not specified in parameters
    ///
    /// # Errors
    ///
    /// Returns an error if any of the outputs can't be captured
    pub fn capture_outputs_with_scale(
        &mut self,
        parameters: Vec<CaptureParameters>,
        default_scale: f64,
    ) -> Result<MultiOutputCaptureResult> {
        self.platform
            .capture_outputs_with_scale(parameters, default_scale)
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
    /// ```rust,no_run
    /// use grim_rs::Grim;
    /// use chrono::Local;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    ///
    /// // Generate timestamped filename
    /// let filename = format!("{}_grim.png", Local::now().format("%Y%m%d_%Hh%Mm%Ss"));
    /// grim.save_png(result.data(), result.width(), result.height(), &filename)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn save_png<P: AsRef<std::path::Path>>(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        path: P,
    ) -> Result<()> {
        self.save_png_with_compression(data, width, height, path, 6) // Default compression level of 6
    }

    /// Save captured data as PNG with compression level control.
    ///
    /// Saves the captured image data to a PNG file with specified compression level.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw RGBA image data from a capture result
    /// * `width` - Width of the image in pixels
    /// * `height` - Height of the image in pixels
    /// * `path` - Path where to save the PNG file
    /// * `compression` - PNG compression level (0-9, where 9 is highest compression)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to create or write to the file
    /// - Image processing failed
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use grim_rs::Grim;
    /// use chrono::Local;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    ///
    /// // Generate timestamped filename
    /// let filename = format!("{}_grim.png", Local::now().format("%Y%m%d_%Hh%Mm%Ss"));
    /// grim.save_png_with_compression(result.data(), result.width(), result.height(), &filename, 9)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn save_png_with_compression<P: AsRef<std::path::Path>>(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        path: P,
        compression: u8,
    ) -> Result<()> {
        use std::io::BufWriter;

        let pixels = u64::from(width) * u64::from(height);
        let expected = pixels.checked_mul(4).ok_or_else(|| {
            Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            ))
        })?;
        if expected > usize::MAX as u64 {
            return Err(Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            )));
        }
        if data.len() != expected as usize {
            return Err(Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            )));
        }

        let file = std::fs::File::create(&path).map_err(|e| Error::IoWithContext {
            operation: format!("creating output file '{}'", path.as_ref().display()),
            source: e,
        })?;
        let writer = BufWriter::new(file);
        let mut encoder = png::Encoder::new(writer, width, height);

        let compression_level = match compression {
            0 => png::Compression::Fast,
            1..=3 => png::Compression::Best,
            4..=6 => png::Compression::Default,
            7..=9 => png::Compression::Best,
            _ => png::Compression::Default,
        };
        encoder.set_compression(compression_level);

        encoder.set_color(png::ColorType::Rgba);
        encoder.set_filter(png::FilterType::NoFilter);

        let mut writer = encoder
            .write_header()
            .map_err(|e| Error::Io(std::io::Error::other(format!("PNG encoding error: {}", e))))?;

        writer
            .write_image_data(data)
            .map_err(|e| Error::Io(std::io::Error::other(format!("PNG encoding error: {}", e))))?;
        writer
            .finish()
            .map_err(|e| Error::Io(std::io::Error::other(format!("PNG encoding error: {}", e))))?;

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
    /// ```rust,no_run
    /// use grim_rs::Grim;
    /// use chrono::Local;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    ///
    /// // Generate timestamped filename
    /// let filename = format!("{}_grim.jpg", Local::now().format("%Y%m%d_%Hh%Mm%Ss"));
    /// grim.save_jpeg(result.data(), result.width(), result.height(), &filename)?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "jpeg")]
    pub fn save_jpeg<P: AsRef<std::path::Path>>(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        path: P,
    ) -> Result<()> {
        self.save_jpeg_with_quality(data, width, height, path, 80)
    }

    /// Save captured data as JPEG with quality control.
    ///
    /// Saves the captured image data to a JPEG file with specified quality.
    ///
    /// This function is only available when the `jpeg` feature is enabled.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw RGBA image data from a capture result
    /// * `width` - Width of the image in pixels
    /// * `height` - Height of the image in pixels
    /// * `path` - Path where to save the JPEG file
    /// * `quality` - JPEG quality level (0-100, where 100 is highest quality)
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
    /// ```rust,no_run
    /// use grim_rs::Grim;
    /// use chrono::Local;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    ///
    /// // Generate timestamped filename
    /// let filename = format!("{}_grim.jpg", Local::now().format("%Y%m%d_%Hh%Mm%Ss"));
    /// grim.save_jpeg_with_quality(result.data(), result.width(), result.height(), &filename, 90)?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "jpeg")]
    pub fn save_jpeg_with_quality<P: AsRef<std::path::Path>>(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        path: P,
        quality: u8,
    ) -> Result<()> {
        let pixels = u64::from(width) * u64::from(height);
        let expected = pixels.checked_mul(4).ok_or_else(|| {
            Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            ))
        })?;
        if expected > usize::MAX as u64 {
            return Err(Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            )));
        }
        if data.len() != expected as usize {
            return Err(Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            )));
        }
        let pixels = expected as usize / 4;
        let rgb_len = pixels.checked_mul(3).ok_or_else(|| {
            Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            ))
        })?;
        let mut rgb_data = vec![0u8; rgb_len];
        for (i, rgba) in data.as_chunks::<4>().0.iter().enumerate() {
            let base = i * 3;
            rgb_data[base] = rgba[0];
            rgb_data[base + 1] = rgba[1];
            rgb_data[base + 2] = rgba[2];
        }

        let mut output_file = std::fs::File::create(&path).map_err(|e| Error::IoWithContext {
            operation: format!("creating output file '{}'", path.as_ref().display()),
            source: e,
        })?;
        let mut _encoder = jpeg_encoder::Encoder::new(&mut output_file, quality);

        _encoder
            .encode(
                &rgb_data,
                width as u16,
                height as u16,
                jpeg_encoder::ColorType::Rgb,
            )
            .map_err(|e| Error::Io(std::io::Error::other(format!("JPEG encoding error: {}", e))))?;

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
    pub fn save_jpeg<P: AsRef<std::path::Path>>(
        &self,
        _data: &[u8],
        _width: u32,
        _height: u32,
        _path: P,
    ) -> Result<()> {
        Err(Error::ImageProcessing(image::ImageError::Unsupported(
            image::error::UnsupportedError::from_format_and_kind(
                image::error::ImageFormatHint::Name("JPEG".to_string()),
                image::error::UnsupportedErrorKind::Format(image::ImageFormat::Jpeg.into()),
            ),
        )))
    }

    /// Save captured data as JPEG with quality control (stub when feature is disabled).
    ///
    /// This stub is used when the `jpeg` feature is disabled.
    ///
    /// # Errors
    ///
    /// Always returns an error indicating that JPEG support is not enabled.
    #[cfg(not(feature = "jpeg"))]
    pub fn save_jpeg_with_quality<P: AsRef<std::path::Path>>(
        &self,
        _data: &[u8],
        _width: u32,
        _height: u32,
        _path: P,
        _quality: u8,
    ) -> Result<()> {
        Err(Error::ImageProcessing(image::ImageError::Unsupported(
            image::error::UnsupportedError::from_format_and_kind(
                image::error::ImageFormatHint::Name("JPEG".to_string()),
                image::error::UnsupportedErrorKind::Format(image::ImageFormat::Jpeg.into()),
            ),
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
    /// ```rust,no_run
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// let jpeg_bytes = grim.to_jpeg(result.data(), result.width(), result.height())?;
    /// println!("JPEG data size: {} bytes", jpeg_bytes.len());
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    #[cfg(feature = "jpeg")]
    pub fn to_jpeg(&self, data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
        self.to_jpeg_with_quality(data, width, height, 80)
    }

    /// Get image data as JPEG bytes with quality control.
    ///
    /// Converts the captured image data to JPEG format with specified quality and returns the bytes.
    ///
    /// This function is only available when the `jpeg` feature is enabled.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw RGBA image data from a capture result
    /// * `width` - Width of the image in pixels
    /// * `height` - Height of the image in pixels
    /// * `quality` - JPEG quality level (0-100, where 100 is highest quality)
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
    /// ```rust,no_run
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// let jpeg_bytes = grim.to_jpeg_with_quality(result.data(), result.width(), result.height(), 90)?;
    /// println!("JPEG data size: {} bytes", jpeg_bytes.len());
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    #[cfg(feature = "jpeg")]
    pub fn to_jpeg_with_quality(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        quality: u8,
    ) -> Result<Vec<u8>> {
        let pixels = u64::from(width) * u64::from(height);
        let expected = pixels.checked_mul(4).ok_or_else(|| {
            Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            ))
        })?;
        if expected > usize::MAX as u64 {
            return Err(Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            )));
        }
        if data.len() != expected as usize {
            return Err(Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            )));
        }
        let pixels = expected as usize / 4;
        let rgb_len = pixels.checked_mul(3).ok_or_else(|| {
            Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            ))
        })?;
        let mut rgb_data = vec![0u8; rgb_len];
        for (i, rgba) in data.as_chunks::<4>().0.iter().enumerate() {
            let base = i * 3;
            rgb_data[base] = rgba[0];
            rgb_data[base + 1] = rgba[1];
            rgb_data[base + 2] = rgba[2];
        }

        let mut jpeg_data = Vec::new();
        let mut _encoder = jpeg_encoder::Encoder::new(&mut jpeg_data, quality);

        _encoder
            .encode(
                &rgb_data,
                width as u16,
                height as u16,
                jpeg_encoder::ColorType::Rgb,
            )
            .map_err(|e| Error::Io(std::io::Error::other(format!("JPEG encoding error: {}", e))))?;

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
                image::error::UnsupportedErrorKind::Format(image::ImageFormat::Jpeg.into()),
            ),
        )))
    }

    /// Get image data as JPEG bytes with quality control (stub when feature is disabled).
    ///
    /// This stub is used when the `jpeg` feature is disabled.
    ///
    /// # Errors
    ///
    /// Always returns an error indicating that JPEG support is not enabled.
    #[cfg(not(feature = "jpeg"))]
    pub fn to_jpeg_with_quality(
        &self,
        _data: &[u8],
        _width: u32,
        _height: u32,
        _quality: u8,
    ) -> Result<Vec<u8>> {
        Err(Error::ImageProcessing(image::ImageError::Unsupported(
            image::error::UnsupportedError::from_format_and_kind(
                image::error::ImageFormatHint::Name("JPEG".to_string()),
                image::error::UnsupportedErrorKind::Format(image::ImageFormat::Jpeg.into()),
            ),
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
    /// ```rust,no_run
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// let png_bytes = grim.to_png(result.data(), result.width(), result.height())?;
    /// println!("PNG data size: {} bytes", png_bytes.len());
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn to_png(&self, data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
        self.to_png_with_compression(data, width, height, 6)
    }

    /// Get image data as PNG bytes with compression level control.
    ///
    /// Converts the captured image data to PNG format with specified compression level and returns the bytes.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw RGBA image data from a capture result
    /// * `width` - Width of the image in pixels
    /// * `height` - Height of the image in pixels
    /// * `compression` - PNG compression level (0-9, where 9 is highest compression)
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
    /// ```rust,no_run
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// let png_bytes = grim.to_png_with_compression(result.data(), result.width(), result.height(), 9)?;
    /// println!("PNG data size: {} bytes", png_bytes.len());
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn to_png_with_compression(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        compression: u8,
    ) -> Result<Vec<u8>> {
        use std::io::Cursor;

        let pixels = u64::from(width) * u64::from(height);
        let expected = pixels.checked_mul(4).ok_or_else(|| {
            Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            ))
        })?;
        if expected > usize::MAX as u64 {
            return Err(Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            )));
        }
        if data.len() != expected as usize {
            return Err(Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            )));
        }

        let mut output = Vec::new();
        {
            let writer = Cursor::new(&mut output);
            let mut encoder = png::Encoder::new(writer, width, height);

            let compression_level = match compression {
                0 => png::Compression::Fast,
                1..=3 => png::Compression::Best,
                4..=6 => png::Compression::Default,
                7..=9 => png::Compression::Best,
                _ => png::Compression::Default,
            };
            encoder.set_compression(compression_level);

            encoder.set_color(png::ColorType::Rgba);
            encoder.set_filter(png::FilterType::NoFilter);

            let mut writer = encoder.write_header().map_err(|e| {
                Error::Io(std::io::Error::other(format!("PNG encoding error: {}", e)))
            })?;

            writer.write_image_data(data).map_err(|e| {
                Error::Io(std::io::Error::other(format!("PNG encoding error: {}", e)))
            })?;
            writer.finish().map_err(|e| {
                Error::Io(std::io::Error::other(format!("PNG encoding error: {}", e)))
            })?;
        }

        Ok(output)
    }

    /// Read region from stdin.
    ///
    /// Reads a region specification from standard input in the format "x,y widthxheight".
    ///
    /// # Returns
    ///
    /// Returns a `Region` representing the region read from stdin.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to read from stdin
    /// - The input format is invalid
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use grim_rs::{Grim, Region};
    ///
    /// // Parse region from string (same format as stdin would provide)
    /// let region = "100,100 800x600".parse::<Region>()?;
    /// println!("Region: {}", region);
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn read_region_from_stdin() -> Result<Region> {
        use std::io::{self, BufRead};

        let stdin = io::stdin();
        let mut handle = stdin.lock();
        let mut line = String::new();

        handle.read_line(&mut line)?;

        // Remove newline characters
        line = line.trim_end().to_string();

        line.parse()
    }

    /// Write image data to stdout as PNG.
    ///
    /// Writes captured image data directly to standard output in PNG format.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw RGBA image data from a capture result
    /// * `width` - Width of the image in pixels
    /// * `height` - Height of the image in pixels
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to write to stdout
    /// - Image processing failed
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// grim.write_png_to_stdout(result.data(), result.width(), result.height())?;
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn write_png_to_stdout(&self, data: &[u8], width: u32, height: u32) -> Result<()> {
        let png_data = self.to_png(data, width, height)?;
        use std::io::Write;
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(&png_data)?;
        handle.flush()?;
        Ok(())
    }

    /// Write image data to stdout as PNG with compression level.
    ///
    /// Writes captured image data directly to standard output in PNG format with specified compression.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw RGBA image data from a capture result
    /// * `width` - Width of the image in pixels
    /// * `height` - Height of the image in pixels
    /// * `compression` - PNG compression level (0-9, where 9 is highest compression)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to write to stdout
    /// - Image processing failed
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// grim.write_png_to_stdout_with_compression(result.data(), result.width(), result.height(), 6)?;
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn write_png_to_stdout_with_compression(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        compression: u8,
    ) -> Result<()> {
        let png_data = self.to_png_with_compression(data, width, height, compression)?;
        use std::io::Write;
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(&png_data)?;
        handle.flush()?;
        Ok(())
    }

    /// Write image data to stdout as JPEG.
    ///
    /// Writes captured image data directly to standard output in JPEG format.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw RGBA image data from a capture result
    /// * `width` - Width of the image in pixels
    /// * `height` - Height of the image in pixels
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to write to stdout
    /// - Image processing failed
    /// - JPEG support is not enabled
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// grim.write_jpeg_to_stdout(result.data(), result.width(), result.height())?;
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    #[cfg(feature = "jpeg")]
    pub fn write_jpeg_to_stdout(&self, data: &[u8], width: u32, height: u32) -> Result<()> {
        self.write_jpeg_to_stdout_with_quality(data, width, height, 80)
    }

    /// Write image data to stdout as JPEG with quality control.
    ///
    /// Writes captured image data directly to standard output in JPEG format with specified quality.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw RGBA image data from a capture result
    /// * `width` - Width of the image in pixels
    /// * `height` - Height of the image in pixels
    /// * `quality` - JPEG quality level (0-100, where 100 is highest quality)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to write to stdout
    /// - Image processing failed
    /// - JPEG support is not enabled
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// grim.write_jpeg_to_stdout_with_quality(result.data(), result.width(), result.height(), 90)?;
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    #[cfg(feature = "jpeg")]
    pub fn write_jpeg_to_stdout_with_quality(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        quality: u8,
    ) -> Result<()> {
        let jpeg_data = self.to_jpeg_with_quality(data, width, height, quality)?;
        use std::io::Write;
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(&jpeg_data)?;
        handle.flush()?;
        Ok(())
    }
}
