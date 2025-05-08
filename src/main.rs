use grim_rs::{capture_screenshot, ScreenshotOptions, ScreenshotFormat, save_screenshot_with_format};

fn main() {
    let img = capture_screenshot(ScreenshotOptions {
        format: ScreenshotFormat::Png, // или Jpeg, или Bmp
        ..Default::default()
    }).unwrap();

    save_screenshot_with_format(&img, "test.png", ScreenshotFormat::Png).unwrap();
    save_screenshot_with_format(&img, "test.jpg", ScreenshotFormat::Jpeg).unwrap();
    save_screenshot_with_format(&img, "test.bmp", ScreenshotFormat::Bmp).unwrap();
}