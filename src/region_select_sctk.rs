//! Модуль для интерактивного выбора области экрана с использованием Wayland layer shell
//! 
//! Этот модуль предоставляет функциональность для создания полупрозрачного оверлея
//! и выбора области экрана с помощью мыши.

use wayland_client::{
    protocol::{wl_compositor, wl_seat, wl_surface, wl_shm, wl_shm_pool, wl_buffer, wl_pointer, wl_keyboard, wl_registry},
    Connection, Dispatch, QueueHandle, globals::{registry_queue_init, GlobalListContents},
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};
use cairo::{Context, ImageSurface, Format};
use std::os::unix::io::{AsRawFd, BorrowedFd};
use tempfile::tempfile;
use std::fs::File;
use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::runtime::Runtime;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_shm::WlShm;

/// Представляет выбранную область экрана
#[derive(Debug, Clone, Copy)]
pub struct Region {
    /// X-координата левого верхнего угла
    pub x: i32,
    /// Y-координата левого верхнего угла
    pub y: i32,
    /// Ширина области
    pub width: i32,
    /// Высота области
    pub height: i32,
}

#[derive(Clone, Debug)]
struct SelectionState {
    selection: Option<(i32, i32, i32, i32)>,
    selection_start: Option<(i32, i32)>,
    current_position: Option<(i32, i32)>,
    exit: bool,
}

impl SelectionState {
    fn new() -> Self {
        Self {
            selection: None,
            selection_start: None,
            current_position: None,
            exit: false,
        }
    }
}

/// Состояние для интерактивного выбора области
/// Хранит все необходимые объекты Wayland и текущее состояние выбора
pub struct RegionSelectState {
    /// Композитор Wayland для создания поверхностей
    compositor: Option<wl_compositor::WlCompositor>,
    /// Общий доступ к памяти для создания буферов
    shm: Option<wl_shm::WlShm>,
    /// Устройство ввода (мышь, клавиатура)
    seat: Option<wl_seat::WlSeat>,
    /// Указатель мыши
    pointer: Option<wl_pointer::WlPointer>,
    /// Клавиатура
    keyboard: Option<wl_keyboard::WlKeyboard>,
    /// Поверхность для отрисовки
    surface: Option<wl_surface::WlSurface>,
    /// Слой для отображения поверх всех окон
    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    /// Поверхность слоя
    layer_surface: Option<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    /// Буфер для отрисовки
    buffer: Option<wl_buffer::WlBuffer>,
    /// Временный файл для буфера
    buffer_file: Option<File>,
    /// Ширина экрана
    width: u32,
    /// Высота экрана
    height: u32,
    /// Флаг активного выбора области
    selecting: bool,
    /// X-координата начальной точки
    start_x: i32,
    /// Y-координата начальной точки
    start_y: i32,
    /// X-координата текущей точки
    end_x: i32,
    /// Y-координата текущей точки
    end_y: i32,
    /// Флаг завершения выбора
    done: bool,
    /// Флаг отмены выбора
    cancelled: bool,
    last_draw: Instant,
    draw_interval: Duration,
    selection_state: Arc<Mutex<SelectionState>>,
    last_update: Arc<Mutex<Instant>>,
    update_interval: Duration,
    back_buffer: Option<wl_buffer::WlBuffer>,
    back_buffer_file: Option<File>,
}

impl RegionSelectState {
    /// Создает новое состояние для выбора области
    pub fn new() -> Self {
        let (width, height) = match crate::get_screen_dimensions() {
            Ok(dimensions) => dimensions,
            Err(e) => {
                log::error!("Failed to get screen dimensions: {}", e);
                (1920, 1080) // Значения по умолчанию
            }
        };
        log::info!("Creating RegionSelectState with screen size: {}x{}", width, height);
        Self {
            compositor: None,
            shm: None,
            surface: None,
            layer_surface: None,
            pointer: None,
            keyboard: None,
            seat: None,
            layer_shell: None,
            buffer: None,
            buffer_file: None,
            back_buffer: None,
            back_buffer_file: None,
            width,
            height,
            start_x: 0,
            start_y: 0,
            end_x: 0,
            end_y: 0,
            selecting: false,
            done: false,
            cancelled: false,
            last_draw: Instant::now(),
            draw_interval: Duration::from_millis(16), // ~60 FPS
            selection_state: Arc::new(Mutex::new(SelectionState::new())),
            last_update: Arc::new(Mutex::new(Instant::now())),
            update_interval: Duration::from_millis(16), // ~60 FPS
        }
    }

    /// Отрисовывает текущее состояние на surface
    /// Включает затемнение фона и отрисовку выделенной области
    fn draw(&mut self, qh: &QueueHandle<Self>) -> Result<(), Box<dyn std::error::Error>> {
        let surface = match self.surface.as_ref() {
            Some(s) => s,
            None => return Ok(()),
        };

        // Создаем новый буфер, если его нет
        if self.back_buffer.is_none() {
            let (buffer, file) = self.create_buffer(qh)?;
            self.back_buffer = Some(buffer);
            self.back_buffer_file = Some(file);
        }

        let buffer = self.back_buffer.as_ref().unwrap();
        let file = self.back_buffer_file.as_ref().unwrap();

        // Создаем временный surface для рисования
        let stride = self.width * 4;
        let size = (stride * self.height) as usize;
        let mmap = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                size,
                libc::PROT_WRITE | libc::PROT_READ,
                libc::MAP_SHARED,
                file.as_raw_fd(),
                0,
            )
        };

        if mmap == libc::MAP_FAILED {
            return Ok(());
        }

        let data_surface = match ImageSurface::create_for_data(
            unsafe { std::slice::from_raw_parts_mut(mmap as *mut u8, size) },
            Format::ARgb32,
            self.width as i32,
            self.height as i32,
            stride as i32,
        ) {
            Ok(s) => s,
            Err(_) => {
                unsafe { libc::munmap(mmap, size) };
                return Ok(());
            }
        };

        let context = match Context::new(&data_surface) {
            Ok(c) => c,
            Err(_) => {
                unsafe { libc::munmap(mmap, size) };
                return Ok(());
            }
        };

        // Очищаем поверхность
        context.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        context.paint().ok();

        // Получаем текущее состояние выделения
        let selection = self.selection_state.blocking_lock().selection;
        
        // Рисуем рамку выделения
        if let Some((x, y, w, h)) = selection {
            // Рисуем рамку
            context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
            context.rectangle(x as f64, y as f64, w as f64, h as f64);
            context.set_line_width(2.0);
            context.stroke().ok();

            // Отображаем размеры
            let text = format!("{}x{}", w, h);
            context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
            context.set_font_size(24.0);
            if let Ok(extents) = context.text_extents(&text) {
                let text_x = x as f64 + (w as f64 / 2.0) - (extents.width() / 2.0);
                let text_y = y as f64 + (h as f64 / 2.0) + (extents.height() / 2.0);
                
                context.set_source_rgba(0.0, 0.0, 0.0, 0.8);
                context.rectangle(
                    text_x - 8.0,
                    text_y - extents.height() - 8.0,
                    extents.width() + 16.0,
                    extents.height() + 16.0
                );
                context.fill().ok();

                context.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                context.move_to(text_x, text_y);
                context.show_text(&text).ok();
            }
        }

        unsafe { libc::munmap(mmap, size) };

        // Прикрепляем буфер к поверхности
        surface.attach(Some(buffer), 0, 0);
        surface.damage(0, 0, self.width as i32, self.height as i32);
        surface.commit();

        // Меняем буферы местами
        std::mem::swap(&mut self.buffer, &mut self.back_buffer);
        std::mem::swap(&mut self.buffer_file, &mut self.back_buffer_file);

        Ok(())
    }

    async fn should_update(&self) -> bool {
        let mut last_update = self.last_update.lock().await;
        let now = Instant::now();
        if now.duration_since(*last_update) >= self.update_interval {
            *last_update = now;
            true
        } else {
            false
        }
    }

    fn draw_selection(&mut self, qh: &QueueHandle<Self>) {
        if let Some(_surface) = &self.surface {
            if let Some(_layer_surface) = &self.layer_surface {
                let selection = {
                    let selection_state = self.selection_state.blocking_lock();
                    selection_state.selection
                };
                
                if selection.is_some() {
                    if let Err(e) = self.draw(qh) {
                        log::error!("Failed to draw: {}", e);
                    }
                }
            }
        }
    }

    fn create_buffer(&self, qh: &QueueHandle<Self>) -> Result<(wl_buffer::WlBuffer, File), Box<dyn std::error::Error>> {
        let stride = self.width * 4;
        let size = (stride * self.height) as usize;
        let file = tempfile()?;
        file.set_len(size as u64)?;

        let shm = self.shm.as_ref().ok_or("No SHM available")?;
        let pool = unsafe {
            let fd = BorrowedFd::borrow_raw(file.as_raw_fd());
            shm.create_pool(fd, size as i32, qh, ())
        };

        let buffer = pool.create_buffer(
            0,
            self.width as i32,
            self.height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            qh,
            (),
        );

        Ok((buffer, file))
    }
}

// Реализация обработки событий Wayland
impl Dispatch<wl_pointer::WlPointer, ()> for RegionSelectState {
    fn event(
        state: &mut Self,
        _pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let mut selection_state = state.selection_state.blocking_lock();
        
        match event {
            wl_pointer::Event::Button { button, state: button_state, .. } => {
                match button_state {
                    wayland_client::WEnum::Value(wl_pointer::ButtonState::Pressed) => {
                        if button == 272 { // Left mouse button
                            if let Some((x, y)) = selection_state.current_position {
                                selection_state.selection_start = Some((x, y));
                                selection_state.selection = Some((x, y, 0, 0));
                                
                                drop(selection_state);
                                if let Err(e) = state.draw(qh) {
                                    log::error!("Failed to draw: {}", e);
                                }
                            }
                        }
                    }
                    wayland_client::WEnum::Value(wl_pointer::ButtonState::Released) => {
                        if button == 272 {
                            selection_state.selection_start = None;
                            if selection_state.selection.is_some() {
                                selection_state.exit = true;
                            }
                        }
                    }
                    _ => {}
                }
            }
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                let x = surface_x as i32;
                let y = surface_y as i32;
                selection_state.current_position = Some((x, y));

                if let Some((start_x, start_y)) = selection_state.selection_start {
                    let w = (x - start_x).abs();
                    let h = (y - start_y).abs();
                    let x = start_x.min(x);
                    let y = start_y.min(y);
                    selection_state.selection = Some((x, y, w, h));

                    drop(selection_state);
                    if let Err(e) = state.draw(qh) {
                        log::error!("Failed to draw: {}", e);
                    }
                }
            }
            _ => {}
        }
    }
}

// Реализация Dispatch для всех нужных объектов (registry, seat, pointer, keyboard, layer_surface, shm_pool, buffer)
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for RegionSelectState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "wl_compositor" => {
                    state.compositor = Some(registry.bind::<wl_compositor::WlCompositor, _, _>(name, version, qh, ()));
                }
                "wl_shm" => {
                    state.shm = Some(registry.bind::<wl_shm::WlShm, _, _>(name, version, qh, ()));
                }
                "wl_seat" => {
                    let seat = registry.bind::<wl_seat::WlSeat, _, _>(name, version, qh, ());
                    state.pointer = Some(seat.get_pointer(qh, ()));
                    state.keyboard = Some(seat.get_keyboard(qh, ()));
                    state.seat = Some(seat);
                }
                "zwlr_layer_shell_v1" => {
                    state.layer_shell = Some(registry.bind::<zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(name, version, qh, ()));
                }
                _ => {}
            }
        }
    }
}
impl Dispatch<wl_compositor::WlCompositor, ()> for RegionSelectState {
    fn event(_: &mut Self, _: &wl_compositor::WlCompositor, _: wl_compositor::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wl_shm::WlShm, ()> for RegionSelectState {
    fn event(_: &mut Self, _: &wl_shm::WlShm, _: wl_shm::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wl_seat::WlSeat, ()> for RegionSelectState {
    fn event(_: &mut Self, _: &wl_seat::WlSeat, _: wl_seat::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wl_keyboard::WlKeyboard, ()> for RegionSelectState {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Key { key, state: key_state, .. } = event {
            // 1 - released, 0 - pressed
            if key == 1 && key_state == wayland_client::WEnum::Value(wl_keyboard::KeyState::Pressed) {
                state.cancelled = true;
            }
        }
    }
}
impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1, ()> for RegionSelectState {
    fn event(_: &mut Self, _: &zwlr_layer_shell_v1::ZwlrLayerShellV1, _: zwlr_layer_shell_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for RegionSelectState {
    fn event(
        state: &mut Self,
        surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure { serial, width, height } = event {
            log::debug!("Layer surface configured: width={}, height={}", width, height);
            surface.ack_configure(serial);
            state.width = width.max(1);
            state.height = height.max(1);
            if let Err(e) = state.draw(qh) {
                log::error!("Failed to draw: {}", e);
            }
        }
    }
}
impl Dispatch<wl_surface::WlSurface, ()> for RegionSelectState {
    fn event(_: &mut Self, _: &wl_surface::WlSurface, _: wl_surface::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wl_shm_pool::WlShmPool, ()> for RegionSelectState {
    fn event(_: &mut Self, _: &wl_shm_pool::WlShmPool, _: wl_shm_pool::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wl_buffer::WlBuffer, ()> for RegionSelectState {
    fn event(_: &mut Self, _: &wl_buffer::WlBuffer, _: wl_buffer::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

/// Вспомогательная структура для получения размеров экрана
struct ScreenInfo {
    compositor: Option<wl_compositor::WlCompositor>,
    surface: Option<wl_surface::WlSurface>,
    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    layer_surface: Option<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    width: u32,
    height: u32,
}

impl ScreenInfo {
    fn new() -> Self {
        Self {
            compositor: None,
            surface: None,
            layer_shell: None,
            layer_surface: None,
            width: 0,
            height: 0,
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for ScreenInfo {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "wl_compositor" => {
                    state.compositor = Some(registry.bind::<wl_compositor::WlCompositor, _, _>(name, version, qh, ()));
                }
                "zwlr_layer_shell_v1" => {
                    state.layer_shell = Some(registry.bind::<zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(name, version, qh, ()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for ScreenInfo {
    fn event(
        state: &mut Self,
        surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure { serial, width, height } = event {
            surface.ack_configure(serial);
            state.width = width.max(1);
            state.height = height.max(1);
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for ScreenInfo {
    fn event(_: &mut Self, _: &wl_compositor::WlCompositor, _: wl_compositor::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1, ()> for ScreenInfo {
    fn event(_: &mut Self, _: &zwlr_layer_shell_v1::ZwlrLayerShellV1, _: zwlr_layer_shell_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wl_surface::WlSurface, ()> for ScreenInfo {
    fn event(_: &mut Self, _: &wl_surface::WlSurface, _: wl_surface::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

/// Интерактивно выбирает область экрана с помощью мыши
/// 
/// Создает полупрозрачный оверлей на весь экран и позволяет пользователю
/// выбрать область с помощью мыши. Выбор осуществляется зажатием левой кнопки мыши
/// и перетаскиванием для определения размера области.
/// 
/// # Возвращаемое значение
/// 
/// Возвращает `Some(Region)` с координатами и размерами выбранной области,
/// или `None`, если выбор был отменен (нажата клавиша Escape).
pub fn select_region_interactive_sctk() -> Option<Region> {
    let conn = match Connection::connect_to_env() {
        Ok(conn) => conn,
        Err(e) => {
            log::error!("Failed to connect to Wayland: {}", e);
            return None;
        }
    };

    let (globals, mut event_queue) = match registry_queue_init::<RegionSelectState>(&conn) {
        Ok(result) => result,
        Err(e) => {
            log::error!("Failed to initialize Wayland registry: {}", e);
            return None;
        }
    };

    let qh = event_queue.handle();
    let mut state = RegionSelectState::new();

    // Initialize Wayland objects
    state.compositor = Some(globals.bind::<wl_compositor::WlCompositor, _, _>(&qh, 1..=4, ()).expect("Failed to bind compositor"));
    state.shm = Some(globals.bind::<wl_shm::WlShm, _, _>(&qh, 1..=1, ()).expect("Failed to bind shm"));
    let seat = globals.bind::<wl_seat::WlSeat, _, _>(&qh, 1..=7, ()).expect("Failed to bind seat");
    state.pointer = Some(seat.get_pointer(&qh, ()));
    state.keyboard = Some(seat.get_keyboard(&qh, ()));
    state.seat = Some(seat);
    state.layer_shell = Some(globals.bind::<zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(&qh, 1..=4, ()).expect("Failed to bind layer shell"));

    // Create surface and layer_surface
    let compositor = state.compositor.as_ref().unwrap();
    let layer_shell = state.layer_shell.as_ref().unwrap();
    let surface = compositor.create_surface(&qh, ());
    let layer_surface = layer_shell.get_layer_surface(
        &surface,
        None,
        zwlr_layer_shell_v1::Layer::Overlay,
        "region-select".to_string(),
        &qh,
        (),
    );

    // Configure layer surface
    layer_surface.set_size(0, 0);
    layer_surface.set_exclusive_zone(-1);
    layer_surface.set_anchor(zwlr_layer_surface_v1::Anchor::all());
    layer_surface.set_margin(0, 0, 0, 0);
    surface.commit();

    state.surface = Some(surface);
    state.layer_surface = Some(layer_surface);

    // Wait for initial configuration
    let mut configured = false;
    while !configured {
        match event_queue.blocking_dispatch(&mut state) {
            Ok(_) => {
                if state.layer_surface.is_some() {
                    configured = true;
                }
            }
            Err(e) => {
                log::error!("Error waiting for surface configuration: {}", e);
                return None;
            }
        }
    }

    // Event loop
    while !state.selection_state.blocking_lock().exit {
        if let Err(e) = event_queue.blocking_dispatch(&mut state) {
            log::error!("Error in event loop: {}", e);
            break;
        }
    }

    // Get final selection
    let selection = {
        let guard = state.selection_state.blocking_lock();
        guard.selection
    };

    // Сначала очищаем поверхность
    if let Some(surface) = &state.surface {
        // Создаем пустой буфер
        let stride = state.width * 4;
        let size = (stride * state.height) as usize;
        let file = match tempfile() {
            Ok(f) => f,
            Err(e) => {
                log::error!("Failed to create tempfile: {}", e);
                return None;
            }
        };

        if let Err(e) = file.set_len(size as u64) {
            log::error!("Failed to set file length: {}", e);
            return None;
        }

        let shm = state.shm.as_ref().ok_or("No SHM available").unwrap();
        let pool = unsafe {
            let fd = BorrowedFd::borrow_raw(file.as_raw_fd());
            shm.create_pool(fd, size as i32, &qh, ())
        };

        let buffer = pool.create_buffer(
            0,
            state.width as i32,
            state.height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            &qh,
            (),
        );

        // Прикрепляем пустой буфер
        surface.attach(Some(&buffer), 0, 0);
        surface.damage(0, 0, state.width as i32, state.height as i32);
        surface.commit();

        // Ждем обработки последнего кадра
        for _ in 0..5 {
            event_queue.blocking_dispatch(&mut state).ok();
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    // Уничтожаем все Wayland объекты
    if let Some(surface) = state.surface.take() {
        surface.destroy();
    }
    if let Some(layer_surface) = state.layer_surface.take() {
        layer_surface.destroy();
    }
    if let Some(pointer) = state.pointer.take() {
        pointer.release();
    }
    if let Some(keyboard) = state.keyboard.take() {
        keyboard.release();
    }
    if let Some(seat) = state.seat.take() {
        seat.release();
    }
    if let Some(layer_shell) = state.layer_shell.take() {
        layer_shell.destroy();
    }

    // Даем время на уничтожение объектов и обработку последнего кадра
    std::thread::sleep(std::time::Duration::from_millis(200));
    
    selection.map(|(x, y, w, h)| Region { x, y, width: w, height: h })
}

impl Drop for RegionSelectState {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            buffer.destroy();
        }
        if let Some(buffer) = self.back_buffer.take() {
            buffer.destroy();
        }
    }
}