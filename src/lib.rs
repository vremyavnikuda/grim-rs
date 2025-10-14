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
    geometry: Box,
    /// Scale factor of the output (e.g., 1 for normal DPI, 2 for HiDPI).
    scale: i32,
    /// Description of the output (e.g., monitor model, manufacturer info).
    description: Option<String>,
}

impl Output {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn geometry(&self) -> &Box {
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
    region: Option<Box>,
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
    pub fn region(mut self, region: Box) -> Self {
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
    pub fn region_ref(&self) -> Option<&Box> {
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

/// Main interface for taking screenshots.
///
/// Provides methods for capturing screenshots of the entire screen,
/// specific outputs, regions, or multiple outputs with different parameters.
pub struct Grim {
    platform_capture: PlatformCapture,
    buffer_pool: std::cell::RefCell<Vec<Vec<u8>>>,
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
    /// ```rust
    /// use grim_rs::Grim;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let grim = Grim::new()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new() -> Result<Self> {
        let platform_capture = PlatformCapture::new()?;
        Ok(Self {
            platform_capture,
            buffer_pool: std::cell::RefCell::new(Vec::new()),
        })
    }

    /// Get a buffer from the pool or allocate a new one.
    ///
    /// Reuses buffers from the pool when available, otherwise allocates a new buffer.
    /// This reduces memory allocations for repeated encode operations.
    ///
    /// This is an internal method used by encode functions.
    ///
    /// # Arguments
    ///
    /// * `size` - Minimum required capacity for the buffer
    ///
    /// # Returns
    ///
    /// A Vec<u8> with at least the requested capacity. The buffer is cleared before return.
    fn get_buffer(&self, size: usize) -> Vec<u8> {
        let mut pool = self.buffer_pool.borrow_mut();

        // Try to reuse a buffer from pool with sufficient capacity
        if let Some(pos) = pool.iter().position(|buf| buf.capacity() >= size) {
            let mut buffer = pool.swap_remove(pos);
            buffer.clear();
            buffer
        } else {
            // Allocate new buffer with requested capacity
            Vec::with_capacity(size)
        }
    }

    /// Return a buffer to the pool for reuse.
    ///
    /// This is a public API for advanced users who want explicit control over buffer lifecycle.
    /// In most cases, buffers are automatically managed and you don't need to call this.
    ///
    /// Only buffers larger than 1MB are pooled to avoid memory fragmentation.
    /// Smaller buffers are dropped immediately.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The buffer to return to the pool
    ///
    /// # Example
    ///
    /// ```rust
    /// use grim_rs::Grim;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    ///
    /// // Use to_png which internally uses buffer pooling
    /// let png_bytes = grim.to_png(result.data(), result.width(), result.height())?;
    ///
    /// // Advanced: manually return the buffer to pool after use
    /// // (usually not needed as Grim manages buffers internally)
    /// grim.return_buffer(png_bytes);
    /// # Ok(())
    /// # }
    /// ```
    pub fn return_buffer(&self, buffer: Vec<u8>) {
        const MIN_POOL_SIZE: usize = 1024 * 1024; // 1MB threshold

        // Only pool large buffers to avoid fragmentation
        if buffer.capacity() >= MIN_POOL_SIZE {
            self.buffer_pool.borrow_mut().push(buffer);
        }
        // Small buffers are dropped automatically
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
    /// ```rust
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
    /// ```rust
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// println!("Captured screen: {}x{}", result.width(), result.height());
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn capture_all(&mut self) -> Result<CaptureResult> {
        self.platform_capture.capture_all()
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
    /// ```rust
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all_with_scale(1.0)?;
    /// println!("Captured screen: {}x{}", result.width(), result.height());
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn capture_all_with_scale(&mut self, scale: f64) -> Result<CaptureResult> {
        self.platform_capture.capture_all_with_scale(scale)
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
    /// ```rust
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
        self.platform_capture.capture_output(output_name)
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
    /// ```rust
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
        self.platform_capture
            .capture_output_with_scale(output_name, scale)
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
    /// ```rust
    /// use grim_rs::{Grim, Box};
    ///
    /// let mut grim = Grim::new()?;
    /// // x=100, y=100, width=800, height=600
    /// let region = Box::new(100, 100, 800, 600);
    /// let result = grim.capture_region(region)?;
    /// println!("Captured region: {}x{}", result.width(), result.height());
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn capture_region(&mut self, region: Box) -> Result<CaptureResult> {
        self.platform_capture.capture_region(region)
    }

    /// Capture a specific region with specified scale factor.
    ///
    /// Captures a screenshot of the specified rectangular region with a specified scale factor.
    ///
    /// # Arguments
    ///
    /// * `region` - The region to capture, specified as a [`Box`]
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
    /// ```rust
    /// use grim_rs::{Grim, Box};
    ///
    /// let mut grim = Grim::new()?;
    /// // x=100, y=100, width=800, height=600
    /// let region = Box::new(100, 100, 800, 600);
    /// let result = grim.capture_region_with_scale(region, 1.0)?;
    /// println!("Captured region: {}x{}", result.width(), result.height());
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn capture_region_with_scale(&mut self, region: Box, scale: f64) -> Result<CaptureResult> {
        self.platform_capture
            .capture_region_with_scale(region, scale)
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
    /// ```rust
    /// use grim_rs::{Grim, CaptureParameters, Box};
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
    ///     let region = Box::new(0, 0, 400, 300);
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
        self.platform_capture.capture_outputs(parameters)
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
        self.platform_capture
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
        use image::{ImageBuffer, Rgba};
        use std::io::BufWriter;

        let _img = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, data.to_vec()).ok_or(
            Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            )),
        )?;

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

        let mut writer = encoder.write_header().map_err(|e| {
            Error::Io(std::io::Error::other(format!("PNG encoding error: {}", e)))
        })?;

        writer.write_image_data(data).map_err(|e| {
            Error::Io(std::io::Error::other(format!("PNG encoding error: {}", e)))
        })?;
        writer.finish().map_err(|e| {
            Error::Io(std::io::Error::other(format!("PNG encoding error: {}", e)))
        })?;

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
        use image::{buffer::ConvertBuffer, ImageBuffer, Rgb, Rgba};

        let rgba_img = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, data.to_vec()).ok_or(
            Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            )),
        )?;

        let rgb_img: ImageBuffer<Rgb<u8>, Vec<u8>> = rgba_img.convert();

        let mut output_file = std::fs::File::create(&path).map_err(|e| Error::IoWithContext {
            operation: format!("creating output file '{}'", path.as_ref().display()),
            source: e,
        })?;
        let mut _encoder = jpeg_encoder::Encoder::new(&mut output_file, quality);
        let rgb_data = rgb_img.as_raw();

        _encoder
            .encode(
                rgb_data,
                width as u16,
                height as u16,
                jpeg_encoder::ColorType::Rgb,
            )
            .map_err(|e| {
                Error::Io(std::io::Error::other(format!("JPEG encoding error: {}", e)))
            })?;

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
    /// ```rust
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
    /// ```rust
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
        use image::{buffer::ConvertBuffer, ImageBuffer, Rgb, Rgba};

        let rgba_img = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, data.to_vec()).ok_or(
            Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            )),
        )?;

        let rgb_img: ImageBuffer<Rgb<u8>, Vec<u8>> = rgba_img.convert();

        // Estimate JPEG size: typically 10-30x compression for quality 60-95
        // Use conservative estimate of 10x compression
        let estimated_size = (width as usize) * (height as usize) * 3 / 10;
        let mut jpeg_data = self.get_buffer(estimated_size);
        let mut _encoder = jpeg_encoder::Encoder::new(&mut jpeg_data, quality);
        let rgb_data = rgb_img.as_raw();

        _encoder
            .encode(
                rgb_data,
                width as u16,
                height as u16,
                jpeg_encoder::ColorType::Rgb,
            )
            .map_err(|e| {
                Error::Io(std::io::Error::other(format!("JPEG encoding error: {}", e)))
            })?;

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
    /// ```rust
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
    /// ```rust
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
        use image::{ImageBuffer, Rgba};
        use std::io::Cursor;

        let _img = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, data.to_vec()).ok_or(
            Error::ImageProcessing(image::ImageError::Parameter(
                image::error::ParameterError::from_kind(
                    image::error::ParameterErrorKind::DimensionMismatch,
                ),
            )),
        )?;

        // Estimate output size: typical PNG compression ratio is 2-4x
        // For RGBA data, raw size is width * height * 4
        let estimated_size = (width as usize) * (height as usize) * 4 / 3;
        let mut output = self.get_buffer(estimated_size);

        {
            let writer = Cursor::new(&mut output);
            let mut encoder = png::Encoder::new(writer, width, height);

            let compression_level = match compression {
                0 => png::Compression::Fast,
                1..=3 => png::Compression::Fast,
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

    /// Save captured data as PPM.
    ///
    /// Saves the captured image data to a PPM file.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw RGBA image data from a capture result
    /// * `width` - Width of the image in pixels
    /// * `height` - Height of the image in pixels
    /// * `path` - Path where to save the PPM file
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
    /// let filename = format!("{}_grim.ppm", Local::now().format("%Y%m%d_%Hh%Mm%Ss"));
    /// grim.save_ppm(result.data(), result.width(), result.height(), &filename)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn save_ppm<P: AsRef<std::path::Path>>(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        path: P,
    ) -> Result<()> {
        let ppm_data = self.to_ppm(data, width, height)?;
        std::fs::write(&path, ppm_data).map_err(|e| Error::IoWithContext {
            operation: format!("writing PPM data to file '{}'", path.as_ref().display()),
            source: e,
        })?;
        Ok(())
    }

    /// Get image data as PPM bytes.
    ///
    /// Converts the captured image data to PPM format and returns the bytes.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw RGBA image data from a capture result
    /// * `width` - Width of the image in pixels
    /// * `height` - Height of the image in pixels
    ///
    /// # Returns
    ///
    /// Returns the PPM-encoded image data as a vector of bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Image processing failed
    ///
    /// # Example
    ///
    /// ```rust
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// let ppm_bytes = grim.to_ppm(result.data(), result.width(), result.height())?;
    /// println!("PPM data size: {} bytes", ppm_bytes.len());
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn to_ppm(&self, data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
        let header = format!("P6\n{} {}\n255\n", width, height);
        let mut ppm_data = header.into_bytes();

        for chunk in data.chunks_exact(4) {
            ppm_data.push(chunk[0]); // R
            ppm_data.push(chunk[1]); // G
            ppm_data.push(chunk[2]); // B
        }

        Ok(ppm_data)
    }

    /// Read region from stdin.
    ///
    /// Reads a region specification from standard input in the format "x,y widthxheight".
    ///
    /// # Returns
    ///
    /// Returns a `Box` representing the region read from stdin.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to read from stdin
    /// - The input format is invalid
    ///
    /// # Example
    ///
    /// ```rust
    /// use grim_rs::{Grim, Box};
    ///
    /// // Parse region from string (same format as stdin would provide)
    /// let region = "100,100 800x600".parse::<Box>()?;
    /// println!("Region: {}", region);
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn read_region_from_stdin() -> Result<Box> {
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
    /// ```rust
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
    /// ```rust
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
    /// ```rust
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
    /// ```rust
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

    /// Write image data to stdout as PPM.
    ///
    /// Writes captured image data directly to standard output in PPM format.
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
    /// ```rust
    /// use grim_rs::Grim;
    ///
    /// let mut grim = Grim::new()?;
    /// let result = grim.capture_all()?;
    /// grim.write_ppm_to_stdout(result.data(), result.width(), result.height())?;
    /// # Ok::<(), grim_rs::Error>(())
    /// ```
    pub fn write_ppm_to_stdout(&self, data: &[u8], width: u32, height: u32) -> Result<()> {
        let ppm_data = self.to_ppm(data, width, height)?;
        use std::io::Write;
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(&ppm_data)?;
        handle.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geometry_parsing() {
        let geometry: Box = "100,200 800x600".parse().unwrap();
        assert_eq!(geometry.x(), 100);
        assert_eq!(geometry.y(), 200);
        assert_eq!(geometry.width(), 800);
        assert_eq!(geometry.height(), 600);
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
                        capture.data.len(),
                        (capture.width * capture.height * 4) as usize
                    );
                } else {
                    assert!(matches!(capture_result, Err(Error::NoOutputs)));
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
    #[cfg(not(feature = "jpeg"))]
    fn test_jpeg_disabled() {
        let grim = Grim::new().unwrap();
        let test_data = vec![255u8; 16];
        let jpeg_result = grim.to_jpeg(&test_data, 4, 4);
        assert!(jpeg_result.is_err());
    }

    #[test]
    fn test_ppm_format() {
        let grim = Grim::new().unwrap();
        let test_data = vec![255u8; 16];
        let ppm_result = grim.to_ppm(&test_data, 2, 2);
        assert!(ppm_result.is_ok());
        let ppm_data = ppm_result.unwrap();
        assert!(ppm_data.starts_with(b"P6\n2 2\n255\n"));
        assert!(ppm_data.len() >= 12);
    }

    #[test]
    fn test_read_region_from_stdin() {
        let region_str = "10,20 300x400";
        let result: std::result::Result<Box, _> = region_str.parse();
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
        match test_capture {
            Ok(_) => {}
            Err(Error::NoOutputs) => {}
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }
}
