use std::collections::HashMap;

use crate::pixel_format::{self, PixelFormat};
use crate::{
    Backend, CaptureParameters, CaptureResult, Error, MultiOutputCaptureResult, Output, Region,
    Result,
};

use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput, IDXGIOutputDuplication,
    IDXGIResource, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
    DXGI_OUTDUPL_POINTER_POSITION, DXGI_OUTDUPL_POINTER_SHAPE_INFO,
    DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME, DXGI_OUTPUT_DESC,
};

const MAX_ACQUIRE_ATTEMPTS: usize = 100;

const ACQUIRE_TIMEOUT_MS: u32 = 100;

/// Read the active pointer shape from an acquired duplication frame.
/// Must be called between `AcquireNextFrame` and `ReleaseFrame`.
fn fetch_cursor_shape(dup: &IDXGIOutputDuplication, buf_size: u32) -> Option<CachedCursorShape> {
    if buf_size == 0 {
        return None;
    }
    let mut shape_info = DXGI_OUTDUPL_POINTER_SHAPE_INFO::default();
    let mut size: u32 = 0;
    let mut buf: Vec<u8> = vec![0u8; buf_size as usize];
    let result = unsafe {
        dup.GetFramePointerShape(
            buf_size,
            buf.as_mut_ptr() as *mut std::ffi::c_void,
            &mut size,
            &mut shape_info,
        )
    };
    if result.is_ok() && size > 0 && shape_info.Width > 0 && shape_info.Height > 0 {
        Some(CachedCursorShape {
            info: shape_info,
            buf,
        })
    } else {
        log::debug!("GetFramePointerShape failed ({result:?}); keeping previous cursor shape");
        None
    }
}

#[derive(Clone)]
struct OutputInfo {
    name: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    rotation: u32,
    description: String,
    adapter_idx: usize,
    output_idx: usize,
}

struct AdapterState {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
}

struct CachedCursorShape {
    info: DXGI_OUTDUPL_POINTER_SHAPE_INFO,
    buf: Vec<u8>,
}

struct DupEntry {
    dup: IDXGIOutputDuplication,
    cursor_shape: Option<CachedCursorShape>,
    // DXGI only sends PointerPosition on frames where the pointer moved
    // (LastMouseUpdateTime != 0); remember the last known position.
    pointer_position: Option<DXGI_OUTDUPL_POINTER_POSITION>,
}

pub struct WindowsCapture {
    adapters: Vec<AdapterState>,
    output_info: Vec<OutputInfo>,
    dup_cache: HashMap<String, DupEntry>,
    factory: IDXGIFactory1,
}

impl WindowsCapture {
    pub fn new(_preference: Backend) -> Result<Self> {
        let _ = _preference;
        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }
            .map_err(|e| Error::DirectXError(format!("CreateDXGIFactory1: {e}")))?;
        let mut adapters = Vec::new();
        let mut output_info = Vec::new();
        for adapter_idx in 0u32.. {
            let adapter: IDXGIAdapter1 = match unsafe { factory.EnumAdapters1(adapter_idx) } {
                Ok(a) => a,
                Err(_) => break,
            };
            let desc = unsafe { adapter.GetDesc1() }
                .map_err(|e| Error::DirectXError(format!("GetDesc1: {e}")))?;
            if (desc.Flags & 1) != 0 {
                continue;
            }
            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;
            let adapter_base: windows::Win32::Graphics::Dxgi::IDXGIAdapter =
                adapter.cast().map_err(|e| {
                    Error::DirectXError(format!("Cast IDXGIAdapter1 -> IDXGIAdapter: {e}"))
                })?;
            unsafe {
                D3D11CreateDevice(
                    Some(&adapter_base),
                    D3D_DRIVER_TYPE_UNKNOWN,
                    None,
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    None,
                    Some(&mut context),
                )
            }
            .map_err(|e| Error::DirectXError(format!("D3D11CreateDevice: {e}")))?;
            let device = device
                .ok_or_else(|| Error::DirectXError("D3D11CreateDevice: null device".to_string()))?;
            let context = context.ok_or_else(|| {
                Error::DirectXError("D3D11CreateDevice: null context".to_string())
            })?;
            let mut has_outputs = false;
            for output_idx in 0u32.. {
                let output: IDXGIOutput = match unsafe { adapter.EnumOutputs(output_idx) } {
                    Ok(o) => o,
                    Err(_) => break,
                };
                let desc: DXGI_OUTPUT_DESC = unsafe { output.GetDesc() }
                    .map_err(|e| Error::DirectXError(format!("GetDesc: {e}")))?;
                if desc.DesktopCoordinates.right == desc.DesktopCoordinates.left
                    || desc.DesktopCoordinates.bottom == desc.DesktopCoordinates.top
                {
                    continue;
                }
                let name = String::from_utf16_lossy(&desc.DeviceName)
                    .trim_end_matches('\0')
                    .to_string();
                let rot = desc.Rotation.0 as u32;
                let (w, h) = match rot {
                    2 | 4 => (
                        desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top,
                        desc.DesktopCoordinates.right - desc.DesktopCoordinates.left,
                    ),
                    _ => (
                        desc.DesktopCoordinates.right - desc.DesktopCoordinates.left,
                        desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top,
                    ),
                };
                output_info.push(OutputInfo {
                    name: name.clone(),
                    x: desc.DesktopCoordinates.left,
                    y: desc.DesktopCoordinates.top,
                    width: w,
                    height: h,
                    rotation: desc.Rotation.0 as u32,
                    description: name.clone(),
                    adapter_idx: adapters.len(),
                    output_idx: output_idx as usize,
                });
                has_outputs = true;
            }
            if has_outputs {
                adapters.push(AdapterState { device, context });
            }
        }
        if output_info.is_empty() {
            return Err(Error::NoOutputs);
        }
        Ok(Self {
            adapters,
            output_info,
            dup_cache: HashMap::new(),
            factory,
        })
    }

    fn get_dup(&mut self, info: &OutputInfo) -> Result<()> {
        if self.dup_cache.contains_key(&info.name) {
            return Ok(());
        }
        let adapter_state = &self.adapters[info.adapter_idx];
        let adapter: IDXGIAdapter1 = unsafe { self.factory.EnumAdapters1(info.adapter_idx as u32) }
            .map_err(|e| Error::DirectXError(format!("EnumAdapters1: {e}")))?;
        let output: IDXGIOutput = unsafe { adapter.EnumOutputs(info.output_idx as u32) }
            .map_err(|e| Error::DirectXError(format!("EnumOutputs: {e}")))?;
        let output1: windows::Win32::Graphics::Dxgi::IDXGIOutput1 = output
            .cast()
            .map_err(|e| Error::DirectXError(format!("QueryInterface IDXGIOutput1: {e}")))?;
        let dup = unsafe { output1.DuplicateOutput(&adapter_state.device) }.map_err(|e| {
            if e.code() == windows::Win32::Foundation::E_ACCESSDENIED {
                Error::ProtectedContent(format!("Output '{}' is protected by DRM", info.name))
            } else {
                Error::DirectXError(format!("DuplicateOutput: {e}"))
            }
        })?;
        // Skip the initial blank frame that DXGI returns after DuplicateOutput.
        // The first AcquireNextFrame may return WAIT_TIMEOUT, so retry until we
        // get a real frame to discard. The pointer shape is reported with this
        // initial frame (and on later frames only when it changes), so cache
        // it here - relying on per-frame PointerShapeBufferSize would leave
        // most captures without a cursor.
        let mut cursor_shape = None;
        let mut pointer_position = None;
        for _ in 0..MAX_ACQUIRE_ATTEMPTS {
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut dummy_resource: Option<IDXGIResource> = None;
            let result = unsafe {
                dup.AcquireNextFrame(
                    ACQUIRE_TIMEOUT_MS,
                    &mut frame_info,
                    std::ptr::addr_of_mut!(dummy_resource),
                )
            };
            if result.is_ok() {
                if cursor_shape.is_none() {
                    cursor_shape = fetch_cursor_shape(&dup, frame_info.PointerShapeBufferSize);
                    log::debug!(
                        "initial cursor shape: size={} cached={}",
                        frame_info.PointerShapeBufferSize,
                        cursor_shape.is_some()
                    );
                }
                if frame_info.LastMouseUpdateTime != 0 || pointer_position.is_none() {
                    pointer_position = Some(frame_info.PointerPosition);
                }
                unsafe {
                    let _ = dup.ReleaseFrame();
                };
                break;
            }
        }
        self.dup_cache.insert(
            info.name.clone(),
            DupEntry {
                dup,
                cursor_shape,
                pointer_position,
            },
        );
        Ok(())
    }

    fn capture_frame(
        &mut self,
        info: &OutputInfo,
        overlay_cursor: bool,
    ) -> Result<(Vec<u8>, u32, u32)> {
        let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut attempts = 0;
        loop {
            let entry = self.dup_cache.get_mut(&info.name).ok_or_else(|| {
                Error::FrameCapture(format!(
                    "no duplication session for output '{}'; call get_dup first",
                    info.name
                ))
            })?;
            let dup = &entry.dup;
            let mut desktop_resource: Option<IDXGIResource> = None;
            let result = unsafe {
                dup.AcquireNextFrame(
                    ACQUIRE_TIMEOUT_MS,
                    &mut frame_info,
                    std::ptr::addr_of_mut!(desktop_resource),
                )
            };
            match result {
                Ok(()) => {
                    let resource = match desktop_resource {
                        Some(r) => r,
                        None => {
                            unsafe { dup.ReleaseFrame() }
                                .map_err(|e| Error::DirectXError(format!("ReleaseFrame: {e}")))?;
                            attempts += 1;
                            if attempts >= MAX_ACQUIRE_ATTEMPTS {
                                return Err(Error::FrameCapture(
                                    "Timeout waiting for non-empty frame".to_string(),
                                ));
                            }
                            continue;
                        }
                    };
                    let adapter = &self.adapters[info.adapter_idx];
                    let texture: ID3D11Texture2D = resource.cast().map_err(|e| {
                        Error::DirectXError(format!("Cast IDXGIResource -> ID3D11Texture2D: {e}"))
                    })?;
                    let mut tex_desc = D3D11_TEXTURE2D_DESC::default();
                    unsafe { texture.GetDesc(&mut tex_desc) };
                    let width = tex_desc.Width;
                    let height = tex_desc.Height;
                    let staging_desc = D3D11_TEXTURE2D_DESC {
                        Width: width,
                        Height: height,
                        MipLevels: 1,
                        ArraySize: 1,
                        Format: tex_desc.Format,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Usage: D3D11_USAGE_STAGING,
                        BindFlags: 0,
                        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                        MiscFlags: 0,
                    };
                    let staging: ID3D11Texture2D = {
                        let mut tex: Option<ID3D11Texture2D> = None;
                        unsafe {
                            adapter
                                .device
                                .CreateTexture2D(&staging_desc, None, Some(&mut tex))
                        }
                        .map_err(|e| {
                            Error::BufferCreation(format!("CreateTexture2D (staging): {e}"))
                        })?;
                        tex.ok_or_else(|| {
                            Error::BufferCreation("CreateTexture2D returned null".to_string())
                        })?
                    };
                    unsafe { adapter.context.CopyResource(&staging, &texture) };
                    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                    unsafe {
                        adapter.context.Map(
                            &staging,
                            0,
                            D3D11_MAP_READ,
                            0,
                            Some(std::ptr::addr_of_mut!(mapped)),
                        )
                    }
                    .map_err(|e| Error::DirectXError(format!("Map: {e}")))?;
                    let row_pitch = mapped.RowPitch as usize;
                    let data = unsafe {
                        std::slice::from_raw_parts(
                            mapped.pData as *const u8,
                            row_pitch * height as usize,
                        )
                    };
                    let pixel_bytes = (width as usize) * 4;
                    let mut pixels = Vec::with_capacity(pixel_bytes * height as usize);
                    for row in 0..height as usize {
                        let src_start = row * row_pitch;
                        pixels.extend_from_slice(&data[src_start..src_start + pixel_bytes]);
                    }
                    unsafe { adapter.context.Unmap(&staging, 0) };
                    // The shape buffer is only filled when the shape changed;
                    // refresh the cached shape before releasing the frame.
                    if overlay_cursor && frame_info.PointerShapeBufferSize > 0 {
                        if let Some(shape) =
                            fetch_cursor_shape(dup, frame_info.PointerShapeBufferSize)
                        {
                            entry.cursor_shape = Some(shape);
                        }
                    }
                    if frame_info.LastMouseUpdateTime != 0 || entry.pointer_position.is_none() {
                        entry.pointer_position = Some(frame_info.PointerPosition);
                    }
                    unsafe { dup.ReleaseFrame() }
                        .map_err(|e| Error::DirectXError(format!("ReleaseFrame: {e}")))?;
                    pixel_format::convert_to_rgba(&mut pixels, PixelFormat::Argb8888);
                    // The shape comes from the cache because DXGI only resends
                    // it when it changes; the position is the last one DXGI
                    // reported (frames without mouse movement carry no update).
                    if overlay_cursor {
                        let pointer = entry.pointer_position;
                        log::debug!(
                            "blending cursor: shape_cached={} pointer={:?}",
                            entry.cursor_shape.is_some(),
                            pointer.map(|p| (p.Position, p.Visible.as_bool()))
                        );
                        if let Some(pointer) = pointer.filter(|p| p.Visible.as_bool()) {
                            if let Some(CachedCursorShape { info, buf }) = &entry.cursor_shape {
                                let screen_x = pointer.Position.x - info.HotSpot.x;
                                let screen_y = pointer.Position.y - info.HotSpot.y;
                                if info.Type == DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME.0 as u32
                                {
                                    // A monochromatic shape packs two 1bpp masks;
                                    // Height counts both, so the visible height is
                                    // half of it.
                                    blend_mono_cursor_rgba(
                                        &mut pixels,
                                        (width, height),
                                        buf,
                                        (info.Width, info.Height / 2),
                                        info.Pitch as usize,
                                        (screen_x, screen_y),
                                    );
                                } else {
                                    // COLOR shapes are 32-bit BGRA. MASKED_COLOR
                                    // stores the color bitmap first with the same
                                    // pitch, so blending it as color is safe (the
                                    // AND mask tail is simply never read).
                                    blend_cursor_rgba(
                                        &mut pixels,
                                        (width, height),
                                        buf,
                                        (info.Width, info.Height),
                                        info.Pitch as usize,
                                        (screen_x, screen_y),
                                    );
                                }
                            }
                        }
                    }
                    return Ok((pixels, width, height));
                }
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                    attempts += 1;
                    if attempts >= MAX_ACQUIRE_ATTEMPTS {
                        return Err(Error::FrameCapture(
                            "Timeout waiting for frame update".to_string(),
                        ));
                    }
                }
                Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => {
                    self.dup_cache.remove(&info.name);
                    attempts += 1;
                    if attempts >= MAX_ACQUIRE_ATTEMPTS {
                        return Err(Error::FrameCapture(
                            "Output mode changed, retry capture".to_string(),
                        ));
                    }
                    self.get_dup(info)?;
                    continue;
                }
                Err(e) => {
                    return Err(Error::DirectXError(format!("AcquireNextFrame: {e}")));
                }
            }
        }
    }

    fn find_output(&self, name: &str) -> Result<OutputInfo> {
        self.output_info
            .iter()
            .find(|o| o.name == name)
            .cloned()
            .ok_or_else(|| Error::OutputNotFound(name.to_string()))
    }

    pub fn get_outputs(&mut self) -> Result<Vec<Output>> {
        Ok(self
            .output_info
            .iter()
            .map(|info| Output {
                name: info.name.clone(),
                geometry: Region::new(info.x, info.y, info.width, info.height),
                scale: 1,
                description: if info.description.is_empty() {
                    None
                } else {
                    Some(info.description.clone())
                },
            })
            .collect())
    }

    pub fn capture_output_raw(
        &mut self,
        output_name: &str,
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
        let info = self.find_output(output_name)?;
        self.get_dup(&info)?;
        let (full_pixels, full_width, full_height) = self.capture_frame(&info, overlay_cursor)?;
        // Apply rotation transform FIRST — the DXGI texture is in GPU-native
        // orientation, but the crop region is in logical (post-rotation) coords.
        use crate::transform::{apply_image_transform, OutputTransform};
        let transform = match info.rotation {
            2 => OutputTransform::Rotate90,
            3 => OutputTransform::Rotate180,
            4 => OutputTransform::Rotate270,
            _ => OutputTransform::Normal,
        };
        let (mut final_data, mut final_width, mut final_height) =
            apply_image_transform(&full_pixels, full_width, full_height, transform);
        // Crop to the requested region (now in logical coords matching the image)
        let crop_x = region.x() as u32;
        let crop_y = region.y() as u32;
        let crop_w = region.width() as u32;
        let crop_h = region.height() as u32;
        if crop_x > 0 || crop_y > 0 || crop_w != final_width || crop_h != final_height {
            let (cropped, cw, ch) = crate::compositor::crop_rgba(
                &final_data,
                final_width,
                final_height,
                crop_x as usize,
                crop_y as usize,
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

    pub fn capture_all(&mut self) -> Result<CaptureResult> {
        let outputs: Vec<(String, Region)> = self
            .output_info
            .iter()
            .map(|o| (o.name.clone(), Region::new(o.x, o.y, o.width, o.height)))
            .collect();
        if outputs.is_empty() {
            return Err(Error::NoOutputs);
        }
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        for (_, geom) in &outputs {
            min_x = min_x.min(geom.x());
            min_y = min_y.min(geom.y());
            max_x = max_x.max(geom.x() + geom.width());
            max_y = max_y.max(geom.y() + geom.height());
        }
        let region = Region::new(min_x, min_y, max_x - min_x, max_y - min_y);
        crate::compositor::composite_region(
            region,
            &outputs,
            false,
            |name, local_region, cursor| self.capture_output_raw(name, local_region, cursor),
        )
    }

    pub fn capture_all_with_scale(&mut self, scale: f64) -> Result<CaptureResult> {
        let result = self.capture_all()?;
        crate::scaling::scale_image_data(result, scale)
    }

    pub fn capture_output(&mut self, output_name: &str) -> Result<CaptureResult> {
        let info = self.find_output(output_name)?;
        let region = Region::new(0, 0, info.width, info.height);
        self.capture_output_raw(output_name, region, false)
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
        let outputs: Vec<(String, Region)> = self
            .output_info
            .iter()
            .map(|o| (o.name.clone(), Region::new(o.x, o.y, o.width, o.height)))
            .collect();
        crate::compositor::composite_region(
            region,
            &outputs,
            false,
            |name, local_region, cursor| self.capture_output_raw(name, local_region, cursor),
        )
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
        let mut results = HashMap::new();
        for param in &parameters {
            let output_name = param.output_name().to_string();
            let region = if let Some(r) = param.region_ref() {
                *r
            } else {
                let info = self.find_output(&output_name)?;
                Region::new(0, 0, info.width, info.height)
            };
            let capture =
                self.capture_output_raw(&output_name, region, param.overlay_cursor_enabled())?;
            results.insert(output_name, capture);
        }
        Ok(MultiOutputCaptureResult::new(results))
    }

    pub fn capture_outputs_with_scale(
        &mut self,
        parameters: Vec<CaptureParameters>,
        default_scale: f64,
    ) -> Result<MultiOutputCaptureResult> {
        let result = self.capture_outputs(parameters)?;
        let mut scaled_results = HashMap::new();
        for (output_name, capture_result) in result.into_outputs() {
            let scaled = crate::scaling::scale_image_data(capture_result, default_scale)?;
            scaled_results.insert(output_name, scaled);
        }
        Ok(MultiOutputCaptureResult::new(scaled_results))
    }
}

pub fn blend_cursor_rgba(
    frame: &mut [u8],
    frame_size: (u32, u32),
    cursor_buf: &[u8],
    cursor_size: (u32, u32),
    cursor_pitch: usize,
    screen_pos: (i32, i32),
) {
    let (frame_w, frame_h) = frame_size;
    let (cursor_w, cursor_h) = cursor_size;
    let (screen_x, screen_y) = screen_pos;
    if cursor_w == 0
        || cursor_h == 0
        || cursor_buf.len() < (cursor_h as usize - 1) * cursor_pitch + cursor_w as usize * 4
    {
        log::debug!(
            "cursor shape buffer too small ({} bytes for {}x{} @ pitch {})",
            cursor_buf.len(),
            cursor_w,
            cursor_h,
            cursor_pitch
        );
        return;
    }
    for cy in 0..cursor_h as i32 {
        for cx in 0..cursor_w as i32 {
            let px = screen_x + cx;
            let py = screen_y + cy;
            if px < 0 || py < 0 || px >= frame_w as i32 || py >= frame_h as i32 {
                continue;
            }
            let ci = cy as usize * cursor_pitch + cx as usize * 4;
            let a = cursor_buf[ci + 3] as f32 / 255.0;
            if a <= 0.0 {
                continue;
            }
            let fi = (py as usize * frame_w as usize + px as usize) * 4;
            // cursor is BGRA: [B,G,R,A], frame is RGBA: [R,G,B,A]
            let cb = cursor_buf[ci] as f32;
            let cg = cursor_buf[ci + 1] as f32;
            let cr = cursor_buf[ci + 2] as f32;
            frame[fi] = (cr * a + frame[fi] as f32 * (1.0 - a)) as u8;
            frame[fi + 1] = (cg * a + frame[fi + 1] as f32 * (1.0 - a)) as u8;
            frame[fi + 2] = (cb * a + frame[fi + 2] as f32 * (1.0 - a)) as u8;
            frame[fi + 3] = 255;
        }
    }
}

fn mono_bit(buf: &[u8], row: usize, x: u32, pitch: usize) -> bool {
    let idx = row.saturating_mul(pitch).saturating_add((x / 8) as usize);
    buf.get(idx)
        .map(|b| b & (0x80u8 >> (x % 8)) != 0)
        .unwrap_or(false)
}

/// Blend a `DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROMATIC` pointer shape into
/// an RGBA frame.
///
/// The buffer holds two 1bpp masks, `cursor_size.1` rows each, MSB first: the
/// AND mask followed by the XOR mask. Bit combinations per pixel:
/// AND=0/XOR=0 -> opaque black, AND=0/XOR=1 -> opaque white,
/// AND=1/XOR=0 -> transparent, AND=1/XOR=1 -> invert the frame pixel.
pub fn blend_mono_cursor_rgba(
    frame: &mut [u8],
    frame_size: (u32, u32),
    cursor_buf: &[u8],
    cursor_size: (u32, u32),
    cursor_pitch: usize,
    screen_pos: (i32, i32),
) {
    let (frame_w, frame_h) = frame_size;
    let (cursor_w, cursor_h) = cursor_size;
    let (screen_x, screen_y) = screen_pos;
    // Two masks, cursor_h rows each; refuse truncated shapes instead of
    // drawing garbage from out-of-range reads.
    if cursor_w == 0 || cursor_h == 0 || cursor_buf.len() < cursor_h as usize * 2 * cursor_pitch {
        log::debug!(
            "mono cursor shape buffer too small ({} bytes for {}x{} @ pitch {})",
            cursor_buf.len(),
            cursor_w,
            cursor_h,
            cursor_pitch
        );
        return;
    }
    for cy in 0..cursor_h as i32 {
        for cx in 0..cursor_w as i32 {
            let px = screen_x + cx;
            let py = screen_y + cy;
            if px < 0 || py < 0 || px >= frame_w as i32 || py >= frame_h as i32 {
                continue;
            }
            let and = mono_bit(cursor_buf, cy as usize, cx as u32, cursor_pitch);
            let xor = mono_bit(
                cursor_buf,
                cursor_h as usize + cy as usize,
                cx as u32,
                cursor_pitch,
            );
            let fi = (py as usize * frame_w as usize + px as usize) * 4;
            match (and, xor) {
                (false, false) => {
                    frame[fi] = 0;
                    frame[fi + 1] = 0;
                    frame[fi + 2] = 0;
                    frame[fi + 3] = 255;
                }
                (false, true) => {
                    frame[fi] = 255;
                    frame[fi + 1] = 255;
                    frame[fi + 2] = 255;
                    frame[fi + 3] = 255;
                }
                (true, true) => {
                    frame[fi] = 255 - frame[fi];
                    frame[fi + 1] = 255 - frame[fi + 1];
                    frame[fi + 2] = 255 - frame[fi + 2];
                    frame[fi + 3] = 255;
                }
                (true, false) => {}
            }
        }
    }
}
