use grim_rs::{Box, Grim, Result};
use std::env;
fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <outputs|geometry|output|all> [args...]", args[0]);
        eprintln!("Examples:");
        eprintln!("  {} outputs", args[0]);
        eprintln!("  {} all output.png", args[0]);
        eprintln!("  {} output DP-1 output.png", args[0]);
        eprintln!("  {} geometry \"100,100 800x600\" region.png", args[0]);
        return Ok(());
    }

    let mut grim = Grim::new()?;

    match args[1].as_str() {
        "outputs" => {
            let outputs = grim.get_outputs()?;
            println!("Available outputs:");
            for output in outputs {
                println!("  {} ({}x{} at {},{}) scale: {}", 
                    output.name, 
                    output.geometry.width, 
                    output.geometry.height,
                    output.geometry.x,
                    output.geometry.y,
                    output.scale
                );
            }
        }
        "all" => {
            let default_output = "screenshot.png".to_string();
            let output_file = args.get(2).map(|s| s.as_str()).unwrap_or(&default_output);
            let result = grim.capture_all()?;
            grim.save_png(&result.data, result.width, result.height, output_file)?;
            println!(
                "Screenshot saved to {} ({}x{})",
                output_file, result.width, result.height
            );
        }
        "output" => {
            if args.len() < 4 {
                eprintln!("Usage: {} output <output_name> <output_file>", args[0]);
                return Ok(());
            }
            let output_name = &args[2];
            let output_file = &args[3];
            let result = grim.capture_output(output_name)?;
            grim.save_png(&result.data, result.width, result.height, output_file)?;
            println!(
                "Output {} screenshot saved to {} ({}x{})",
                output_name, output_file, result.width, result.height
            );
        }
        "geometry" => {
            if args.len() < 4 {
                eprintln!(
                    "Usage: {} geometry \"<x>,<y> <w>x<h>\" <output_file>",
                    args[0]
                );
                return Ok(());
            }
            let geometry: Box = args[2].parse()?;
            let output_file = &args[3];
            let result = grim.capture_region(geometry)?;
            grim.save_png(&result.data, result.width, result.height, output_file)?;
            println!(
                "Region screenshot saved to {} ({}x{})",
                output_file, result.width, result.height
            );
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            eprintln!("Available commands: outputs, all, output, geometry");
        }
    }

    Ok(())
}
