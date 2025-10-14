use grim_rs::Grim;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let mut grim = Grim::new()?;

    println!("grim-rs Performance Profiling\n");

    println!("1. Full screen capture:");
    let start = Instant::now();
    let result = grim.capture_all()?;
    let capture_time = start.elapsed();
    println!("Time: {:?}", capture_time);
    println!("Size: {}x{}", result.width(), result.height());
    println!(
        "Data: {} bytes ({:.2} MB)\n",
        result.data().len(),
        result.data().len() as f64 / 1024.0 / 1024.0
    );

    println!("2. PNG encoding (different compression levels):");
    for level in [1, 6, 9] {
        let start = Instant::now();
        let png_data =
            grim.to_png_with_compression(result.data(), result.width(), result.height(), level)?;
        let encode_time = start.elapsed();
        println!(
            "Level {}: {:?} -> {} bytes ({:.2} MB)",
            level,
            encode_time,
            png_data.len(),
            png_data.len() as f64 / 1024.0 / 1024.0
        );
    }

    #[cfg(feature = "jpeg")]
    {
        println!("3. JPEG encoding (different quality levels):");
        for quality in [60, 80, 95] {
            let start = Instant::now();
            let jpeg_data =
                grim.to_jpeg_with_quality(result.data(), result.width(), result.height(), quality)?;
            let encode_time = start.elapsed();
            println!(
                "Quality {}: {:?} -> {} bytes ({:.2} MB)",
                quality,
                encode_time,
                jpeg_data.len(),
                jpeg_data.len() as f64 / 1024.0 / 1024.0
            );
        }
        println!();
    }

    println!("4. PPM encoding:");
    let start = Instant::now();
    let ppm_data = grim.to_ppm(result.data(), result.width(), result.height())?;
    let encode_time = start.elapsed();
    println!(
        "Time: {:?} -> {} bytes ({:.2} MB)\n",
        encode_time,
        ppm_data.len(),
        ppm_data.len() as f64 / 1024.0 / 1024.0
    );

    println!("5. Capture with different scales:");
    for scale in [0.5, 1.0, 2.0] {
        let start = Instant::now();
        let scaled_result = grim.capture_all_with_scale(scale)?;
        let scale_time = start.elapsed();
        println!(
            "Scale {:.1}: {:?} -> {}x{} ({:.2} MB)",
            scale,
            scale_time,
            scaled_result.width(),
            scaled_result.height(),
            scaled_result.data().len() as f64 / 1024.0 / 1024.0
        );
    }

    println!("6. Get outputs info:");
    let start = Instant::now();
    let outputs = grim.get_outputs()?;
    let outputs_time = start.elapsed();
    println!("Time: {:?}", outputs_time);
    println!("Outputs found: {}", outputs.len());
    for output in &outputs {
        println!(
            "- {} ({}x{}) scale={}",
            output.name(),
            output.geometry().width(),
            output.geometry().height(),
            output.scale()
        );
    }

    let iterations = 10;
    println!("7. Stress test ({} iterations):", iterations);
    let start = Instant::now();
    for i in 0..iterations {
        let _result = grim.capture_all()?;
        if (i + 1) % 5 == 0 {
            println!("Progress: {}/{}", i + 1, iterations);
        }
    }
    let total = start.elapsed();
    let avg = total / iterations;
    println!("Total time: {:?}", total);
    println!("Average per capture: {:?}", avg);
    println!("Captures per second: {:.2}\n", 1.0 / avg.as_secs_f64());

    println!("8. Region capture (different sizes):");
    let regions = [
        ("Small (100x100)", grim_rs::Box::new(0, 0, 100, 100)),
        ("Medium (500x500)", grim_rs::Box::new(0, 0, 500, 500)),
        ("Large (1920x1080)", grim_rs::Box::new(0, 0, 1920, 1080)),
    ];

    for (name, region) in &regions {
        let start = Instant::now();
        match grim.capture_region(*region) {
            Ok(region_result) => {
                let region_time = start.elapsed();
                println!(
                    "{}: {:?} -> {}x{} ({:.2} MB)",
                    name,
                    region_time,
                    region_result.width(),
                    region_result.height(),
                    region_result.data().len() as f64 / 1024.0 / 1024.0
                );
            }
            Err(e) => {
                println!("{}: Error - {}", name, e);
            }
        }
    }
    println!();

    println!("Profiling Complete");

    Ok(())
}
