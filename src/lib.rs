//! Grim-rs - утилита для создания скриншотов в Wayland
//! 
//! Эта библиотека предоставляет функциональность для создания скриншотов в Wayland окружении,
//! включая интерактивный выбор области экрана.

use image::{ExtendedColorType, ImageBuffer, ImageEncoder, Rgba, RgbaImage};
use smithay_client_toolkit::output;
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
use image::codecs::png::PngEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::bmp::BmpEncoder;
use image::ColorType;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};
use wayland_protocols::xdg::shell::client::{xdg_wm_base, xdg_surface, xdg_toplevel};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use log::{info, error, warn, debug};
use chrono;
mod region_select;
mod region_select_sctk;

/// Интерактивно выбрать область экрана с помощью мыши.
/// 
/// Эта функция создает полупрозрачный оверлей на весь экран и позволяет пользователю
/// выбрать область с помощью мыши. Выбор осуществляется зажатием левой кнопки мыши
/// и перетаскиванием для определения размера области.
/// 
/// # Примеры
/// 
/// ```no_run
/// use grim_rs::{select_region_interactive, capture_screenshot, ScreenshotOptions, ScreenshotFormat};
/// 
/// // Выбрать область интерактивно
/// match select_region_interactive() {
///     Ok(region) => {
///         // Создать опции для скриншота с выбранной областью
///         let options = ScreenshotOptions {
///             output_name: None,
///             region: Some((region.x, region.y, region.width, region.height)),
///             format: ScreenshotFormat::Png,
///         };
///         
///         // Сделать скриншот выбранной области
///         if let Ok(image) = capture_screenshot(options) {
///             // Сохранить скриншот
///             image.save_as_png("screenshot.png").unwrap();
///         }
///     }
///     Err(e) => eprintln!("Failed to select region: {}", e),
/// }
/// ```
pub use region_select::{RegionSelection, RegionSelectionError, select_region_interactive};
pub use region_select_sctk::{select_region_interactive_sctk, Region};

/// Опции для создания скриншота
#[derive(Debug, Clone)]
pub struct ScreenshotOptions {
    /// Имя или идентификатор вывода (экрана). Если None — первый доступный.
    pub output_name: Option<String>,
    /// Область экрана (x, y, width, height). Если None — весь экран.
    pub region: Option<(u32, u32, u32, u32)>,
    /// Формат сохранения (PNG, JPG, BMP).
    pub format: ScreenshotFormat,
}

/// Поддерживаемые форматы сохранения скриншотов
#[derive(Debug, Clone, Copy)]
pub enum ScreenshotFormat {
    /// Формат PNG с поддержкой прозрачности
    Png,
    /// Формат JPEG с настраиваемым качеством
    Jpeg,
    /// Формат BMP без сжатия
    Bmp,
}

impl ScreenshotFormat {
    /// Возвращает расширение файла для данного формата
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

/// Ошибки, которые могут возникнуть при создании скриншота
#[derive(Debug)]
pub enum ScreenshotError {
    /// Ошибка подключения к Wayland
    WaylandConnect(String),
    /// Ошибка протокола Wayland
    Protocol(String),
    /// Ошибка ввода-вывода
    Io(std::io::Error),
    /// Внутренняя ошибка
    Internal(String),
}

impl fmt::Display for ScreenshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScreenshotError::WaylandConnect(e) => write!(f, "Wayland connection error: {}", e),
            ScreenshotError::Protocol(e) => write!(f, "Wayland protocol error: {}", e),
            ScreenshotError::Io(e) => write!(f, "IO error: {}", e),
            ScreenshotError::Internal(e) => write!(f, "Internal error: {}", e),
        }
    }
}

impl Error for ScreenshotError {}

impl From<std::io::Error> for ScreenshotError {
    fn from(err: std::io::Error) -> Self {
        ScreenshotError::Io(err)
    }
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
        debug!("Creating new GrimState with options: {:?}", options);
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
        debug!("Mapping shared memory of size: {}", size);
        let file = tempfile::tempfile().map_err(|e| {
            error!("Failed to create temporary file: {}", e);
            ScreenshotError::Io(e)
        })?;
        
        file.set_len(size as u64).map_err(|e| {
            error!("Failed to set file length: {}", e);
            ScreenshotError::Io(e)
        })?;
        
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
                let err = std::io::Error::last_os_error();
                error!("mmap failed: {}", err);
                return Err(ScreenshotError::Internal(format!("mmap failed: {}", err)));
            }
            debug!("Successfully mapped shared memory at {:p}", ptr);
            self.shm_file = Some(file);
            Ok((ptr as *mut u8, size))
        }
    }

    fn unmap_shm(&mut self) {
        if let (Some(ptr), Some(size)) = (self.ptr.take(), self.size.take()) {
            debug!("Unmapping shared memory at {:p} of size {}", ptr, size);
            unsafe {
                if libc::munmap(ptr as *mut libc::c_void, size) != 0 {
                    warn!("Failed to unmap shared memory: {}", std::io::Error::last_os_error());
                }
            }
        }
        self.shm_file.take();
    }
}

impl Drop for GrimState {
    fn drop(&mut self) {
        debug!("Dropping GrimState");
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
                debug!("Received global event: interface={}, version={}, name={}", interface, version, name);
                match interface.as_str() {
                    "wl_shm" => {
                        debug!("Binding wl_shm");
                        let shm = registry.bind::<wl_shm::WlShm, _, _>(name, version, qh, ());
                        state.shm = Some(shm);
                    }
                    "wl_output" => {
                        debug!("Binding wl_output");
                        if !state.output_name_matched {
                            let output = registry.bind::<wl_output::WlOutput, _, _>(name, version, qh, ());
                            if state.output_name.is_none() {
                                debug!("Using first available output");
                                state.output = Some(output);
                                state.output_name_matched = true;
                            } else {
                                debug!("Checking output name match");
                                state.output = Some(output);
                                state.output_name_matched = true;
                            }
                            if let (Some(output), Some(manager)) = (&state.output, &state.screencopy_manager) {
                                debug!("Creating screencopy frame");
                                let frame = if let Some(region) = state.region {
                                    debug!("Capturing region: {:?}", region);
                                    manager.capture_output_region(0, output, region.0 as i32, region.1 as i32, region.2 as i32, region.3 as i32, qh, ())
                                } else {
                                    debug!("Capturing full output");
                                    manager.capture_output(0, output, qh, ())
                                };
                                state.frame = Some(frame);
                            }
                        }
                    }
                    "zwlr_screencopy_manager_v1" => {
                        debug!("Binding zwlr_screencopy_manager_v1");
                        let manager = registry.bind::<zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1, _, _>(name, version, qh, ());
                        state.screencopy_manager = Some(manager);
                    }
                    _ => debug!("Ignoring unknown interface: {}", interface),
                }
            }
            wl_registry::Event::GlobalRemove { name } => {
                debug!("Global removed: {}", name);
            }
            _ => debug!("Unhandled registry event"),
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
                debug!("Received buffer event: format={:?}, width={}, height={}, stride={}", format, width, height, stride);
                let buffer_format = if let WEnum::Value(fmt) = format {
                    if fmt != wl_shm::Format::Argb8888 && fmt != wl_shm::Format::Xrgb8888 {
                        error!("Unsupported buffer format: {:?}", fmt);
                        state.error = Some(format!("Unsupported buffer format: {:?}", fmt));
                        return;
                    }
                    debug!("Using buffer format: {:?}", fmt);
                    state.format = Some(fmt);
                    fmt
                } else {
                    error!("Invalid buffer format received");
                    state.error = Some("Invalid buffer format".to_string());
                    return;
                };
                state.width = Some(width);
                state.height = Some(height);
                state.stride = Some(stride);
                let size = (stride * height) as usize;
                debug!("Calculated buffer size: {}", size);
                let (ptr, size) = match state.map_shm(size) {
                    Ok(result) => result,
                    Err(e) => {
                        error!("Failed to map shared memory: {}", e);
                        state.error = Some(format!("Failed to map shared memory: {e:?}"));
                        return;
                    }
                };
                state.ptr = Some(ptr);
                state.size = Some(size);
                if let Some(shm) = &state.shm {
                    if let Some(file) = &state.shm_file {
                        debug!("Creating shared memory pool and buffer");
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
                        debug!("Copying frame to buffer");
                        frame.copy(&buffer);
                    } else {
                        error!("No shared memory file available");
                        state.error = Some("No shared memory file available".to_string());
                    }
                } else {
                    error!("No shared memory interface available");
                    state.error = Some("No shared memory interface available".to_string());
                }
            }
            zwlr_screencopy_frame_v1::Event::Ready { tv_sec_hi, tv_sec_lo, tv_nsec } => {
                debug!("Frame ready event received: tv_sec_hi={}, tv_sec_lo={}, tv_nsec={}", tv_sec_hi, tv_sec_lo, tv_nsec);
                state.done = true;
            }
            zwlr_screencopy_frame_v1::Event::Failed => {
                error!("Screencopy frame failed");
                state.error = Some("Screencopy failed".to_string());
            }
            zwlr_screencopy_frame_v1::Event::BufferDone => {
                debug!("Buffer done event received");
            }
            _ => {
                debug!("Unhandled frame event received");
            }
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
    info!("Starting screenshot capture with options: {:?}", options);
    
    let conn = Connection::connect_to_env().map_err(|e| {
        error!("Failed to connect to Wayland: {}", e);
        ScreenshotError::WaylandConnect(e.to_string())
    })?;
    
    let display = conn.display();
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();
    
    let mut state = GrimState::new(&options);
    
    let _registry = display.get_registry(&qh, ());
    
    info!("Waiting for Wayland events");
    event_queue.roundtrip(&mut state).map_err(|e| {
        error!("Failed to process Wayland events: {}", e);
        ScreenshotError::Protocol(e.to_string())
    })?;
    
    if let Some(error) = &state.error {
        error!("Screenshot capture failed: {}", error);
        return Err(ScreenshotError::Internal(error.to_string()));
    }
    
    // Wait for frame events
    info!("Waiting for frame events");
    while !state.done && state.error.is_none() {
        event_queue.blocking_dispatch(&mut state).map_err(|e| {
            error!("Failed to process frame events: {}", e);
            ScreenshotError::Protocol(e.to_string())
        })?;
    }
    
    if let Some(error) = &state.error {
        error!("Screenshot capture failed: {}", error);
        return Err(ScreenshotError::Internal(error.to_string()));
    }
    
    if state.done {
        info!("Screenshot capture completed successfully");
        debug!("State at completion: width={:?}, height={:?}, stride={:?}, ptr={:?}", 
               state.width, state.height, state.stride, state.ptr.is_some());
        
        let width = state.width.ok_or_else(|| {
            error!("Missing width in state");
            ScreenshotError::Internal("Missing width in state".to_string())
        })?;
        let height = state.height.ok_or_else(|| {
            error!("Missing height in state");
            ScreenshotError::Internal("Missing height in state".to_string())
        })?;
        let stride = state.stride.ok_or_else(|| {
            error!("Missing stride in state");
            ScreenshotError::Internal("Missing stride in state".to_string())
        })?;
        let ptr = state.ptr.ok_or_else(|| {
            error!("Missing pointer in state");
            ScreenshotError::Internal("Missing pointer in state".to_string())
        })?;
        
        debug!("Creating image from buffer: width={}, height={}, stride={}", width, height, stride);
        let data = unsafe {
            slice::from_raw_parts(ptr, (stride * height) as usize)
        };
        
        // Clone the data before creating the image buffer
        let data_vec = data.to_vec();
        
        // Создаем новое изображение
        let mut rgba_image = RgbaImage::new(width, height);
        
        // Конвертируем BGRA в RGBA
        for y in 0..height {
            for x in 0..width {
                let idx = (y * stride + x * 4) as usize;
                if idx + 3 < data_vec.len() {
                    let b = data_vec[idx];
                    let g = data_vec[idx + 1];
                    let r = data_vec[idx + 2];
                    let a = data_vec[idx + 3];
                    rgba_image.put_pixel(x, y, Rgba([r, g, b, a]));
                }
            }
        }
        
        info!("Successfully created image buffer: {}x{}", width, height);
        Ok(rgba_image)
    } else {
        error!("Screenshot capture did not complete. State: done={}, error={:?}", 
               state.done, state.error);
        Err(ScreenshotError::Internal("Screenshot capture did not complete".to_string()))
    }
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
            encoder.write_image(img, img.width(), img.height(), ColorType::Rgba8)
                .map_err(|e| ScreenshotError::Internal(format!("BMP encode error: {e}")))
        }
    }
}

pub trait ScreenshotSaveExt {
    fn save_as_png(&self, path: &str) -> Result<(), ScreenshotError>;
    fn save_as_jpeg(&self, path: &str) -> Result<(), ScreenshotError>;
    fn save_as_bmp(&self, path: &str) -> Result<(), ScreenshotError>;
    fn generate_filename(&self, format: ScreenshotFormat) -> String;
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
    fn generate_filename(&self, format: ScreenshotFormat) -> String {
        let now = chrono::Local::now();
        format!("./{}_{}_grim-rs.{}", 
            now.format("%Y-%m-%d"),
            now.format("%H-%M-%S"),
            format.extension())
    }
}

/// Получить размеры основного экрана
pub fn get_screen_dimensions() -> Result<(u32, u32), ScreenshotError> {
    use wayland_client::{
        protocol::{wl_output, wl_registry},
        Connection, QueueHandle,
        globals::{registry_queue_init, GlobalListContents},
        Dispatch,
    };

    #[derive(Default)]
    struct OutputState {
        width: Option<u32>,
        height: Option<u32>,
    }

    struct OutputHandler {
        state: OutputState,
    }

    impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for OutputHandler {
        fn event(
            _: &mut Self,
            _: &wl_registry::WlRegistry,
            _: wl_registry::Event,
            _: &GlobalListContents,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {}
    }

    impl Dispatch<wl_output::WlOutput, ()> for OutputHandler {
        fn event(
            state: &mut Self,
            _: &wl_output::WlOutput,
            event: wl_output::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            match event {
                wl_output::Event::Mode { width, height, .. } => {
                    state.state.width = Some(width as u32);
                    state.state.height = Some(height as u32);
                }
                _ => {}
            }
        }
    }

    let conn = Connection::connect_to_env().map_err(|e| ScreenshotError::WaylandConnect(e.to_string()))?;
    let (globals, mut event_queue) = registry_queue_init::<OutputHandler>(&conn)
        .map_err(|e| ScreenshotError::Protocol(e.to_string()))?;
    let qh = event_queue.handle();

    let _output = globals.bind::<wl_output::WlOutput, _, _>(&qh, 1..=4, ())
        .map_err(|e| ScreenshotError::Protocol(e.to_string()))?;

    let mut handler = OutputHandler {
        state: OutputState::default(),
    };

    // Wait for mode event
    while handler.state.width.is_none() || handler.state.height.is_none() {
        event_queue.blocking_dispatch(&mut handler)
            .map_err(|e| ScreenshotError::Protocol(e.to_string()))?;
    }

    Ok((
        handler.state.width.unwrap(),
        handler.state.height.unwrap(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screenshot_format_extension() {
        assert_eq!(ScreenshotFormat::Png.extension(), "png");
        assert_eq!(ScreenshotFormat::Jpeg.extension(), "jpg");
        assert_eq!(ScreenshotFormat::Bmp.extension(), "bmp");
    }

    #[test]
    fn test_screenshot_options_default() {
        let options = ScreenshotOptions::default();
        assert!(options.output_name.is_none());
        assert!(options.region.is_none());
        assert!(matches!(options.format, ScreenshotFormat::Png));
    }
}