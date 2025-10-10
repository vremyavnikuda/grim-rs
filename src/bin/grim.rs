use grim_rs::{Box as GrimBox, CaptureParameters, Grim};
use std::env;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

fn main() -> grim_rs::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut opts = Options::default();
    let mut output_file = None;
    let mut arg_idx = 1;

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
                opts.scale = Some(args[arg_idx].parse::<f64>().map_err(|_| {
                    grim_rs::Error::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Invalid scale factor",
                    ))
                })?);
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
                let quality: i32 = args[arg_idx].parse().map_err(|_| {
                    grim_rs::Error::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Invalid quality value",
                    ))
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
                let level: i32 = args[arg_idx].parse().map_err(|_| {
                    grim_rs::Error::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Invalid compression level",
                    ))
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

    let output_file = if let Some(file) = output_file {
        file
    } else {
        generate_default_filename(opts.filetype)?
    };

    let mut grim = Grim::new()?;

    let result = if let Some(ref output_name) = opts.output_name {
        if opts.with_cursor {
            let mut params =
                CaptureParameters::new(output_name.clone()).overlay_cursor(opts.with_cursor);
            if let Some(region) = opts.geometry {
                params = params.region(region);
            }
            if let Some(scale) = opts.scale {
                params = params.scale(scale);
            }
            let multi_result =
                grim.capture_outputs_with_scale(vec![params], opts.scale.unwrap_or(1.0))?;
            if let Some(capture_result) = multi_result.get(output_name) {
                capture_result.clone()
            } else {
                return Err(grim_rs::Error::OutputNotFound(output_name.clone()));
            }
        } else {
            grim.capture_output_with_scale(output_name, opts.scale.unwrap_or(1.0))?
        }
    } else if let Some(ref geometry) = opts.geometry {
        grim.capture_region_with_scale(*geometry, opts.scale.unwrap_or(1.0))?
    } else {
        grim.capture_all_with_scale(opts.scale.unwrap_or(1.0))?
    };

    if output_file == "-" {
        // Output to stdout
        match opts.filetype {
            FileType::Png => {
                if opts.png_level == 6 {
                    grim.write_png_to_stdout(&result.data(), result.width(), result.height())?;
                } else {
                    grim.write_png_to_stdout_with_compression(
                        &result.data(),
                        result.width(),
                        result.height(),
                        opts.png_level,
                    )?;
                }
            }
            FileType::Ppm => {
                grim.write_ppm_to_stdout(&result.data(), result.width(), result.height())?;
            }
            FileType::Jpeg => {
                if opts.jpeg_quality == 80 {
                    #[cfg(feature = "jpeg")]
                    grim.write_jpeg_to_stdout(&result.data(), result.width(), result.height())?;
                } else {
                    #[cfg(feature = "jpeg")]
                    grim.write_jpeg_to_stdout_with_quality(
                        &result.data(),
                        result.width(),
                        result.height(),
                        opts.jpeg_quality,
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
                    grim.save_png(&result.data(), result.width(), result.height(), path)?;
                } else {
                    grim.save_png_with_compression(
                        &result.data(),
                        result.width(),
                        result.height(),
                        path,
                        opts.png_level,
                    )?;
                }
            }
            FileType::Ppm => {
                grim.save_ppm(&result.data(), result.width(), result.height(), path)?;
            }
            FileType::Jpeg => {
                if opts.jpeg_quality == 80 {
                    #[cfg(feature = "jpeg")]
                    grim.save_jpeg(&result.data(), result.width(), result.height(), path)?;
                    #[cfg(not(feature = "jpeg"))]
                    return Err(grim_rs::Error::ImageProcessing(
                        image::ImageError::Unsupported(
                            image::error::UnsupportedError::from_format_and_kind(
                                image::error::ImageFormatHint::Name("JPEG".to_string()),
                                image::error::UnsupportedErrorKind::Format(
                                    image::ImageFormat::Jpeg.into(),
                                ),
                            ),
                        ),
                    ));
                } else {
                    #[cfg(feature = "jpeg")]
                    grim.save_jpeg_with_quality(
                        &result.data(),
                        result.width(),
                        result.height(),
                        path,
                        opts.jpeg_quality,
                    )?;
                    #[cfg(not(feature = "jpeg"))]
                    return Err(grim_rs::Error::ImageProcessing(
                        image::ImageError::Unsupported(
                            image::error::UnsupportedError::from_format_and_kind(
                                image::error::ImageFormatHint::Name("JPEG".to_string()),
                                image::error::UnsupportedErrorKind::Format(
                                    image::ImageFormat::Jpeg.into(),
                                ),
                            ),
                        ),
                    ));
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
    use chrono::Local;

    // Format: YYYYMMDD_HHhMMmSSs_grim.ext (e.g., 20241004_10h30m45s_grim.png)
    let now = Local::now();
    let timestamp = now.format("%Y%m%d_%Hh%Mm%Ss");

    let ext = match filetype {
        FileType::Png => "png",
        FileType::Ppm => "ppm",
        FileType::Jpeg => "jpeg",
    };

    let output_dir = get_output_dir();
    let filename = format!("{}_grim.{}", timestamp, ext);

    Ok(output_dir.join(filename).to_string_lossy().to_string())
}

/// ~/.config/user-dirs.dirs
fn get_xdg_pictures_dir() -> Option<PathBuf> {
    // XDG_PICTURES_DIR
    if let Ok(pictures_dir) = env::var("XDG_PICTURES_DIR") {
        let expanded = expand_home_dir(&pictures_dir);
        return Some(PathBuf::from(expanded));
    }

    // Parse ~/.config/user-dirs.dirs
    let config_home = env::var("XDG_CONFIG_HOME").ok().or_else(|| {
        env::var("HOME")
            .ok()
            .map(|home| format!("{}/.config", home))
    })?;

    let user_dirs_file = PathBuf::from(config_home).join("user-dirs.dirs");

    if !user_dirs_file.exists() {
        return None;
    }

    let file = fs::File::open(user_dirs_file).ok()?;
    let reader = io::BufReader::new(file);

    for line in reader.lines().map_while(Result::ok) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Look for XDG_PICTURES_DIR="..."
        if line.starts_with("XDG_PICTURES_DIR=") {
            if let Some(value) = line.strip_prefix("XDG_PICTURES_DIR=") {
                // Remove quotes
                let value = value.trim_matches('"').trim_matches('\'');
                let expanded = expand_home_dir(value);
                return Some(PathBuf::from(expanded));
            }
        }
    }

    None
}

/// Expand $HOME in paths
fn expand_home_dir(path: &str) -> String {
    if path.starts_with("$HOME") {
        if let Ok(home) = env::var("HOME") {
            return path.replace("$HOME", &home);
        }
    }
    path.to_string()
}

/// GRIM_DEFAULT_DIR > XDG_PICTURES_DIR > "."
fn get_output_dir() -> PathBuf {
    // GRIM_DEFAULT_DIR
    if let Ok(default_dir) = env::var("GRIM_DEFAULT_DIR") {
        let path = PathBuf::from(default_dir);
        if path.exists() || path.parent().map(|p| p.exists()).unwrap_or(false) {
            return path;
        }
    }

    // XDG_PICTURES_DIR
    if let Some(pictures_dir) = get_xdg_pictures_dir() {
        if pictures_dir.exists() {
            return pictures_dir;
        }
    }

    PathBuf::from(".")
}
