use super::*;
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::Options as ExtCopyOptions;

impl WaylandCapture {
    fn refresh_outputs(&mut self) -> Result<()> {
        self.globals.outputs.clear();
        self.globals.output_info.clear();
        self.globals.output_xdg_map.clear();
        let mut event_queue = self._connection.new_event_queue();
        let qh = event_queue.handle();
        let _registry = self._connection.display().get_registry(&qh, ());
        event_queue.roundtrip(self).map_err(|e| {
            Error::WaylandConnection(format!("Failed to refresh Wayland globals: {}", e))
        })?;
        if self.globals.output_info.is_empty() {
            return Err(Error::NoOutputs);
        }
        for _ in 0..2 {
            event_queue.roundtrip(self).map_err(|e| {
                Error::WaylandConnection(format!("Failed to process output events: {}", e))
            })?;
        }
        if self.globals.xdg_output_manager.is_none() {
            for info in self.globals.output_info.values_mut() {
                if !info.logical_scale_known {
                    guess_output_logical_geometry(info);
                }
            }
        }
        Ok(())
    }

    pub(super) fn collect_outputs_snapshot(&self) -> Vec<(WlOutput, OutputInfo)> {
        self.globals
            .outputs
            .iter()
            .filter_map(|output| {
                let id = output.id().protocol_id();
                self.globals
                    .output_info
                    .get(&id)
                    .cloned()
                    .map(|info| (output.clone(), info))
            })
            .collect()
    }

    pub(super) fn capture_region_for_output(
        &mut self,
        output: &WlOutput,
        region: Region,
        overlay_cursor: bool,
    ) -> Result<CaptureResult> {
        if region.width() <= 0 || region.height() <= 0 {
            return Err(Error::InvalidRegion(
                "Capture region must have positive width and height".to_string(),
            ));
        }
        if region.x() < 0 || region.y() < 0 {
            return Err(Error::InvalidRegion(
                "Capture region origin must be non-negative".to_string(),
            ));
        }

        let backend = self.backend_cloned()?;
        match backend {
            CaptureBackend::WlrScreencopy { manager } => {
                self.capture_region_wlr(&manager, output, region, overlay_cursor)
            }
            CaptureBackend::ExtImageCopyCapture {
                source_manager,
                copy_manager,
            } => self.capture_region_ext(
                &source_manager,
                &copy_manager,
                output,
                region,
                overlay_cursor,
            ),
        }
    }

    fn capture_region_wlr(
        &mut self,
        manager: &ZwlrScreencopyManagerV1,
        output: &WlOutput,
        region: Region,
        overlay_cursor: bool,
    ) -> Result<CaptureResult> {
        let screencopy_manager = manager;
        let mut event_queue = self._connection.new_event_queue();
        let qh = event_queue.handle();
        let frame_state = Arc::new(Mutex::new(FrameState {
            buffer: None,
            width: 0,
            height: 0,
            format: None,
            ready: false,
            flags: 0,
            linux_dmabuf_received: false,
            constraints_done: false,
            failure: None,
        }));
        let frame = screencopy_manager.capture_output_region(
            if overlay_cursor { 1 } else { 0 },
            output,
            region.x(),
            region.y(),
            region.width(),
            region.height(),
            &qh,
            frame_state.clone(),
        );
        let mut attempts = 0;
        loop {
            {
                let state = lock_frame_state(&frame_state)?;
                if state.buffer.is_some() || state.ready || state.linux_dmabuf_received {
                    if state.ready && state.buffer.is_none() && !state.linux_dmabuf_received {
                        return Err(Error::FrameCapture(
                            "Frame is ready but buffer was not received".to_string(),
                        ));
                    }
                    break;
                }
            }
            if attempts >= MAX_ATTEMPTS {
                return Err(Error::FrameCapture(
                    "Timeout waiting for frame buffer".to_string(),
                ));
            }
            event_queue.blocking_dispatch(self).map_err(|e| {
                Error::FrameCapture(format!("Failed to dispatch frame events: {}", e))
            })?;
            attempts += 1;
        }
        {
            let mut state = lock_frame_state(&frame_state)?;
            if state.buffer.is_none() && state.linux_dmabuf_received {
                let stride = state.width * 4;
                let size =
                    crate::buffer::checked_buffer_size(state.width, state.height, 4, Some(stride))?;
                state.buffer = Some(vec![0u8; size]);
            }
        }
        let shm = self
            .globals
            .shm
            .as_ref()
            .ok_or_else(|| Error::UnsupportedProtocol("wl_shm not available".to_string()))?;

        let (width, height, stride, size, format) = {
            let state = lock_frame_state(&frame_state)?;
            if state.width == 0 || state.height == 0 {
                return Err(Error::CaptureFailed);
            }
            let width = state.width;
            let height = state.height;
            let stride = width * 4;
            let size = crate::buffer::checked_buffer_size(width, height, 4, Some(stride))?;
            let format = state.format.unwrap_or(ShmFormat::Xrgb8888);
            (width, height, stride, size, format)
        };
        let mut tmp_file = tempfile::NamedTempFile::new().map_err(|e| {
            Error::BufferCreation(format!("failed to create temporary file: {}", e))
        })?;
        tmp_file.as_file_mut().set_len(size as u64).map_err(|e| {
            Error::BufferCreation(format!("failed to resize buffer to {} bytes: {}", size, e))
        })?;
        let mmap = unsafe {
            memmap2::MmapMut::map_mut(&tmp_file)
                .map_err(|e| Error::BufferCreation(format!("failed to memory-map buffer: {}", e)))?
        };
        let pool = shm.create_pool(
            unsafe { BorrowedFd::borrow_raw(tmp_file.as_file().as_raw_fd()) },
            size as i32,
            &qh,
            (),
        );
        let buffer = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride as i32,
            format,
            &qh,
            (),
        );
        frame.copy(&buffer);
        let mut attempts = 0;
        loop {
            {
                let state = lock_frame_state(&frame_state)?;
                if state.ready {
                    if state.buffer.is_none() {
                        return Err(Error::FrameCapture(
                            "Frame is ready but buffer was not received".to_string(),
                        ));
                    }
                    break;
                }
            }
            if attempts >= MAX_ATTEMPTS {
                return Err(Error::FrameCapture(
                    "Timeout waiting for frame capture completion".to_string(),
                ));
            }
            event_queue.blocking_dispatch(self).map_err(|e| {
                Error::FrameCapture(format!("Failed to dispatch frame events: {}", e))
            })?;
            attempts += 1;
        }
        let mut buffer_data = mmap.to_vec();
        if let Some(pf) = shm_to_pixel(format) {
            crate::pixel_format::convert_to_rgba(&mut buffer_data, pf);
        }
        let output_id = output.id().protocol_id();
        let mut final_data = buffer_data;
        let mut final_width = width;
        let mut final_height = height;
        if let Some(info) = self.globals.output_info.get(&output_id) {
            if !matches!(
                info.transform,
                wayland_client::protocol::wl_output::Transform::Normal
            ) {
                let (transformed_data, new_width, new_height) =
                    crate::transform::apply_image_transform(
                        &final_data,
                        final_width,
                        final_height,
                        info.transform.into(),
                    );
                final_data = transformed_data;
                final_width = new_width;
                final_height = new_height;
            }
        }
        let flags = {
            let state = lock_frame_state(&frame_state)?;
            state.flags
        };
        if (flags & ZWLR_SCREENCOPY_FRAME_V1_FLAGS_Y_INVERT) != 0 {
            let (inverted_data, inv_width, inv_height) =
                crate::transform::flip_vertical(&final_data, final_width, final_height);
            final_data = inverted_data;
            final_width = inv_width;
            final_height = inv_height;
        }
        Ok(CaptureResult {
            data: final_data,
            width: final_width,
            height: final_height,
        })
    }

    fn capture_region_ext(
        &mut self,
        source_manager: &ExtOutputImageCaptureSourceManagerV1,
        copy_manager: &ExtImageCopyCaptureManagerV1,
        output: &WlOutput,
        region: Region,
        overlay_cursor: bool,
    ) -> Result<CaptureResult> {
        let mut event_queue = self._connection.new_event_queue();
        let qh = event_queue.handle();
        let frame_state = Arc::new(Mutex::new(FrameState {
            buffer: None,
            width: 0,
            height: 0,
            format: None,
            ready: false,
            flags: 0,
            linux_dmabuf_received: false,
            constraints_done: false,
            failure: None,
        }));
        let source = source_manager.create_source(output, &qh, ());
        let mut options = ExtCopyOptions::empty();
        if overlay_cursor {
            options |= ExtCopyOptions::PaintCursors;
        }
        let _session = copy_manager.create_session(&source, options, &qh, frame_state.clone());
        let mut attempts = 0;
        loop {
            {
                let state = lock_frame_state(&frame_state)?;
                if let Some(failure) = &state.failure {
                    return Err(Error::FrameCapture(failure.clone()));
                }
                if state.constraints_done {
                    break;
                }
            }
            if attempts >= MAX_ATTEMPTS {
                return Err(Error::FrameCapture(
                    "Timeout waiting for session constraints".to_string(),
                ));
            }
            event_queue.blocking_dispatch(self).map_err(|e| {
                Error::FrameCapture(format!("Failed to dispatch session events: {}", e))
            })?;
            attempts += 1;
        }
        let (full_width, full_height, format) = {
            let state = lock_frame_state(&frame_state)?;
            if state.width == 0 || state.height == 0 {
                return Err(Error::CaptureFailed);
            }
            (
                state.width,
                state.height,
                state.format.unwrap_or(ShmFormat::Xrgb8888),
            )
        };
        let stride = full_width * 4;
        let size = crate::buffer::checked_buffer_size(full_width, full_height, 4, Some(stride))?;
        let shm = self
            .globals
            .shm
            .as_ref()
            .ok_or_else(|| Error::UnsupportedProtocol("wl_shm not available".to_string()))?;
        let mut tmp_file = tempfile::NamedTempFile::new().map_err(|e| {
            Error::BufferCreation(format!("failed to create temporary file: {}", e))
        })?;
        tmp_file.as_file_mut().set_len(size as u64).map_err(|e| {
            Error::BufferCreation(format!("failed to resize buffer to {} bytes: {}", size, e))
        })?;
        let mmap = unsafe {
            memmap2::MmapMut::map_mut(&tmp_file)
                .map_err(|e| Error::BufferCreation(format!("failed to memory-map buffer: {}", e)))?
        };
        let pool = shm.create_pool(
            unsafe { BorrowedFd::borrow_raw(tmp_file.as_file().as_raw_fd()) },
            size as i32,
            &qh,
            (),
        );
        let buffer = pool.create_buffer(
            0,
            full_width as i32,
            full_height as i32,
            stride as i32,
            format,
            &qh,
            (),
        );
        {
            let mut state = lock_frame_state(&frame_state)?;
            state.buffer = Some(vec![0u8; size]);
        }
        let frame = _session.create_frame(&qh, frame_state.clone());
        frame.attach_buffer(&buffer);
        frame.damage_buffer(0, 0, full_width as i32, full_height as i32);
        frame.capture();
        let mut attempts = 0;
        loop {
            {
                let state = lock_frame_state(&frame_state)?;
                if let Some(failure) = &state.failure {
                    return Err(Error::FrameCapture(failure.clone()));
                }
                if state.ready {
                    break;
                }
            }
            if attempts >= MAX_ATTEMPTS {
                return Err(Error::FrameCapture(
                    "Timeout waiting for frame capture completion".to_string(),
                ));
            }
            event_queue.blocking_dispatch(self).map_err(|e| {
                Error::FrameCapture(format!("Failed to dispatch frame events: {}", e))
            })?;
            attempts += 1;
        }
        let mut buffer_data = mmap.to_vec();
        if let Some(pf) = shm_to_pixel(format) {
            crate::pixel_format::convert_to_rgba(&mut buffer_data, pf);
        }
        let output_id = output.id().protocol_id();
        let (mut final_data, mut final_width, mut final_height) =
            self.apply_output_transform(buffer_data, full_width, full_height, output_id);
        let region_w = region.width() as u32;
        let region_h = region.height() as u32;
        let need_crop = {
            let info = self.globals.output_info.get(&output_id);
            info.is_none_or(|info| {
                region.x() != 0
                    || region.y() != 0
                    || region_w as i32 != info.logical_width
                    || region_h as i32 != info.logical_height
            })
        };
        if need_crop && region_w > 0 && region_h > 0 {
            let scale = self
                .globals
                .output_info
                .get(&output_id)
                .map_or(1.0, |info| {
                    if info.logical_scale_known && info.logical_scale.is_finite() {
                        info.logical_scale
                    } else {
                        info.scale as f64
                    }
                });
            let crop_x = ((region.x() as f64) * scale) as usize;
            let crop_y = ((region.y() as f64) * scale) as usize;
            let crop_w = ((region_w as f64) * scale) as u32;
            let crop_h = ((region_h as f64) * scale) as u32;
            let (cropped, cw, ch) = crate::compositor::crop_rgba(
                &final_data,
                final_width,
                final_height,
                crop_x,
                crop_y,
                crop_w,
                crop_h,
            );
            final_data = cropped;
            final_width = cw;
            final_height = ch;
        }

        Ok(CaptureResult {
            data: final_data,
            width: final_width,
            height: final_height,
        })
    }

    fn composite_region(
        &mut self,
        region: Region,
        outputs: &[(WlOutput, OutputInfo)],
        overlay_cursor: bool,
    ) -> Result<CaptureResult> {
        let scale_map: HashMap<String, f64> = outputs
            .iter()
            .map(|(_, info)| {
                let s = if info.logical_scale_known && info.logical_scale.is_finite() {
                    info.logical_scale
                } else {
                    info.scale as f64
                };
                (info.name.clone(), s)
            })
            .collect();

        let output_tuples: Vec<(String, Region)> = outputs
            .iter()
            .map(|(_, info)| {
                (
                    info.name.clone(),
                    Region::new(
                        info.logical_x,
                        info.logical_y,
                        info.logical_width,
                        info.logical_height,
                    ),
                )
            })
            .collect();

        crate::compositor::composite_region(
            region,
            &output_tuples,
            overlay_cursor,
            |name, local_region, cursor| {
                let mut capture = self.capture_output_raw(name, local_region, cursor)?;
                let scale = scale_map.get(name).copied().unwrap_or(1.0);
                if scale != 1.0 {
                    capture = crate::scaling::scale_image_data(capture, 1.0 / scale)?;
                }
                Ok(capture)
            },
        )
    }

    pub fn get_outputs(&mut self) -> Result<Vec<Output>> {
        self.refresh_outputs()?;
        let snapshot = self.collect_outputs_snapshot();
        let outputs = snapshot
            .into_iter()
            .map(|(_, info)| {
                let (x, y, width, height) = if info.logical_scale_known {
                    (
                        info.logical_x,
                        info.logical_y,
                        info.logical_width,
                        info.logical_height,
                    )
                } else {
                    (info.x, info.y, info.width, info.height)
                };

                Output {
                    name: info.name.clone(),
                    geometry: Region::new(x, y, width, height),
                    scale: info.scale,
                    description: info.description.clone(),
                }
            })
            .collect::<Vec<_>>();
        if outputs.is_empty() {
            return Err(Error::NoOutputs);
        }
        Ok(outputs)
    }

    pub fn capture_all(&mut self) -> Result<CaptureResult> {
        self.refresh_outputs()?;
        let snapshot = self.collect_outputs_snapshot();
        if snapshot.is_empty() {
            return Err(Error::NoOutputs);
        }

        let (_, first_info) = &snapshot[0];
        let mut min_x = first_info.logical_x;
        let mut min_y = first_info.logical_y;
        let mut max_x = first_info.logical_x + first_info.logical_width;
        let mut max_y = first_info.logical_y + first_info.logical_height;

        for (_, info) in &snapshot {
            min_x = min_x.min(info.logical_x);
            min_y = min_y.min(info.logical_y);
            max_x = max_x.max(info.logical_x + info.logical_width);
            max_y = max_y.max(info.logical_y + info.logical_height);
        }

        let region = Region::new(min_x, min_y, max_x - min_x, max_y - min_y);
        self.composite_region(region, &snapshot, false)
    }

    pub fn capture_all_with_scale(&mut self, scale: f64) -> Result<CaptureResult> {
        let result = self.capture_all()?;
        crate::scaling::scale_image_data(result, scale)
    }

    pub fn capture_output(&mut self, output_name: &str) -> Result<CaptureResult> {
        self.refresh_outputs()?;
        let snapshot = self.collect_outputs_snapshot();
        let (output_handle, info) = snapshot
            .into_iter()
            .find(|(_, info)| info.name == output_name)
            .ok_or_else(|| Error::OutputNotFound(output_name.to_string()))?;

        let local_region = Region::new(0, 0, info.logical_width, info.logical_height);
        self.capture_region_for_output(&output_handle, local_region, false)
    }

    pub fn capture_output_with_scale(
        &mut self,
        output_name: &str,
        scale: f64,
    ) -> Result<CaptureResult> {
        let result = self.capture_output(output_name)?;
        crate::scaling::scale_image_data(result, scale)
    }

    pub fn capture_region(&mut self, region: Region) -> Result<CaptureResult> {
        self.refresh_outputs()?;
        let snapshot = self.collect_outputs_snapshot();
        self.composite_region(region, &snapshot, false)
    }

    pub fn capture_region_with_scale(
        &mut self,
        region: Region,
        scale: f64,
    ) -> Result<CaptureResult> {
        let result = self.capture_region(region)?;
        crate::scaling::scale_image_data(result, scale)
    }

    pub fn capture_outputs(
        &mut self,
        parameters: Vec<CaptureParameters>,
    ) -> Result<MultiOutputCaptureResult> {
        self.refresh_outputs()?;

        let backend = self.backend_cloned()?;
        match backend {
            CaptureBackend::WlrScreencopy { manager } => {
                self.capture_outputs_wlr(manager, parameters)
            }
            CaptureBackend::ExtImageCopyCapture {
                source_manager,
                copy_manager,
            } => self.capture_outputs_ext(source_manager, copy_manager, parameters),
        }
    }

    fn capture_outputs_wlr(
        &mut self,
        manager: ZwlrScreencopyManagerV1,
        parameters: Vec<CaptureParameters>,
    ) -> Result<MultiOutputCaptureResult> {
        let screencopy_manager = &manager;
        let mut event_queue = self._connection.new_event_queue();
        let qh = event_queue.handle();
        let mut frame_states: HashMap<String, Arc<Mutex<FrameState>>> = HashMap::new();
        let mut frames: HashMap<String, ZwlrScreencopyFrameV1> = HashMap::new();
        for param in &parameters {
            let (output_id, output_info) = self
                .globals
                .output_info
                .iter()
                .find(|(_, info)| info.name == param.output_name())
                .ok_or_else(|| Error::OutputNotFound(param.output_name().to_string()))?;

            let output = self
                .globals
                .outputs
                .iter()
                .find(|o| o.id().protocol_id() == *output_id)
                .ok_or_else(|| Error::OutputNotFound(param.output_name().to_string()))?;
            let region = if let Some(region) = param.region_ref() {
                let output_right = output_info.logical_width;
                let output_bottom = output_info.logical_height;
                if region.x() < 0
                    || region.y() < 0
                    || region.x() + region.width() > output_right
                    || region.y() + region.height() > output_bottom
                {
                    return Err(Error::InvalidRegion(
                        "Capture region extends outside output boundaries".to_string(),
                    ));
                }
                *region
            } else {
                Region::new(0, 0, output_info.logical_width, output_info.logical_height)
            };
            let frame_state = Arc::new(Mutex::new(FrameState {
                buffer: None,
                width: 0,
                height: 0,
                format: None,
                ready: false,
                flags: 0,
                linux_dmabuf_received: false,
                constraints_done: false,
                failure: None,
            }));
            let frame = screencopy_manager.capture_output_region(
                if param.overlay_cursor_enabled() { 1 } else { 0 },
                output,
                region.x(),
                region.y(),
                region.width(),
                region.height(),
                &qh,
                frame_state.clone(),
            );
            frame_states.insert(param.output_name().to_string(), frame_state);
            frames.insert(param.output_name().to_string(), frame);
        }
        let mut attempts = 0;
        let mut completed_frames = 0;
        let total_frames = parameters.len();
        while completed_frames < total_frames && attempts < MAX_ATTEMPTS {
            completed_frames = frame_states
                .iter()
                .filter(|(_, state)| {
                    state
                        .lock()
                        .ok()
                        .is_some_and(|s| s.buffer.is_some() || s.ready || s.linux_dmabuf_received)
                })
                .count();
            if completed_frames >= total_frames {
                break;
            }
            event_queue.blocking_dispatch(self).map_err(|e| {
                Error::FrameCapture(format!("Failed to dispatch frame events: {}", e))
            })?;
            attempts += 1;
        }
        if attempts >= MAX_ATTEMPTS {
            return Err(Error::FrameCapture(
                "Timeout waiting for frame buffers".to_string(),
            ));
        }
        for frame_state in frame_states.values() {
            let mut state = lock_frame_state(frame_state)?;
            if state.buffer.is_none() {
                if state.linux_dmabuf_received {
                    let stride = state.width * 4;
                    let size = crate::buffer::checked_buffer_size(
                        state.width,
                        state.height,
                        4,
                        Some(stride),
                    )?;
                    state.buffer = Some(vec![0u8; size]);
                } else {
                    return Err(Error::CaptureFailed);
                }
            }
        }
        let mut buffers: HashMap<String, (tempfile::NamedTempFile, memmap2::MmapMut)> =
            HashMap::new();
        for (output_name, frame_state) in &frame_states {
            let (width, height, stride, size) = {
                let state = lock_frame_state(frame_state)?;
                let width = state.width;
                let height = state.height;
                let stride = width * 4;
                let size = crate::buffer::checked_buffer_size(width, height, 4, Some(stride))?;
                (width, height, stride, size)
            };
            let mut tmp_file = tempfile::NamedTempFile::new().map_err(|e| {
                Error::BufferCreation(format!(
                    "failed to create temporary file for output '{}': {}",
                    output_name, e
                ))
            })?;
            tmp_file.as_file_mut().set_len(size as u64).map_err(|e| {
                Error::BufferCreation(format!(
                    "failed to resize buffer for output '{}' to {} bytes: {}",
                    output_name, size, e
                ))
            })?;
            let mmap = unsafe {
                memmap2::MmapMut::map_mut(&tmp_file).map_err(|e| {
                    Error::BufferCreation(format!(
                        "failed to memory-map buffer for output '{}': {}",
                        output_name, e
                    ))
                })?
            };
            let shm = self.globals.shm.as_ref().ok_or(Error::UnsupportedProtocol(
                "wl_shm not available".to_string(),
            ))?;
            {
                let format = {
                    let state = lock_frame_state(frame_state)?;
                    state.format.unwrap_or(ShmFormat::Xrgb8888)
                };
                let pool = shm.create_pool(
                    unsafe { BorrowedFd::borrow_raw(tmp_file.as_file().as_raw_fd()) },
                    size as i32,
                    &qh,
                    (),
                );
                let buffer = pool.create_buffer(
                    0,
                    width as i32,
                    height as i32,
                    stride as i32,
                    format,
                    &qh,
                    (),
                );
                if let Some(frame) = frames.get(output_name) {
                    frame.copy(&buffer);
                }
            }
            buffers.insert(output_name.clone(), (tmp_file, mmap));
        }
        let mut attempts = 0;
        let mut completed_frames = 0;
        while completed_frames < total_frames && attempts < MAX_ATTEMPTS {
            completed_frames = frame_states
                .iter()
                .filter(|(_, state)| state.lock().ok().is_some_and(|s| s.ready))
                .count();
            if completed_frames >= total_frames {
                break;
            }
            event_queue.blocking_dispatch(self).map_err(|e| {
                Error::FrameCapture(format!("Failed to dispatch frame events: {}", e))
            })?;
            attempts += 1;
        }
        if attempts >= MAX_ATTEMPTS {
            return Err(Error::FrameCapture(
                "Timeout waiting for frame capture completion".to_string(),
            ));
        }
        for frame_state in frame_states.values() {
            let state = lock_frame_state(frame_state)?;
            if state.ready && state.buffer.is_none() {
                return Err(Error::FrameCapture(
                    "Frame is ready but buffer was not received".to_string(),
                ));
            }
        }
        let mut results: HashMap<String, CaptureResult> = HashMap::new();
        for (output_name, (_tmp_file, mmap)) in buffers {
            let frame_state = &frame_states[&output_name];
            let (width, height) = {
                let state = lock_frame_state(frame_state)?;
                (state.width, state.height)
            };
            let format = {
                let state = lock_frame_state(frame_state)?;
                state.format.unwrap_or(ShmFormat::Xrgb8888)
            };
            let mut buffer_data = mmap.to_vec();
            if let Some(pf) = shm_to_pixel(format) {
                crate::pixel_format::convert_to_rgba(&mut buffer_data, pf);
            }
            let mut final_data = buffer_data;
            let mut final_width = width;
            let mut final_height = height;

            if let Some(info) = self
                .globals
                .output_info
                .values()
                .find(|info| info.name == output_name)
            {
                if !matches!(
                    info.transform,
                    wayland_client::protocol::wl_output::Transform::Normal
                ) {
                    let (transformed_data, new_width, new_height) =
                        crate::transform::apply_image_transform(
                            &final_data,
                            final_width,
                            final_height,
                            info.transform.into(),
                        );
                    final_data = transformed_data;
                    final_width = new_width;
                    final_height = new_height;
                }
            }
            let flags = {
                let state = lock_frame_state(frame_state)?;
                state.flags
            };
            if (flags & ZWLR_SCREENCOPY_FRAME_V1_FLAGS_Y_INVERT) != 0 {
                let (inverted_data, inv_width, inv_height) =
                    crate::transform::flip_vertical(&final_data, final_width, final_height);
                final_data = inverted_data;
                final_width = inv_width;
                final_height = inv_height;
            }
            results.insert(
                output_name,
                CaptureResult {
                    data: final_data,
                    width: final_width,
                    height: final_height,
                },
            );
        }
        Ok(MultiOutputCaptureResult::new(results))
    }

    fn capture_outputs_ext(
        &mut self,
        source_manager: ExtOutputImageCaptureSourceManagerV1,
        copy_manager: ExtImageCopyCaptureManagerV1,
        parameters: Vec<CaptureParameters>,
    ) -> Result<MultiOutputCaptureResult> {
        let mut results = HashMap::new();
        for param in &parameters {
            let (output_id, output_info) = self
                .globals
                .output_info
                .iter()
                .find(|(_, info)| info.name == param.output_name())
                .ok_or_else(|| Error::OutputNotFound(param.output_name().to_string()))?;

            let region = if let Some(region) = param.region_ref() {
                let output_right = output_info.logical_width;
                let output_bottom = output_info.logical_height;
                if region.x() < 0
                    || region.y() < 0
                    || region.x() + region.width() > output_right
                    || region.y() + region.height() > output_bottom
                {
                    return Err(Error::InvalidRegion(
                        "Capture region extends outside output boundaries".to_string(),
                    ));
                }
                *region
            } else {
                Region::new(0, 0, output_info.logical_width, output_info.logical_height)
            };
            let output_clone = {
                let output = self
                    .globals
                    .outputs
                    .iter()
                    .find(|o| o.id().protocol_id() == *output_id)
                    .ok_or_else(|| Error::OutputNotFound(param.output_name().to_string()))?;
                output.clone()
            };
            let capture = self.capture_region_ext(
                &source_manager,
                &copy_manager,
                &output_clone,
                region,
                param.overlay_cursor_enabled(),
            )?;
            results.insert(param.output_name().to_string(), capture);
        }
        Ok(MultiOutputCaptureResult::new(results))
    }

    pub fn capture_outputs_with_scale(
        &mut self,
        parameters: Vec<CaptureParameters>,
        default_scale: f64,
    ) -> Result<MultiOutputCaptureResult> {
        let result = self.capture_outputs(parameters)?;
        let mut scaled_results = std::collections::HashMap::new();
        for (output_name, capture_result) in result.into_outputs() {
            let scale = default_scale;
            let scaled_result = crate::scaling::scale_image_data(capture_result, scale)?;
            scaled_results.insert(output_name, scaled_result);
        }
        Ok(MultiOutputCaptureResult::new(scaled_results))
    }

    fn apply_output_transform(
        &self,
        data: Vec<u8>,
        width: u32,
        height: u32,
        output_id: u32,
    ) -> (Vec<u8>, u32, u32) {
        if let Some(info) = self.globals.output_info.get(&output_id) {
            if !matches!(
                info.transform,
                wayland_client::protocol::wl_output::Transform::Normal
            ) {
                return crate::transform::apply_image_transform(
                    &data,
                    width,
                    height,
                    info.transform.into(),
                );
            }
        }
        (data, width, height)
    }
}
