# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2025-10-04

### Added
- **Output Transform Support**: Full support for all 8 Wayland output transform types (Normal, 90°, 180°, 270°, Flipped, Flipped90, Flipped180, Flipped270)
  - Automatic detection and application of display rotation/flipping
  - Functions: `apply_output_transform()`, `apply_image_transform()`, `rotate_90/180/270()`, `flip_horizontal/vertical()`
- **Y-invert Flag Handling**: Proper handling of `ZWLR_SCREENCOPY_FRAME_V1_FLAGS_Y_INVERT` flag
  - Y-invert applied after output transform (per Wayland specification)
- **High-Quality Image Scaling**: Adaptive algorithm selection with 4-tier gradation for optimal quality/performance balance
  - Upscaling (>1.0): Triangle filter - smooth interpolation, avoids pixelation
  - Mild downscaling (0.75-1.0): Triangle - fast, high-quality for small changes
  - Moderate downscaling (0.5-0.75): CatmullRom - sharper results, faster than Lanczos3
  - Heavy downscaling (<0.5): Lanczos3 convolution - best quality for extreme reduction
  - New functions: `capture_all_with_scale()`, `capture_region_with_scale()`, `capture_output_with_scale()`
  - Comprehensive scaling demonstrations with real screenshots
- **XDG Pictures Directory Support**: Automatic file placement in user's Pictures folder
  - Parses `~/.config/user-dirs.dirs` for XDG_PICTURES_DIR
  - Priority system: `$GRIM_DEFAULT_DIR` → `$XDG_PICTURES_DIR` (env) → `XDG_PICTURES_DIR` (config) → current directory
  - Functions: `get_xdg_pictures_dir()`, `expand_home_dir()`, `get_output_dir()`
  - Full compatibility with original grim behavior
- **Human-Readable Filename Generation**: Improved default filename format for better usability
  - New format: `YYYYMMDD_HHhMMmSSs_grim.ext` (e.g., `20241004_14h25m30s_grim.png`)
  - Replaces Unix timestamp format (`1728023456.png`) with human-readable date/time
  - Benefits:
    - Instantly readable: shows exact date and time at a glance
    - Automatic chronological sorting in file managers
    - Source identification: `_grim` suffix identifies files created by grim-rs
    - Cross-platform safe: no spaces or special characters
  - Uses `chrono` crate for reliable datetime formatting
- **Grid-Aligned Compositing Detection**: Optimized multi-monitor compositing with layout analysis
  - New functions: `check_outputs_overlap()`, `is_grid_aligned()` for detecting non-overlapping layouts
  - Enhanced `composite_region()` with grid-aligned detection logic
  - Grid-aligned layouts (no overlaps) use optimized SRC-mode direct copy instead of OVER blending
  - Benefits:
    - Correct identification of layouts suitable for optimization
    - Foundation for future optimizations (e.g., parallel capture)
    - Better performance for standard multi-monitor setups
- **Enhanced Error Handling**: Improved error messages with detailed context information
  - New error types: `TransformNotSupported`, `InvertFailed`, `ScalingFailed`, `IoWithContext`
  - Buffer creation errors now include specific failure details and affected output names
  - File I/O errors now include operation context and file paths
  - Scaling errors include source and target dimensions for better debugging

### Changed
- **Multi-Monitor Compositing**: Simplified `capture_region()` implementation
  - Reduced from 162 lines to 4 lines (-158 lines)
  - Now properly calls `composite_region()` for correct multi-monitor handling
  - Regions spanning monitor boundaries are composited automatically
- **Image Processing Pipeline**: Enhanced processing flow
  - Wayland Screencopy → Buffer → Output Transform → Y-invert → Scaling → Format Conversion → Save
  - Transforms applied in correct order per Wayland specification
- **Default Filename Generation**: Now uses XDG Pictures directory by default
  - Respects `GRIM_DEFAULT_DIR` environment variable
  - Falls back gracefully to current directory if XDG not configured
- **Default Filename Format**: Changed from Unix timestamp to human-readable date format
  - Old: `1728023456.png` → New: `20241004_14h25m30s_grim_rs.png`
  - Easier to identify and sort screenshots by date/time

### Dependencies
- **Added**: `chrono = "0.4"` for improved datetime formatting in filenames
- **Added**: `regex = "1.10"` (dev-dependency) for filename format testing

### Fixed
- Multi-monitor region capture now correctly composites images from multiple outputs
- Output transform handling ensures screenshots are correctly oriented on rotated/flipped displays
- Y-invert flag properly handled for compositors that require vertical flipping
- Output detection reliability improved with protocol_id usage and proper event queue binding
- Fallback `guess_output_logical_geometry()` for systems without xdg_output_manager

### Performance
- Adaptive scaling algorithm selection optimizes speed vs quality trade-off
- Grid-aligned compositing detection enables optimized rendering path for non-overlapping monitors
- Direct memory copy (SRC mode) used when outputs don't overlap, avoiding unnecessary alpha blending

### Documentation
- Updated README.md with comprehensive API reference and usage examples

## [0.1.0] - 2025-09-23

### Added
- Initial release of grim-rs
- Pure Rust implementation of grim screenshot utility for Wayland
- Support for capturing entire screen (all outputs)
- Support for capturing specific output by name
- Support for capturing specific region
- Support for capturing multiple outputs with different parameters
- PNG output format support
- JPEG output format support (via feature flag)
- Comprehensive API documentation

### Changed
- Improved Wayland event handling
- Fixed hardcoded default values for outputs
- Enhanced error handling with more informative messages
- Better handling of output information mapping between wl_output and internal structures

### Fixed
- Removed debug prints from wayland_capture.rs
- Corrected buffer creation and management
- Fixed timeout handling when waiting for Wayland events