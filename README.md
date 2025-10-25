# grim-rs
[![Crates.io Version](https://img.shields.io/crates/v/grim-rs.svg)](https://crates.io/crates/grim-rs)
>if you like this project, then the best way to express gratitude is to give it a star ⭐, it doesn't cost you anything, but I understand that I'm moving the project in the right direction.

Rust implementation of `grim-rs` screenshot utility for Wayland compositors.

> **⚠️ Breaking Changes in v0.1.3**  
> Version 0.1.3 introduces breaking changes related to struct field encapsulation. See [MIGRATION.md](MIGRATION.md) for upgrade guide.

## Features

- **Pure Rust implementation** - no external dependencies on C libraries
- **Native Wayland protocol support** via `wayland-client`
- **Multi-monitor support** with automatic compositing across monitor boundaries
- **Output transforms** - full support for rotated/flipped displays (all 8 Wayland transform types)
- **High-quality image scaling** - 4-tier adaptive algorithm selection:
  - Upscaling (>1.0): Triangle filter for smooth interpolation
  - Mild downscaling (0.75-1.0): Triangle for fast, high-quality results
  - Moderate downscaling (0.5-0.75): CatmullRom for sharp results with good performance
  - Heavy downscaling (<0.5): Lanczos3 for best quality at extreme reduction
- **Region-based screenshot capture** with pixel-perfect accuracy
- **Multiple output formats**:
  - PNG with configurable compression (0-9)
  - JPEG with quality control (0-100)
  - PPM (uncompressed)
- **XDG Pictures directory support** - automatic file placement in `~/Pictures`
- **Y-invert flag handling** - correct screenshot orientation on all compositors
- **Cursor overlay support** (compositor-dependent)
- **Zero external tool dependencies**
- **Comprehensive API documentation** with examples

## Usage

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
grim-rs = "0.1.3"
```

**Upgrading from 0.1.2?** See [MIGRATION.md](MIGRATION.md) for breaking changes.

### Basic Capture Operations

```rust
use grim_rs::{Grim, Box};

fn main() -> grim_rs::Result<()> {
    let mut grim = Grim::new()?;
    
    // Capture entire screen (all outputs)
    let result = grim.capture_all()?;
    grim.save_png(result.data(), result.width(), result.height(), "screenshot.png")?;
    
    // Capture specific region (automatically composites across monitors)
    let region = Box::new(100, 100, 800, 600);
    let result = grim.capture_region(region)?;
    grim.save_png(result.data(), result.width(), result.height(), "region.png")?;
    
    // Capture specific output by name (handles transforms/rotation automatically)
    let result = grim.capture_output("DP-1")?;
    grim.save_png(result.data(), result.width(), result.height(), "output.png")?;
    
    Ok(())
}
```

### Getting Output Information

```rust
use grim_rs::Grim;

fn main() -> grim_rs::Result<()> {
    let mut grim = Grim::new()?;
    
    // Get list of available outputs with their properties
    let outputs = grim.get_outputs()?;
    for output in outputs {
        println!("Output: {}", output.name());
        println!("  Position: ({}, {})", output.geometry().x(), output.geometry().y());
        println!("  Size: {}x{}", output.geometry().width(), output.geometry().height());
        println!("  Scale: {}", output.scale());
        if let Some(desc) = output.description() {
            println!("  Description: {}", desc);
        }
    }
    
    Ok(())
}
```

### Capture with Scaling

```rust
use grim_rs::{Grim, Box};

fn main() -> grim_rs::Result<()> {
    let mut grim = Grim::new()?;
    
    // Capture entire screen with scaling (high-quality downscaling)
    let result = grim.capture_all_with_scale(0.5)?; // 50% size, uses Lanczos3 filter
    grim.save_png(result.data(), result.width(), result.height(), "thumbnail.png")?;
    
    // Capture region with scaling
    let region = Box::new(0, 0, 1920, 1080);
    let result = grim.capture_region_with_scale(region, 0.8)?; // 80% size, uses Triangle filter
    grim.save_png(result.data(), result.width(), result.height(), "scaled.png")?;
    
    // Capture specific output with scaling
    let result = grim.capture_output_with_scale("DP-1", 0.5)?;
    grim.save_png(result.data(), result.width(), result.height(), "output_scaled.png")?;
    
    Ok(())
}
```

### Multiple Output Capture

```rust
use grim_rs::{Grim, Box, CaptureParameters};

fn main() -> grim_rs::Result<()> {
    let mut grim = Grim::new()?;
    
    // Capture multiple outputs with different parameters
    let parameters = vec![
        CaptureParameters::new("DP-1")
            .overlay_cursor(true),
        CaptureParameters::new("HDMI-A-1")
            .region(Box::new(0, 0, 1920, 1080))
            .scale(0.5)
    ];
    
    let results = grim.capture_outputs(parameters)?;
    for (output_name, result) in results.into_outputs() {
        let filename = format!("{}.png", output_name);
        grim.save_png(result.data(), result.width(), result.height(), &filename)?;
    }
    
    Ok(())
}
```

### Saving to Different Formats

```rust
use grim_rs::Grim;

fn main() -> grim_rs::Result<()> {
    let mut grim = Grim::new()?;
    let result = grim.capture_all()?;
    
    // Save as PNG with default compression (level 6)
    grim.save_png(result.data(), result.width(), result.height(), "screenshot.png")?;
    
    // Save as PNG with custom compression (0-9, where 9 is highest)
    grim.save_png_with_compression(result.data(), result.width(), result.height(), "compressed.png", 9)?;
    
    // Save as JPEG with default quality (80)
    grim.save_jpeg(result.data(), result.width(), result.height(), "screenshot.jpg")?;
    
    // Save as JPEG with custom quality (0-100, where 100 is highest)
    grim.save_jpeg_with_quality(result.data(), result.width(), result.height(), "quality.jpg", 95)?;
    
    // Save as PPM (uncompressed)
    grim.save_ppm(result.data(), result.width(), result.height(), "screenshot.ppm")?;
    
    Ok(())
}
```

### Converting to Bytes (without saving to file)

```rust
use grim_rs::Grim;

fn main() -> grim_rs::Result<()> {
    let mut grim = Grim::new()?;
    let result = grim.capture_all()?;
    
    // Convert to PNG bytes
    let png_bytes = grim.to_png(result.data(), result.width(), result.height())?;
    println!("PNG size: {} bytes", png_bytes.len());
    
    // Convert to PNG bytes with custom compression
    let png_bytes = grim.to_png_with_compression(result.data(), result.width(), result.height(), 9)?;
    
    // Convert to JPEG bytes
    let jpeg_bytes = grim.to_jpeg(result.data(), result.width(), result.height())?;
    println!("JPEG size: {} bytes", jpeg_bytes.len());
    
    // Convert to JPEG bytes with custom quality
    let jpeg_bytes = grim.to_jpeg_with_quality(result.data(), result.width(), result.height(), 85)?;
    
    // Convert to PPM bytes
    let ppm_bytes = grim.to_ppm(result.data(), result.width(), result.height())?;
    println!("PPM size: {} bytes", ppm_bytes.len());
    
    Ok(())
}
```

### Writing to Stdout (for piping)

```rust
use grim_rs::Grim;

fn main() -> grim_rs::Result<()> {
    let mut grim = Grim::new()?;
    let result = grim.capture_all()?;
    
    // Write PNG to stdout
    grim.write_png_to_stdout(result.data(), result.width(), result.height())?;
    
    // Write PNG to stdout with custom compression
    grim.write_png_to_stdout_with_compression(result.data(), result.width(), result.height(), 6)?;
    
    // Write JPEG to stdout
    grim.write_jpeg_to_stdout(result.data(), result.width(), result.height())?;
    
    // Write JPEG to stdout with custom quality
    grim.write_jpeg_to_stdout_with_quality(result.data(), result.width(), result.height(), 90)?;
    
    // Write PPM to stdout
    grim.write_ppm_to_stdout(result.data(), result.width(), result.height())?;
    
    Ok(())
}
```

### Reading Region from Stdin

```rust
use grim_rs::Grim;

fn main() -> grim_rs::Result<()> {
    let mut grim = Grim::new()?;
    
    // Read region specification from stdin (format: "x,y widthxheight")
    let region = Grim::read_region_from_stdin()?;
    
    let result = grim.capture_region(region)?;
    grim.save_png(result.data(), result.width(), result.height(), "region.png")?;
    
    Ok(())
}
```

### Command Line Usage

The `grim-rs` binary supports the same functionality as the library API. By default, saves to `~/Pictures` (XDG Pictures directory) with timestamped filenames.

**Available Options:**
```bash
-h              Show help message and quit
-s <factor>     Set the output image's scale factor (e.g., 0.5 for 50%)
-g <geometry>   Set the region to capture (format: "x,y widthxheight")
-t png|ppm|jpeg Set the output filetype (default: png)
-q <quality>    Set the JPEG compression quality (0-100, default: 80)
-l <level>      Set the PNG compression level (0-9, default: 6)
-o <output>     Set the output name to capture (e.g., "DP-1", "HDMI-A-1")
-c              Include cursor in the screenshot
```

**Usage Examples:**

```bash
# Build the binary first
cargo build --release

# Capture entire screen (saves to ~/Pictures/<timestamp>.png)
cargo run --bin grim-rs

# Capture with specific filename
cargo run --bin grim-rs -- screenshot.png

# Capture specific region
cargo run --bin grim-rs -- -g "100,100 800x600" region.png

# Capture with scaling (50% size, creates thumbnail)
cargo run --bin grim-rs -- -s 0.5 thumbnail.png

# Capture specific output by name
cargo run --bin grim-rs -- -o DP-1 monitor.png

# Capture with cursor included
cargo run --bin grim-rs -- -c -o DP-1 with_cursor.png

# Save as JPEG with custom quality
cargo run --bin grim-rs -- -t jpeg -q 90 screenshot.jpg

# Save as PNG with maximum compression
cargo run --bin grim-rs -- -l 9 compressed.png

# Save as PPM (uncompressed)
cargo run --bin grim-rs -- -t ppm screenshot.ppm

# Combine options: region + scaling + cursor
cargo run --bin grim-rs -- -g "0,0 1920x1080" -s 0.8 -c scaled_region.png

# Capture to stdout and pipe to another program
cargo run --bin grim-rs -- - > screenshot.png

# Save to custom directory via environment variable
GRIM_DEFAULT_DIR=/tmp cargo run --bin grim-rs

# Read region from stdin
echo "100,100 800x600" | cargo run --bin grim-rs -- -g -
```

**Using the installed binary:**

After installation with `cargo install grim-rs`, you can use it directly:

```bash
# Capture entire screen
grim-rs

# All the same options work without 'cargo run'
grim-rs -g "100,100 800x600" -s 0.5 thumbnail.png
grim-rs -o DP-1 -c monitor.png
grim-rs - | wl-copy  # Pipe to clipboard
```

**Note:** The binary is named `grim-rs` to avoid conflicts with the original C implementation of `grim`.

### Supported Wayland Protocols

- `wl_shm` - Shared memory buffers
- `zwlr_screencopy_manager_v1` - Screenshot capture (wlroots extension)
- `wl_output` - Output information

## API Reference

### Core Methods

#### Initialization
- `Grim::new()` - Create new Grim instance and connect to Wayland compositor

#### Getting Display Information
- `get_outputs()` - Get list of available outputs with their properties (name, geometry, scale)

#### Capture Methods
- `capture_all()` - Capture entire screen (all outputs)
- `capture_all_with_scale(scale: f64)` - Capture entire screen with scaling
- `capture_output(output_name: &str)` - Capture specific output by name
- `capture_output_with_scale(output_name: &str, scale: f64)` - Capture output with scaling
- `capture_region(region: Box)` - Capture specific rectangular region
- `capture_region_with_scale(region: Box, scale: f64)` - Capture region with scaling
- `capture_outputs(parameters: Vec<CaptureParameters>)` - Capture multiple outputs with different parameters
- `capture_outputs_with_scale(parameters: Vec<CaptureParameters>, default_scale: f64)` - Capture multiple outputs with scaling

#### Saving to Files
- `save_png(&data, width, height, path)` - Save as PNG with default compression (level 6)
- `save_png_with_compression(&data, width, height, path, compression: u8)` - Save as PNG with custom compression (0-9)
- `save_jpeg(&data, width, height, path)` - Save as JPEG with default quality (80) [requires `jpeg` feature]
- `save_jpeg_with_quality(&data, width, height, path, quality: u8)` - Save as JPEG with custom quality (0-100) [requires `jpeg` feature]
- `save_ppm(&data, width, height, path)` - Save as PPM (uncompressed)

#### Converting to Bytes
- `to_png(&data, width, height)` - Convert to PNG bytes with default compression
- `to_png_with_compression(&data, width, height, compression: u8)` - Convert to PNG bytes with custom compression
- `to_jpeg(&data, width, height)` - Convert to JPEG bytes with default quality [requires `jpeg` feature]
- `to_jpeg_with_quality(&data, width, height, quality: u8)` - Convert to JPEG bytes with custom quality [requires `jpeg` feature]
- `to_ppm(&data, width, height)` - Convert to PPM bytes

#### Writing to Stdout
- `write_png_to_stdout(&data, width, height)` - Write PNG to stdout with default compression
- `write_png_to_stdout_with_compression(&data, width, height, compression: u8)` - Write PNG to stdout with custom compression
- `write_jpeg_to_stdout(&data, width, height)` - Write JPEG to stdout with default quality [requires `jpeg` feature]
- `write_jpeg_to_stdout_with_quality(&data, width, height, quality: u8)` - Write JPEG to stdout with custom quality [requires `jpeg` feature]
- `write_ppm_to_stdout(&data, width, height)` - Write PPM to stdout

#### Stdin Input
- `Grim::read_region_from_stdin()` - Read region specification from stdin (format: "x,y widthxheight")

### Data Structures

#### `CaptureResult`
Contains captured image data:
- `data: Vec<u8>` - Raw RGBA image data (4 bytes per pixel)
- `width: u32` - Image width in pixels
- `height: u32` - Image height in pixels

#### `CaptureParameters`
Parameters for capturing specific outputs:
- `output_name: String` - Name of the output to capture
- `region: Option<Box>` - Optional region within the output
- `overlay_cursor: bool` - Whether to include cursor in capture
- `scale: Option<f64>` - Optional scale factor for the output

#### `MultiOutputCaptureResult`
Result of capturing multiple outputs:
- `outputs: HashMap<String, CaptureResult>` - Map of output names to their capture results

#### `Output`
Information about a display output:
- `name: String` - Output name (e.g., "eDP-1", "HDMI-A-1")
- `geometry: Box` - Output position and size
- `scale: i32` - Scale factor (1 for normal DPI, 2 for HiDPI)
- `description: Option<String>` - Monitor model and manufacturer information

#### `Box`
Rectangular region:
- `x: i32` - X coordinate
- `y: i32` - Y coordinate
- `width: i32` - Width
- `height: i32` - Height
- Can be parsed from string: "x,y widthxheight"

### Feature Flags

- **`jpeg`** - Enable JPEG support (enabled by default)
  - Adds `save_jpeg*`, `to_jpeg*`, and `write_jpeg_to_stdout*` methods
  
To disable JPEG support:
```toml
[dependencies]
grim-rs = { version = "0.1.0", default-features = false }
```

## Full API Documentation

Comprehensive API documentation is available at [docs.rs](https://docs.rs/grim-rs) or can be generated locally:

```bash
cargo doc --open
```

## Comparison with Original grim

| Feature | Original grim | grim-rs |
|---------|---------------|---------|
| Language | C | Rust |
| Dependencies | libpng, pixman, wayland, libjpeg | Pure Rust crates |
| Output formats | PNG, JPEG, PPM | PNG, JPEG, PPM |
| Installation | System package | Rust crate |
| Integration | External process | Library + Binary |
| Memory safety | Manual | Guaranteed by Rust |
| Output transforms | ✅ | ✅ |
| Y-invert handling | ✅ | ✅ |
| Multi-monitor compositing | ✅ | ✅ |
| Image scaling | Nearest-neighbor | 4-tier adaptive (Triangle/CatmullRom/Lanczos3) |
| XDG Pictures support | ✅ | ✅ |
| Output descriptions | ✅ | ✅ |
| Color accuracy | ✅ | ✅ |
| Real capture | ✅ | ✅ |

## Architecture

```
┌─────────────────┐
│   Application   │
├─────────────────┤
│    grim-rs      │
├─────────────────┤
│ wayland-client  │
├─────────────────┤
│    Wayland      │
│   Compositor    │
└─────────────────┘
```

### Key Components

1. **Grim** - Main interface for taking screenshots
2. **CaptureResult** - Contains screenshot data and dimensions
3. **CaptureParameters** - Parameters for multi-output capture
4. **Buffer** - Shared memory buffer management
5. **Box** - Region and coordinate handling
6. **Output** - Monitor information with transform support
7. **Error** - Comprehensive error handling

### Image Processing Pipeline

```
Wayland Screencopy → Buffer → Output Transform → Y-invert → Scaling → Format Conversion → Save
                                    ↓                ↓          ↓
                             (rotation/flip)   (vertical)  (Bilinear/Lanczos3)
```

### Scaling Quality

Adaptive 4-tier algorithm selection ensures optimal quality/performance balance:

- **Upscaling (scale > 1.0)**: Triangle filter
  - Smooth interpolation for enlarging images
  - Avoids pixelation when scaling up
  - Example: 1920×1080 → 2560×1440 (1.33×)

- **Mild downscaling (0.75 ≤ scale ≤ 1.0)**: Triangle filter
  - Fast, high-quality for small size reductions
  - Perfect for minor adjustments: 1920×1080 → 1536×864 (0.8×)
  
- **Moderate downscaling (0.5 ≤ scale < 0.75)**: CatmullRom filter
  - Sharper results than Triangle
  - Better performance than Lanczos3
  - Ideal for medium reduction: 1920×1080 → 1280×720 (0.67×)

- **Heavy downscaling (scale < 0.5)**: Lanczos3 convolution
  - Best quality for significant reduction
  - Ideal for thumbnails: 3840×2160 → 960×540 (0.25×)
  - Superior detail preservation at extreme scales

## Environment Variables

- **`GRIM_DEFAULT_DIR`** - Override default screenshot directory (highest priority)
- **`XDG_PICTURES_DIR`** - XDG Pictures directory (from env or `~/.config/user-dirs.dirs`)

Priority order: `GRIM_DEFAULT_DIR` → `XDG_PICTURES_DIR` → current directory

## Supported Compositors

- ✅ Hyprland
- ✅ Sway
- ✅ River
- ✅ Wayfire
- ✅ Any wlroots-based compositor with `zwlr_screencopy_manager_v1`

## Limitations

- Requires compositor with `zwlr_screencopy_manager_v1` protocol support
- Linux-only (due to shared memory implementation)
- Cursor overlay depends on compositor support

## Building

```bash
cd grim-rs
cargo build --release
```

## Testing

```bash
# Run tests
cargo test

# Run examples
cargo run --example simple all screenshot.png
cargo run --example multi_output
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes
4. Add tests
5. Submit a pull request

## License

MIT License - see [LICENSE](./LICENSE) file for details.
