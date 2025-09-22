use crate::{CaptureResult, Output, Box, Result, Error};
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
        
        // Создаем временный экземпляр для инициализации
        let mut instance = Self {
            _connection: connection,
            globals,
        };

        event_queue.roundtrip(&mut instance).map_err(|e| {
            Error::WaylandConnection(format!("Failed to initialize Wayland globals: {}", e))
        })?;

        // Проверяем, что все необходимые глобальные объекты доступны
        if instance.globals.screencopy_manager.is_none() {
            return Err(Error::UnsupportedProtocol("zwlr_screencopy_manager_v1 not available".to_string()));
        }
        
        if instance.globals.shm.is_none() {
            return Err(Error::UnsupportedProtocol("wl_shm not available".to_string()));
        }

        Ok(instance)
    }

    pub fn get_outputs(&mut self) -> Result<Vec<Output>> {
        // Обновляем информацию о выходах
        let mut event_queue = self._connection.new_event_queue();
        
        // Диспетчеризуем события для получения актуальной информации о выходах
        event_queue.roundtrip(self).map_err(|e| {
            Error::WaylandConnection(format!("Failed to get output information: {}", e))
        })?;
        
        let mut outputs = Vec::new();
        
        for info in self.globals.output_info.values() {
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
        // Получаем информацию о всех выходах
        let outputs = self.get_outputs()?;
        
        // Находим общий bounding box для всех выходов
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
        // Находим выход по имени
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
        eprintln!("Начинаем захват области: {:?}", region);
        
        // Проверяем, что у нас есть хотя бы один выход
        let output = self.globals.outputs.first().ok_or(Error::NoOutputs)?;
        eprintln!("Найден выход для захвата");
        
        let screencopy_manager = self.globals.screencopy_manager.as_ref().ok_or(
            Error::UnsupportedProtocol("zwlr_screencopy_manager_v1 not available".to_string())
        )?;
        eprintln!("Найден screencopy manager");

        let mut event_queue = self._connection.new_event_queue();
        let qh = event_queue.handle();
        eprintln!("Создана очередь событий");

        // Состояние для обработки фрейма
        let frame_state = Arc::new(Mutex::new(FrameState {
            buffer: None,
            width: 0,
            height: 0,
            format: None,
            ready: false,
        }));
        eprintln!("Создано состояние фрейма");

        // Создаем фрейм для захвата области
        let frame = screencopy_manager.capture_output_region(
            0, // overlay_cursor
            output,
            region.x,
            region.y,
            region.width,
            region.height,
            &qh,
            frame_state.clone()
        );
        eprintln!("Создан фрейм для захвата области");

        // Ждем события Buffer от фрейма
        eprintln!("Начинаем ожидание события Buffer...");
        loop {
            {
                let state = frame_state.lock().unwrap();
                if state.buffer.is_some() || state.ready {
                    eprintln!("Получено событие Buffer или завершение, buffer.is_some()={}, ready={}", state.buffer.is_some(), state.ready);
                    break;
                }
            }
            eprintln!("Ожидание события Buffer...");
            event_queue.blocking_dispatch(self)
                .map_err(|e| Error::FrameCapture(format!("Failed to dispatch frame events: {}", e)))?;
        }

        // Если буфер не был получен, но фрейм завершен, это ошибка
        {
            let state = frame_state.lock().unwrap();
            if state.buffer.is_none() {
                return Err(Error::CaptureFailed);
            }
        }
        
        eprintln!("Создаем буфер через wl_shm");
        // Создаем буфер через wl_shm
        let (width, height, stride, size) = {
            let state = frame_state.lock().unwrap();
            let width = state.width;
            let height = state.height;
            let stride = width * 4; // Для RGBA формата
            let size = (stride * height) as usize;
            (width, height, stride, size)
        };
        
        // Создаем временный файл для буфера
        let mut tmp_file = tempfile::NamedTempFile::new()
            .map_err(|_| Error::BufferCreation)?;
        tmp_file.as_file_mut().set_len(size as u64)
            .map_err(|_| Error::BufferCreation)?;
        
        // Отображаем файл в память
        let mmap = unsafe { memmap2::MmapMut::map_mut(&tmp_file)
            .map_err(|_| Error::BufferCreation)?
        };
        
        // Создаем wl_shm_pool и буфер (получаем shm внутри этого блока)
        {
            let shm = self.globals.shm.as_ref().ok_or(
                Error::UnsupportedProtocol("wl_shm not available".to_string())
            )?;
            eprintln!("Найден shm");
            
            // Получаем формат из состояния фрейма
            let format = {
                let state = frame_state.lock().unwrap();
                state.format.unwrap_or(ShmFormat::Xrgb8888) // По умолчанию Xrgb8888
            };
            
            let pool = shm.create_pool(
                unsafe { BorrowedFd::borrow_raw(tmp_file.as_file().as_raw_fd()) },
                size as i32,
                &qh,
                ()
            );
            let buffer = pool.create_buffer(0, width as i32, height as i32, stride as i32, format, &qh, ());
            
            // Передаем буфер фрейму
            frame.copy(&buffer);
        }
        
        // Ждем завершения захвата
        eprintln!("Ждем завершения захвата...");
        loop {
            {
                let state = frame_state.lock().unwrap();
                if state.ready {
                    eprintln!("Захват завершен, ready={}", state.ready);
                    break;
                }
            }
            event_queue.blocking_dispatch(self)
                .map_err(|e| Error::FrameCapture(format!("Failed to dispatch frame events: {}", e)))?;
        }
        
        // Читаем данные из отображенного файла
        let mut buffer_data = mmap.to_vec();
        
        // Преобразуем данные из формата XRGB8888 в RGBA8888 если нужно
        if let Some(format) = {
            let state = frame_state.lock().unwrap();
            state.format
        } {
            match format {
                ShmFormat::Xrgb8888 => {
                    eprintln!("Преобразуем данные из XRGB8888 в RGBA8888");
                    for chunk in buffer_data.chunks_exact_mut(4) {
                        let b = chunk[0]; // Синий
                        let g = chunk[1]; // Зеленый
                        let r = chunk[2]; // Красный
                        chunk[0] = r; // R
                        chunk[1] = g; // G
                        chunk[2] = b; // B
                        chunk[3] = 255; // A
                    }
                }
                ShmFormat::Argb8888 => {
                    eprintln!("Данные уже в формате ARGB8888");
                    // Ничего не делаем, формат уже правильный
                }
                _ => {
                    eprintln!("Неизвестный формат пикселей: {:?}", format);
                }
            }
        }
        
        eprintln!("Завершено ожидание событий фрейма");

        // Конвертируем буфер в нужный формат (если необходимо)
        // Для простоты предполагаем, что формат уже RGBA
        Ok(CaptureResult {
            data: buffer_data,
            width,
            height,
        })
    }
}

// Реализация Dispatch для реестра
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
                    state.globals.output_info.insert(name, OutputInfo {
                        name: format!("output-{}", name),
                        width: 1920,
                        height: 1080,
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

// Реализация Dispatch для выходов
impl Dispatch<WlOutput, ()> for WaylandCapture {
    fn event(
        state: &mut Self,
        _output: &WlOutput,
        event: <WlOutput as Proxy>::Event,
        _: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_client::protocol::wl_output::{Event};
        
        // Находим OutputInfo для этого выхода по ссылке на объект
        // В данном случае мы используем упрощенный подход
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
                // Обновляем координаты для всех выходов (упрощенный подход)
                for (_, info) in state.globals.output_info.iter_mut() {
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
                // Обновляем размеры для всех выходов (упрощенный подход)
                for (_, info) in state.globals.output_info.iter_mut() {
                    info.width = width;
                    info.height = height;
                }
            }
            Event::Scale { factor } => {
                // Обновляем масштаб для всех выходов (упрощенный подход)
                for (_, info) in state.globals.output_info.iter_mut() {
                    info.scale = factor;
                }
            }
            Event::Name { name } => {
                // Обновляем имя для всех выходов (упрощенный подход)
                for (_, info) in state.globals.output_info.iter_mut() {
                    info.name = name.clone();
                }
            }
            Event::Description { description: _ } => {}
            _ => {}
        }
    }
}

// Реализации Dispatch для других протоколов
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
    ) {
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrScreencopyManagerV1,
        _event: <ZwlrScreencopyManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

// Реализация Dispatch для фрейма screencopy
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
                eprintln!("Получено событие Buffer: format={:?}, width={}, height={}, stride={}", format, width, height, stride);
                let mut state = frame_state.lock().unwrap();
                state.width = width;
                state.height = height;
                // Обрабатываем WEnum
                if let wayland_client::WEnum::Value(val) = format {
                    state.format = Some(val);
                }
                state.buffer = Some(vec![0u8; (stride * height) as usize]);
            }
            Event::Flags { flags } => {
                eprintln!("Получено событие Flags: flags={:?}", flags);
            }
            Event::Ready { tv_sec_hi, tv_sec_lo, tv_nsec } => {
                eprintln!("Получено событие Ready: tv_sec_hi={}, tv_sec_lo={}, tv_nsec={}", tv_sec_hi, tv_sec_lo, tv_nsec);
                let mut state = frame_state.lock().unwrap();
                state.ready = true;
                frame.destroy();
            }
            Event::Failed => {
                eprintln!("Получено событие Failed");
                // Обработка ошибки
                let mut state = frame_state.lock().unwrap();
                state.ready = true;
            }
            Event::LinuxDmabuf { format, width, height } => {
                eprintln!("Получено событие LinuxDmabuf: format={}, width={}, height={}", format, width, height);
            }
            Event::BufferDone => {
                eprintln!("Получено событие BufferDone");
            }
            _ => {
                eprintln!("Получено неизвестное событие: {:?}", event);
            }
        }
    }
}

// Реализация Dispatch для буфера
impl Dispatch<WlBuffer, ()> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _proxy: &WlBuffer,
        _event: <WlBuffer as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

// Реализация Dispatch для shm_pool
impl Dispatch<WlShmPool, ()> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _proxy: &WlShmPool,
        _event: <WlShmPool as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

// Состояние для обработки захвата кадра
#[derive(Debug, Clone)]
struct FrameState {
    buffer: Option<Vec<u8>>,
    width: u32,
    height: u32,
    format: Option<ShmFormat>,
    ready: bool,
}