//! Capture screenshots using `ext-image-copy-capture-v1` — the new Wayland
//! screenshot protocol.
//!
//! Uses `Grim::new_ext()` to force the new protocol. Fails on compositors
//! that don't support it (KDE, GNOME, old Sway).
//!
//! Usage:
//!   cargo run --example capture_screenshots
//!
//! All screenshots are saved to the `examples/` directory.

use grim_rs::{CaptureParameters, Grim, LocalDateTime, Region, Result};

fn filename(label: &str, ext: &str) -> String {
    let ts = LocalDateTime::now();
    format!("examples/{}_{}.{}", ts, label, ext)
}

fn main() -> Result<()> {
    env_logger::init();
    let mut grim = Grim::new_ext()?;
    let outputs = grim.get_outputs()?;
    println!("Outputs: {}", outputs.len());
    for o in &outputs {
        println!(
            "  {} — {}x{} at ({},{}), scale={}",
            o.name(),
            o.geometry().width(),
            o.geometry().height(),
            o.geometry().x(),
            o.geometry().y(),
            o.scale()
        );
    }
    // Full screen
    let r = grim.capture_all()?;
    let f = filename("full", "png");
    grim.save_png(r.data(), r.width(), r.height(), &f)?;
    println!("[1] full screen  — {}x{} → {}", r.width(), r.height(), f);
    // Each output individually
    for o in &outputs {
        let r = grim.capture_output(o.name())?;
        let f = filename(&format!("output_{}", o.name()), "png");
        grim.save_png(r.data(), r.width(), r.height(), &f)?;
        println!(
            "[2] output {} — {}x{} → {}",
            o.name(),
            r.width(),
            r.height(),
            f
        );
    }
    // Region
    let region = Region::new(0, 0, 800, 600);
    let r = grim.capture_region(region)?;
    let f = filename("region_800x600", "png");
    grim.save_png(r.data(), r.width(), r.height(), &f)?;
    println!("[3] region 800x600 — {}x{} → {}", r.width(), r.height(), f);
    // With cursor overlay
    if let Some(first) = outputs.first() {
        let params = CaptureParameters::new(first.name()).overlay_cursor(true);
        let multi = grim.capture_outputs(vec![params])?;
        if let Some(r) = multi.get(first.name()) {
            let f = filename("cursor", "png");
            grim.save_png(r.data(), r.width(), r.height(), &f)?;
            println!("[4] cursor — {}x{} → {}", r.width(), r.height(), f);
        }
    }
    // Half scale
    let r = grim.capture_all_with_scale(0.5)?;
    let f = filename("halfscale", "png");
    grim.save_png(r.data(), r.width(), r.height(), &f)?;
    println!("[5] half scale — {}x{} → {}", r.width(), r.height(), f);
    // JPEG
    #[cfg(feature = "jpeg")]
    {
        let r = grim.capture_all()?;
        let f = filename("full", "jpg");
        grim.save_jpeg(r.data(), r.width(), r.height(), &f)?;
        println!("[6] JPEG — {}x{} → {}", r.width(), r.height(), f);
    }
    println!("Done.");
    Ok(())
}
