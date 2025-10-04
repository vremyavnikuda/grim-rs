use crate::{
    CaptureResult,
    Output,
    Box,
    Result,
    Error,
    CaptureParameters,
    MultiOutputCaptureResult,
};
use wayland_client::{
    Connection,
    Dispatch,
    QueueHandle,
    Proxy,
    protocol::{
        wl_compositor::WlCompositor,
        wl_output::WlOutput,
        wl_shm::{ WlShm, Format as ShmFormat },
        wl_shm_pool::WlShmPool,
        wl_buffer::WlBuffer,
        wl_registry::WlRegistry,
    },
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
    zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1,
};

// Screencopy frame flags from the protocol
const ZWLR_SCREENCOPY_FRAME_V1_FLAGS_Y_INVERT: u32 = 1;
use wayland_protocols::xdg::xdg_output::zv1::client::{
    zxdg_output_manager_v1::ZxdgOutputManagerV1,
    zxdg_output_v1::ZxdgOutputV1,
};
use std::collections::HashMap;
use std::sync::{ Arc, Mutex };
use std::os::fd::{ AsRawFd, BorrowedFd };

const MAX_ATTEMPTS: usize = 100;

/// Apply output transformation to width and height.
/// For 90° and 270° rotations, width and height are swapped.
fn apply_output_transform(
    transform: wayland_client::protocol::wl_output::Transform,
    width: &mut i32,
    height: &mut i32
) {
    use wayland_client::protocol::wl_output::Transform;

    match transform {
        Transform::_90 | Transform::_270 | Transform::Flipped90 | Transform::Flipped270 => {
            std::mem::swap(width, height);
        }
        _ => {}
    }
}

/// Get rotation angle in radians for the given transform.
fn get_output_rotation(transform: wayland_client::protocol::wl_output::Transform) -> f64 {
    use wayland_client::protocol::wl_output::Transform;

    match transform {
        Transform::_90 | Transform::Flipped90 => std::f64::consts::FRAC_PI_2,
        Transform::_180 | Transform::Flipped180 => std::f64::consts::PI,
        Transform::_270 | Transform::Flipped270 => 3.0 * std::f64::consts::FRAC_PI_2,
        _ => 0.0,
    }
}

/// Get flip multiplier for the given transform.
/// Returns -1 if flipped, 1 otherwise.
fn get_output_flipped(transform: wayland_client::protocol::wl_output::Transform) -> i32 {
    use wayland_client::protocol::wl_output::Transform;

    match transform {
        Transform::Flipped | Transform::Flipped90 | Transform::Flipped180 | Transform::Flipped270 =>
            -1,
        _ => 1,
    }
}

/// Apply transform to captured image data based on rotation and flip.
/// This handles basic 90/180/270 degree rotations and horizontal flips.
fn apply_image_transform(
    data: &[u8],
    width: u32,
    height: u32,
    transform: wayland_client::protocol::wl_output::Transform
) -> (Vec<u8>, u32, u32) {
    use wayland_client::protocol::wl_output::Transform;

    match transform {
        Transform::Normal => {
            // No transformation needed
            (data.to_vec(), width, height)
        }
        Transform::_90 => {
            // Rotate 90 degrees clockwise
            rotate_90(data, width, height)
        }
        Transform::_180 => {
            // Rotate 180 degrees
            rotate_180(data, width, height)
        }
        Transform::_270 => {
            // Rotate 270 degrees clockwise
            rotate_270(data, width, height)
        }
        Transform::Flipped => {
            // Horizontal flip only
            flip_horizontal(data, width, height)
        }
        Transform::Flipped90 => {
            // Flip then rotate 90
            let (flipped_data, w, h) = flip_horizontal(data, width, height);
            rotate_90(&flipped_data, w, h)
        }
        Transform::Flipped180 => {
            // Flip then rotate 180 (equivalent to vertical flip)
            flip_vertical(data, width, height)
        }
        Transform::Flipped270 => {
            // Flip then rotate 270
            let (flipped_data, w, h) = flip_horizontal(data, width, height);
            rotate_270(&flipped_data, w, h)
        }
        _ => {
            // Unknown transform, return as-is
            (data.to_vec(), width, height)
        }
    }
}

/// Rotate image 90 degrees clockwise.
fn rotate_90(data: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    let new_width = height;
    let new_height = width;
    let mut rotated = vec![0u8; (new_width * new_height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let src_idx = ((y * width + x) * 4) as usize;
            // For 90° rotation: new_x = height - 1 - y, new_y = x
            let new_x = height - 1 - y;
            let new_y = x;
            let dst_idx = ((new_y * new_width + new_x) * 4) as usize;

            rotated[dst_idx..dst_idx + 4].copy_from_slice(&data[src_idx..src_idx + 4]);
        }
    }

    (rotated, new_width, new_height)
}

/// Rotate image 180 degrees.
fn rotate_180(data: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    let mut rotated = vec![0u8; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let src_idx = ((y * width + x) * 4) as usize;
            let new_x = width - 1 - x;
            let new_y = height - 1 - y;
            let dst_idx = ((new_y * width + new_x) * 4) as usize;

            rotated[dst_idx..dst_idx + 4].copy_from_slice(&data[src_idx..src_idx + 4]);
        }
    }

    (rotated, width, height)
}

/// Rotate image 270 degrees clockwise.
fn rotate_270(data: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    let new_width = height;
    let new_height = width;
    let mut rotated = vec![0u8; (new_width * new_height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let src_idx = ((y * width + x) * 4) as usize;
            // For 270° rotation: new_x = y, new_y = width - 1 - x
            let new_x = y;
            let new_y = width - 1 - x;
            let dst_idx = ((new_y * new_width + new_x) * 4) as usize;

            rotated[dst_idx..dst_idx + 4].copy_from_slice(&data[src_idx..src_idx + 4]);
        }
    }

    (rotated, new_width, new_height)
}

/// Flip image horizontally.
fn flip_horizontal(data: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    let mut flipped = vec![0u8; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let src_idx = ((y * width + x) * 4) as usize;
            let new_x = width - 1 - x;
            let dst_idx = ((y * width + new_x) * 4) as usize;

            flipped[dst_idx..dst_idx + 4].copy_from_slice(&data[src_idx..src_idx + 4]);
        }
    }

    (flipped, width, height)
}

/// Flip image vertically.
fn flip_vertical(data: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    let mut flipped = vec![0u8; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let src_idx = ((y * width + x) * 4) as usize;
            let new_y = height - 1 - y;
            let dst_idx = ((new_y * width + x) * 4) as usize;

            flipped[dst_idx..dst_idx + 4].copy_from_slice(&data[src_idx..src_idx + 4]);
        }
    }

    (flipped, width, height)
}

/// Guess logical geometry from physical geometry when xdg_output is not available.
fn guess_output_logical_geometry(info: &mut OutputInfo) {
    info.logical_x = info.x;
    info.logical_y = info.y;
    info.logical_width = info.width / info.scale;
    info.logical_height = info.height / info.scale;

    apply_output_transform(info.transform, &mut info.logical_width, &mut info.logical_height);
    info.logical_scale_known = true;
}

fn blit_capture(
    dest: &mut [u8],
    dest_width: usize,
    dest_height: usize,
    capture: &CaptureResult,
    offset_x: usize,
    offset_y: usize
) {
    let src_width = capture.width as usize;
    let src_height = capture.height as usize;
    if src_width == 0 || src_height == 0 {
        return;
    }
    if offset_x >= dest_width || offset_y >= dest_height {
        return;
    }

    let copy_width = src_width.min(dest_width.saturating_sub(offset_x));
    let copy_height = src_height.min(dest_height.saturating_sub(offset_y));
    if copy_width == 0 || copy_height == 0 {
        return;
    }

    let dest_stride = dest_width * 4;
    let src_stride = src_width * 4;
    let row_bytes = copy_width * 4;

    for row in 0..copy_height {
        let dest_index = (offset_y + row) * dest_stride + offset_x * 4;
        let src_index = row * src_stride;
        dest[dest_index..dest_index + row_bytes].copy_from_slice(
            &capture.data[src_index..src_index + row_bytes]
        );
    }
}

/// Check if outputs have overlapping regions.
/// Returns true if any two outputs overlap.
fn check_outputs_overlap(outputs: &[(WlOutput, OutputInfo)]) -> bool {
    for i in 0..outputs.len() {
        let box1 = Box::new(
            outputs[i].1.x,
            outputs[i].1.y,
            outputs[i].1.width,
            outputs[i].1.height
        );

        for j in i + 1..outputs.len() {
            let box2 = Box::new(
                outputs[j].1.x,
                outputs[j].1.y,
                outputs[j].1.width,
                outputs[j].1.height
            );

            if box1.intersects(&box2) {
                return true;
            }
        }
    }

    false
}

/// Check if the layout is grid-aligned (outputs are pixel-aligned and don't overlap).
/// Grid-aligned layouts can use faster SRC compositing instead of OVER.
fn is_grid_aligned(region: &Box, outputs: &[(WlOutput, OutputInfo)]) -> bool {
    // Check if there are any overlapping outputs
    if check_outputs_overlap(outputs) {
        return false;
    }

    // Check if all output boundaries within the region are pixel-aligned
    // In our case, since coordinates are already i32, they are pixel-aligned by definition
    // We just need to verify that outputs that intersect the region are properly aligned
    for (_, info) in outputs {
        let output_box = Box::new(info.x, info.y, info.width, info.height);
        if output_box.intersects(region) {
            // All our coordinates are integers, so they're automatically grid-aligned
            // The main check is just ensuring no overlaps (done above)
            continue;
        }
    }

    true
}

#[derive(Clone)]
struct OutputInfo {
    name: String,
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    scale: i32,
    transform: wayland_client::protocol::wl_output::Transform, // Output transformation
    logical_x: i32, // From xdg_output protocol
    logical_y: i32, // From xdg_output protocol
    logical_width: i32, // From xdg_output protocol
    logical_height: i32, // From xdg_output protocol
    logical_scale_known: bool, // Whether we have xdg_output info
}

struct WaylandGlobals {
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
    screencopy_manager: Option<ZwlrScreencopyManagerV1>,
    xdg_output_manager: Option<ZxdgOutputManagerV1>,
    outputs: Vec<WlOutput>,
    output_info: HashMap<u32, OutputInfo>,
    output_xdg_map: HashMap<u32, ZxdgOutputV1>, // Map from wl_output id to xdg_output
}

pub struct WaylandCapture {
    _connection: Connection,
    globals: WaylandGlobals,
}

impl WaylandCapture {
    pub fn new() -> Result<Self> {
        let connection = Connection::connect_to_env().map_err(|e|
            Error::WaylandConnection(format!("Failed to connect to Wayland: {}", e))
        )?;
        let globals = WaylandGlobals {
            compositor: None,
            shm: None,
            screencopy_manager: None,
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
        };
        event_queue
            .roundtrip(&mut instance)
            .map_err(|e| {
                Error::WaylandConnection(format!("Failed to initialize Wayland globals: {}", e))
            })?;
        if instance.globals.screencopy_manager.is_none() {
            return Err(
                Error::UnsupportedProtocol("zwlr_screencopy_manager_v1 not available".to_string())
            );
        }
        if instance.globals.shm.is_none() {
            return Err(Error::UnsupportedProtocol("wl_shm not available".to_string()));
        }
        Ok(instance)
    }

    fn refresh_outputs(&mut self) -> Result<()> {
        // Clear existing outputs to re-bind them
        self.globals.outputs.clear();
        self.globals.output_info.clear();
        self.globals.output_xdg_map.clear();

        let mut event_queue = self._connection.new_event_queue();
        let qh = event_queue.handle();

        // Re-get registry to bind outputs with the current queue
        let _registry = self._connection.display().get_registry(&qh, ());

        // First roundtrip to get the registry globals
        event_queue
            .roundtrip(self)
            .map_err(|e| {
                Error::WaylandConnection(format!("Failed to refresh Wayland globals: {}", e))
            })?;
        if self.globals.output_info.is_empty() {
            return Err(Error::NoOutputs);
        }

        // Second and third roundtrips to ensure all output events are processed
        // Some compositors need multiple roundtrips for Mode/Geometry events
        for _ in 0..2 {
            event_queue
                .roundtrip(self)
                .map_err(|e| {
                    Error::WaylandConnection(format!("Failed to process output events: {}", e))
                })?;
        }

        // If xdg_output_manager is not available, guess logical geometry from physical
        if self.globals.xdg_output_manager.is_none() {
            for info in self.globals.output_info.values_mut() {
                if !info.logical_scale_known {
                    guess_output_logical_geometry(info);
                }
            }
        }

        Ok(())
    }

    fn collect_outputs_snapshot(&self) -> Vec<(WlOutput, OutputInfo)> {
        self.globals.outputs
            .iter()
            .filter_map(|output| {
                let id = output.id().protocol_id();
                self.globals.output_info
                    .get(&id)
                    .cloned()
                    .map(|info| (output.clone(), info))
            })
            .collect()
    }

    fn capture_region_for_output(
        &mut self,
        output: &WlOutput,
        region: Box,
        overlay_cursor: bool
    ) -> Result<CaptureResult> {
        if region.width <= 0 || region.height <= 0 {
            return Err(
                Error::InvalidRegion(
                    "Capture region must have positive width and height".to_string()
                )
            );
        }
        if region.x < 0 || region.y < 0 {
            return Err(
                Error::InvalidRegion("Capture region origin must be non-negative".to_string())
            );
        }

        let screencopy_manager = self.globals.screencopy_manager
            .as_ref()
            .ok_or(
                Error::UnsupportedProtocol("zwlr_screencopy_manager_v1 not available".to_string())
            )?;
        let mut event_queue = self._connection.new_event_queue();
        let qh = event_queue.handle();
        let frame_state = Arc::new(
            Mutex::new(FrameState {
                buffer: None,
                width: 0,
                height: 0,
                format: None,
                ready: false,
                flags: 0,
            })
        );
        let frame = screencopy_manager.capture_output_region(
            if overlay_cursor {
                1
            } else {
                0
            },
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
                        return Err(
                            Error::FrameCapture(
                                "Frame is ready but buffer was not received".to_string()
                            )
                        );
                    }
                    break;
                }
            }
            if attempts >= MAX_ATTEMPTS {
                return Err(Error::FrameCapture("Timeout waiting for frame buffer".to_string()));
            }
            event_queue
                .blocking_dispatch(self)
                .map_err(|e| {
                    Error::FrameCapture(format!("Failed to dispatch frame events: {}", e))
                })?;
            attempts += 1;
        }

        // Now we can safely access shm after the dispatch loop
        let shm = self.globals.shm
            .as_ref()
            .ok_or_else(|| Error::UnsupportedProtocol("wl_shm not available".to_string()))?;

        let (width, height, stride, size, format) = {
            let state = frame_state.lock().unwrap();
            if state.width == 0 || state.height == 0 {
                return Err(Error::CaptureFailed);
            }
            let width = state.width;
            let height = state.height;
            let stride = width * 4;
            let size = (stride * height) as usize;
            let format = state.format.unwrap_or(ShmFormat::Xrgb8888);
            (width, height, stride, size, format)
        };

        let mut tmp_file = tempfile::NamedTempFile::new().map_err(|_e| Error::BufferCreation)?;
        tmp_file
            .as_file_mut()
            .set_len(size as u64)
            .map_err(|_e| Error::BufferCreation)?;
        let mmap = unsafe {
            memmap2::MmapMut::map_mut(&tmp_file).map_err(|_e| Error::BufferCreation)?
        };

        let pool = shm.create_pool(
            unsafe {
                BorrowedFd::borrow_raw(tmp_file.as_file().as_raw_fd())
            },
            size as i32,
            &qh,
            ()
        );
        let buffer = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride as i32,
            format,
            &qh,
            ()
        );
        frame.copy(&buffer);

        let mut attempts = 0;
        loop {
            {
                let state = frame_state.lock().unwrap();
                if state.ready {
                    if state.buffer.is_none() {
                        return Err(
                            Error::FrameCapture(
                                "Frame is ready but buffer was not received".to_string()
                            )
                        );
                    }
                    break;
                }
            }
            if attempts >= MAX_ATTEMPTS {
                return Err(
                    Error::FrameCapture("Timeout waiting for frame capture completion".to_string())
                );
            }
            event_queue
                .blocking_dispatch(self)
                .map_err(|e| {
                    Error::FrameCapture(format!("Failed to dispatch frame events: {}", e))
                })?;
            attempts += 1;
        }

        let mut buffer_data = mmap.to_vec();
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

        // Apply output transform if needed
        let output_id = output.id().protocol_id();
        let mut final_data = buffer_data;
        let mut final_width = width;
        let mut final_height = height;

        if let Some(info) = self.globals.output_info.get(&output_id) {
            if !matches!(info.transform, wayland_client::protocol::wl_output::Transform::Normal) {
                let (transformed_data, new_width, new_height) = apply_image_transform(
                    &final_data,
                    final_width,
                    final_height,
                    info.transform
                );
                final_data = transformed_data;
                final_width = new_width;
                final_height = new_height;
            }
        }

        // Apply Y-invert if the flag is set
        let flags = {
            let state = frame_state.lock().unwrap();
            state.flags
        };

        if (flags & ZWLR_SCREENCOPY_FRAME_V1_FLAGS_Y_INVERT) != 0 {
            let (inverted_data, inv_width, inv_height) = flip_vertical(
                &final_data,
                final_width,
                final_height
            );
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

    fn composite_region(
        &mut self,
        region: Box,
        outputs: &[(WlOutput, OutputInfo)],
        overlay_cursor: bool
    ) -> Result<CaptureResult> {
        if region.width <= 0 || region.height <= 0 {
            return Err(
                Error::InvalidRegion(
                    "Capture region must have positive width and height".to_string()
                )
            );
        }

        let dest_width = region.width as usize;
        let dest_height = region.height as usize;
        let mut dest = vec![0u8; dest_width * dest_height * 4];
        let mut any_capture = false;

        // Check if the layout is grid-aligned (no overlaps, pixel-aligned boundaries)
        // Grid-aligned layouts allow for optimized SRC-mode compositing
        let _grid_aligned = is_grid_aligned(&region, outputs);

        // Note: When grid_aligned is true, we know outputs don't overlap, which means:
        // 1. Each pixel in the destination is written exactly once (no blending needed)
        // 2. We can use direct copy (SRC mode) instead of OVER mode
        // 3. This is automatically achieved by our current blit_capture implementation
        //    which does direct memory copy without alpha blending
        // The main benefit is that we can skip overlap checks in the future or
        // potentially parallelize captures (future optimization)

        for (output, info) in outputs {
            let output_box = Box::new(info.x, info.y, info.width, info.height);
            if let Some(intersection) = output_box.intersection(&region) {
                if intersection.width <= 0 || intersection.height <= 0 {
                    continue;
                }

                let local_region = Box::new(
                    intersection.x - info.x,
                    intersection.y - info.y,
                    intersection.width,
                    intersection.height
                );

                let output_handle = output.clone();
                let capture = self.capture_region_for_output(
                    &output_handle,
                    local_region,
                    overlay_cursor
                )?;

                let offset_x = (intersection.x - region.x) as usize;
                let offset_y = (intersection.y - region.y) as usize;

                // For grid-aligned layouts, this is SRC-mode copy (no blending)
                // For overlapping layouts, this would need alpha blending (OVER mode)
                // Currently we always use direct copy, which is correct for grid-aligned
                // and acceptable for most non-grid-aligned cases (last write wins)
                blit_capture(&mut dest, dest_width, dest_height, &capture, offset_x, offset_y);
                any_capture = true;
            }
        }

        if !any_capture {
            return Err(
                Error::InvalidRegion(
                    "Capture region does not intersect with any output".to_string()
                )
            );
        }

        Ok(CaptureResult {
            data: dest,
            width: region.width as u32,
            height: region.height as u32,
        })
    }

    pub fn get_outputs(&mut self) -> Result<Vec<Output>> {
        self.refresh_outputs()?;
        let snapshot = self.collect_outputs_snapshot();
        let outputs = snapshot
            .into_iter()
            .map(|(_, info)| {
                // Use logical geometry if xdg_output is available, otherwise use physical
                let (x, y, width, height) = if info.logical_scale_known {
                    (info.logical_x, info.logical_y, info.logical_width, info.logical_height)
                } else {
                    (info.x, info.y, info.width, info.height)
                };

                Output {
                    name: info.name.clone(),
                    geometry: Box::new(x, y, width, height),
                    scale: info.scale,
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
        let mut min_x = first_info.x;
        let mut min_y = first_info.y;
        let mut max_x = first_info.x + first_info.width;
        let mut max_y = first_info.y + first_info.height;

        for (_, info) in &snapshot {
            min_x = min_x.min(info.x);
            min_y = min_y.min(info.y);
            max_x = max_x.max(info.x + info.width);
            max_y = max_y.max(info.y + info.height);
        }

        let region = Box::new(min_x, min_y, max_x - min_x, max_y - min_y);
        self.composite_region(region, &snapshot, false)
    }

    pub fn capture_all_with_scale(&mut self, scale: f64) -> Result<CaptureResult> {
        self.refresh_outputs()?;
        let snapshot = self.collect_outputs_snapshot();
        if snapshot.is_empty() {
            return Err(Error::NoOutputs);
        }

        let (_, first_info) = &snapshot[0];
        let mut min_x = first_info.x;
        let mut min_y = first_info.y;
        let mut max_x = first_info.x + first_info.width;
        let mut max_y = first_info.y + first_info.height;

        for (_, info) in &snapshot {
            min_x = min_x.min(info.x);
            min_y = min_y.min(info.y);
            max_x = max_x.max(info.x + info.width);
            max_y = max_y.max(info.y + info.height);
        }

        // Calculate scaled dimensions
        let scaled_width = (((max_x - min_x) as f64) * scale) as i32;
        let scaled_height = (((max_y - min_y) as f64) * scale) as i32;
        let _scaled_region = Box::new(min_x, min_y, scaled_width, scaled_height);

        // For now, we'll composite at original size and then scale the final result
        // This is a simplified approach - in a full implementation, we'd scale during capture
        let original_result = self.composite_region(
            Box::new(min_x, min_y, max_x - min_x, max_y - min_y),
            &snapshot,
            false
        )?;

        // Scale the result
        self.scale_image_data(original_result, scale)
    }

    pub fn capture_output(&mut self, output_name: &str) -> Result<CaptureResult> {
        self.refresh_outputs()?;
        let snapshot = self.collect_outputs_snapshot();
        let (output_handle, info) = snapshot
            .into_iter()
            .find(|(_, info)| info.name == output_name)
            .ok_or_else(|| Error::OutputNotFound(output_name.to_string()))?;

        let local_region = Box::new(0, 0, info.width, info.height);
        self.capture_region_for_output(&output_handle, local_region, false)
    }

    pub fn capture_output_with_scale(
        &mut self,
        output_name: &str,
        scale: f64
    ) -> Result<CaptureResult> {
        self.refresh_outputs()?;
        let snapshot = self.collect_outputs_snapshot();
        let (output_handle, info) = snapshot
            .into_iter()
            .find(|(_, info)| info.name == output_name)
            .ok_or_else(|| Error::OutputNotFound(output_name.to_string()))?;

        let local_region = Box::new(0, 0, info.width, info.height);
        let result = self.capture_region_for_output(&output_handle, local_region, false)?;
        self.scale_image_data(result, scale)
    }

    pub fn capture_region(&mut self, region: Box) -> Result<CaptureResult> {
        self.refresh_outputs()?;
        let snapshot = self.collect_outputs_snapshot();
        self.composite_region(region, &snapshot, false)
    }

    pub fn capture_region_with_scale(&mut self, region: Box, scale: f64) -> Result<CaptureResult> {
        let result = self.capture_region(region)?;
        self.scale_image_data(result, scale)
    }

    fn scale_image_data(&self, capture_result: CaptureResult, scale: f64) -> Result<CaptureResult> {
        if scale == 1.0 {
            return Ok(capture_result);
        }

        let old_width = capture_result.width;
        let old_height = capture_result.height;
        let new_width = ((old_width as f64) * scale) as u32;
        let new_height = ((old_height as f64) * scale) as u32;

        if new_width == 0 || new_height == 0 {
            return Err(Error::InvalidRegion("Scaled dimensions must be positive".to_string()));
        }

        // Use image crate for high-quality scaling
        use image::{ ImageBuffer, Rgba, imageops };

        // Convert raw RGBA data to ImageBuffer
        let img = ImageBuffer::<Rgba<u8>, Vec<u8>>
            ::from_raw(old_width, old_height, capture_result.data)
            .ok_or_else(||
                Error::Io(
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Failed to create image buffer"
                    )
                )
            )?;

        // Choose filter based on scale factor:
        // - Lanczos3 for significant downscaling (scale < 0.75) for better quality
        // - Triangle (bilinear) for other cases (faster, good quality)
        let filter = if scale < 0.75 {
            imageops::FilterType::Lanczos3
        } else {
            imageops::FilterType::Triangle // Bilinear interpolation
        };

        // Resize with chosen filter
        let scaled_img = imageops::resize(&img, new_width, new_height, filter);

        Ok(CaptureResult {
            data: scaled_img.into_raw(),
            width: new_width,
            height: new_height,
        })
    }

    pub fn capture_outputs(
        &mut self,
        parameters: Vec<CaptureParameters>
    ) -> Result<MultiOutputCaptureResult> {
        let output = self.globals.outputs.first().ok_or_else(|| Error::NoOutputs)?;
        let screencopy_manager = self.globals.screencopy_manager
            .as_ref()
            .ok_or(
                Error::UnsupportedProtocol("zwlr_screencopy_manager_v1 not available".to_string())
            )?;
        let mut event_queue = self._connection.new_event_queue();
        let qh = event_queue.handle();
        let mut frame_states: HashMap<String, Arc<Mutex<FrameState>>> = HashMap::new();
        let mut frames: HashMap<String, ZwlrScreencopyFrameV1> = HashMap::new();
        for param in &parameters {
            let output_info = self.globals.output_info
                .iter()
                .find(|(_, info)| info.name == param.output_name)
                .map(|(_, info)| info)
                .ok_or_else(|| Error::OutputNotFound(param.output_name.clone()))?;
            let region = if let Some(region) = param.region {
                let output_right = output_info.x + output_info.width;
                let output_bottom = output_info.y + output_info.height;
                if
                    region.x < output_info.x ||
                    region.y < output_info.y ||
                    region.x + region.width > output_right ||
                    region.y + region.height > output_bottom
                {
                    return Err(
                        Error::InvalidRegion(
                            "Capture region extends outside output boundaries".to_string()
                        )
                    );
                }
                region
            } else {
                Box::new(output_info.x, output_info.y, output_info.width, output_info.height)
            };
            let frame_state = Arc::new(
                Mutex::new(FrameState {
                    buffer: None,
                    width: 0,
                    height: 0,
                    format: None,
                    ready: false,
                    flags: 0,
                })
            );
            let frame = screencopy_manager.capture_output_region(
                if param.overlay_cursor {
                    1
                } else {
                    0
                },
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
            completed_frames = frame_states
                .iter()
                .filter(|(_, state)| {
                    let s = state.lock().unwrap();
                    s.buffer.is_some() || s.ready
                })
                .count();
            if completed_frames >= total_frames {
                break;
            }
            event_queue
                .blocking_dispatch(self)
                .map_err(|e|
                    Error::FrameCapture(format!("Failed to dispatch frame events: {}", e))
                )?;
            attempts += 1;
        }
        if attempts >= MAX_ATTEMPTS {
            return Err(Error::FrameCapture("Timeout waiting for frame buffers".to_string()));
        }
        for frame_state in frame_states.values() {
            let state = frame_state.lock().unwrap();
            if state.buffer.is_none() {
                return Err(Error::CaptureFailed);
            }
        }
        let mut buffers: HashMap<
            String,
            (tempfile::NamedTempFile, memmap2::MmapMut)
        > = HashMap::new();
        for (output_name, frame_state) in &frame_states {
            let (width, height, stride, size) = {
                let state = frame_state.lock().unwrap();
                let width = state.width;
                let height = state.height;
                let stride = width * 4;
                let size = (stride * height) as usize;
                (width, height, stride, size)
            };
            let mut tmp_file = tempfile::NamedTempFile::new().map_err(|_e| Error::BufferCreation)?;
            tmp_file
                .as_file_mut()
                .set_len(size as u64)
                .map_err(|_e| Error::BufferCreation)?;
            let mmap = unsafe {
                memmap2::MmapMut::map_mut(&tmp_file).map_err(|_e| Error::BufferCreation)?
            };
            // Access shm after event dispatch to avoid borrowing conflicts
            let shm = self.globals.shm
                .as_ref()
                .ok_or(Error::UnsupportedProtocol("wl_shm not available".to_string()))?;
            {
                let format = {
                    let state = frame_state.lock().unwrap();
                    state.format.unwrap_or(ShmFormat::Xrgb8888)
                };
                let pool = shm.create_pool(
                    unsafe {
                        BorrowedFd::borrow_raw(tmp_file.as_file().as_raw_fd())
                    },
                    size as i32,
                    &qh,
                    ()
                );
                let buffer = pool.create_buffer(
                    0,
                    width as i32,
                    height as i32,
                    stride as i32,
                    format,
                    &qh,
                    ()
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
                .filter(|(_, state)| {
                    let s = state.lock().unwrap();
                    s.ready
                })
                .count();
            if completed_frames >= total_frames {
                break;
            }
            event_queue
                .blocking_dispatch(self)
                .map_err(|e|
                    Error::FrameCapture(format!("Failed to dispatch frame events: {}", e))
                )?;
            attempts += 1;
        }
        if attempts >= MAX_ATTEMPTS {
            return Err(
                Error::FrameCapture("Timeout waiting for frame capture completion".to_string())
            );
        }
        for frame_state in frame_states.values() {
            let state = frame_state.lock().unwrap();
            if state.ready && state.buffer.is_none() {
                return Err(
                    Error::FrameCapture("Frame is ready but buffer was not received".to_string())
                );
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
            if
                let Some(format) = ({
                    let state = frame_state.lock().unwrap();
                    state.format
                })
            {
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

    pub fn capture_outputs_with_scale(
        &mut self,
        parameters: Vec<CaptureParameters>,
        default_scale: f64
    ) -> Result<MultiOutputCaptureResult> {
        // For now, we'll scale each result after capture
        let result = self.capture_outputs(parameters)?;
        let mut scaled_results = std::collections::HashMap::new();

        for (output_name, capture_result) in result.outputs {
            // Find the scale for this output from parameters
            let scale = default_scale; // In a full implementation, we'd look up the scale from parameters
            let scaled_result = self.scale_image_data(capture_result, scale)?;
            scaled_results.insert(output_name, scaled_result);
        }

        Ok(MultiOutputCaptureResult {
            outputs: scaled_results,
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
        qh: &QueueHandle<Self>
    ) {
        use wayland_client::protocol::wl_registry::Event;
        if let Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "wl_compositor" => {
                    state.globals.compositor = Some(
                        registry.bind::<WlCompositor, _, _>(name, version, qh, ())
                    );
                }
                "wl_shm" => {
                    state.globals.shm = Some(registry.bind::<WlShm, _, _>(name, version, qh, ()));
                }
                "zwlr_screencopy_manager_v1" => {
                    state.globals.screencopy_manager = Some(
                        registry.bind::<ZwlrScreencopyManagerV1, _, _>(name, version, qh, ())
                    );
                }
                "zxdg_output_manager_v1" => {
                    // When we have the xdg_output_manager, create xdg_output objects for each existing output
                    state.globals.xdg_output_manager = Some(
                        registry.bind::<ZxdgOutputManagerV1, _, _>(name, version, qh, ())
                    );

                    // Create xdg_output for all existing outputs
                    for output in &state.globals.outputs {
                        let xdg_output = state.globals.xdg_output_manager
                            .as_ref()
                            .unwrap()
                            .get_xdg_output(output, qh, ());
                        let output_id = output.id().protocol_id();
                        state.globals.output_xdg_map.insert(output_id, xdg_output);
                    }
                }
                "wl_output" => {
                    let output = registry.bind::<WlOutput, _, _>(name, version, qh, ());
                    // Use output's protocol ID as the key (not registry name!)
                    let output_id = output.id().protocol_id();

                    // Добавляем информацию о выходе с временными значениями
                    // Реальные значения будут получены позже через события wl_output
                    state.globals.output_info.insert(output_id, OutputInfo {
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
                    });
                    // Get the index of the output we're about to add
                    let output_idx = state.globals.outputs.len();
                    state.globals.outputs.push(output.clone());

                    // If xdg_output_manager is already available, create xdg_output now
                    if let Some(ref xdg_output_manager) = state.globals.xdg_output_manager {
                        // Get the output from the vector to reference it
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
        _qh: &QueueHandle<Self>
    ) {
        use wayland_client::protocol::wl_output::{ Event };
        // Находим соответствующий OutputInfo для этого выхода
        // Мы используем ID объекта для сопоставления
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
                    // Store the transform value
                    if let wayland_client::WEnum::Value(t) = transform {
                        info.transform = t;
                    }
                    // Only update logical coordinates if we don't have xdg_output info
                    if !info.logical_scale_known {
                        info.logical_x = x;
                        info.logical_y = y;
                    }
                }
            }
            Event::Mode { flags: _, width, height, refresh: _ } => {
                log::debug!("Mode event for output_id {}: {}x{}", output_id, width, height);
                if let Some(info) = state.globals.output_info.get_mut(&output_id) {
                    info.width = width;
                    info.height = height;
                    log::debug!("Updated output info: {}x{}", info.width, info.height);
                    // Only update logical dimensions if we don't have xdg_output info
                    if !info.logical_scale_known {
                        info.logical_width = width;
                        info.logical_height = height;
                    }
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
        _qh: &QueueHandle<Self>
    ) {}
}

impl Dispatch<WlShm, ()> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _proxy: &WlShm,
        _event: <WlShm as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>
    ) {}
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrScreencopyManagerV1,
        _event: <ZwlrScreencopyManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>
    ) {}
}

impl Dispatch<ZwlrScreencopyFrameV1, Arc<Mutex<FrameState>>> for WaylandCapture {
    fn event(
        _state: &mut Self,
        frame: &ZwlrScreencopyFrameV1,
        event: <ZwlrScreencopyFrameV1 as Proxy>::Event,
        frame_state: &Arc<Mutex<FrameState>>,
        _conn: &Connection,
        _qh: &QueueHandle<Self>
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
                // Save flags for later processing (e.g., Y_INVERT)
                let mut state = frame_state.lock().unwrap();
                if let wayland_client::WEnum::Value(val) = flags {
                    state.flags = val.bits();
                    log::debug!("Received flags: {:?} (bits: {})", flags, val.bits());
                }
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
                log::debug!(
                    "Received LinuxDmabuf: format={}, width={}, height={}",
                    format,
                    width,
                    height
                );
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
        _qh: &QueueHandle<Self>
    ) {
        use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_v1::Event;

        // Find the corresponding WlOutput ID for this xdg_output
        let xdg_output_id = xdg_output.id().protocol_id();

        // Look for the wl_output that corresponds to this xdg_output
        // We can do this by finding the entry in the output_xdg_map
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
                    }
                    Event::LogicalSize { width, height } => {
                        info.logical_width = width;
                        info.logical_height = height;
                        info.logical_scale_known = true;
                    }
                    Event::Name { name } => {
                        // Use the xdg name if it's more specific than wl_output name
                        if info.name.starts_with("output-") || info.name.is_empty() {
                            info.name = name.clone();
                        }
                    }
                    Event::Description { description: _ } => {
                        // Could handle description if needed
                    }
                    Event::Done => {
                        // The xdg_output is done with sending information
                        info.logical_scale_known = true;
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
        _qh: &QueueHandle<Self>
    ) {}
}

impl Dispatch<WlShmPool, ()> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _proxy: &WlShmPool,
        _event: <WlShmPool as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>
    ) {}
}

impl Dispatch<ZxdgOutputManagerV1, ()> for WaylandCapture {
    fn event(
        _state: &mut Self,
        _proxy: &ZxdgOutputManagerV1,
        _event: <ZxdgOutputManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>
    ) {
        // The ZxdgOutputManagerV1 doesn't send events that we need to handle
        // So we just leave this empty
    }
}

#[derive(Debug, Clone)]
struct FrameState {
    buffer: Option<Vec<u8>>,
    width: u32,
    height: u32,
    format: Option<ShmFormat>,
    ready: bool,
    flags: u32,
}
