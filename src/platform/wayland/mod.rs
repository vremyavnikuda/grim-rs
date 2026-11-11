pub(super) use crate::{
    CaptureParameters, CaptureResult, Error, MultiOutputCaptureResult, Output, Region, Result,
};
pub(super) use std::collections::HashMap;
pub(super) use std::os::fd::{AsRawFd, BorrowedFd};
pub(super) use std::sync::{Arc, Mutex};
pub(super) use wayland_client::{
    protocol::{
        wl_buffer::WlBuffer,
        wl_compositor::WlCompositor,
        wl_output::WlOutput,
        wl_registry::WlRegistry,
        wl_shm::{Format as ShmFormat, WlShm},
        wl_shm_pool::WlShmPool,
    },
    Connection, Dispatch, Proxy, QueueHandle,
};
pub(super) use wayland_protocols::xdg::xdg_output::zv1::client::{
    zxdg_output_manager_v1::ZxdgOutputManagerV1, zxdg_output_v1::ZxdgOutputV1,
};
pub(super) use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};

pub(super) use wayland_protocols::ext::image_capture_source::v1::client::{
    ext_image_capture_source_v1::ExtImageCaptureSourceV1,
    ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
};
pub(super) use wayland_protocols::ext::image_copy_capture::v1::client::{
    ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1,
    ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1,
    ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1,
};

mod capture;
mod events;

pub(super) const ZWLR_SCREENCOPY_FRAME_V1_FLAGS_Y_INVERT: u32 = 1;
pub(super) const MAX_ATTEMPTS: usize = 100;

/// Captures which capture protocol is in use and holds its specific manager objects.
#[derive(Clone)]
pub(super) enum CaptureBackend {
    WlrScreencopy {
        manager: ZwlrScreencopyManagerV1,
    },
    ExtImageCopyCapture {
        source_manager: ExtOutputImageCaptureSourceManagerV1,
        copy_manager: ExtImageCopyCaptureManagerV1,
    },
}

#[derive(Debug, Clone)]
pub(super) struct FrameState {
    buffer: Option<Vec<u8>>,
    width: u32,
    height: u32,
    format: Option<ShmFormat>,
    ready: bool,
    flags: u32,
    linux_dmabuf_received: bool,
    constraints_done: bool,
    failure: Option<String>,
}

/// Safely lock a FrameState mutex, converting poisoned mutex errors to Result.
pub(super) fn lock_frame_state(
    frame_state: &Arc<Mutex<FrameState>>,
) -> Result<std::sync::MutexGuard<'_, FrameState>> {
    frame_state
        .lock()
        .map_err(|e| Error::FrameCapture(format!("Frame state mutex poisoned: {}", e)))
}

/// Convert a `wl_shm::Format` to our internal [`PixelFormat`].
fn shm_to_pixel(format: ShmFormat) -> Option<crate::pixel_format::PixelFormat> {
    match format {
        ShmFormat::Argb8888 => Some(crate::pixel_format::PixelFormat::Argb8888),
        ShmFormat::Xrgb8888 => Some(crate::pixel_format::PixelFormat::Xrgb8888),
        ShmFormat::Abgr8888 => Some(crate::pixel_format::PixelFormat::Abgr8888),
        ShmFormat::Xbgr8888 => Some(crate::pixel_format::PixelFormat::Xbgr8888),
        _ => None,
    }
}

/// Map a DRM fourcc code through to [`PixelFormat`], used internally for dmabuf events.
fn drm_fourcc_to_pixel(fourcc: u32) -> Option<crate::pixel_format::PixelFormat> {
    crate::pixel_format::fourcc_to_format(fourcc)
}

fn pixel_to_shm(format: crate::pixel_format::PixelFormat) -> ShmFormat {
    match format {
        crate::pixel_format::PixelFormat::Argb8888 => ShmFormat::Argb8888,
        crate::pixel_format::PixelFormat::Xrgb8888 => ShmFormat::Xrgb8888,
        crate::pixel_format::PixelFormat::Abgr8888 => ShmFormat::Abgr8888,
        crate::pixel_format::PixelFormat::Xbgr8888 => ShmFormat::Xbgr8888,
    }
}

/// Guess logical geometry from physical geometry when xdg_output is not available.
pub(super) fn guess_output_logical_geometry(info: &mut OutputInfo) {
    info.logical_x = info.x;
    info.logical_y = info.y;
    info.logical_width = info.width / info.scale;
    info.logical_height = info.height / info.scale;

    crate::transform::apply_output_transform(
        info.transform.into(),
        &mut info.logical_width,
        &mut info.logical_height,
    );
    info.logical_scale_known = true;
    update_logical_scale(info);
}

/// Infer a (possibly fractional) logical scale.
pub(super) fn update_logical_scale(info: &mut OutputInfo) {
    if info.width <= 0 || info.height <= 0 || info.logical_width <= 0 || info.logical_height <= 0 {
        return;
    }

    let mut physical_width = info.width;
    let mut physical_height = info.height;
    crate::transform::apply_output_transform(
        info.transform.into(),
        &mut physical_width,
        &mut physical_height,
    );

    info.logical_scale = (physical_width as f64) / (info.logical_width as f64);
}

#[derive(Clone)]
pub(super) struct OutputInfo {
    pub(super) name: String,
    pub(super) width: i32,
    pub(super) height: i32,
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) scale: i32,
    pub(super) transform: wayland_client::protocol::wl_output::Transform,
    pub(super) logical_x: i32,
    pub(super) logical_y: i32,
    pub(super) logical_width: i32,
    pub(super) logical_height: i32,
    pub(super) logical_scale_known: bool,
    pub(super) logical_scale: f64,
    pub(super) description: Option<String>,
}

pub(super) struct WaylandGlobals {
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
    screencopy_manager: Option<ZwlrScreencopyManagerV1>,
    ext_source_manager: Option<ExtOutputImageCaptureSourceManagerV1>,
    ext_copy_manager: Option<ExtImageCopyCaptureManagerV1>,
    xdg_output_manager: Option<ZxdgOutputManagerV1>,
    outputs: Vec<WlOutput>,
    output_info: HashMap<u32, OutputInfo>,
    output_xdg_map: HashMap<u32, ZxdgOutputV1>,
}

pub struct WaylandCapture {
    _connection: Connection,
    globals: WaylandGlobals,
    backend: Option<CaptureBackend>,
}

impl WaylandCapture {
    fn try_backend(&self) -> Result<&CaptureBackend> {
        self.backend.as_ref().ok_or_else(|| {
            Error::WaylandConnection("WaylandCapture backend not initialized".to_string())
        })
    }

    fn backend_cloned(&self) -> Result<CaptureBackend> {
        Ok(self.try_backend()?.clone())
    }

    pub fn new(preference: crate::Backend) -> Result<Self> {
        let connection = Connection::connect_to_env().map_err(|e| {
            Error::WaylandConnection(format!("Failed to connect to Wayland: {}", e))
        })?;
        let globals = WaylandGlobals {
            compositor: None,
            shm: None,
            screencopy_manager: None,
            ext_source_manager: None,
            ext_copy_manager: None,
            xdg_output_manager: None,
            outputs: Vec::new(),
            output_info: HashMap::new(),
            output_xdg_map: HashMap::new(),
        };

        let mut event_queue = connection.new_event_queue();
        let qh = event_queue.handle();
        let _registry = connection.display().get_registry(&qh, ());
        let mut instance = Self {
            _connection: connection,
            globals,
            backend: None,
        };
        event_queue.roundtrip(&mut instance).map_err(|e| {
            Error::WaylandConnection(format!("Failed to initialize Wayland globals: {}", e))
        })?;

        let backend = match preference {
            crate::Backend::ExtImageCopyCapture => {
                let source_manager =
                    instance.globals.ext_source_manager.take().ok_or_else(|| {
                        Error::UnsupportedProtocol(
                            "ext-image-copy-capture-v1 not available".to_string(),
                        )
                    })?;
                let copy_manager = instance.globals.ext_copy_manager.take().ok_or_else(|| {
                    Error::UnsupportedProtocol(
                        "ext-image-copy-capture-v1 not available".to_string(),
                    )
                })?;
                CaptureBackend::ExtImageCopyCapture {
                    source_manager,
                    copy_manager,
                }
            }
            crate::Backend::WlrScreencopy => {
                let mgr = instance.globals.screencopy_manager.take().ok_or_else(|| {
                    Error::UnsupportedProtocol(
                        "zwlr-screencopy-manager-v1 not available".to_string(),
                    )
                })?;
                CaptureBackend::WlrScreencopy { manager: mgr }
            }
            crate::Backend::Auto => {
                // Prefer ext-image-copy-capture, fall back to wlr-screencopy
                match (
                    instance.globals.ext_source_manager.take(),
                    instance.globals.ext_copy_manager.take(),
                ) {
                    (Some(source_manager), Some(copy_manager)) => {
                        CaptureBackend::ExtImageCopyCapture {
                            source_manager,
                            copy_manager,
                        }
                    }
                    _ => {
                        let manager =
                            instance.globals.screencopy_manager.take().ok_or_else(|| {
                                Error::UnsupportedProtocol(
                                    "No capture protocol available (tried \
                                     ext-image-copy-capture-v1 and \
                                     zwlr-screencopy-manager-v1)"
                                        .to_string(),
                                )
                            })?;
                        CaptureBackend::WlrScreencopy { manager }
                    }
                }
            }
        };

        if instance.globals.shm.is_none() {
            return Err(Error::UnsupportedProtocol(
                "wl_shm not available".to_string(),
            ));
        }

        let backend_label = match &backend {
            CaptureBackend::WlrScreencopy { .. } => "wlr-screencopy",
            CaptureBackend::ExtImageCopyCapture { .. } => "ext-image-copy-capture-v1",
        };
        log::info!("grim-rs: using {} backend", backend_label);
        instance.backend = Some(backend);
        Ok(instance)
    }

    /// Capture a region within a specific output by name.
    ///
    /// Assumes outputs have been refreshed via `get_outputs()`.
    pub(crate) fn capture_output_raw(
        &mut self,
        output_name: &str,
        region: Region,
        overlay_cursor: bool,
    ) -> Result<CaptureResult> {
        let snapshot = self.collect_outputs_snapshot();
        let (output_handle, _info) = snapshot
            .into_iter()
            .find(|(_, info)| info.name == output_name)
            .ok_or_else(|| Error::OutputNotFound(output_name.to_string()))?;
        self.capture_region_for_output(&output_handle, region, overlay_cursor)
    }
}
