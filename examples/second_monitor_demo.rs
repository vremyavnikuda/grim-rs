use grim_rs::{Grim, LocalDateTime, Region, Result};

fn generate_filename(description: &str, extension: &str) -> String {
    let now = LocalDateTime::now();
    format!("{}_second_monitor_{}.{}", now, description, extension)
}

fn main() -> Result<()> {
    let mut grim = Grim::new()?;
    let outputs = grim.get_outputs()?;
    println!("Found {} output(s)\n", outputs.len());
    if outputs.is_empty() {
        eprintln!("Error: No outputs found!");
        return Ok(());
    }
    for (i, output) in outputs.iter().enumerate() {
        println!("Output #{}: {}", i + 1, output.name());
        println!(
            "Position: ({}, {})",
            output.geometry().x(),
            output.geometry().y()
        );
        println!(
            "Size: {}x{}",
            output.geometry().width(),
            output.geometry().height()
        );
        println!("Scale: {}x", output.scale());
        if let Some(desc) = output.description() {
            println!("Description: {}", desc);
        }
        println!();
    }
    if outputs.len() < 2 {
        eprintln!("Error: Second monitor not found!");
        eprintln!("This demo requires at least 2 monitors.");
        return Ok(());
    }
    let second_output = &outputs[1];
    println!("Using second monitor: {}", second_output.name());
    println!(
        "Resolution: {}x{}\n",
        second_output.geometry().width(),
        second_output.geometry().height()
    );
    let result = grim.capture_output(second_output.name())?;
    println!("Captured: {}x{} pixels", result.width(), result.height());
    let filename = generate_filename("full", "png");
    grim.save_png(result.data(), result.width(), result.height(), &filename)?;
    println!("Saved: {}\n", filename);
    let result_half = grim.capture_output_with_scale(second_output.name(), 0.5)?;
    println!(
        "Captured: {}x{} pixels",
        result_half.width(),
        result_half.height()
    );
    let filename = generate_filename("half_scale", "png");
    grim.save_png(
        result_half.data(),
        result_half.width(),
        result_half.height(),
        &filename,
    )?;
    println!("Saved: {}\n", filename);
    let result_quarter = grim.capture_output_with_scale(second_output.name(), 0.25)?;
    println!(
        "Captured: {}x{} pixels",
        result_quarter.width(),
        result_quarter.height()
    );
    let filename = generate_filename("quarter_scale", "png");
    grim.save_png(
        result_quarter.data(),
        result_quarter.width(),
        result_quarter.height(),
        &filename,
    )?;
    println!("Saved: {}\n", filename);
    let geom = second_output.geometry();
    let region = Region::new(
        geom.x(),
        geom.y(),
        (400).min(geom.width()),
        (300).min(geom.height()),
    );
    let result = grim.capture_region(region)?;
    println!("Captured: {}x{} pixels", result.width(), result.height());
    let filename = generate_filename("top_left", "png");
    grim.save_png(result.data(), result.width(), result.height(), &filename)?;
    println!("Saved: {}\n", filename);
    let center_width = (800).min(geom.width());
    let center_height = (600).min(geom.height());
    let region = Region::new(
        geom.x() + (geom.width() - center_width) / 2,
        geom.y() + (geom.height() - center_height) / 2,
        center_width,
        center_height,
    );
    let result = grim.capture_region(region)?;
    println!("Captured: {}x{} pixels", result.width(), result.height());
    let filename = generate_filename("center", "png");
    grim.save_png(result.data(), result.width(), result.height(), &filename)?;
    println!("Saved: {}\n", filename);
    let corner_width = (400).min(geom.width());
    let corner_height = (300).min(geom.height());
    let region = Region::new(
        geom.x() + geom.width() - corner_width,
        geom.y() + geom.height() - corner_height,
        corner_width,
        corner_height,
    );
    let result = grim.capture_region(region)?;
    println!("Captured: {}x{} pixels", result.width(), result.height());
    let filename = generate_filename("bottom_right", "png");
    grim.save_png(result.data(), result.width(), result.height(), &filename)?;
    println!("Saved: {}\n", filename);
    let result = grim.capture_output(second_output.name())?;
    println!("- PNG (default compression)...");
    let filename = generate_filename("format", "png");
    grim.save_png(result.data(), result.width(), result.height(), &filename)?;
    println!("Saved: {}", filename);
    let filename = generate_filename("format_best_comp", "png");
    grim.save_png_with_compression(result.data(), result.width(), result.height(), &filename, 9)?;
    println!("Saved: {}", filename);
    #[cfg(feature = "jpeg")]
    {
        println!("- JPEG (default quality)...");
        let filename = generate_filename("format", "jpg");
        grim.save_jpeg(result.data(), result.width(), result.height(), &filename)?;
        println!("Saved: {}", filename);
        println!("- JPEG (quality 95)...");
        let filename = generate_filename("format_q95", "jpg");
        grim.save_jpeg_with_quality(
            result.data(),
            result.width(),
            result.height(),
            &filename,
            95,
        )?;
        println!("Saved: {}", filename);
    }
    let center_width = (800).min(geom.width());
    let center_height = (600).min(geom.height());
    let region = Region::new(
        geom.x() + (geom.width() - center_width) / 2,
        geom.y() + (geom.height() - center_height) / 2,
        center_width,
        center_height,
    );
    let result = grim.capture_region_with_scale(region, 0.75)?;
    println!("Captured: {}x{} pixels", result.width(), result.height());
    let filename = generate_filename("center_scaled", "png");
    grim.save_png(result.data(), result.width(), result.height(), &filename)?;
    println!("Saved: {}\n", filename);
    let strip_height = (200).min(geom.height());
    let region = Region::new(
        geom.x(),
        geom.y() + (geom.height() - strip_height) / 2,
        geom.width(),
        strip_height,
    );
    let result = grim.capture_region(region)?;
    println!("Captured: {}x{} pixels", result.width(), result.height());
    let filename = generate_filename("horizontal_strip", "png");
    grim.save_png(result.data(), result.width(), result.height(), &filename)?;
    println!("Saved: {}\n", filename);
    let strip_width = (200).min(geom.width());
    let region = Region::new(
        geom.x() + (geom.width() - strip_width) / 2,
        geom.y(),
        strip_width,
        geom.height(),
    );
    let result = grim.capture_region(region)?;
    println!("Captured: {}x{} pixels", result.width(), result.height());
    let filename = generate_filename("vertical_strip", "png");
    grim.save_png(result.data(), result.width(), result.height(), &filename)?;
    println!("Saved: {}\n", filename);
    let result = grim.capture_output(second_output.name())?;
    let png_bytes = grim.to_png(result.data(), result.width(), result.height())?;
    println!("PNG bytes: {} bytes", png_bytes.len());
    #[cfg(feature = "jpeg")]
    {
        let jpeg_bytes = grim.to_jpeg(result.data(), result.width(), result.height())?;
        println!("JPEG bytes: {} bytes", jpeg_bytes.len());
    }
    let grid_size = 4;
    let cell_width = geom.width() / grid_size;
    let cell_height = geom.height() / grid_size;
    for row in 0..grid_size {
        for col in 0..grid_size {
            let region = Region::new(
                geom.x() + col * cell_width,
                geom.y() + row * cell_height,
                cell_width,
                cell_height,
            );
            let result = grim.capture_region(region)?;
            let filename = generate_filename(&format!("grid_{}_{}", row, col), "png");
            grim.save_png(result.data(), result.width(), result.height(), &filename)?;
        }
    }
    println!(
        "Created {}x{} grid ({} images)\n",
        grid_size,
        grid_size,
        grid_size * grid_size
    );
    println!(
        "All screenshots saved to: {}",
        std::env::current_dir()?.display()
    );
    Ok(())
}
