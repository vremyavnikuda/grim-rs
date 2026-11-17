use grim_rs::LocalDateTime;

/// Comprehensive demonstration of all grim-rs screenshot capabilities.
///
/// This example demonstrates every available method in the grim-rs API,
/// including different capture modes, output formats, and advanced features.
///
/// Usage:
///     cargo run --example comprehensive_demo
///
/// All screenshots will be saved to the project root directory.
use grim_rs::{CaptureParameters, Grim, Region, Result};
use std::fs::File;
use std::io::Write;

/// Generate filename with timestamp (like grim-rs does by default)
/// Format: YYYYMMDD_HHhMMmSSs_grim_demo.ext
fn generate_demo_filename(label: &str, extension: &str) -> String {
    let now = LocalDateTime::now();
    format!("{}_grim_rs_demo_{}.{}", now, label, extension)
}

fn main() -> Result<()> {
    let mut grim = Grim::new()?;
    println!("Getting Display Outputs Information");
    let outputs = grim.get_outputs()?;
    println!("Found {} output(s):\n", outputs.len());
    for (i, output) in outputs.iter().enumerate() {
        println!("Output #{}: {}", i + 1, output.name());
        println!(
            "  Position: ({}, {})",
            output.geometry().x(),
            output.geometry().y()
        );
        println!(
            "  Size: {}x{}",
            output.geometry().width(),
            output.geometry().height()
        );
        println!("  Scale: {}x", output.scale());
        println!();
    }
    if outputs.is_empty() {
        eprintln!("No outputs found! Cannot proceed.");
        return Ok(());
    }
    let result = grim.capture_all()?;
    println!(
        "Captured: {}x{} pixels ({} bytes)",
        result.width(),
        result.height(),
        result.data().len()
    );
    let filename = generate_demo_filename("capture_all", "png");
    grim.save_png(result.data(), result.width(), result.height(), &filename)?;
    println!("Saved: {}\n", filename);
    let result_scaled = grim.capture_all_with_scale(0.5)?;
    println!(
        "Captured at 0.5x scale: {}x{} pixels",
        result_scaled.width(),
        result_scaled.height()
    );
    let filename = generate_demo_filename("capture_all_half", "png");
    grim.save_png(
        result_scaled.data(),
        result_scaled.width(),
        result_scaled.height(),
        &filename,
    )?;
    println!("Saved: {}", filename);
    let result_scaled_25 = grim.capture_all_with_scale(0.25)?;
    println!(
        "Captured at 0.25x scale: {}x{} pixels",
        result_scaled_25.width(),
        result_scaled_25.height()
    );
    let filename = generate_demo_filename("capture_all_quarter", "png");
    grim.save_png(
        result_scaled_25.data(),
        result_scaled_25.width(),
        result_scaled_25.height(),
        &filename,
    )?;
    println!("Saved: {}\n", filename);
    let first_output_name = outputs[0].name();
    println!("Capturing output: {}", first_output_name);
    let output_result = grim.capture_output(first_output_name)?;
    println!(
        "Captured output: {}x{} pixels",
        output_result.width(),
        output_result.height()
    );
    let filename = generate_demo_filename("output_full", "png");
    grim.save_png(
        output_result.data(),
        output_result.width(),
        output_result.height(),
        &filename,
    )?;
    println!("Saved: {}\n", filename);
    let output_scaled = grim.capture_output_with_scale(first_output_name, 0.5)?;
    println!(
        "Captured output at 0.5x scale: {}x{} pixels",
        output_scaled.width(),
        output_scaled.height()
    );
    let filename = generate_demo_filename("output_half", "png");
    grim.save_png(
        output_scaled.data(),
        output_scaled.width(),
        output_scaled.height(),
        &filename,
    )?;
    println!("Saved: {}\n", filename);
    let region = Region::new(100, 100, 800, 600);
    println!("Region: {}", region);
    let region_result = grim.capture_region(region)?;
    println!(
        "Captured region: {}x{} pixels",
        region_result.width(),
        region_result.height()
    );
    let filename = generate_demo_filename("region_full", "png");
    grim.save_png(
        region_result.data(),
        region_result.width(),
        region_result.height(),
        &filename,
    )?;
    println!("Saved: {}\n", filename);
    let region_scaled = grim.capture_region_with_scale(region, 0.75)?;
    println!(
        "Captured region at 0.75x scale: {}x{} pixels",
        region_scaled.width(),
        region_scaled.height()
    );
    let filename = generate_demo_filename("region_scaled", "png");
    grim.save_png(
        region_scaled.data(),
        region_scaled.width(),
        region_scaled.height(),
        &filename,
    )?;
    println!("Saved: {}\n", filename);
    if outputs.len() >= 2 {
        println!("Capturing Multiple Outputs with Different Parameters");
        let params = vec![
            CaptureParameters::new(outputs[0].name()).overlay_cursor(true),
            CaptureParameters::new(outputs[1].name()).region(Region::new(0, 0, 400, 300)),
        ];
        let multi_result = grim.capture_outputs(params)?;
        println!("Captured {} outputs", multi_result.outputs().len());
        for (output_name, capture) in multi_result.outputs().iter() {
            let filename =
                generate_demo_filename(&format!("multi_{}", output_name.to_lowercase()), "png");
            grim.save_png(capture.data(), capture.width(), capture.height(), &filename)?;
            println!(
                "Saved: {} ({}x{})",
                filename,
                capture.width(),
                capture.height()
            );
        }
        println!();
    } else {
        println!("Skipping multi-output capture (only 1 output available)\n");
    }
    let format_region = Region::new(0, 0, 400, 300);
    let format_result = grim.capture_region(format_region)?;
    let filename_png = generate_demo_filename("format_png_default", "png");
    grim.save_png(
        format_result.data(),
        format_result.width(),
        format_result.height(),
        &filename_png,
    )?;
    println!("Saved PNG (default compression): {}", filename_png);
    let filename_png_compressed = generate_demo_filename("format_png_best", "png");
    grim.save_png_with_compression(
        format_result.data(),
        format_result.width(),
        format_result.height(),
        &filename_png_compressed,
        9,
    )?;
    println!("Saved PNG (best compression): {}", filename_png_compressed);
    #[cfg(feature = "jpeg")]
    {
        let filename_jpeg = generate_demo_filename("format_jpeg_default", "jpg");
        grim.save_jpeg(
            format_result.data(),
            format_result.width(),
            format_result.height(),
            &filename_jpeg,
        )?;
        println!("Saved JPEG (default quality): {}", filename_jpeg);
        let filename_jpeg_hq = generate_demo_filename("format_jpeg_q95", "jpg");
        grim.save_jpeg_with_quality(
            format_result.data(),
            format_result.width(),
            format_result.height(),
            &filename_jpeg_hq,
            95,
        )?;
        println!("Saved JPEG (quality 95): {}", filename_jpeg_hq);
    }
    #[cfg(not(feature = "jpeg"))]
    {
        println!("JPEG support not enabled (use --features jpeg)");
    }
    let small_region = Region::new(0, 0, 200, 150);
    let small_result = grim.capture_region(small_region)?;
    let png_bytes = grim.to_png(
        small_result.data(),
        small_result.width(),
        small_result.height(),
    )?;
    println!("PNG bytes: {} bytes", png_bytes.len());
    #[cfg(feature = "jpeg")]
    {
        let jpeg_bytes = grim.to_jpeg(
            small_result.data(),
            small_result.width(),
            small_result.height(),
        )?;
        println!("JPEG bytes: {} bytes", jpeg_bytes.len());
        let jpeg_hq_bytes = grim.to_jpeg_with_quality(
            small_result.data(),
            small_result.width(),
            small_result.height(),
            90,
        )?;
        println!("JPEG bytes (quality 90): {} bytes", jpeg_hq_bytes.len());
    }
    let filename = generate_demo_filename("bytes_png", "png");
    let mut file = File::create(&filename)?;
    file.write_all(&png_bytes)?;
    println!("Saved PNG from bytes: {}\n", filename);
    if outputs.len() >= 2 {
        let output1 = outputs[0].geometry();
        let span_x = output1.x() + output1.width() - 200;
        let span_width = 400;
        let span_region = Region::new(span_x, output1.y(), span_width, 400);
        println!("Spanning region: {}", span_region);
        let span_result = grim.capture_region(span_region)?;
        println!(
            "Captured spanning region: {}x{} pixels",
            span_result.width(),
            span_result.height()
        );
        let filename = generate_demo_filename("span_region", "png");
        grim.save_png(
            span_result.data(),
            span_result.width(),
            span_result.height(),
            &filename,
        )?;
        println!("Saved: {}\n", filename);
    } else {
        println!("Skipping spanning region (only 1 output available)\n");
    }
    let test_region = Region::new(0, 0, 640, 480);
    let test_result = grim.capture_region(test_region)?;
    let filename_png = generate_demo_filename("compare_png", "png");
    grim.save_png(
        test_result.data(),
        test_result.width(),
        test_result.height(),
        &filename_png,
    )?;
    let png_size = std::fs::metadata(&filename_png)?.len();
    println!(
        "Image size: {}x{}",
        test_result.width(),
        test_result.height()
    );
    println!("PNG ({}): {} bytes", filename_png, png_size);
    #[cfg(feature = "jpeg")]
    {
        let filename_jpg = generate_demo_filename("compare_jpeg", "jpg");
        grim.save_jpeg(
            test_result.data(),
            test_result.width(),
            test_result.height(),
            &filename_jpg,
        )?;
        let jpeg_size = std::fs::metadata(&filename_jpg)?.len();
        println!("JPEG ({}): {} bytes", filename_jpg, jpeg_size);
    }
    println!("All files saved to: {}", std::env::current_dir()?.display());
    if outputs.len() >= 2 {
        println!("Multi-output captures");
        println!("Spanning region captures");
    }
    println!(
        "Multiple formats: PNG{}",
        if cfg!(feature = "jpeg") { ", JPEG" } else { "" }
    );
    Ok(())
}
