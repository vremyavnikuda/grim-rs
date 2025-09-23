use crate::{CaptureResult, Output, Box, Result, Error, CaptureParameters, MultiOutputCaptureResult};
use wayland_client::{
    Connection, Dispatch, QueueHandle, Proxy,
    protocol::{
        wl_compositor::WlCompositor, 
        wl_output::WlOutput,
        wl_shm::{WlShm, Format as ShmFormat},
        wl_shm_pool::WlShmPool,
        wl_buffer::WlBuffer,
        wl_registry::WlRegistry,
    },
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
    zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::os::fd::{AsRawFd, BorrowedFd};

const MAX_ATTEMPTS: usize = 100;

struct OutputInfo {
    name: String,
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    scale: i32,
}

struct WaylandGlobals {
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
    screencopy_manager: Option<ZwlrScreencopyManagerV1>,
    outputs: Vec<WlOutput>,
    output_info: HashMap<u32, OutputInfo>,
}

pub struct WaylandCapture {
    _connection: Connection,
    globals: WaylandGlobals,
}

impl WaylandCapture {
    pub fn new() -> Result<Self> {
        let connection = Connection::connect_to_env()
            .map_err(|e| Error::WaylandConnection(format!("Failed to connect to Wayland: {}", e)))?;
        let globals = WaylandGlobals {
            compositor: None,
            shm: None,
            screencopy_manager: None,
            outputs: Vec::new(),
            output_info: HashMap::new(),
        };
        let mut event_queue = connection.new_event_queue();
        let qh = event_queue.handle();
        let _registry = connection.display().get_registry(&qh, ());
        let mut instance = Self {
            _connection: connection,
            globals,
        };
        event_queue.roundtrip(&mut instance).map_err(|e| {
            Error::WaylandConnection(format!("Failed to initialize Wayland globals: {}", e))
        })?;
        if instance.globals.screencopy_manager.is_none() {
            return Err(Error::UnsupportedProtocol("zwlr_screencopy_manager_v1 not available".to_string()));
        }
        if instance.globals.shm.is_none() {
            return Err(Error::UnsupportedProtocol("wl_shm not available".to_string()));
        }
        Ok(instance)
    }

    pub fn get_outputs(&mut self) -> Result<Vec<Output>> {
        let mut event_queue = self._connection.new_event_queue();
        event_queue.roundtrip(self).map_err(|e| {
            Error::WaylandConnection(format!("Failed to get output information: {}", e))
        })?;
        let mut outputs = Vec::new();
        for (_id, info) in &self.globals.output_info {
            outputs.push(Output {
                name: info.name.clone(),
                geometry: Box::new(info.x, info.y, info.width, info.height),
                scale: info.scale,
            });
        }
        if outputs.is_empty() {
            return Err(Error::NoOutputs);
        }
        Ok(outputs)
    }

    pub fn capture_all(&mut self) -> Result<CaptureResult> {
        let outputs = self.get_outputs()?;
        if outputs.is_empty() {
            return Err(Error::NoOutputs);
        }
        let mut min_x = outputs[0].geometry.x;
        let mut min_y = outputs[0].geometry.y;
        let mut max_x = outputs[0].geometry.x + outputs[0].geometry.width;
        let mut max_y = outputs[0].geometry.y + outputs[0].geometry.height;
        for output in &outputs {
            min_x = min_x.min(output.geometry.x);
            min_y = min_y.min(output.geometry.y);
            max_x = max_x.max(output.geometry.x + output.geometry.width);
            max_y = max_y.max(output.geometry.y + output.geometry.height);
        }
        let region = Box::new(min_x, min_y, max_x - min_x, max_y - min_y);
        self.capture_region(region)
    }

    pub fn capture_output(&mut self, output_name: &str) -> Result<CaptureResult> {
        let output_info = self.globals.output_info.iter()
            .find(|(_, info)| info.name == output_name)
            .map(|(_, info)| info)
            .ok_or_else(|| Error::OutputNotFound(output_name.to_string()))?;
        
        let region = Box::new(
            output_info.x,
            output_info.y,
            output_info.width,
            output_info.height
        );
        self.capture_region(region)
    }

    pub fn capture_region(&mut self, region: Box) -> Result<CaptureResult> {
        let output = self.globals.outputs.first()
            .ok_or_else(|| Error::NoOutputs)?;
        
        let screencopy_manager = self.globals.screencopy_manager.as_ref().ok_or(
            Error::UnsupportedProtocol("zwlr_screencopy_manager_v1 not available".to_string())
        )?;
        let mut event_queue = self._connection.new_event_queue();
        let qh = event_queue.handle();
        let frame_state = Arc::new(Mutex::new(FrameState {
            buffer: None,
            width: 0,
            height: 0,
            format: None,
            ready: false,
        }));
        let frame = screencopy_manager.capture_output_region(
            0,
            output,
            region.x,
            region.y,
            region.width,
            region.height,
            &qh,
            frame_state.clone()
        );
        let mut attempts = 0;
        loop {
            {
                let state = frame_state.lock().unwrap();
                if state.buffer.is_some() || state.ready {
                    if state.ready && state.buffer.is_none() {
                        return Err(Error::FrameCapture("Frame is ready but buffer was not received".to_string()));
                    }
                    break;
                }
            }
            if attempts >= MAX_ATTEMPTS {
                return Err(Error::FrameCapture("Timeout waiting for frame buffer".to_string()));
            }
            event_queue.blocking_dispatch(self)
                .map_err(|e| Error::FrameCapture(format!("Failed to dispatch frame events: {}", e)))?;
            
            attempts += 1;
        }
        {
            let state = frame_state.lock().unwrap();
            if state.buffer.is_none() {
                return Err(Error::CaptureFailed);
            }
        }
        let (width, height, stride, size) = {
            let state = frame_state.lock().unwrap();
            let width = state.width;
            let height = state.height;
            let stride = width * 4;
            let size = (stride * height) as usize;
            (width, height, stride, size)
        };
        let mut tmp_file = tempfile::NamedTempFile::new()
            .map_err(|_e| Error::BufferCreation)?;
        tmp_file.as_file_mut().set_len(size as u64)
            .map_err(|_e| Error::BufferCreation)?;
        let mmap = unsafe { memmap2::MmapMut::map_mut(&tmp_file)
            .map_err(|_e| Error::BufferCreation)?
        };
        {
            let shm = self.globals.shm.as_ref().ok_or(
                Error::UnsupportedProtocol("wl_shm not available".to_string())
            )?;
            let format = {
                let state = frame_state.lock().unwrap();
                state.format.unwrap_or(ShmFormat::Xrgb8888)
            };
            let pool = shm.create_pool(
                unsafe { BorrowedFd::borrow_raw(tmp_file.as_file().as_raw_fd()) },
                size as i32,
                &qh,
                ()
            );
            let buffer = pool.create_buffer(0, width as i32, height as i32, stride as i32, format, &qh, ());
            frame.copy(&buffer);
        }
        let mut attempts = 0;
        loop {
            {
                let state = frame_state.lock().unwrap();
                if state.ready {
                    if state.buffer.is_none() {
                        return Err(Error::FrameCapture("Frame is ready but buffer was not received".to_string()));
                    }
                    break;
                }
            }
            if attempts >= MAX_ATTEMPTS {
                return Err(Error::FrameCapture("Timeout waiting for frame capture completion".to_string()));
            }
            event_queue.blocking_dispatch(self)
                .map_err(|e| Error::FrameCapture(format!("Failed to dispatch frame events: {}", e)))?;
            attempts += 1;
        }
        let mut buffer_data = mmap.to_vec();
        if let Some(format) = {
            let state = frame_state.lock().unwrap();
            state.format
        } {
            match format {
                ShmFormat::Xrgb8888 => {
                    for chunk in buffer_data.chunks_exact_mut(4) {
                        let b = chunk[0];
                        let g = chunk[1];
                        let r = chunk[2];
                        chunk[0] = r;
                        chunk[1] = g;
                        chunk[2] = b;
                        chunk[3] = 255;
                    }
                }
                ShmFormat::Argb8888 => {}
                _ => {}
            }
        }
        Ok(CaptureResult {
            data: buffer_data,
            width,
            height,
        })
    }

    pub fn capture_outputs(&mut self, parameters: Vec<CaptureParameters>) -> Result<MultiOutputCaptureResult> {
        let output = self.globals.outputs.first()
            .ok_or_else(|| Error::NoOutputs)?;
        let screencopy_manager = self.globals.screencopy_manager.as_ref().ok_or(
            Error::UnsupportedProtocol("zwlr_screencopy_manager_v1 not available".to_string())
        )?;
        let mut event_queue = self._connection.new_event_queue();
        let qh = event_queue.handle();
        let mut frame_states: HashMap<String, Arc<Mutex<FrameState>>> = HashMap::new();
        let mut frames: HashMap<String, ZwlrScreencopyFrameV1> = HashMap::new();
        for param in &parameters {
            let output_info = self.globals.output_info.iter()
                .find(|(_, info)| info.name == param.output_name)
                .map(|(_, info)| info)
                .ok_or_else(|| Error::OutputNotFound(param.output_name.clone()))?;
            let region = if let Some(region) = param.region {
                let output_right = output_info.x + output_info.width;
                let output_bottom = output_info.y + output_info.height;
                if region.x < output_info.x || region.y < output_info.y || 
                   region.x + region.width > output_right || region.y + region.height > output_bottom {
                    return Err(Error::InvalidRegion("Capture region extends outside output boundaries".to_string()));
                }
                region
            } else {
                Box::new(output_info.x, output_info.y, output_info.width, output_info.height)
            };
            let frame_state = Arc::new(Mutex::new(FrameState {
                buffer: None,
                width: 0,
                height: 0,
                format: None,
                ready: false,
            }));
            let frame = screencopy_manager.capture_output_region(
                if param.overlay_cursor { 1 } else { 0 },
                output,
                region.x,
                region.y,
                region.width,
                region.height,
                &qh,
                frame_state.clone()
            );
            frame_states.insert(param.output_name.clone(), frame_state);
            frames.insert(param.output_name.clone(), frame);
        }
        let mut attempts = 0;
        let mut completed_frames = 0;
        let total_frames = parameters.len();
        while completed_frames < total_frames && attempts < MAX_ATTEMPTS {
            completed_frames = frame_states.iter().filter(|(_, state)| {
                let s = state.lock().unwrap();
                s.buffer.is_some() || s.ready
            }).count();
            if completed_frames >= total_frames {
                break;
            }
            event_queue.blocking_dispatch(self)
                .map_err(|e| Error::FrameCapture(format!("Failed to dispatch frame events: {}", e)))?;
            attempts += 1;
        }
        if attempts >= MAX_ATTEMPTS {
            return Err(Error::FrameCapture("Timeout waiting for frame buffers".to_string()));
        }
        for (_output_name, frame_state) in &frame_states {
            let state = frame_state.lock().unwrap();
            if state.buffer.is_none() {
                return Err(Error::CaptureFailed);
            }
        }
        let mut buffers: HashMap<String, (tempfile::NamedTempFile, memmap2::MmapMut)> = HashMap::new();
        for (output_name, frame_state) in &frame_states {
            let (width, height, stride, size) = {
                let state = frame_state.lock().unwrap();
                let width = state.width;
                let height = state.height;
                let stride = width * 4;
                let size = (stride * height) as usize;
                (width, height, stride, size)
            };
            let mut tmp_file = tempfile::NamedTempFile::new()
                .map_err(|_e| Error::BufferCreation)?;
            tmp_file.as_file_mut().set_len(size as u64)
                .map_err(|_e| Error::BufferCreation)?;
            let mmap = unsafe { memmap2::MmapMut::map_mut(&tmp_file)
                .map_err(|_e| Error::BufferCreation)?
            };
            {
                let shm = self.globals.shm.as_ref().ok_or(
                    Error::UnsupportedProtocol("wl_shm not available".to_string())
                )?;
                let format = {
                    let state = frame_state.lock().unwrap();
                    state.format.unwrap_or(ShmFormat::Xrgb8888)
                };
                let pool = shm.create_pool(
                    unsafe { BorrowedFd::borrow_raw(tmp_file.as_file().as_raw_fd()) },
                    size as i32,
                    &qh,
                    ()
                );
                let buffer = pool.create_buffer(0, width as i32, height as i32, stride as i32, format, &qh, ());
                if let Some(frame) = frames.get(output_name) {
                    frame.copy(&buffer);
                }
            }
            buffers.insert(output_name.clone(), (tmp_file, mmap));
        }
        let mut attempts = 0;
        let mut completed_frames = 0;
        while completed_frames < total_frames && attempts < MAX_ATTEMPTS {
            completed_frames = frame_states.iter().filter(|(_, state)| {
                let s = state.lock().unwrap();
                s.ready
            }).count();
            if completed_frames >= total_frames {
                break;
            }
            event_queue.blocking_dispatch(self)
                .map_err(|e| Error::FrameCapture(format!("Failed to dispatch frame events: {}", e)))?;
            attempts += 1;
        }
        if attempts >= MAX_ATTEMPTS {
            return Err(Error::FrameCapture("Timeout waiting for frame capture completion".to_string()));
        }
        for (_output_name, frame_state) in &frame_states {
            let state = frame_state.lock().unwrap();
            if state.ready && state.buffer.is_none() {
                return Err(Error::FrameCapture("Frame is ready but buffer was not received".to_string()));
            }
        }
        let mut results: HashMap<String, CaptureResult> = HashMap::new();
        for (output_name, (_tmp_file, mmap)) in buffers {
            let frame_state = &frame_states[&output_name];
            let (width, height) = {
                let state = frame_state.lock().unwrap();
                (state.width, state.height)
            };
            let mut buffer_data = mmap.to_vec();
            if let Some(format) = {
                let state = frame_state.lock().unwrap();
                state.format
            } {
                match format {
                    ShmFormat::Xrgb8888 => {
                        for chunk in buffer_data.chunks_exact_mut(4) {
                            let b = chunk[0];
                            let g = chunk[1];
                            let r = chunk[2];
                            chunk[0] = r;
                            chunk[1] = g;
                            chunk[2] = b;
                            chunk[3] = 255;
                        }
                    }
                    ShmFormat::Argb8888 => {}
                    _ => {}
                }
            }
            results.insert(output_name, CaptureResult {
                data: buffer_data,
                width,
                height,
            });
        }
        Ok(MultiOutputCaptureResult {
            outputs: results,
        })
    }
}

impl Dispatch<WlRegistry, ()> for WaylandCapture {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: <WlRegistry as Proxy>::Event,
        _: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use wayland_client::protocol::wl_registry::Event;
        if let Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "wl_compositor" => {
                    state.globals.compositor = Some(registry.bind::<WlCompositor, _, _>(name, version, qh, ()));
                }
                "wl_shm" => {
                    state.globals.shm = Some(registry.bind::<WlShm, _, _>(name, version, qh, ()));
                }
                "zwlr_screencopy_manager_v1" => {
                    state.globals.screencopy_manager = Some(registry.bind::<ZwlrScreencopyManagerV1, _, _>(name, version, qh, ()));
                }
                "wl_output" => {
                    let output = registry.bind::<WlOutput, _, _>(name, version, qh, ());
                    // Добавляем информацию о выходе с временными значениями
                    // Реальные значения будут получены позже через события wl_output
                    state.globals.output_info.insert(name, OutputInfo {
                        name: format!("output-{}", name),
                        width: 0,
                        height: 0,
                        x: 0,
                        y: 0,
                        scale: 1,
                    });
                    state.globals.outputs.push(output);
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<WlOutput, ()> for WaylandCapture {
    fn event(
        state: &mut Self,
        output: &WlOutput,
        event: <WlOutput as Proxy>::Event,
        _: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_client::protocol::wl_output::{Event};
        // Находим соответствующий OutputInfo для этого выхода
        // Мы используем ID объекта для сопоставления
        let output_id = output.id().protocol_id();
        match event {
            Event::Geometry { 
                x, y, 
                physical_width: _, 
                physical_height: _, 
                subpixel: _, 
                make: _, 
                model: _, 
                transform: _ 
            } => {
                if let Some(info) = state.globals.output_info.get_mut(&output_id) {
                    info.x = x;
                    info.y = y;
                }
            }
            Event::Mode { 
                flags: _, 
                width, 
                height, 
                refresh: _ 
            } => {
                if let Some(info) = state.globals.output_info.get_mut(&output_id) {
                    info.width = width;
                    info.height = height;
                }
            }
            Event::Scale { factor } => {
                if let Some(info) = state.globals.output_info.get_mut(&output_id) {
                    info.scale = factor;
                }
            }
            Event::Name { name } => {
                if let Some(info) = state.globals.output_info.get_mut(&output_id) {
                    info.name = name.clone();
                }
            }
            Event::Description { description: _ } => {}
            _ => {}
        }
    }
}

impl Dispatch<WlCompositor, ()> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _proxy: &WlCompositor,
        _event: <WlCompositor as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlShm, ()> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _proxy: &WlShm,
        _event: <WlShm as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrScreencopyManagerV1,
        _event: <ZwlrScreencopyManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<ZwlrScreencopyFrameV1, Arc<Mutex<FrameState>>> for WaylandCapture {
    fn event(
        _state: &mut Self,
        frame: &ZwlrScreencopyFrameV1,
        event: <ZwlrScreencopyFrameV1 as Proxy>::Event,
        frame_state: &Arc<Mutex<FrameState>>,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::Event;
        match event {
            Event::Buffer { format, width, height, stride } => {
                let mut state = frame_state.lock().unwrap();
                state.width = width;
                state.height = height;
                if let wayland_client::WEnum::Value(val) = format {
                    state.format = Some(val);
                }
                state.buffer = Some(vec![0u8; (stride * height) as usize]);
            }
            Event::Flags { flags } => {
                // Обработка флагов - можно использовать для дополнительной информации
                // Пока просто логируем для отладки
                log::debug!("Received flags: {:?}", flags);
            }
            Event::Ready { tv_sec_hi: _, tv_sec_lo: _, tv_nsec: _ } => {
                let mut state = frame_state.lock().unwrap();
                state.ready = true;
                frame.destroy();
            }
            Event::Failed => {
                let mut state = frame_state.lock().unwrap();
                state.ready = true;
            }
            Event::LinuxDmabuf { format, width, height } => {
                // Обработка LinuxDmabuf - альтернативный способ передачи данных
                // Пока не поддерживаем, но логируем для отладки
                log::debug!("Received LinuxDmabuf: format={}, width={}, height={}", format, width, height);
            }
            Event::BufferDone => {
                log::debug!("Buffer copy completed");
            }
            _ => {
                log::warn!("Received unknown event: {:?}", event);
            }
        }
    }
}

impl Dispatch<WlBuffer, ()> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _proxy: &WlBuffer,
        _event: <WlBuffer as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<WlShmPool, ()> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _proxy: &WlShmPool,
        _event: <WlShmPool as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {}
}

#[derive(Debug, Clone)]
struct FrameState {
    buffer: Option<Vec<u8>>,
    width: u32,
    height: u32,
    format: Option<ShmFormat>,
    ready: bool,
}