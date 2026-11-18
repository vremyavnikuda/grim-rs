# API Reference (`grim-rs`)

This is a practical API index for the public surface.
For full rustdoc details, see [docs.rs/grim-rs](https://docs.rs/grim-rs).

> an example of usage is shown in [library examples](/doc/library_examples.md)

## Core Methods

### Initialization

- `Grim::new()` - Create new Grim instance with auto-detected backend (Linux: prefers `ext-image-copy-capture-v1`, falls back to `wlr-screencopy`; Windows: DXGI Desktop Duplication)
- `Grim::new_ext()` - Force `ext-image-copy-capture-v1` backend (**Linux/Wayland only** — fails on Windows)
- `Grim::new_wlr()` - Force `wlr-screencopy` backend (**Linux/Wayland only** — fails on Windows)
- `Backend` enum — `Auto`, `ExtImageCopyCapture` (Linux only), `WlrScreencopy` (Linux only)

### Getting Display Information

- `get_outputs()` - Get list of available outputs with their properties (name, geometry, scale); returns `Error::NoOutputs` when no outputs are available (same contract on Linux and Windows)

### Capture Methods

- `capture_all()` - Capture entire screen (all outputs)
- `capture_all_with_scale(scale: f64)` - Capture entire screen with scaling
- `capture_output(output_name: &str)` - Capture specific output by name
- `capture_output_with_scale(output_name: &str, scale: f64)` - Capture output with scaling
- `capture_region(region: Region)` - Capture specific rectangular region
- `capture_region_with_scale(region: Region, scale: f64)` - Capture region with scaling
- `capture_outputs(parameters: Vec<CaptureParameters>)` - Capture multiple outputs with different parameters
- `capture_outputs_with_scale(parameters: Vec<CaptureParameters>, default_scale: f64)` - Capture multiple outputs with scaling

### Saving to Files

- `save_png(&data, width, height, path)` - Save as PNG with default compression (level 6)
- `save_png_with_compression(&data, width, height, path, compression: u8)` - Save as PNG with custom compression
- `save_jpeg(&data, width, height, path)` - Save as JPEG with default quality (80) [requires `jpeg` feature]
- `save_jpeg_with_quality(&data, width, height, path, quality: u8)` - Save as JPEG with custom quality (0-100) [requires `jpeg` feature]

### Converting to Bytes

- `to_png(&data, width, height)` - Convert to PNG bytes with default compression
- `to_png_with_compression(&data, width, height, compression: u8)` - Convert to PNG bytes with custom compression
- `to_jpeg(&data, width, height)` - Convert to JPEG bytes with default quality [requires `jpeg` feature]
- `to_jpeg_with_quality(&data, width, height, quality: u8)` - Convert to JPEG bytes with custom quality [requires `jpeg` feature]

### Writing to Stdout

- `write_png_to_stdout(&data, width, height)` - Write PNG to stdout with default compression
- `write_png_to_stdout_with_compression(&data, width, height, compression: u8)` - Write PNG to stdout with custom compression
- `write_jpeg_to_stdout(&data, width, height)` - Write JPEG to stdout with default quality [requires `jpeg` feature]
- `write_jpeg_to_stdout_with_quality(&data, width, height, quality: u8)` - Write JPEG to stdout with custom quality [requires `jpeg` feature]

### Stdin Input

- `Grim::read_region_from_stdin()` - Read region specification from stdin (format: `"x,y widthxheight"`)

## Data Structures

### `CaptureResult`

- Fields are private (encapsulated)
- `data()` - Raw RGBA image data as `&[u8]`
- `width()` - Image width in pixels
- `height()` - Image height in pixels
- `into_data()` - Consume and return owned pixel buffer

### `CaptureParameters`

- Fields are private (builder + getters API)
- `CaptureParameters::new(output_name)` - Create parameters for an output
- Builder methods: `.region(...)`, `.overlay_cursor(...)`
- Accessors: `output_name()`, `region_ref()`, `overlay_cursor_enabled()`
- Deprecated: `.scale(...)` / `scale_factor()` (since 0.2.2) — per-output scale was never implemented by any backend and the value is ignored. Scale the whole capture via the `capture_*_with_scale(...)` methods instead.

### `MultiOutputCaptureResult`

- Fields are private
- `get(output_name)` - Get one output result by name
- `outputs()` - Borrow all output results
- `into_outputs()` - Consume and return `HashMap<String, CaptureResult>`

### `Output`

- Fields are private
- `name()` - Output name (e.g., `eDP-1`, `HDMI-A-1`)
- `geometry()` - Output position and size (`Region`)
- `scale()` - Output scale factor
- `description()` - Optional monitor description

### `Region`

- Fields are private
- `Region::new(x, y, width, height)` - Create region
- Accessors: `x()`, `y()`, `width()`, `height()`
- Utilities: `is_empty()`, `intersects(...)`, `intersection(...)`
- Parse from string: `"x,y widthxheight"`

### `pixel_format` Module

Public pixel format conversion utilities, independent of Wayland types.

- `PixelFormat` enum — `Argb8888`, `Xrgb8888`, `Abgr8888`, `Xbgr8888`
- `fourcc_to_format(fourcc: u32) -> Option<PixelFormat>` — map DRM fourcc to PixelFormat
- `convert_to_rgba(data: &mut [u8], format: PixelFormat)` — in-place byte swizzle to RGBA

```rust
use grim_rs::pixel_format::{self, PixelFormat};

let mut pixels = get_raw_frame();
pixel_format::convert_to_rgba(&mut pixels, PixelFormat::Argb8888);

if let Some(fmt) = pixel_format::fourcc_to_format(0x34325241) {
    pixel_format::convert_to_rgba(&mut pixels, fmt);
}
```

## Feature Flags

- **`png_support`** - Enable PNG support (enabled by default). When disabled, the PNG methods are compiled as stubs that return an `Unsupported` error, so code calling them still builds.
- **`jpeg`** - Enable JPEG support (enabled by default)
  - Adds `save_jpeg*`, `to_jpeg*`, and `write_jpeg_to_stdout*` methods

To disable default features:

```toml
[dependencies]
grim-rs = { version = "0.2", default-features = false }
```
