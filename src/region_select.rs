//! Интерактивное выделение области экрана мышью через Wayland layer-shell
use wayland_client::{
    globals::{self, registry_queue_init, GlobalListContents}, protocol::{wl_buffer, wl_compositor, wl_pointer, wl_region, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface}, Connection, Dispatch, QueueHandle, WEnum
};
use std::fs::File;
use std::os::unix::io::{AsRawFd, BorrowedFd};
use std::ptr;
use std::sync::{Arc, Mutex};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use crate::ScreenshotError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionSelection {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug)]
pub enum RegionSelectionError {
    Wayland(String),
    Cancelled,
}

#[derive(Default)]
struct SelectionState {
    start_x: Option<i32>,
    start_y: Option<i32>,
    current_x: Option<i32>,
    current_y: Option<i32>,
    is_selecting: bool,
    selection: Option<RegionSelection>,
}

struct SelectionHandler {
    state: Arc<Mutex<SelectionState>>,
    surface: Option<wl_surface::WlSurface>,
    buffer: Option<wl_buffer::WlBuffer>,
    buffer_file: Option<File>,
    width: u32,
    height: u32,
    shm: Option<wl_shm::WlShm>,
    compositor: Option<wl_compositor::WlCompositor>,
    pointer: Option<wl_pointer::WlPointer>,
    seat: Option<wl_seat::WlSeat>,
    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    layer_surface: Option<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
}

impl SelectionHandler {
    fn create_buffer(&mut self, qh: &QueueHandle<Self>) -> Result<(), ScreenshotError> {
        let stride = self.width * 4;
        let size = (stride * self.height) as usize;
        let file = tempfile::tempfile().map_err(ScreenshotError::Io)?;
        file.set_len(size as u64).map_err(ScreenshotError::Io)?;
        let data = unsafe {
            let fd = BorrowedFd::borrow_raw(file.as_raw_fd());
            let ptr = libc::mmap(
                ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd.as_raw_fd(),
                0,
            );
            if ptr == libc::MAP_FAILED {
                return Err(ScreenshotError::Internal("mmap failed".to_string()));
            }
            ptr as *mut u8
        };
        unsafe { std::ptr::write_bytes(data, 0, size) };
        let shm = self.shm.as_ref().ok_or(ScreenshotError::Internal("shm not initialized".to_string()))?;
        let pool = shm.create_pool(unsafe { BorrowedFd::borrow_raw(file.as_raw_fd()) }, size as i32, qh, ());
        let buffer = pool.create_buffer(
            0,
            self.width as i32,
            self.height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            qh,
            (),
        );
        self.buffer = Some(buffer);
        self.buffer_file = Some(file);
        Ok(())
    }

    fn redraw(&mut self, qh: &QueueHandle<Self>) {
        log::info!("Redraw called");
        if let (Some(buffer), Some(file)) = (&self.buffer, &self.buffer_file) {
            let selection_state = self.state.lock().unwrap();
            if let (Some(start_x), Some(start_y), Some(current_x), Some(current_y)) = (
                selection_state.start_x,
                selection_state.start_y,
                selection_state.current_x,
                selection_state.current_y,
            ) {
                let stride = self.width * 4;
                let size = (stride * self.height) as usize;
                let data = unsafe {
                    let ptr = libc::mmap(
                        ptr::null_mut(),
                        size,
                        libc::PROT_READ | libc::PROT_WRITE,
                        libc::MAP_SHARED,
                        file.as_raw_fd(),
                        0,
                    );
                    if ptr == libc::MAP_FAILED {
                        return;
                    }
                    std::slice::from_raw_parts_mut(ptr as *mut u8, size)
                };

                // Clear the buffer
                unsafe { std::ptr::write_bytes(data.as_mut_ptr(), 0, size) };

                // Draw overlay (тест: полностью непрозрачный)
                for y in 0..self.height as i32 {
                    for x in 0..self.width as i32 {
                        let offset = (y * stride as i32 + x * 4) as usize;
                        if offset + 3 < data.len() {
                            data[offset] = 0;     // B
                            data[offset + 1] = 0; // G
                            data[offset + 2] = 0; // R
                            data[offset + 3] = 255; // A (полностью непрозрачный)
                        }
                    }
                }

                // Draw selection rectangle
                let x1 = start_x.min(current_x);
                let y1 = start_y.min(current_y);
                let x2 = start_x.max(current_x);
                let y2 = start_y.max(current_y);

                // Clear the selected area (делаем прозрачным)
                for y in y1..=y2 {
                    for x in x1..=x2 {
                        if x >= 0 && x < self.width as i32 && y >= 0 && y < self.height as i32 {
                            let offset = (y * stride as i32 + x * 4) as usize;
                            if offset + 3 < data.len() {
                                data[offset] = 0;     // B
                                data[offset + 1] = 0; // G
                                data[offset + 2] = 0; // R
                                data[offset + 3] = 0; // A (прозрачный)
                            }
                        }
                    }
                }

                // Draw border
                let border_width = 2;
                for y in y1..=y2 {
                    for x in x1..=x2 {
                        if x >= 0 && x < self.width as i32 && y >= 0 && y < self.height as i32 {
                            if y == y1 || y == y2 || x == x1 || x == x2 {
                                for dy in -border_width..=border_width {
                                    for dx in -border_width..=border_width {
                                        let bx = x + dx;
                                        let by = y + dy;
                                        if bx >= 0 && bx < self.width as i32 && by >= 0 && by < self.height as i32 {
                                            let offset = (by * stride as i32 + bx * 4) as usize;
                                            if offset + 3 < data.len() {
                                                data[offset] = 255;     // B
                                                data[offset + 1] = 255; // G
                                                data[offset + 2] = 255; // R
                                                data[offset + 3] = 255; // A
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                unsafe { libc::munmap(data.as_mut_ptr() as *mut libc::c_void, size) };
                if let Some(surface) = &self.surface {
                    surface.attach(Some(buffer), 0, 0);
                    surface.commit();
                    log::info!("Surface commit");
                }
            }
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for SelectionHandler {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        log::info!("wl_registry event: {:?}", event);
        if let wl_registry::Event::Global { name, interface, version } = event {
            log::info!("Wayland global: {} (version {})", interface, version);
            if interface == "wl_compositor" {
                state.compositor = Some(registry.bind::<wl_compositor::WlCompositor, _, _>(name, version, qh, ()));
            }
            if interface == "wl_shm" {
                state.shm = Some(registry.bind::<wl_shm::WlShm, _, _>(name, version, qh, ()));
            }
            if interface == "wl_seat" {
                let seat = registry.bind::<wl_seat::WlSeat, _, _>(name, version, qh, ());
                state.seat = Some(seat);
            }
            if interface == "zwlr_layer_shell_v1" {
                let bind_version = version.min(4);
                log::info!("Binding zwlr_layer_shell_v1 with version {}", bind_version);
                state.layer_shell = Some(registry.bind::<zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(name, bind_version, qh, ()));
            }
        }
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for SelectionHandler {
    fn event(
        _: &mut Self,
        _: &wl_shm_pool::WlShmPool,
        _: wl_shm_pool::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_buffer::WlBuffer, ()> for SelectionHandler {
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        _: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_compositor::WlCompositor, ()> for SelectionHandler {
    fn event(
        _: &mut Self,
        _: &wl_compositor::WlCompositor,
        _: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_shm::WlShm, ()> for SelectionHandler {
    fn event(
        _: &mut Self,
        _: &wl_shm::WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_seat::WlSeat, ()> for SelectionHandler {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_pointer::WlPointer, ()> for SelectionHandler {
    fn event(
        state: &mut Self,
        _pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        log::info!("Pointer event: {:?}", event);
        match event {
            wl_pointer::Event::Button { button, state: button_state, .. } => {
                if button == 272 { // BTN_LEFT
                    let mut selection = state.state.lock().unwrap();
                    match button_state {
                        WEnum::Value(wl_pointer::ButtonState::Pressed) => {
                            if let (Some(x), Some(y)) = (selection.current_x, selection.current_y) {
                                selection.start_x = Some(x);
                                selection.start_y = Some(y);
                                selection.is_selecting = true;
                            }
                        }
                        WEnum::Value(wl_pointer::ButtonState::Released) => {
                            if selection.is_selecting {
                                if let (Some(start_x), Some(start_y), Some(current_x), Some(current_y)) = (
                                    selection.start_x,
                                    selection.start_y,
                                    selection.current_x,
                                    selection.current_y,
                                ) {
                                    let x1 = start_x.min(current_x);
                                    let y1 = start_y.min(current_y);
                                    let x2 = start_x.max(current_x);
                                    let y2 = start_y.max(current_y);
                                    
                                    if x2 - x1 > 0 && y2 - y1 > 0 {
                                        selection.selection = Some(RegionSelection {
                                            x: x1.max(0) as u32,
                                            y: y1.max(0) as u32,
                                            width: (x2 - x1).max(0) as u32,
                                            height: (y2 - y1).max(0) as u32,
                                        });
                                    }
                                }
                                selection.is_selecting = false;
                            }
                        }
                        _ => {}
                    }
                }
            }
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                let is_selecting = {
                    let mut selection = state.state.lock().unwrap();
                    selection.current_x = Some(surface_x as i32);
                    selection.current_y = Some(surface_y as i32);
                    selection.is_selecting
                };
                if is_selecting {
                    state.redraw(qh);
                }
            }
            wl_pointer::Event::Enter { surface_x, surface_y, .. } => {
                let mut selection = state.state.lock().unwrap();
                selection.current_x = Some(surface_x as i32);
                selection.current_y = Some(surface_y as i32);
            }
            wl_pointer::Event::Leave { .. } => {
                let mut selection = state.state.lock().unwrap();
                selection.current_x = None;
                selection.current_y = None;
            }
            _ => {}
        }
    }
}

impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1, ()> for SelectionHandler {
    fn event(
        _: &mut Self,
        _: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
        _: zwlr_layer_shell_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_surface::WlSurface, ()> for SelectionHandler {
    fn event(
        _: &mut Self,
        _: &wl_surface::WlSurface,
        _: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for SelectionHandler {
    fn event(
        _: &mut Self,
        _: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        _: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl wayland_client::Dispatch<wl_region::WlRegion, ()> for SelectionHandler {
    fn event(
        _state: &mut Self,
        _proxy: &wl_region::WlRegion,
        _event: wl_region::Event,
        _: &(),
        _: &wayland_client::Connection,
        _: &wayland_client::QueueHandle<Self>,
    ) {
        // wl_region has no events
    }
}

pub fn select_region_interactive() -> Result<RegionSelection, RegionSelectionError> {
    let conn = Connection::connect_to_env()
        .map_err(|e| RegionSelectionError::Wayland(e.to_string()))?;
    let (_globals, mut event_queue) = registry_queue_init::<SelectionHandler>(&conn)
        .map_err(|e| RegionSelectionError::Wayland(e.to_string()))?;
    let qh = event_queue.handle();

    // Get screen dimensions
    let (width, height) = crate::get_screen_dimensions()
        .map_err(|e| RegionSelectionError::Wayland(e.to_string()))?;

    let state = Arc::new(Mutex::new(SelectionState::default()));
    let mut handler = SelectionHandler {
        state: state.clone(),
        surface: None,
        buffer: None,
        buffer_file: None,
        width,
        height,
        shm: None,
        compositor: None,
        pointer: None,
        seat: None,
        layer_shell: None,
        layer_surface: None,
    };

    log::info!("Waiting for all required globals...");
    // Wait for all required globals
    while handler.layer_shell.is_none() || handler.compositor.is_none() || handler.shm.is_none() || handler.seat.is_none() {
        event_queue.blocking_dispatch(&mut handler)
            .map_err(|e| RegionSelectionError::Wayland(e.to_string()))?;
    }
    log::info!("All required globals acquired");

    // Create surface and layer surface
    if let (Some(compositor), Some(layer_shell)) = (&handler.compositor, &handler.layer_shell) {
        let surface = compositor.create_surface(&qh, ());
        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            None,
            Layer::Top,
            "grim-rs".to_string(),
            &qh,
            (),
        );
        layer_surface.set_anchor(wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Anchor::all());
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_size(handler.width, handler.height);
        // Установить input и opaque region на всю поверхность
        let region = compositor.create_region(&qh, ());
        region.add(0, 0, handler.width as i32, handler.height as i32);
        surface.set_input_region(Some(&region));
        surface.set_opaque_region(Some(&region));
        surface.commit();
        log::info!("Surface and layer_surface created and committed");
        handler.surface = Some(surface);
        handler.layer_surface = Some(layer_surface);
        // Теперь создаём pointer, если seat уже есть
        if let Some(seat) = &handler.seat {
            handler.pointer = Some(seat.get_pointer(&qh, ())); 
            log::info!("Pointer created after surface commit");
        }
    }

    // Create buffer and start event loop
    handler.create_buffer(&qh)
        .map_err(|e| RegionSelectionError::Wayland(e.to_string()))?;
    handler.redraw(&qh);

    // Main event loop
    loop {
        event_queue.blocking_dispatch(&mut handler)
            .map_err(|e| RegionSelectionError::Wayland(e.to_string()))?;
        
        let state = handler.state.lock().unwrap();
        if let Some(selection) = state.selection {
            return Ok(selection);
        }
    }
}