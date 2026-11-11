use super::*;
use crate::checked_buffer_size;

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
        if let Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => {
                    state.globals.compositor =
                        Some(registry.bind::<WlCompositor, _, _>(name, version, qh, ()));
                }
                "wl_shm" => {
                    state.globals.shm = Some(registry.bind::<WlShm, _, _>(name, version, qh, ()));
                }
                "zwlr_screencopy_manager_v1" => {
                    state.globals.screencopy_manager =
                        Some(registry.bind::<ZwlrScreencopyManagerV1, _, _>(name, version, qh, ()));
                }
                "zxdg_output_manager_v1" => {
                    state.globals.xdg_output_manager =
                        Some(registry.bind::<ZxdgOutputManagerV1, _, _>(name, version, qh, ()));

                    if let Some(ref xdg_output_manager) = state.globals.xdg_output_manager {
                        for output in &state.globals.outputs {
                            let xdg_output = xdg_output_manager.get_xdg_output(output, qh, ());
                            let output_id = output.id().protocol_id();
                            state.globals.output_xdg_map.insert(output_id, xdg_output);
                        }
                    }
                }
                "ext_output_image_capture_source_manager_v1" => {
                    state.globals.ext_source_manager =
                        Some(registry.bind::<ExtOutputImageCaptureSourceManagerV1, _, _>(
                            name,
                            version,
                            qh,
                            (),
                        ));
                }
                "ext_image_copy_capture_manager_v1" => {
                    state.globals.ext_copy_manager = Some(
                        registry.bind::<ExtImageCopyCaptureManagerV1, _, _>(name, version, qh, ()),
                    );
                }
                "wl_output" => {
                    let output = registry.bind::<WlOutput, _, _>(name, version, qh, ());
                    let output_id = output.id().protocol_id();
                    state.globals.output_info.insert(
                        output_id,
                        OutputInfo {
                            name: format!("output-{}", name),
                            width: 0,
                            height: 0,
                            x: 0,
                            y: 0,
                            scale: 1,
                            transform: wayland_client::protocol::wl_output::Transform::Normal,
                            logical_x: 0,
                            logical_y: 0,
                            logical_width: 0,
                            logical_height: 0,
                            logical_scale_known: false,
                            logical_scale: 1.0,
                            description: None,
                        },
                    );
                    let output_idx = state.globals.outputs.len();
                    state.globals.outputs.push(output.clone());
                    if let Some(ref xdg_output_manager) = state.globals.xdg_output_manager {
                        let output_to_use = &state.globals.outputs[output_idx];
                        let xdg_output = xdg_output_manager.get_xdg_output(output_to_use, qh, ());
                        let output_id = output_to_use.id().protocol_id();
                        state.globals.output_xdg_map.insert(output_id, xdg_output);
                    }
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
        use wayland_client::protocol::wl_output::Event;
        let output_id = output.id().protocol_id();
        match event {
            Event::Geometry {
                x,
                y,
                physical_width: _,
                physical_height: _,
                subpixel: _,
                make: _,
                model: _,
                transform,
            } => {
                if let Some(info) = state.globals.output_info.get_mut(&output_id) {
                    info.x = x;
                    info.y = y;
                    if let wayland_client::WEnum::Value(t) = transform {
                        info.transform = t;
                    }
                    if !info.logical_scale_known {
                        info.logical_x = x;
                        info.logical_y = y;
                    }

                    update_logical_scale(info);
                }
            }
            Event::Mode {
                flags: _,
                width,
                height,
                refresh: _,
            } => {
                log::debug!(
                    "Mode event for output_id {}: {}x{}",
                    output_id,
                    width,
                    height
                );
                if let Some(info) = state.globals.output_info.get_mut(&output_id) {
                    info.width = width;
                    info.height = height;
                    log::debug!("Updated output info: {}x{}", info.width, info.height);
                    if !info.logical_scale_known {
                        info.logical_width = width;
                        info.logical_height = height;
                    }

                    update_logical_scale(info);
                }
            }
            Event::Scale { factor } => {
                if let Some(info) = state.globals.output_info.get_mut(&output_id) {
                    info.scale = factor;
                    update_logical_scale(info);
                }
            }
            Event::Name { name } => {
                if let Some(info) = state.globals.output_info.get_mut(&output_id) {
                    info.name = name.clone();
                }
            }
            Event::Description { description } => {
                if let Some(info) = state.globals.output_info.get_mut(&output_id) {
                    info.description = Some(description);
                }
            }
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
            Event::Buffer {
                format,
                width,
                height,
                stride,
            } => {
                let mut state = match lock_frame_state(frame_state) {
                    Ok(state) => state,
                    Err(err) => {
                        log::error!(
                            "Dropping screencopy Buffer event due to mutex error: {}",
                            err
                        );
                        return;
                    }
                };
                state.width = width;
                state.height = height;
                if let wayland_client::WEnum::Value(val) = format {
                    state.format = Some(val);
                }
                match checked_buffer_size(width, height, 4, Some(stride)) {
                    Ok(size) => {
                        state.buffer = Some(vec![0u8; size]);
                    }
                    Err(err) => {
                        log::error!(
                            "Dropping screencopy Buffer event due to size check failure: {}",
                            err
                        );
                        state.buffer = None;
                        state.ready = true;
                    }
                }
            }
            Event::Flags { flags } => {
                let mut state = match lock_frame_state(frame_state) {
                    Ok(state) => state,
                    Err(err) => {
                        log::error!(
                            "Dropping screencopy Flags event due to mutex error: {}",
                            err
                        );
                        return;
                    }
                };
                if let wayland_client::WEnum::Value(val) = flags {
                    state.flags = val.bits();
                    log::debug!("Received flags: {:?} (bits: {})", flags, val.bits());
                }
            }
            Event::Ready {
                tv_sec_hi: _,
                tv_sec_lo: _,
                tv_nsec: _,
            } => {
                let mut state = match lock_frame_state(frame_state) {
                    Ok(state) => state,
                    Err(err) => {
                        log::error!(
                            "Dropping screencopy Ready event due to mutex error: {}",
                            err
                        );
                        return;
                    }
                };
                state.ready = true;
                frame.destroy();
            }
            Event::Failed => {
                let mut state = match lock_frame_state(frame_state) {
                    Ok(state) => state,
                    Err(err) => {
                        log::error!(
                            "Dropping screencopy Failed event due to mutex error: {}",
                            err
                        );
                        return;
                    }
                };
                state.ready = true;
            }
            Event::LinuxDmabuf {
                format,
                width,
                height,
            } => {
                let mut state = match lock_frame_state(frame_state) {
                    Ok(state) => state,
                    Err(err) => {
                        log::error!(
                            "Dropping screencopy LinuxDmabuf event due to mutex error: {}",
                            err
                        );
                        return;
                    }
                };
                if state.width == 0 {
                    state.width = width;
                }
                if state.height == 0 {
                    state.height = height;
                }
                if state.format.is_none() {
                    state.format = super::drm_fourcc_to_pixel(format).map(super::pixel_to_shm);
                    if state.format.is_none() {
                        log::warn!(
                            "Unknown dmabuf DRM fourcc 0x{:08x}, falling back to Xrgb8888",
                            format
                        );
                        state.format = Some(ShmFormat::Xrgb8888);
                    }
                }
                state.linux_dmabuf_received = true;
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

impl Dispatch<ZxdgOutputV1, ()> for WaylandCapture {
    fn event(
        state: &mut Self,
        xdg_output: &ZxdgOutputV1,
        event: <ZxdgOutputV1 as Proxy>::Event,
        _: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_v1::Event;

        let xdg_output_id = xdg_output.id().protocol_id();

        let mut found_output_id = None;
        for (wl_output_id, mapped_xdg_output) in &state.globals.output_xdg_map {
            if mapped_xdg_output.id().protocol_id() == xdg_output_id {
                found_output_id = Some(*wl_output_id);
                break;
            }
        }
        if let Some(wl_output_id) = found_output_id {
            if let Some(info) = state.globals.output_info.get_mut(&wl_output_id) {
                match event {
                    Event::LogicalPosition { x, y } => {
                        info.logical_x = x;
                        info.logical_y = y;
                        info.logical_scale_known = true;
                        update_logical_scale(info);
                    }
                    Event::LogicalSize { width, height } => {
                        info.logical_width = width;
                        info.logical_height = height;
                        info.logical_scale_known = true;
                        update_logical_scale(info);
                    }
                    Event::Name { name }
                        if info.name.starts_with("output-") || info.name.is_empty() =>
                    {
                        info.name = name.clone();
                    }
                    Event::Description { description } => {
                        info.description = Some(description);
                    }
                    Event::Done => {
                        info.logical_scale_known = true;
                        update_logical_scale(info);
                    }
                    _ => {}
                }
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
    ) {
    }
}

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

impl Dispatch<ZxdgOutputManagerV1, ()> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _proxy: &ZxdgOutputManagerV1,
        _event: <ZxdgOutputManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtImageCaptureSourceV1, ()> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _proxy: &ExtImageCaptureSourceV1,
        _event: <ExtImageCaptureSourceV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtOutputImageCaptureSourceManagerV1, ()> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _proxy: &ExtOutputImageCaptureSourceManagerV1,
        _event: <ExtOutputImageCaptureSourceManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtImageCopyCaptureManagerV1, ()> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _proxy: &ExtImageCopyCaptureManagerV1,
        _event: <ExtImageCopyCaptureManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtImageCopyCaptureSessionV1, Arc<Mutex<FrameState>>> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _session: &ExtImageCopyCaptureSessionV1,
        event: <ExtImageCopyCaptureSessionV1 as Proxy>::Event,
        frame_state: &Arc<Mutex<FrameState>>,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_session_v1::Event;
        let mut s = match lock_frame_state(frame_state) {
            Ok(s) => s,
            Err(err) => {
                log::error!("Dropping ext session event due to mutex error: {}", err);
                return;
            }
        };
        match event {
            Event::BufferSize { width, height } => {
                s.width = width;
                s.height = height;
            }
            Event::ShmFormat {
                format: wayland_client::WEnum::Value(f),
            } => {
                s.format = Some(f);
            }
            Event::ShmFormat { .. } => {}
            Event::Done => {
                s.constraints_done = true;
            }
            Event::Stopped => {
                log::warn!("Capture session stopped by compositor");
                s.ready = true;
                s.failure = Some("capture session stopped by compositor".to_string());
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, Arc<Mutex<FrameState>>> for WaylandCapture {
    fn event(
        _state: &mut Self,
        frame: &ExtImageCopyCaptureFrameV1,
        event: <ExtImageCopyCaptureFrameV1 as Proxy>::Event,
        frame_state: &Arc<Mutex<FrameState>>,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_frame_v1::Event;
        let mut s = match lock_frame_state(frame_state) {
            Ok(s) => s,
            Err(err) => {
                log::error!("Dropping ext frame event due to mutex error: {}", err);
                return;
            }
        };
        match event {
            Event::Ready => {
                s.ready = true;
                frame.destroy();
            }
            Event::Failed { reason } => {
                log::error!("Capture frame failed: reason={:?}", reason);
                s.ready = true;
                s.failure = Some(format!("capture frame failed: {reason:?}"));
                frame.destroy();
            }
            _ => {}
        }
    }
}
