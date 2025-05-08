//! Библиотека для создания скриншотов в Wayland.

use image::{ExtendedColorType, ImageBuffer, ImageEncoder, Rgba, RgbaImage};
use std::os::unix::io::{AsRawFd, BorrowedFd};
use std::fs::File;
use std::ptr;
use std::slice;
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    protocol::{wl_output, wl_shm, wl_registry, wl_buffer, wl_shm_pool},
    WEnum,
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1, zwlr_screencopy_manager_v1,
};
use image::DynamicImage;
use image::ImageFormat;
use image::codecs::png::PngEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::bmp::BmpEncoder;
use image::ColorType;

/// Опции для создания скриншота
#[derive(Debug, Clone)]
pub struct ScreenshotOptions {
    /// Имя или идентификатор вывода (экрана). Если None — первый доступный.
    pub output_name: Option<String>,
    /// Область экрана (x, y, width, height). Если None — весь экран.
    pub region: Option<(u32, u32, u32, u32)>,
    /// Формат сохранения (пока только PNG, но можно расширить).
    pub format: ScreenshotFormat,
}

#[derive(Debug, Clone, Copy)]
pub enum ScreenshotFormat {
    Png,
    Jpeg,
    Bmp,
}

impl ScreenshotFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ScreenshotFormat::Png => "png",
            ScreenshotFormat::Jpeg => "jpg",
            ScreenshotFormat::Bmp => "bmp",
        }
    }
}

impl Default for ScreenshotOptions {
    fn default() -> Self {
        Self {
            output_name: None,
            region: None,
            format: ScreenshotFormat::Png,
        }
    }
}

#[derive(Debug)]
pub enum ScreenshotError {
    WaylandConnect(String),
    Protocol(String),
    Io(std::io::Error),
    Internal(String),
}

struct GrimState {
    output: Option<wl_output::WlOutput>,
    output_name: Option<String>,
    output_name_matched: bool,
    region: Option<(u32, u32, u32, u32)>,
    frame: Option<zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1>,
    width: Option<u32>,
    height: Option<u32>,
    stride: Option<u32>,
    ptr: Option<*mut u8>,
    size: Option<usize>,
    shm_file: Option<File>,
    done: bool,
    shm: Option<wl_shm::WlShm>,
    screencopy_manager: Option<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1>,
    format: Option<wl_shm::Format>,
    error: Option<String>,
}

impl GrimState {
    fn new(options: &ScreenshotOptions) -> Self {
        GrimState {
            output: None,
            output_name: options.output_name.clone(),
            output_name_matched: false,
            region: options.region,
            frame: None,
            width: None,
            height: None,
            stride: None,
            ptr: None,
            size: None,
            shm_file: None,
            done: false,
            shm: None,
            screencopy_manager: None,
            format: None,
            error: None,
        }
    }

    fn map_shm(&mut self, size: usize) -> Result<(*mut u8, usize), ScreenshotError> {
        let file = tempfile::tempfile().map_err(ScreenshotError::Io)?;
        file.set_len(size as u64).map_err(ScreenshotError::Io)?;
        unsafe {
            let ptr = libc::mmap(
                ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                file.as_raw_fd(),
                0,
            );
            if ptr == libc::MAP_FAILED {
                return Err(ScreenshotError::Internal(format!("mmap failed: {}", std::io::Error::last_os_error())));
            }
            self.shm_file = Some(file);
            Ok((ptr as *mut u8, size))
        }
    }
    fn unmap_shm(&mut self) {
        if let (Some(ptr), Some(size)) = (self.ptr.take(), self.size.take()) {
            unsafe {
                libc::munmap(ptr as *mut libc::c_void, size);
            }
        }
        self.shm_file.take();
    }
}

impl Drop for GrimState {
    fn drop(&mut self) {
        self.unmap_shm();
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for GrimState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global { name, interface, version } => {
                match interface.as_str() {
                    "wl_shm" => {
                        let shm = registry.bind::<wl_shm::WlShm, _, _>(name, version, qh, ());
                        state.shm = Some(shm);
                    }
                    "wl_output" => {
                        if !state.output_name_matched {
                            let output = registry.bind::<wl_output::WlOutput, _, _>(name, version, qh, ());
                            if state.output_name.is_none() {
                                state.output = Some(output);
                                state.output_name_matched = true;
                            } else {
                                state.output = Some(output);
                                state.output_name_matched = true;
                            }
                            if let (Some(output), Some(manager)) = (&state.output, &state.screencopy_manager) {
                                let frame = if let Some(region) = state.region {
                                    manager.capture_output_region(0, output, region.0 as i32, region.1 as i32, region.2 as i32, region.3 as i32, qh, ())
                                } else {
                                    manager.capture_output(0, output, qh, ())
                                };
                                state.frame = Some(frame);
                            }
                        }
                    }
                    "zwlr_screencopy_manager_v1" => {
                        let manager = registry.bind::<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, _, _>(name, version, qh, ());
                        state.screencopy_manager = Some(manager);
                        if let (Some(output), Some(manager)) = (&state.output, &state.screencopy_manager) {
                            let frame = manager.capture_output(0, output, qh, ());
                            state.frame = Some(frame);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for GrimState {
    fn event(
        _: &mut Self,
        _: &wl_output::WlOutput,
        _: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, ()> for GrimState {
    fn event(
        _: &mut Self,
        _: &zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
        _: zwlr_screencopy_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1, ()> for GrimState {
    fn event(
        state: &mut Self,
        frame: &zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        _: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer { format, width, height, stride } => {
                let buffer_format = if let WEnum::Value(fmt) = format {
                    if fmt != wl_shm::Format::Argb8888 && fmt != wl_shm::Format::Xrgb8888 {
                        state.error = Some(format!("Unsupported buffer format: {:?}", fmt));
                        return;
                    }
                    state.format = Some(fmt);
                    fmt
                } else {
                    state.error = Some("Invalid buffer format".to_string());
                    return;
                };
                state.width = Some(width);
                state.height = Some(height);
                state.stride = Some(stride);
                let size = (stride * height) as usize;
                let (ptr, size) = match state.map_shm(size) {
                    Ok(result) => result,
                    Err(e) => {
                        state.error = Some(format!("Failed to map shared memory: {e:?}"));
                        return;
                    }
                };
                state.ptr = Some(ptr);
                state.size = Some(size);
                if let Some(shm) = &state.shm {
                    if let Some(file) = &state.shm_file {
                        let fd = unsafe { BorrowedFd::borrow_raw(file.as_raw_fd()) };
                        let pool = shm.create_pool(fd, size as i32, qh, ());
                        let buffer = pool.create_buffer(
                            0,
                            width as i32,
                            height as i32,
                            stride as i32,
                            buffer_format,
                            qh,
                            (),
                        );
                        frame.copy(&buffer);
                    }
                }
            }
            zwlr_screencopy_frame_v1::Event::Ready { tv_sec_hi: _, tv_sec_lo: _, tv_nsec: _ } => {
                state.done = true;
            }
            zwlr_screencopy_frame_v1::Event::Failed => {
                state.error = Some("Screencopy failed".to_string());
            }
            zwlr_screencopy_frame_v1::Event::BufferDone => {}
            _ => {}
        }
    }
}

impl Dispatch<wl_shm::WlShm, ()> for GrimState {
    fn event(
        _: &mut Self,
        _: &wl_shm::WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for GrimState {
    fn event(
        _: &mut Self,
        _: &wl_shm_pool::WlShmPool,
        _: wl_shm_pool::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_buffer::WlBuffer, ()> for GrimState {
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        _: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

pub fn capture_screenshot(options: ScreenshotOptions) -> Result<RgbaImage, ScreenshotError> {
    let conn = Connection::connect_to_env().map_err(|e| ScreenshotError::WaylandConnect(e.to_string()))?;
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();
    let mut state = GrimState::new(&options);
    let display = conn.display();
    let _registry = display.get_registry(&qh, ());
    while !state.done && state.error.is_none() {
        event_queue.blocking_dispatch(&mut state).map_err(|e| ScreenshotError::Protocol(e.to_string()))?;
    }
    if let Some(err) = &state.error {
        return Err(ScreenshotError::Internal(err.to_string()));
    }
    let width = state.width.ok_or_else(|| ScreenshotError::Internal("No width".to_string()))?;
    let height = state.height.ok_or_else(|| ScreenshotError::Internal("No height".to_string()))?;
    let stride = state.stride.ok_or_else(|| ScreenshotError::Internal("No stride".to_string()))?;
    let ptr = state.ptr.ok_or_else(|| ScreenshotError::Internal("No ptr".to_string()))?;
    let size = state.size.ok_or_else(|| ScreenshotError::Internal("No size".to_string()))?;
    let data = unsafe { slice::from_raw_parts(ptr, size) };
    let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let offset = (y * stride + x * 4) as usize;
            if offset + 3 < data.len() {
                match state.format {
                    Some(wl_shm::Format::Xrgb8888) => {
                        rgba_data.extend_from_slice(&[
                            data[offset + 2],
                            data[offset + 1],
                            data[offset + 0],
                            255,
                        ]);
                    }
                    Some(wl_shm::Format::Argb8888) => {
                        rgba_data.extend_from_slice(&[
                            data[offset + 2],
                            data[offset + 1],
                            data[offset + 0],
                            data[offset + 3],
                        ]);
                    }
                    _ => return Err(ScreenshotError::Internal("Unknown format".to_string())),
                }
            }
        }
    }
    ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, rgba_data)
        .ok_or_else(|| ScreenshotError::Internal("Failed to create image buffer".to_string()))
}

/// Сохранить изображение в нужном формате
pub fn save_screenshot_with_format(img: &RgbaImage, path: &str, format: ScreenshotFormat) -> Result<(), ScreenshotError> {
    use std::fs::File;
    use std::io::BufWriter;
    let file = File::create(path).map_err(ScreenshotError::Io)?;
    let mut writer = BufWriter::new(file);
    match format {
        ScreenshotFormat::Png => {
            let encoder = PngEncoder::new(&mut writer);
            encoder.write_image(img, img.width(), img.height(), ColorType::Rgba8.into())
                .map_err(|e| ScreenshotError::Internal(format!("PNG encode error: {e}")))
        }
        ScreenshotFormat::Jpeg => {
            let mut encoder = JpegEncoder::new_with_quality(&mut writer, 90);
            encoder.encode_image(img)
                .map_err(|e| ScreenshotError::Internal(format!("JPEG encode error: {e}")))
        }
        ScreenshotFormat::Bmp => {
            let encoder = BmpEncoder::new(&mut writer);
            encoder.write_image(img, img.width(), img.height(), ExtendedColorType::Rgba8)
                .map_err(|e| ScreenshotError::Internal(format!("BMP encode error: {e}")))
        }
    }
}

pub trait ScreenshotSaveExt {
    fn save_as_png(&self, path: &str) -> Result<(), ScreenshotError>;
    fn save_as_jpeg(&self, path: &str) -> Result<(), ScreenshotError>;
    fn save_as_bmp(&self, path: &str) -> Result<(), ScreenshotError>;
}

impl ScreenshotSaveExt for RgbaImage {
    fn save_as_png(&self, path: &str) -> Result<(), ScreenshotError> {
        save_screenshot_with_format(self, path, ScreenshotFormat::Png)
    }
    fn save_as_jpeg(&self, path: &str) -> Result<(), ScreenshotError> {
        save_screenshot_with_format(self, path, ScreenshotFormat::Jpeg)
    }
    fn save_as_bmp(&self, path: &str) -> Result<(), ScreenshotError> {
        save_screenshot_with_format(self, path, ScreenshotFormat::Bmp)
    }
}