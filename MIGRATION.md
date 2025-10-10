# Migration Guide: 0.1.2 → 0.1.3

This guide helps you upgrade from grim-rs version 0.1.2 to 0.1.3.

Version 0.1.3 introduces **breaking changes** related to struct field encapsulation, following Rust API guidelines. All public fields have been made private with proper accessor methods.

---

## Summary of Breaking Changes

1. **Box struct**: Fields made private, added getters
2. **CaptureResult struct**: Fields made private, added getters and `into_data()`
3. **Output struct**: Fields made private, added getters
4. **CaptureParameters struct**: Fields made private, added builder pattern
5. **MultiOutputCaptureResult struct**: Field made private, added accessor methods
6. **Grim struct**: Removed `impl Default` - use `Grim::new()?` instead of `Grim::default()`

---

## 1. Box Struct Encapsulation

### What Changed

All fields (`x`, `y`, `width`, `height`) are now private.

### Migration

**Before (0.1.2):**
```rust
use grim_rs::geometry::Box;

let region = Box::new(100, 200, 800, 600);

// Direct field access
println!("x: {}", region.x);
println!("y: {}", region.y);
println!("width: {}", region.width);
println!("height: {}", region.height);

// Modifying fields directly
let mut region = Box::new(0, 0, 100, 100);
region.x = 50;
region.width = 200;
```

**After (0.1.3):**
```rust
use grim_rs::geometry::Box;

let region = Box::new(100, 200, 800, 600);

// Use getter methods
println!("x: {}", region.x());
println!("y: {}", region.y());
println!("width: {}", region.width());
println!("height: {}", region.height());

// Create new instances instead of modifying fields
let region = Box::new(0, 0, 100, 100);
let region = Box::new(50, region.y(), 200, region.height());
```

### Available Methods

```rust
impl Box {
    pub fn x(&self) -> i32
    pub fn y(&self) -> i32
    pub fn width(&self) -> i32
    pub fn height(&self) -> i32
}
```

### Practical Examples

**Working with geometry calculations:**
```rust
use grim_rs::geometry::Box;

let region = Box::new(100, 200, 800, 600);

// Calculate center point
let center_x = region.x() + region.width() / 2;
let center_y = region.y() + region.height() / 2;
println!("Center: ({}, {})", center_x, center_y);

// Check if region is valid
if region.width() > 0 && region.height() > 0 {
    println!("Valid region: {}x{}", region.width(), region.height());
}

// Calculate area
let area = region.width() * region.height();
println!("Area: {} pixels", area);
```

**Using intersection:**
```rust
let screen = Box::new(0, 0, 1920, 1080);
let window = Box::new(100, 100, 800, 600);

if let Some(visible_area) = screen.intersection(&window) {
    println!("Visible window area: {}x{} at ({}, {})",
        visible_area.width(), visible_area.height(),
        visible_area.x(), visible_area.y());
}
```

---

## 2. CaptureResult Struct Encapsulation

### What Changed

All fields (`data`, `width`, `height`) are now private with proper accessors.

### Migration

**Before (0.1.2):**
```rust
use grim_rs::Grim;

let mut grim = Grim::new()?;
let result = grim.capture_all()?;

// Direct field access
println!("Width: {}, Height: {}", result.width, result.height);
let data = result.data.clone(); // Had to clone

// Save using fields
grim.save_png(&result.data, result.width, result.height, "screenshot.png")?;
```

**After (0.1.3):**
```rust
use grim_rs::Grim;

let mut grim = Grim::new()?;
let result = grim.capture_all()?;

// Use getter methods
println!("Width: {}, Height: {}", result.width(), result.height());
let data = result.data(); // Returns &[u8] - no clone needed!

// Save using getters
grim.save_png(result.data(), result.width(), result.height(), "screenshot.png")?;

// Take ownership without cloning
let owned_data = result.into_data(); // Moves Vec<u8> out
```

### Available Methods

```rust
impl CaptureResult {
    // Constructor
    pub fn new(data: Vec<u8>, width: u32, height: u32) -> Self
    
    // Getters
    pub fn data(&self) -> &[u8]           // Returns reference (no copy)
    pub fn width(&self) -> u32
    pub fn height(&self) -> u32
    pub fn into_data(self) -> Vec<u8>     // Takes ownership
}
```

### Benefits

- **More efficient**: `data()` returns `&[u8]` instead of cloning
- **Ownership transfer**: `into_data()` moves data without copying

### Practical Examples

**Processing captured data:**
```rust
use grim_rs::Grim;

let mut grim = Grim::new()?;
let result = grim.capture_all()?;

// Efficient borrowing - no cloning needed
let pixels = result.data();
println!("Captured {} bytes", pixels.len());

// Process without cloning
for chunk in result.data().chunks_exact(4) {
    let (r, g, b, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
    // Process pixel...
}

// Save multiple formats from same capture
grim.save_png(result.data(), result.width(), result.height(), "capture.png")?;
grim.save_ppm(result.data(), result.width(), result.height(), "capture.ppm")?;
```

**Taking ownership when needed:**
```rust
let result = grim.capture_all()?;

// Move data out of CaptureResult
let (width, height) = (result.width(), result.height());
let owned_data = result.into_data(); // Consumes result

// Now you own the data and can modify it
let mut pixel_data = owned_data;
for chunk in pixel_data.chunks_exact_mut(4) {
    chunk[3] = 255; // Set alpha to fully opaque
}
```

---

## 3. Output Struct Encapsulation

### What Changed

All fields (`name`, `geometry`, `scale`, `description`) are now private.

### Migration

**Before (0.1.2):**
```rust
use grim_rs::Grim;

let mut grim = Grim::new()?;
let outputs = grim.get_outputs()?;

for output in &outputs {
    // Direct field access
    println!("Output: {}", output.name);
    println!("Position: {},{}", output.geometry.x, output.geometry.y);
    println!("Scale: {}", output.scale);
    
    if let Some(desc) = &output.description {
        println!("Description: {}", desc);
    }
}
```

**After (0.1.3):**
```rust
use grim_rs::Grim;

let mut grim = Grim::new()?;
let outputs = grim.get_outputs()?;

for output in &outputs {
    // Use getter methods
    println!("Output: {}", output.name());
    println!("Position: {},{}", output.geometry().x(), output.geometry().y());
    println!("Scale: {}", output.scale());
    
    if let Some(desc) = output.description() {
        println!("Description: {}", desc);
    }
}
```

### Available Methods

```rust
impl Output {
    pub fn name(&self) -> &str
    pub fn geometry(&self) -> &Box
    pub fn scale(&self) -> i32
    pub fn description(&self) -> Option<&str>
}
```

---

## 4. CaptureParameters Builder Pattern

### What Changed

All fields are now private. Use **builder pattern** for construction.

### Migration

**Before (0.1.2):**
```rust
use grim_rs::{Grim, CaptureParameters};
use grim_rs::geometry::Box;

let mut grim = Grim::new()?;

// Struct literal initialization
let params = CaptureParameters {
    output_name: "HDMI-A-1".to_string(),
    region: Some(Box::new(0, 0, 1920, 1080)),
    overlay_cursor: true,
    scale: Some(2.0),
};

let result = grim.capture_output_with_params(&params)?;
```

**After (0.1.3):**
```rust
use grim_rs::{Grim, CaptureParameters};
use grim_rs::geometry::Box;

let mut grim = Grim::new()?;

// Builder pattern (recommended)
let params = CaptureParameters::new("HDMI-A-1")
    .region(Box::new(0, 0, 1920, 1080))
    .overlay_cursor(true)
    .scale(2.0);

let result = grim.capture_output_with_params(&params)?;

// Access with getters
println!("Output: {}", params.output_name());
println!("Cursor: {}", params.overlay_cursor_enabled());
if let Some(scale) = params.scale_factor() {
    println!("Scale: {}", scale);
}
```

### Available Methods

```rust
impl CaptureParameters {
    // Constructor (required)
    pub fn new(output_name: impl Into<String>) -> Self
    
    // Builder methods (chainable)
    pub fn region(self, region: Box) -> Self
    pub fn overlay_cursor(self, overlay_cursor: bool) -> Self
    pub fn scale(self, scale: f64) -> Self
    
    // Getters
    pub fn output_name(&self) -> &str
    pub fn region_ref(&self) -> Option<&Box>
    pub fn overlay_cursor_enabled(&self) -> bool
    pub fn scale_factor(&self) -> Option<f64>
}
```

### Builder Pattern Examples

```rust
// Basic capture (just output name)
let params = CaptureParameters::new("HDMI-A-1");

// With region only
let params = CaptureParameters::new("HDMI-A-1")
    .region(Box::new(100, 100, 800, 600));

// With cursor overlay
let params = CaptureParameters::new("eDP-1")
    .overlay_cursor(true);

// Full configuration
let params = CaptureParameters::new("DP-1")
    .region(Box::new(0, 0, 2560, 1440))
    .overlay_cursor(true)
    .scale(1.5);
```

---

## 5. MultiOutputCaptureResult Encapsulation

### What Changed

The `outputs` field is now private with accessor methods.

### Migration

**Before (0.1.2):**
```rust
use grim_rs::{Grim, CaptureParameters};

let mut grim = Grim::new()?;
let params = vec![
    CaptureParameters { output_name: "HDMI-A-1".into(), /* ... */ },
    CaptureParameters { output_name: "eDP-1".into(), /* ... */ },
];

let results = grim.capture_outputs(&params)?;

// Direct field access
for (name, result) in &results.outputs {
    println!("Captured {}: {}x{}", name, result.width, result.height);
}

// Get specific output
if let Some(result) = results.outputs.get("HDMI-A-1") {
    grim.save_png(&result.data, result.width, result.height, "hdmi.png")?;
}
```

**After (0.1.3):**
```rust
use grim_rs::{Grim, CaptureParameters};

let mut grim = Grim::new()?;
let params = vec![
    CaptureParameters::new("HDMI-A-1"),
    CaptureParameters::new("eDP-1"),
];

let results = grim.capture_outputs(&params)?;

// Use accessor methods
for (name, result) in results.outputs() {
    println!("Captured {}: {}x{}", name, result.width(), result.height());
}

// Get specific output
if let Some(result) = results.get("HDMI-A-1") {
    grim.save_png(result.data(), result.width(), result.height(), "hdmi.png")?;
}

// Take ownership of all results
let owned_outputs = results.into_outputs();
```

### Available Methods

```rust
impl MultiOutputCaptureResult {
    // Constructor
    pub fn new(outputs: HashMap<String, CaptureResult>) -> Self
    
    // Accessors
    pub fn get(&self, name: &str) -> Option<&CaptureResult>
    pub fn outputs(&self) -> &HashMap<String, CaptureResult>
    pub fn into_outputs(self) -> HashMap<String, CaptureResult>
}
```

---

## 6. Removed `impl Default for Grim`

### What Changed

`Grim` no longer implements the `Default` trait. This was removed because `Grim::new()` can fail (Wayland connection), and `Default::default()` should not panic according to Rust API guidelines.

### Migration

**Before (0.1.2):**
```rust
use grim_rs::Grim;

// Using Default trait (this panicked on failure)
let mut grim = Grim::default(); // ❌ No longer compiles!
```

**After (0.1.3):**
```rust
use grim_rs::Grim;

// Use new() which returns Result
let mut grim = Grim::new()?; // ✅ Proper error handling
```

### Why This Change?

According to Rust API Guidelines, `Default::default()` should not panic. Since `Grim::new()` can fail when:
- Wayland connection cannot be established
- Required Wayland protocols are not available

It's better to use `Result` for proper error handling rather than hiding failures in `Default`.

---

## Non-Breaking Improvements

### Error Handling

All `.unwrap()` calls in production code have been replaced with proper error handling:

```rust
// Before: Could panic on poisoned mutex
let state = frame_state.lock().unwrap();

// After: Returns Result with descriptive error
let state = lock_frame_state(&frame_state)?;
```

**Benefits:**
- No more panics from poisoned mutex
- Better error messages
- More robust error recovery

### Bug Fixes

**Critical bug in `capture_outputs()` fixed**: Previously, when capturing multiple outputs, all captures incorrectly used the first output instead of the specific output requested. This is now fixed - each output is captured independently as intended.

This fix doesn't require code changes but significantly improves multi-monitor capture reliability.

---

## Migration Checklist

- Replace `Grim::default()` with `Grim::new()?`
- Replace `box.x` with `box.x()`
- Replace `box.y` with `box.y()`
- Replace `box.width` with `box.width()`
- Replace `box.height` with `box.height()`
- Replace `result.data` with `result.data()`
- Replace `result.width` with `result.width()`
- Replace `result.height` with `result.height()`
- Replace `output.name` with `output.name()`
- Replace `output.geometry` with `output.geometry()`
- Replace `output.scale` with `output.scale()`
- Replace `output.description` with `output.description()`
- Replace `CaptureParameters { ... }` with `CaptureParameters::new(...).field(...)`
- Replace `multi_result.outputs.get(...)` with `multi_result.get(...)`
- Replace `multi_result.outputs` with `multi_result.outputs()`

---

## Automated Migration

Use `cargo fix` and your IDE's "Find & Replace" to speed up migration:

### Regex Find & Replace Examples

**For Box struct:**
```regex
Find: \.([xy]|width|height)\b
Replace: .$1()
```

**For CaptureResult:**
```regex
Find: result\.(data|width|height)\b
Replace: result.$1()
```

**For Output:**
```regex
Find: output\.(name|geometry|scale|description)\b
Replace: output.$1()
```

---

## Testing Your Migration

After migrating, run:

```bash
cargo build
cargo test
cargo clippy
```

All tests should pass without warnings.

---

## Need Help?

- **Issues**: https://github.com/vremyavnikuda/grim-rs/issues
- **Documentation**: Run `cargo doc --open`
- **Examples**: Check `examples/` directory for updated code

---

## Why These Changes?

These breaking changes follow **Rust API Guidelines** ([rust-lang.github.io/api-guidelines](https://rust-lang.github.io/api-guidelines/)):

1. **Encapsulation** (C-STRUCT-PRIVATE): Makes future changes non-breaking
2. **Builder Pattern** (C-BUILDER): Ergonomic optional parameters
3. **Borrowing** (C-BORROWED): `data()` returns `&[u8]` instead of cloning
4. **Ownership Transfer** (C-OWNS): `into_data()` for zero-copy ownership