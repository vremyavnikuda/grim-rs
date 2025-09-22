# grim-rs

Pure Rust implementation of `grim` screenshot utility for Wayland compositors.

## Features

- ✅ Pure Rust implementation - no external dependencies on C libraries
- ✅ Native Wayland protocol support via `wayland-client`
- ✅ Multiple output support
- ✅ Region-based screenshot capture
- ✅ PNG output format
- ✅ Real screenshot capture (not mock data)
- ✅ Correct color palette transformation
- ✅ Zero external tool dependencies (no need for system `grim`)

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
    grim.save_png(&data, 1920, 1080, "screenshot.png")?;
    
    // Capture specific region
    let region = Box::new(100, 100, 800, 600);
    let data = grim.capture_region(region)?;
    grim.save_png(&data, 800, 600, "region.png")?;
    
    // Capture specific output
    let data = grim.capture_output("DP-1")?;
    grim.save_png(&data, 1920, 1080, "output.png")?;
    
    Ok(())
}
```

### Supported Wayland Protocols

- `wl_shm` - Shared memory buffers
- `zwlr_screencopy_manager_v1` - Screenshot capture (wlroots extension)
- `wl_output` - Output information

## Comparison with Original grim

| Feature | Original grim | grim-rs |
|---------|---------------|---------|
| Language | C | Rust |
| Dependencies | libpng, pixman, wayland | Pure Rust crates |
| Output formats | PNG, JPEG, PPM | PNG (extensible) |
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

1. **Screenshot** - Main screenshot capture logic
2. **Buffer** - Shared memory buffer management
3. **Geometry** - Region and coordinate handling
4. **Error** - Comprehensive error handling

## Limitations

- Currently supports PNG output only (JPEG and PPM can be added)
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

# Run example
cargo run --example simple all screenshot.png
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes
4. Add tests
5. Submit a pull request

## License

MIT License - see LICENSE file for details.