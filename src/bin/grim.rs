use grim_rs::{ Grim, Box as GrimBox, CaptureParameters };
use std::env;
use std::path::Path;

fn main() -> grim_rs::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut opts = Options::default();
    let mut output_file = None;
    let mut arg_idx = 1;

    // Parse command line arguments similar to the original grim
    while arg_idx < args.len() {
        match args[arg_idx].as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-s" => {
                arg_idx += 1;
                if arg_idx >= args.len() {
                    eprintln!("Error: -s requires an argument");
                    std::process::exit(1);
                }
                opts.scale = Some(
                    args[arg_idx]
                        .parse::<f64>()
                        .map_err(|_| {
                            grim_rs::Error::Io(
                                std::io::Error::new(
                                    std::io::ErrorKind::InvalidInput,
                                    "Invalid scale factor"
                                )
                            )
                        })?
                );
            }
            "-g" => {
                arg_idx += 1;
                if arg_idx >= args.len() {
                    eprintln!("Error: -g requires an argument");
                    std::process::exit(1);
                }
                if args[arg_idx] == "-" {
                    opts.geometry = Some(Grim::read_region_from_stdin()?);
                } else {
                    opts.geometry = Some(args[arg_idx].parse()?);
                }
            }
            "-t" => {
                arg_idx += 1;
                if arg_idx >= args.len() {
                    eprintln!("Error: -t requires an argument");
                    std::process::exit(1);
                }
                match args[arg_idx].as_str() {
                    "png" => {
                        opts.filetype = FileType::Png;
                    }
                    "ppm" => {
                        opts.filetype = FileType::Ppm;
                    }
                    "jpeg" => {
                        opts.filetype = FileType::Jpeg;
                    }
                    _ => {
                        eprintln!("Error: invalid filetype: {}", args[arg_idx]);
                        std::process::exit(1);
                    }
                }
            }
            "-q" => {
                arg_idx += 1;
                if arg_idx >= args.len() {
                    eprintln!("Error: -q requires an argument");
                    std::process::exit(1);
                }
                let quality: i32 = args[arg_idx]
                    .parse()
                    .map_err(|_| {
                        grim_rs::Error::Io(
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "Invalid quality value"
                            )
                        )
                    })?;
                if !(0..=100).contains(&quality) {
                    eprintln!("Error: JPEG quality must be between 0 and 100");
                    std::process::exit(1);
                }
                opts.jpeg_quality = quality as u8;
            }
            "-l" => {
                arg_idx += 1;
                if arg_idx >= args.len() {
                    eprintln!("Error: -l requires an argument");
                    std::process::exit(1);
                }
                let level: i32 = args[arg_idx]
                    .parse()
                    .map_err(|_| {
                        grim_rs::Error::Io(
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "Invalid compression level"
                            )
                        )
                    })?;
                if level > 9 {
                    eprintln!("Error: PNG compression level must be between 0 and 9");
                    std::process::exit(1);
                }
                opts.png_level = level as u8;
            }
            "-o" => {
                arg_idx += 1;
                if arg_idx >= args.len() {
                    eprintln!("Error: -o requires an argument");
                    std::process::exit(1);
                }
                opts.output_name = Some(args[arg_idx].clone());
            }
            "-c" => {
                opts.with_cursor = true;
            }
            _ => {
                // Assume it's the output file
                if output_file.is_none() {
                    output_file = Some(args[arg_idx].clone());
                } else {
                    eprintln!("Error: too many arguments");
                    std::process::exit(1);
                }
            }
        }
        arg_idx += 1;
    }

    // Set default output file if not provided
    let output_file = if let Some(file) = output_file {
        file
    } else {
        // Generate default filename
        generate_default_filename(opts.filetype)?
    };

    // Create Grim instance
    let mut grim = Grim::new()?;

    // Perform the capture
    let result = if let Some(ref output_name) = opts.output_name {
        // Capture specific output
        if opts.with_cursor {
            // For cursor support, we need to use the capture parameters approach
            let params = vec![CaptureParameters {
                output_name: output_name.clone(),
                region: opts.geometry,
                overlay_cursor: opts.with_cursor,
                scale: opts.scale,
            }];
            let multi_result = grim.capture_outputs_with_scale(params, opts.scale.unwrap_or(1.0))?;
            if let Some(capture_result) = multi_result.outputs.get(output_name) {
                capture_result.clone()
            } else {
                return Err(grim_rs::Error::OutputNotFound(output_name.clone()));
            }
        } else {
            grim.capture_output_with_scale(output_name, opts.scale.unwrap_or(1.0))?
        }
    } else if let Some(ref geometry) = opts.geometry {
        // Capture specific region
        grim.capture_region_with_scale(*geometry, opts.scale.unwrap_or(1.0))?
    } else {
        // Capture all outputs
        grim.capture_all_with_scale(opts.scale.unwrap_or(1.0))?
    };

    // Save or output the result
    if output_file == "-" {
        // Output to stdout
        match opts.filetype {
            FileType::Png => {
                if opts.png_level == 6 {
                    grim.write_png_to_stdout(&result.data, result.width, result.height)?;
                } else {
                    grim.write_png_to_stdout_with_compression(
                        &result.data,
                        result.width,
                        result.height,
                        opts.png_level
                    )?;
                }
            }
            FileType::Ppm => {
                grim.write_ppm_to_stdout(&result.data, result.width, result.height)?;
            }
            FileType::Jpeg => {
                if opts.jpeg_quality == 80 {
                    #[cfg(feature = "jpeg")]
                    grim.write_jpeg_to_stdout(&result.data, result.width, result.height)?;
                } else {
                    #[cfg(feature = "jpeg")]
                    grim.write_jpeg_to_stdout_with_quality(
                        &result.data,
                        result.width,
                        result.height,
                        opts.jpeg_quality
                    )?;
                }
            }
        }
    } else {
        // Save to file
        let path = Path::new(&output_file);
        match opts.filetype {
            FileType::Png => {
                if opts.png_level == 6 {
                    grim.save_png(&result.data, result.width, result.height, path)?;
                } else {
                    grim.save_png_with_compression(
                        &result.data,
                        result.width,
                        result.height,
                        path,
                        opts.png_level
                    )?;
                }
            }
            FileType::Ppm => {
                grim.save_ppm(&result.data, result.width, result.height, path)?;
            }
            FileType::Jpeg => {
                if opts.jpeg_quality == 80 {
                    #[cfg(feature = "jpeg")]
                    grim.save_jpeg(&result.data, result.width, result.height, path)?;
                    #[cfg(not(feature = "jpeg"))]
                    return Err(
                        grim_rs::Error::ImageProcessing(
                            image::ImageError::Unsupported(
                                image::error::UnsupportedError::from_format_and_kind(
                                    image::error::ImageFormatHint::Name("JPEG".to_string()),
                                    image::error::UnsupportedErrorKind::Format(
                                        image::ImageFormat::Jpeg.into()
                                    )
                                )
                            )
                        )
                    );
                } else {
                    #[cfg(feature = "jpeg")]
                    grim.save_jpeg_with_quality(
                        &result.data,
                        result.width,
                        result.height,
                        path,
                        opts.jpeg_quality
                    )?;
                    #[cfg(not(feature = "jpeg"))]
                    return Err(
                        grim_rs::Error::ImageProcessing(
                            image::ImageError::Unsupported(
                                image::error::UnsupportedError::from_format_and_kind(
                                    image::error::ImageFormatHint::Name("JPEG".to_string()),
                                    image::error::UnsupportedErrorKind::Format(
                                        image::ImageFormat::Jpeg.into()
                                    )
                                )
                            )
                        )
                    );
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
struct Options {
    scale: Option<f64>,
    geometry: Option<GrimBox>,
    filetype: FileType,
    jpeg_quality: u8,
    png_level: u8,
    output_name: Option<String>,
    with_cursor: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            scale: None,
            geometry: None,
            filetype: FileType::Png,
            jpeg_quality: 80,
            png_level: 6,
            output_name: None,
            with_cursor: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum FileType {
    Png,
    Ppm,
    Jpeg,
}

fn print_help() {
    println!(
        "Usage: grim [options...] [output-file]\n\
         \n\
         Options:\n\
         -h              Show help message and quit.\n\
         -s <factor>     Set the output image's scale factor.\n\
         -g <geometry>   Set the region to capture.\n\
         -t png|ppm|jpeg Set the output filetype.\n\
         -q <quality>    Set the JPEG filetype compression rate (0-100).\n\
         -l <level>      Set the PNG filetype compression level (0-9).\n\
         -o <output>     Set the output name to capture.\n\
         -c              Include cursors in the screenshot.\n\
         \n\
         If output-file is '-', output to standard output.\n\
         If no output-file is specified, use a default timestamped filename."
    );
}

fn generate_default_filename(filetype: FileType) -> grim_rs::Result<String> {
    use std::time::{ SystemTime, UNIX_EPOCH };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| grim_rs::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    let timestamp = now.as_secs();

    let ext = match filetype {
        FileType::Png => "png",
        FileType::Ppm => "ppm",
        FileType::Jpeg => "jpeg",
    };

    Ok(format!("{}.{}", timestamp, ext))
}
