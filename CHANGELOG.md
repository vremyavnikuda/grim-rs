# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.1] 2026-11-11

### Fixed

- **PNG compression levels 1–3 requested maximum compression instead of fast**: Levels 1–3 of the 0–9 scale mapped to `png::Compression::Best`, producing output identical to levels 7–9 and turning "fast, weak compression" into the slowest option. Levels are now quantized monotonically onto the three modes the `png` crate exposes (the `Huffman`/`Rle` variants are deprecated since 0.17.6): 0–2 fast, 3–6 default, 7–9 best. Default level 6 is unchanged. Measured on a 5360×1440 capture: level 0 yields 30.6 MB, level 9 — 706 KB.
- **JPEG encoding silently corrupted images wider or taller than 65535 px**: The JPEG header stores dimensions as `u16`, so oversized captures were written with wrapped dimensions and an unreadable file. `save_jpeg`, `save_jpeg_with_quality`, `to_jpeg`, and `to_jpeg_with_quality` now reject such sizes with a `DimensionMismatch` error before any encoding work.
- **ext-image-copy-capture returned a blank image when the compositor aborted the capture**: Both failure signals — `stopped` on the capture session (screen lock, revoked permission) and `failed` on the frame — only set the ready flag, and since the pixel buffer is allocated up front, the capture completed "successfully" with an all-zero image. Both paths now surface a `FrameCapture` error with the reason, in single-output, region, and multi-output captures.

### Changed

- **Dependency updates**: Bumped `crossbeam-epoch` to v0.9.20, `quick-xml` to v0.41 (via `wayland-scanner` v0.31.11), `anyhow` to v1.0.104, `memmap2` to v0.9.12, `rand` to v0.8.8/v0.9.5 — clears all `cargo audit` vulnerabilities (RUSTSEC-2026-0204, RUSTSEC-2026-0194, RUSTSEC-2026-0195) and unsound warnings. Lockfile-only; the one remaining advisory (`paste` unmaintained) has no fix and does not fail CI.
- **Replaced deprecated iterator idioms**: `chunks_exact(4)`/`chunks_exact_mut(4)` swapped for `as_chunks::<4>()`/`as_chunks_mut::<4>()` in the RGBA→RGB conversion and pixel format swizzling. Resolves the `chunks_exact_to_as_chunks` lint that fails the clippy CI job (`-D warnings`) on current stable toolchains.
- **Documentation cleanup**: removed stray text accidentally embedded in the `write_jpeg_to_stdout_with_quality` doc comment.

## [0.2.0] 2026-06-11

### Added

- **Windows backend via DXGI Desktop Duplication API**: `grim-rs` now supports native screenshot capture on Windows 8+. Uses `IDXGIOutputDuplication` with D3D11 hardware acceleration. All public API methods work identically on both Linux and Windows — `Grim::new()` auto-detects the platform (DXGI on Windows, Wayland protocols on Linux). Added cursor overlay support on Windows via `GetFramePointerShape`.
- **Platform abstraction layer**: Internal `Platform` enum with `Wayland` and `Windows` variants providing static dispatch — no trait objects, no allocation overhead. Shared compositing, scaling, buffer management, and transform logic across both platforms.
- **New error variants**: `DirectXError`, `ProtectedContent`, `NoGpuAdapter` for Windows-specific failure modes.
- **`windows` crate dependency**: Windows-only dependency (`windows = "0.58"`) with Direct3D, D3D11, DXGI, GDI, and Threading features — zero dependency cost on Linux.
- **`examples/windows_capture.rs`**: Full-featured Windows capture example demonstrating all API methods.

### Changed

- **`Grim::new_ext()` and `Grim::new_wlr()`**: Now return `Error::UnsupportedProtocol` when called on Windows — these backends are Wayland-only.
- **Dependency organization**: `wayland-client`, `wayland-protocols`, `wayland-protocols-wlr`, `memmap2`, and `tempfile` are now `cfg(target_os = "linux")`-gated.
## [0.1.9] 2026-05-18

### Added

- **`ext-image-copy-capture-v1` backend**: New capture protocol supported alongside the existing `wlr-screencopy`. Auto-detection prefers `ext-image-copy-capture-v1` when available (Sway ≥ 2025, Hyprland, COSMIC), falling back to `wlr-screencopy`. New constructors: `Grim::new_ext()` and `Grim::new_wlr()` to force a specific backend. `Grim::new()` continues to work with auto-detection. Public `Backend` enum exposed: `Auto`, `ExtImageCopyCapture`, `WlrScreencopy`. See [library examples — Backend selection](doc/library_examples.md#backend-selection) for usage.
- **`pixel_format` module**: Public pixel format conversion utilities with `PixelFormat` enum (`Argb8888`, `Xrgb8888`, `Abgr8888`, `Xbgr8888`), `fourcc_to_format()` for DRM fourcc mapping, and `convert_to_rgba()` for in-place byte swizzle. Fully tested (9 tests). No Wayland dependency — usable for any RGBA pixel processing.

### Changed

- **`Box` renamed to `Region`**: The geometry struct `Box` has been renamed to `Region` to eliminate the name collision with `std::boxed::Box`. This is a breaking change — all imports must change from `use grim_rs::Box` (or `use grim_rs::geometry::Box`) to `use grim_rs::Region`. The migration is a simple find-and-replace: `Box::new(` → `Region::new(`, `: Box` → `: Region`. See [MIGRATION.md](MIGRATION.md) for full details.

### Fixed

- **dmabuf frame delivery for `zwlr_screencopy`**: Completed the `LinuxDmabuf` event handler so frames delivered via dmabuf (instead of `wl_shm`) are correctly processed. Captures no longer time out on compositors that prefer dmabuf — the handler populates frame dimensions and format, and the existing shm-buffer path performs the cross-device copy transparently. When a compositor sends both `Buffer` and `LinuxDmabuf` events, the `Buffer` format takes precedence to avoid mismatches.

## [0.1.8] 2026-05-14

### Removed

- **PPM output format**: Removed `save_ppm`, `to_ppm`, and `write_ppm_to_stdout` from the public API. Removed `-t ppm` from the CLI. PPM is an uncompressed format with no practical advantage over PNG for screenshots — shrinking the API surface eliminates ~80 lines of encode/decode glue with zero user impact. Migration: [MIGRATION.md](MIGRATION.md).

## [0.1.7] 2026-05-05

### Fixed

- **Rotated output capture on vertical monitors**: Fixed incorrect screencopy region handling by keeping full-output and region requests in output-local logical coordinates. Fixes [#16](https://github.com/shikoucore/grim-rs/issues/16).
- **Cursor capture orientation on rotated outputs**: Applied output transform and `Y-invert` consistently in the multi-output cursor path so rotated captures are returned with the correct orientation.

### Changed

- **Multi-output capture flow**: Refreshed output state before multi-output capture and aligned full-output requests with logical output dimensions.
- **CI checks**: Tightened the all-features pipeline and made strict clippy warnings blocking for the main workflow.

## [0.1.6] 2026-03-04

### Fixed

- **Incorrect color channels with `wl_shm::Argb8888`**: Fixed red/blue channel swap in screenshots on setups where screencopy reports `Argb8888` (e.g. AMD/Hyprland) [#14](https://github.com/shikoucore/grim-rs/issues/14) (thx [Windblows2000](https://github.com/Windblows2000)) by converting `BGRA` memory layout to crate-level `RGBA`..

### Changed

- **Unified shm format conversion**: Centralized `wl_shm` → `RGBA` byte conversion in a single internal helper and reused it in both single-output and multi-output capture paths to keep behavior consistent.
- **Format handling parity**: Aligned default format fallback behavior between single and multi-output capture paths.

## [0.1.5] 2026-02-06

### Fixed

- **Safe buffer sizing**: Added checked buffer size calculations with a global pixel limit to prevent overflow and OOM during capture, scaling, and compositing.

### Performance

- **Lower peak memory during PNG/JPEG encoding**: Removed redundant full-frame copies when encoding from RGBA buffers, reducing peak allocations on large images.

### Documentation

- **Profiling guide**: Added a step-by-step [profiling manual](doc/profiling_manual.md) with a reproducible workflow.

### Changed

- **Dependency cleanup**: Removed unused `anyhow` and moved `env_logger` to dev-dependencies.
- **Dependency update**: Bumped `log` to v0.4.29.
- **Dependency update**: Bumped `chrono` to v0.4.43.
- **Dependency update**: Bumped `tempfile` to v3.24.0.
- **Dependency update**: Bumped `memmap2` to v0.9.9.
- **Dependency pin**: Kept `image` on v0.25.8 after testing showed regressions with newer versions.
- **Dependency update**: Bumped `jpeg-encoder` to v0.7.0.
- **Dependency update**: Bumped `thiserror` to v2.0.18.
- **MSRV**: Minimum supported Rust version is now 1.68.

### Testing

- **Integration test cleanup**: Moved `lib.rs` tests into the `tests/` suite and aligned assertions with public getters.

## [0.1.4]

### Fixed

- **Wayland region capture under fractional scaling**: Corrected logical/physical mapping by using a fractional logical scale (when available) and floor/ceil rounding to avoid incorrect region sizes. Fixes [#9](https://github.com/shikoucore/grim-rs/issues/9), [@Jeremis70](https://github.com/Jeremis70).

## [0.1.3] - 2025-10-11

### Changed

- **Box struct encapsulation**: Made all fields (`x`, `y`, `width`, `height`) private
  - Added getter methods: `x()`, `y()`, `width()`, `height()`
  - Migration: [doc](./MIGRATION.md)
- **CaptureResult struct encapsulation**: Made all fields (`data`, `width`, `height`) private
  - Added getter methods: `data()` → `&[u8]`, `width()` → `u32`, `height()` → `u32`
  - Added `into_data(self)` → `Vec<u8>` for ownership transfer without cloning
  - Added `new(data, width, height)` constructor for creating instances
  - Migration: [doc](./MIGRATION.md)
- **Output struct encapsulation**: Made all fields (`name`, `geometry`, `scale`, `description`) private
  - Added getter methods: `name()` → `&str`, `geometry()` → `&Box`, `scale()` → `i32`, `description()` → `Option<&str>`
  - Migration: [doc](./MIGRATION.md)
- **CaptureParameters struct encapsulation with Builder Pattern**: Made all fields private
  - Fields: `output_name`, `region`, `overlay_cursor`, `scale`
  - Added builder pattern: `CaptureParameters::new(name).region(box).overlay_cursor(true).scale(1.5)`
  - Added getters: `output_name()` → `&str`, `region_ref()` → `Option<&Box>`, `overlay_cursor_enabled()` → `bool`, `scale_factor()` → `Option<f64>`
  - Migration: [doc](./MIGRATION.md)
- **MultiOutputCaptureResult struct encapsulation**: Made `outputs` field private
  - Added methods: `get(name)` → `Option<&CaptureResult>`, `outputs()` → `&HashMap<String, CaptureResult>`, `into_outputs()` → `HashMap<String, CaptureResult>`
  - Added constructor: `new(outputs)` for creating instances
  - Migration: [doc](./MIGRATION.md)

### Fixed

- **Critical bug in `capture_outputs()`**: Fixed issue where all captures used the first output instead of the specific output for each parameter.

### Improved

- Better API design following Rust conventions
- More efficient data access with `data()` returning `&[u8]` slice instead of owned `Vec<u8>`
- Ownership transfer optimization with `into_data()` method
- Improved error handling: replaced all `.unwrap()` calls in production code with proper error propagation
- Created `lock_frame_state()` helper function for safe mutex locking. Prevents panics from poisoned mutex errors
- Removed `impl Default for Grim` to follow Rust API guidelines (Default should not panic)
- Code quality improvements following Rust best practices:
  - Removed unnecessary parentheses around `let` expressions
  - Simplified duplicate conditional branches in image scaling filter selection
  - Replaced manual range checks with `.contains()` method for clearer intent
  - Replaced verbose `match` statements with `if let` for single-pattern destructuring
  - Replaced `Iterator::flatten()` with `map_while(Result::ok)` to prevent potential infinite loops on errors
  - Replaced unnecessary `vec![]` allocations with stack-allocated arrays where heap allocation not needed
  - Removed needless borrows in multiple locations for cleaner code
  - Replaced `.map_or(false, |s| ...)` with `.is_some_and(|s| ...)` for better readability
  - Removed unused functions: `get_output_rotation()`, `get_output_flipped()`, `check_outputs_overlap()`, `is_grid_aligned()`
  - Removed unused variables: `_grid_aligned`, `_scaled_region`
  - Created clean function hierarchy: `save_or_write_result()` → format dispatchers → format-specific handlers
  - Improved maintainability: each function has single responsibility
  - Centralized error handling with proper `#[cfg(feature = "jpeg")]` attributes

### Performance

- Optimized memory usage by removing unnecessary cloning:
  - Eliminated redundant `WlOutput` clone in `capture_region()` that was immediately borrowed
  - Reduced Arc reference counting overhead by one clone per output in multi-monitor scenarios

### Testing

- Added comprehensive test coverage

## [0.1.2] - 2025-10-04

### Fixed issues: [#2](https://github.com/vremyavnikuda/grim-rs/issues/2)

- **Multi-Monitor Capture Compositing**: Fixed critical issue where capturing multiple monitors would overlay images on top of each other instead of placing them side-by-side
  - Root cause: Mixing of logical and physical coordinates during image composition
  - Solution: Proper coordinate transformation between logical and physical spaces with scale factor handling
  - The fix ensures correct layout for multi-monitor setups:
    - Before: Two monitors (3440x1440 + 1920x1080) created overlapped images
    - After: Correctly creates 5360x1440 pixel image with monitors side-by-side
  - Changes in `composite_region()`:
    - Convert logical coordinates to physical coordinates before capture (multiply by scale factor)
    - Capture images in physical pixel space
    - Scale captured images back to logical size for proper composition
    - Composite in logical coordinate space with correct offsets
  - Updated helper functions to use logical coordinates consistently:
    - `check_outputs_overlap()` - now uses logical dimensions for overlap detection
    - `is_grid_aligned()` - now uses logical dimensions for layout analysis
    - `capture_all()` and `capture_all_with_scale()` - calculate bounding box using logical coordinates

### Performance

- Multi-monitor capture now correctly positions images without overlapping, resulting in expected memory usage and correct visual output

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
- **Output Description Support**: Added comprehensive support for display output descriptions
  - New `description: Option<String>` field in `Output` struct providing monitor model and manufacturer information
  - Automatically captures descriptions from both `wl_output::Event::Description` and `zxdg_output_v1::Event::Description` Wayland protocols
  - Provides detailed information about connected displays (e.g., "Dell Inc. DELL U2520D", "Samsung Electric Company S27R35x")
  - Useful for multi-monitor setups to identify specific displays by their hardware description
  - Returns `None` if compositor doesn't provide description information
  - Full compatibility with original grim's output information API
  - Benefits:
    - Easier identification of specific monitors in multi-display configurations
    - Better logging and debugging with human-readable display information
    - Enables display-specific capture logic based on hardware model
    - Consistent with Wayland protocol specifications for output metadata

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
