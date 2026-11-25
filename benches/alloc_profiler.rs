use dhat::{Alloc, Profiler};
use grim_rs::{CaptureParameters, Grim, Region};
use std::env;
use std::fs;
use std::process::Command;

#[global_allocator]
static ALLOC: Alloc = Alloc;

fn generate_test_data(width: u32, height: u32) -> Vec<u8> {
    let size = (width * height * 4) as usize;
    vec![0xAA; size]
}

fn parse_arg_value(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == key)
        .and_then(|idx| args.get(idx + 1))
        .cloned()
}

fn run_case(kind: &str, width: u32, height: u32, out_file: &str) -> grim_rs::Result<()> {
    let _profiler = Profiler::builder().file_name(out_file).build();
    let mut grim = Grim::new()?;
    match kind {
        "png" => {
            let data = generate_test_data(width, height);
            let out = grim.to_png(&data, width, height)?;
            drop(out);
        }
        "png_compress" => {
            let data = generate_test_data(width, height);
            let out = grim.to_png_with_compression(&data, width, height, 9)?;
            drop(out);
        }
        #[cfg(feature = "jpeg")]
        "jpeg" => {
            let data = generate_test_data(width, height);
            let out = grim.to_jpeg(&data, width, height)?;
            drop(out);
        }
        #[cfg(feature = "jpeg")]
        "jpeg_quality" => {
            let data = generate_test_data(width, height);
            let out = grim.to_jpeg_with_quality(&data, width, height, 90)?;
            drop(out);
        }
        #[cfg(not(feature = "jpeg"))]
        "jpeg" => {
            eprintln!("JPEG feature not enabled; skipping");
            return Ok(());
        }
        #[cfg(not(feature = "jpeg"))]
        "jpeg_quality" => {
            eprintln!("JPEG feature not enabled; skipping");
            return Ok(());
        }
        "save_png" => {
            let data = generate_test_data(width, height);
            let mut path = std::env::temp_dir();
            path.push(format!(
                "grim-rs-save_png-{}x{}-{}.png",
                width,
                height,
                std::process::id()
            ));
            grim.save_png(&data, width, height, &path)?;
        }
        "save_png_compress" => {
            let data = generate_test_data(width, height);
            let mut path = std::env::temp_dir();
            path.push(format!(
                "grim-rs-save_png_compress-{}x{}-{}.png",
                width,
                height,
                std::process::id()
            ));
            grim.save_png_with_compression(&data, width, height, &path, 9)?;
        }
        #[cfg(feature = "jpeg")]
        "save_jpeg" => {
            let data = generate_test_data(width, height);
            let mut path = std::env::temp_dir();
            path.push(format!(
                "grim-rs-save_jpeg-{}x{}-{}.jpg",
                width,
                height,
                std::process::id()
            ));
            grim.save_jpeg(&data, width, height, &path)?;
        }
        #[cfg(feature = "jpeg")]
        "save_jpeg_quality" => {
            let data = generate_test_data(width, height);
            let mut path = std::env::temp_dir();
            path.push(format!(
                "grim-rs-save_jpeg_quality-{}x{}-{}.jpg",
                width,
                height,
                std::process::id()
            ));
            grim.save_jpeg_with_quality(&data, width, height, &path, 90)?;
        }
        #[cfg(not(feature = "jpeg"))]
        "save_jpeg" => {
            eprintln!("JPEG feature not enabled; skipping");
            return Ok(());
        }
        #[cfg(not(feature = "jpeg"))]
        "save_jpeg_quality" => {
            eprintln!("JPEG feature not enabled; skipping");
            return Ok(());
        }
        "capture_all" => {
            let out = grim.capture_all()?;
            drop(out);
        }
        "capture_all_scale" => {
            let out = grim.capture_all_with_scale(0.5)?;
            drop(out);
        }
        "capture_region" => {
            let outputs = grim.get_outputs()?;
            let first = match outputs.first() {
                Some(output) => output,
                None => return Ok(()),
            };
            let geom = first.geometry();
            let region = Region::new(
                geom.x() + geom.width() / 4,
                geom.y() + geom.height() / 4,
                (geom.width() / 2).max(1),
                (geom.height() / 2).max(1),
            );
            let out = grim.capture_region(region)?;
            drop(out);
        }
        "capture_region_scale" => {
            let outputs = grim.get_outputs()?;
            let first = match outputs.first() {
                Some(output) => output,
                None => return Ok(()),
            };
            let geom = first.geometry();
            let region = Region::new(
                geom.x() + geom.width() / 4,
                geom.y() + geom.height() / 4,
                (geom.width() / 2).max(1),
                (geom.height() / 2).max(1),
            );
            let out = grim.capture_region_with_scale(region, 0.5)?;
            drop(out);
        }
        "capture_output" => {
            let outputs = grim.get_outputs()?;
            let first = match outputs.first() {
                Some(output) => output,
                None => return Ok(()),
            };
            let out = grim.capture_output(first.name())?;
            drop(out);
        }
        "capture_output_scale" => {
            let outputs = grim.get_outputs()?;
            let first = match outputs.first() {
                Some(output) => output,
                None => return Ok(()),
            };
            let out = grim.capture_output_with_scale(first.name(), 0.5)?;
            drop(out);
        }
        "capture_outputs" => {
            let outputs = grim.get_outputs()?;
            if outputs.is_empty() {
                return Ok(());
            }
            let params: Vec<CaptureParameters> = outputs
                .iter()
                .map(|output| CaptureParameters::new(output.name()))
                .collect();
            let out = grim.capture_outputs(params)?;
            drop(out);
        }
        "capture_outputs_scale" => {
            let outputs = grim.get_outputs()?;
            if outputs.is_empty() {
                return Ok(());
            }
            let params: Vec<CaptureParameters> = outputs
                .iter()
                .map(|output| CaptureParameters::new(output.name()))
                .collect();
            let out = grim.capture_outputs_with_scale(params, 0.5)?;
            drop(out);
        }
        "capture_all_save_png" => {
            let out = grim.capture_all()?;
            let mut path = std::env::temp_dir();
            path.push(format!(
                "grim-rs-capture_all_save_png-{}x{}-{}.png",
                out.width(),
                out.height(),
                std::process::id()
            ));
            grim.save_png(out.data(), out.width(), out.height(), &path)?;
        }
        #[cfg(feature = "jpeg")]
        "capture_all_save_jpeg" => {
            let out = grim.capture_all()?;
            let mut path = std::env::temp_dir();
            path.push(format!(
                "grim-rs-capture_all_save_jpeg-{}x{}-{}.jpg",
                out.width(),
                out.height(),
                std::process::id()
            ));
            grim.save_jpeg(out.data(), out.width(), out.height(), &path)?;
        }
        #[cfg(not(feature = "jpeg"))]
        "capture_all_save_jpeg" => {
            eprintln!("JPEG feature not enabled; skipping");
            return Ok(());
        }
        other => {
            eprintln!("Unknown kind: {}", other);
            return Ok(());
        }
    }
    Ok(())
}

fn spawn_cases() -> grim_rs::Result<()> {
    let exe = env::current_exe().map_err(grim_rs::Error::Io)?;
    let sizes = [(640, 480), (1920, 1080), (3840, 2160)];
    let output_dir = "target/dhat";
    fs::create_dir_all(output_dir).map_err(grim_rs::Error::Io)?;
    let kinds = [
        "png",
        "png_compress",
        "save_png",
        "save_png_compress",
        "capture_all",
        "capture_all_scale",
        "capture_region",
        "capture_region_scale",
        "capture_output",
        "capture_output_scale",
        "capture_outputs",
        "capture_outputs_scale",
        "capture_all_save_png",
    ];
    for (width, height) in sizes {
        for kind in kinds {
            let filename = format!("{}/{}_{}x{}.json", output_dir, kind, width, height);
            let status = Command::new(&exe)
                .arg("--case")
                .arg(kind)
                .arg("--width")
                .arg(width.to_string())
                .arg("--height")
                .arg(height.to_string())
                .arg("--out")
                .arg(&filename)
                .status()
                .map_err(grim_rs::Error::Io)?;
            if !status.success() {
                eprintln!("Case failed: {} {}x{}", kind, width, height);
            } else {
                println!("Wrote {}", filename);
            }
        }
        #[cfg(feature = "jpeg")]
        for kind in [
            "jpeg",
            "jpeg_quality",
            "save_jpeg",
            "save_jpeg_quality",
            "capture_all_save_jpeg",
        ] {
            let filename = format!("{}/{}_{}x{}.json", output_dir, kind, width, height);
            let status = Command::new(&exe)
                .arg("--case")
                .arg(kind)
                .arg("--width")
                .arg(width.to_string())
                .arg("--height")
                .arg(height.to_string())
                .arg("--out")
                .arg(&filename)
                .status()
                .map_err(grim_rs::Error::Io)?;
            if !status.success() {
                eprintln!("Case failed: jpeg {}x{}", width, height);
            } else {
                println!("Wrote {}", filename);
            }
        }
    }
    Ok(())
}

fn main() -> grim_rs::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|arg| arg == "--case") {
        let kind = parse_arg_value(&args, "--case").unwrap_or_else(|| "png".to_string());
        let width = parse_arg_value(&args, "--width")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(1920);
        let height = parse_arg_value(&args, "--height")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(1080);
        let out_file =
            parse_arg_value(&args, "--out").unwrap_or_else(|| "dhat-heap.json".to_string());
        return run_case(&kind, width, height, &out_file);
    }
    spawn_cases()
}
