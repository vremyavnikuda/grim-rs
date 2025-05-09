//! Grim-rs - утилита для создания скриншотов в Wayland
//! 
//! Эта программа позволяет сделать скриншот выбранной области экрана
//! в Wayland окружении.

use grim_rs::{capture_screenshot, select_region_interactive_sctk, ScreenshotOptions, ScreenshotFormat, ScreenshotSaveExt};
use std::process;
use log::{info, error, warn};

fn main() {
    // Инициализируем логгер
    env_logger::init();
    info!("Starting grim-rs screenshot tool");

    let format = ScreenshotFormat::Png;

    // Интерактивно выбираем область экрана
    info!("Starting interactive region selection");
    if let Some(region) = select_region_interactive_sctk() {
        info!("Region selected: x={}, y={}, width={}, height={}", 
            region.x, region.y, region.width, region.height);

        // Создаем опции для скриншота
        let options = ScreenshotOptions {
            output_name: None,
            region: Some((
                region.x.try_into().unwrap_or(0),
                region.y.try_into().unwrap_or(0),
                region.width.try_into().unwrap_or(1),
                region.height.try_into().unwrap_or(1)
            )),
            format,
        };
        
        // Делаем скриншот и сохраняем его
        info!("Capturing screenshot with selected region");
        match capture_screenshot(options) {
            Ok(image) => {
                let output_path = image.generate_filename(format);
                info!("Screenshot captured successfully, saving to {}", output_path);
                if let Err(e) = image.save_as_png(&output_path) {
                    error!("Failed to save screenshot: {}", e);
                    process::exit(1);
                }
                info!("Screenshot saved successfully to {}", output_path);
            }
            Err(e) => {
                error!("Failed to capture screenshot: {}", e);
                process::exit(1);
            }
        }
    } else {
        warn!("Region selection was cancelled or failed");
        process::exit(1);
    }
}