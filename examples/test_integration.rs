use grim_rs::{Grim, Box, Result};

fn main() -> Result<()> {
    let mut grim = Grim::new()?;
    let outputs = grim.get_outputs()?;
    for output in &outputs {
        println!("   - {}: {}x{}", output.name, output.geometry.width, output.geometry.height);
    }
    let screen_result = grim.capture_all()?;
    let region = Box::new(100, 100, 200, 150);
    let region_result = grim.capture_region(region)?;
    grim.save_png(&region_result.data, region_result.width, region_result.height, "test_region.png")?;
    let png_bytes = grim.to_png(&screen_result.data, screen_result.width, screen_result.height)?;
    println!("size: {} ", png_bytes.len());
    Ok(())
}