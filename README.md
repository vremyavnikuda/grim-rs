# grim-rs
[![Crates.io Version](https://img.shields.io/crates/v/grim-rs.svg)](https://crates.io/crates/grim-rs)

Rust implementation of `grim-rs` screenshot utility for Wayland compositors.

## Features

- Rust implementation - no external dependencies on C libraries
- Native Wayland protocol support via `wayland-client`
- Multiple output support
- Region-based screenshot capture
- PNG and JPEG output formats
- Real screenshot capture
- Correct color palette transformation
- Zero external tool dependencies
- Comprehensive API documentation

## Usage

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
grim-rs = "0.1.0"
```

Basic usage:

```rust
use grim_rs::{Grim, Box};

fn main() -> grim_rs::Result<()> {
    let mut grim = Grim::new()?;
    
    // Capture entire screen
    let data = grim.capture_all()?;
    grim.save_png(&data.data, data.width, data.height, "screenshot.png")?;
    
    // Capture specific region
    let region = Box::new(100, 100, 800, 600);
    let data = grim.capture_region(region)?;
    grim.save_png(&data.data, data.width, data.height, "region.png")?;
    
    // Capture specific output
    let data = grim.capture_output("DP-1")?;
    grim.save_png(&data.data, data.width, data.height, "output.png")?;
    
    // Capture multiple outputs with different parameters
    let parameters = vec![
        grim_rs::CaptureParameters {
            output_name: "DP-1".to_string(),
            // Capture entire output
            region: None,
            // Include cursor
            overlay_cursor: true,
        },
        grim_rs::CaptureParameters {
            output_name: "HDMI-A-1".to_string(),
            // Capture specific region
            region: Some(Box::new(0, 0, 1920, 1080)),
            // Exclude cursor
            overlay_cursor: false,
        }
    ];
    let results = grim.capture_outputs(parameters)?;
    for (output_name, capture_result) in results.outputs {
        let filename = format!("{}.png", output_name);
        grim.save_png(&capture_result.data, capture_result.width, capture_result.height, &filename)?;
    }
    
    Ok(())
}
```

### Command Line Usage

```bash
# List available outputs
cargo run --example simple outputs

# Capture entire screen
cargo run --example simple all screenshot.png

# Capture specific region
cargo run --example simple geometry "100,100 800x600" region.png
```

### Supported Wayland Protocols

- `wl_shm` - Shared memory buffers
- `zwlr_screencopy_manager_v1` - Screenshot capture (wlroots extension)
- `wl_output` - Output information

## API Documentation

Comprehensive API documentation is available at [docs.rs](https://docs.rs/grim-rs) or can be generated locally:

```bash
cargo doc --open
```

## Comparison with Original grim

| Feature | Original grim | grim-rs |
|---------|---------------|---------|
| Language | C | Rust |
| Dependencies | libpng, pixman, wayland | Pure Rust crates |
| Output formats | PNG, JPEG, PPM | PNG, JPEG (extensible) |
| Installation | System package | Rust crate |
| Integration | External process | Library |
| Memory safety | Manual | Guaranteed by Rust |
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
5. **Geometry** - Region and coordinate handling
6. **Error** - Comprehensive error handling

## Limitations

- Requires wlroots-based compositor (Hyprland, Sway, etc.)
- Linux-only (due to shared memory implementation)

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

MIT License - see LICENSE file for details.