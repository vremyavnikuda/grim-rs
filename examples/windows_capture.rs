//! Windows screenshot demo using DXGI Desktop Duplication.
//!
//! Usage:
//!   cargo run --example windows_capture

use grim_rs::{CaptureParameters, Grim, LocalDateTime, Region};

fn filename(label: &str, ext: &str) -> String {
    let ts = LocalDateTime::now();
    format!("{}_{}.{}", ts, label, ext)
}

fn main() -> grim_rs::Result<()> {
    env_logger::init();
    let mut grim = Grim::new()?;
    let outputs = grim.get_outputs()?;
    println!("{} monitor(s) detected", outputs.len());
    for (i, o) in outputs.iter().enumerate() {
        println!(
            "  {}: {} {}x{} +{}+{} scale={}",
            i,
            o.name(),
            o.geometry().width(),
            o.geometry().height(),
            o.geometry().x(),
            o.geometry().y(),
            o.scale()
        );
    }
    if outputs.is_empty() {
        return Ok(());
    }
    let r = grim.capture_all()?;
    let f = filename("full_desktop", "png");
    grim.save_png(r.data(), r.width(), r.height(), &f)?;
    println!("full desktop: {} {}x{}", f, r.width(), r.height());
    for (i, o) in outputs.iter().enumerate() {
        let r = grim.capture_output(o.name())?;
        let f = filename(&format!("output_{}", i), "png");
        grim.save_png(r.data(), r.width(), r.height(), &f)?;
        println!("output {}: {} {}x{}", o.name(), f, r.width(), r.height());
    }
    let region = Region::new(0, 0, 800, 600);
    let r = grim.capture_region(region)?;
    let f = filename("region", "png");
    grim.save_png(r.data(), r.width(), r.height(), &f)?;
    println!("region {}: {} {}x{}", region, f, r.width(), r.height());
    if let Some(first) = outputs.first() {
        let params = CaptureParameters::new(first.name()).overlay_cursor(true);
        let multi = grim.capture_outputs(vec![params])?;
        if let Some(r) = multi.get(first.name()) {
            let f = filename("cursor", "png");
            grim.save_png(r.data(), r.width(), r.height(), &f)?;
            println!("cursor: {} {}x{}", f, r.width(), r.height());
        }
    }
    let r = grim.capture_all_with_scale(0.5)?;
    let f = filename("halfscale", "png");
    grim.save_png(r.data(), r.width(), r.height(), &f)?;
    println!("scale 0.5: {} {}x{}", f, r.width(), r.height());
    let r = grim.capture_all_with_scale(2.0)?;
    let f = filename("2x", "png");
    grim.save_png(r.data(), r.width(), r.height(), &f)?;
    println!("scale 2x: {} {}x{}", f, r.width(), r.height());
    #[cfg(feature = "jpeg")]
    {
        let r = grim.capture_all()?;
        let f = filename("desktop", "jpg");
        grim.save_jpeg(r.data(), r.width(), r.height(), &f)?;
        println!("jpeg q80: {} {}x{}", f, r.width(), r.height());
        let f = filename("desktop_hq", "jpg");
        grim.save_jpeg_with_quality(r.data(), r.width(), r.height(), &f, 95)?;
        println!("jpeg q95: {} {}x{}", f, r.width(), r.height());
    }
    if outputs.len() >= 2 {
        let params = vec![
            CaptureParameters::new(outputs[0].name()).overlay_cursor(true),
            CaptureParameters::new(outputs[1].name()).region(Region::new(0, 0, 400, 300)),
        ];
        let multi = grim.capture_outputs_with_scale(params, 1.0)?;
        for (name, capture) in multi.outputs() {
            let short = name.trim_start_matches("\\\\.\\");
            let f = filename(&format!("multi_{}", short), "png");
            grim.save_png(capture.data(), capture.width(), capture.height(), &f)?;
            println!(
                "multi {}: {} {}x{}",
                short,
                f,
                capture.width(),
                capture.height()
            );
        }
    }
    Ok(())
}
