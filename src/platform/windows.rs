use std::collections::HashMap;

use crate::pixel_format::{self, PixelFormat};
use crate::{
    Backend, CaptureParameters, CaptureResult, Error, MultiOutputCaptureResult, Output, Region,
    Result,
};

use windows::core::{Interface, PCWSTR};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput,
    IDXGIOutputDuplication, IDXGIResource, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT,
    DXGI_OUTDUPL_FRAME_INFO, DXGI_OUTDUPL_POINTER_POSITION, DXGI_OUTDUPL_POINTER_SHAPE_INFO,
    DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME, DXGI_OUTPUT_DESC,
};
use windows::Win32::Graphics::Gdi::{
    EnumDisplaySettingsExW, DEVMODEW, DM_PELSHEIGHT, DM_PELSWIDTH, DM_POSITION,
    ENUM_CURRENT_SETTINGS, ENUM_DISPLAY_SETTINGS_FLAGS,
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
    adapter_slot: usize,
    dxgi_adapter_idx: u32,
    output_idx: usize,
}

impl OutputInfo {
    fn from_desc(
        desc: &DXGI_OUTPUT_DESC,
        adapter_slot: usize,
        dxgi_adapter_idx: u32,
        output_idx: usize,
    ) -> Self {
        let name = String::from_utf16_lossy(&desc.DeviceName)
            .trim_end_matches('\0')
            .to_string();
        Self {
            name: name.clone(),
            x: desc.DesktopCoordinates.left,
            y: desc.DesktopCoordinates.top,
            // DesktopCoordinates already includes the output rotation.
            width: desc.DesktopCoordinates.right - desc.DesktopCoordinates.left,
            height: desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top,
            rotation: desc.Rotation.0 as u32,
            description: name,
            adapter_slot,
            dxgi_adapter_idx,
            output_idx,
        }
    }

    fn dxgi_adapter(&self, factory: &IDXGIFactory1) -> Result<IDXGIAdapter1> {
        unsafe { factory.EnumAdapters1(self.dxgi_adapter_idx) }
            .map_err(|e| Error::DirectXError(format!("EnumAdapters1: {e}")))
    }
}

fn physical_output_geometry(name: &str) -> Option<Region> {
    let name: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
    let mut mode = DEVMODEW {
        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    let required_fields = DM_POSITION | DM_PELSWIDTH | DM_PELSHEIGHT;
    let found = unsafe {
        EnumDisplaySettingsExW(
            PCWSTR(name.as_ptr()),
            ENUM_CURRENT_SETTINGS,
            &mut mode,
            ENUM_DISPLAY_SETTINGS_FLAGS(0),
        )
    };
    if !found.as_bool() || mode.dmFields & required_fields != required_fields {
        return None;
    }
    let position = unsafe { mode.Anonymous1.Anonymous2.dmPosition };
    let width = i32::try_from(mode.dmPelsWidth).ok()?;
    let height = i32::try_from(mode.dmPelsHeight).ok()?;
    let geometry = Region::new(position.x, position.y, width, height);
    position.x.checked_add(width)?;
    position.y.checked_add(height)?;
    (!geometry.is_empty()).then_some(geometry)
}

fn apply_physical_output_geometry(
    outputs: &mut [OutputInfo],
    mut read_geometry: impl FnMut(&str) -> Option<Region>,
) {
    // shortcut: a missing GDI mode leaves all outputs in DXGI space, add a second physical source if this affects scaled displays.
    let Some(geometry) = outputs
        .iter()
        .map(|output| read_geometry(&output.name))
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    for (output, region) in outputs.iter_mut().zip(geometry) {
        output.x = region.x();
        output.y = region.y();
        output.width = region.width();
        output.height = region.height();
    }
}

struct AdapterState {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
}

impl AdapterState {
    fn new(adapter: &IDXGIAdapter) -> Result<Self> {
        let mut device = None;
        let mut context = None;
        unsafe {
            D3D11CreateDevice(
                Some(adapter),
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
        let context = context
            .ok_or_else(|| Error::DirectXError("D3D11CreateDevice: null context".to_string()))?;
        Ok(Self { device, context })
    }
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
    // The desktop surface accumulates updates between frames, so the last
    // copied frame stays valid until DXGI reports a new one; serve it on
    // WAIT_TIMEOUT instead of failing on a static screen.
    last_frame: Option<(Vec<u8>, u32, u32)>,
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
        let candidates = (0u32..).map_while(|index| {
            unsafe { factory.EnumAdapters1(index) }
                .ok()
                .map(|adapter| (index, adapter))
        });
        let (adapters, mut output_info) = Self::initialize_adapters(candidates, AdapterState::new)?;
        apply_physical_output_geometry(&mut output_info, physical_output_geometry);
        Ok(Self {
            adapters,
            output_info,
            dup_cache: HashMap::new(),
            factory,
        })
    }

    fn initialize_adapters(
        mut candidates: impl Iterator<Item = (u32, IDXGIAdapter1)>,
        mut create_device: impl FnMut(&IDXGIAdapter) -> Result<AdapterState>,
    ) -> Result<(Vec<AdapterState>, Vec<OutputInfo>)> {
        let mut adapters = Vec::new();
        let mut output_info = Vec::new();
        let mut first_device_error = None;
        let result = candidates.try_for_each(|(adapter_idx, adapter)| {
            let desc = unsafe { adapter.GetDesc1() }
                .map_err(|e| Error::DirectXError(format!("GetDesc1: {e}")))?;
            if (desc.Flags & 1) != 0 {
                return Ok(());
            }
            let adapter_base = adapter.cast().map_err(|e| {
                Error::DirectXError(format!("Cast IDXGIAdapter1 -> IDXGIAdapter: {e}"))
            })?;
            let state = match create_device(&adapter_base) {
                Ok(state) => state,
                Err(error) => {
                    first_device_error.get_or_insert(error);
                    return Ok(());
                }
            };
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
                output_info.push(OutputInfo::from_desc(
                    &desc,
                    adapters.len(),
                    adapter_idx,
                    output_idx as usize,
                ));
                has_outputs = true;
            }
            if has_outputs {
                adapters.push(state);
            }
            Ok(())
        });
        // If initialization still fails, report the error that would have stopped it first.
        match result {
            Err(error) => Err(first_device_error.unwrap_or(error)),
            Ok(()) if output_info.is_empty() => Err(first_device_error.unwrap_or(Error::NoOutputs)),
            Ok(()) => Ok((adapters, output_info)),
        }
    }

    fn get_dup(&mut self, info: &OutputInfo) -> Result<()> {
        if self.dup_cache.contains_key(&info.name) {
            return Ok(());
        }
        let adapter_state = &self.adapters[info.adapter_slot];
        let adapter = info.dxgi_adapter(&self.factory)?;
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
        // Discard the first successfully acquired frame to work around blank startup
        // surfaces. Acquisition is bounded by MAX_ACQUIRE_ATTEMPTS; this does not
        // verify that the frame contains a desktop update.
        // Preserve cursor metadata from the discarded frame because later frames
        // may omit unchanged cursor shape and position information.
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
                last_frame: None,
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
            let (pixels, width, height) = match result {
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
                    let adapter = &self.adapters[info.adapter_slot];
                    let texture: ID3D11Texture2D = resource.cast().map_err(|e| {
                        let _ = unsafe { dup.ReleaseFrame() };
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
                        let r = unsafe {
                            adapter
                                .device
                                .CreateTexture2D(&staging_desc, None, Some(&mut tex))
                        };
                        if let Err(e) = r {
                            let _ = unsafe { dup.ReleaseFrame() };
                            return Err(Error::BufferCreation(format!(
                                "CreateTexture2D (staging): {e}"
                            )));
                        }
                        match tex {
                            Some(t) => t,
                            None => {
                                let _ = unsafe { dup.ReleaseFrame() };
                                return Err(Error::BufferCreation(
                                    "CreateTexture2D returned null".to_string(),
                                ));
                            }
                        }
                    };
                    unsafe { adapter.context.CopyResource(&staging, &texture) };
                    // The shape buffer is only filled when the shape changed;
                    // refresh the cached shape while the frame is still held.
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
                    // Release before Map: every error path below must not
                    // leave the acquired frame hanging, or the duplication
                    // session stops producing frames.
                    unsafe { dup.ReleaseFrame() }
                        .map_err(|e| Error::DirectXError(format!("ReleaseFrame: {e}")))?;
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
                    pixel_format::convert_to_rgba(&mut pixels, PixelFormat::Argb8888);
                    (pixels, width, height)
                }
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                    // Static desktop: the last copied frame is still current.
                    match &entry.last_frame {
                        Some(cached) => cached.clone(),
                        None => {
                            attempts += 1;
                            if attempts >= MAX_ACQUIRE_ATTEMPTS {
                                return Err(Error::FrameCapture(
                                    "Timeout waiting for frame update".to_string(),
                                ));
                            }
                            continue;
                        }
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
            };
            entry.last_frame = Some((pixels.clone(), width, height));
            if overlay_cursor {
                log::debug!(
                    "blending cursor: shape_cached={} pointer={:?}",
                    entry.cursor_shape.is_some(),
                    entry
                        .pointer_position
                        .map(|p| (p.Position, p.Visible.as_bool()))
                );
            }
            return Ok(prepare_frame(
                (pixels, width, height),
                info.rotation,
                entry.cursor_shape.as_ref(),
                if overlay_cursor {
                    entry.pointer_position
                } else {
                    None
                },
            ));
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
        build_outputs(&self.output_info)
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
        if region.right() > info.width || region.bottom() > info.height {
            return Err(Error::InvalidRegion(
                "Capture region extends outside output boundaries".to_string(),
            ));
        }
        self.get_dup(&info)?;
        let (mut final_data, mut final_width, mut final_height) =
            self.capture_frame(&info, overlay_cursor)?;
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

fn prepare_frame(
    (pixels, width, height): (Vec<u8>, u32, u32),
    rotation: u32,
    cursor_shape: Option<&CachedCursorShape>,
    pointer: Option<DXGI_OUTDUPL_POINTER_POSITION>,
) -> (Vec<u8>, u32, u32) {
    use crate::transform::{apply_image_transform, OutputTransform};
    let transform = match rotation {
        2 => OutputTransform::Rotate90,
        3 => OutputTransform::Rotate180,
        4 => OutputTransform::Rotate270,
        _ => OutputTransform::Normal,
    };
    // Pointer positions and crop regions use the rotated desktop coordinates.
    let (mut pixels, width, height) = if transform == OutputTransform::Normal {
        (pixels, width, height)
    } else {
        apply_image_transform(&pixels, width, height, transform)
    };
    if let Some(pointer) = pointer.filter(|p| p.Visible.as_bool()) {
        if let Some(CachedCursorShape { info, buf }) = cursor_shape {
            // DXGI reports the shape's top-left corner, independent of HotSpot.
            let screen_x = pointer.Position.x;
            let screen_y = pointer.Position.y;
            if info.Type == DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME.0 as u32 {
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
    (pixels, width, height)
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

fn build_outputs(output_info: &[OutputInfo]) -> Result<Vec<Output>> {
    if output_info.is_empty() {
        return Err(Error::NoOutputs);
    }
    Ok(output_info
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

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::Graphics::Dxgi::Common::DXGI_MODE_ROTATION;
    use windows::Win32::Graphics::Dxgi::DXGI_OUTDUPL_POINTER_SHAPE_TYPE_COLOR;

    #[test]
    fn output_geometry_keeps_desktop_coordinates_for_every_rotation() {
        for rotation in 0..=4 {
            let desc = DXGI_OUTPUT_DESC {
                DesktopCoordinates: RECT {
                    left: -1080,
                    top: -100,
                    right: 0,
                    bottom: 1820,
                },
                Rotation: DXGI_MODE_ROTATION(rotation),
                ..Default::default()
            };
            let info = OutputInfo::from_desc(&desc, 0, 0, 0);
            let outputs = build_outputs(&[info]).unwrap();
            assert_eq!(
                outputs[0].geometry(),
                &Region::new(-1080, -100, 1080, 1920),
                "rotation {rotation}"
            );
        }
    }

    #[test]
    fn desktop_geometry_uses_physical_pixels_only_when_all_outputs_are_available() {
        let fixture = |name: &str, desktop_coordinates, rotation| {
            let desc = DXGI_OUTPUT_DESC {
                DesktopCoordinates: desktop_coordinates,
                Rotation: DXGI_MODE_ROTATION(rotation),
                ..Default::default()
            };
            let mut output = OutputInfo::from_desc(&desc, 0, 0, 0);
            output.name = name.to_owned();
            output
        };
        let mut outputs = [
            fixture(
                "left",
                RECT {
                    left: -1280,
                    top: 0,
                    right: 0,
                    bottom: 720,
                },
                1,
            ),
            fixture(
                "portrait",
                RECT {
                    left: 0,
                    top: 0,
                    right: 864,
                    bottom: 1536,
                },
                2,
            ),
        ];
        let physical = [
            Region::new(-1920, 0, 1920, 1080),
            Region::new(0, 0, 1080, 1920),
        ];
        let dxgi = [
            Region::new(-1280, 0, 1280, 720),
            Region::new(0, 0, 864, 1536),
        ];
        for (portrait_available, expected) in [(false, dxgi), (true, physical)] {
            apply_physical_output_geometry(&mut outputs, |name| match name {
                "left" => Some(physical[0]),
                "portrait" => portrait_available.then_some(physical[1]),
                _ => None,
            });
            let outputs = build_outputs(&outputs).unwrap();
            assert_eq!([*outputs[0].geometry(), *outputs[1].geometry()], expected);
        }
        assert_eq!(outputs[1].rotation, 2);
    }

    #[test]
    fn prepare_frame_orients_non_square_desktop_pixels() {
        let pixels: Vec<u8> = (1..=6).flat_map(|id| [id, 0, 0, 255]).collect();
        for (rotation, width, height, expected) in [
            (0, 3, 2, [1, 2, 3, 4, 5, 6]),
            (1, 3, 2, [1, 2, 3, 4, 5, 6]),
            (2, 2, 3, [4, 1, 5, 2, 6, 3]),
            (3, 3, 2, [6, 5, 4, 3, 2, 1]),
            (4, 2, 3, [3, 6, 2, 5, 1, 4]),
        ] {
            let (data, w, h) = prepare_frame((pixels.clone(), 3, 2), rotation, None, None);
            assert_eq!((w, h), (width, height));
            let expected: Vec<u8> = expected
                .into_iter()
                .flat_map(|id| [id, 0, 0, 255])
                .collect();
            assert_eq!(data, expected, "rotation {rotation}");
        }
    }

    #[test]
    fn prepare_frame_draws_cursor_in_desktop_space_without_hotspot_offset() {
        let pixels: Vec<u8> = (1..=6).flat_map(|id| [id, 0, 0, 255]).collect();
        let pointer = DXGI_OUTDUPL_POINTER_POSITION {
            Position: POINT { x: 0, y: 1 },
            Visible: true.into(),
        };
        let color = CachedCursorShape {
            info: DXGI_OUTDUPL_POINTER_SHAPE_INFO {
                Type: DXGI_OUTDUPL_POINTER_SHAPE_TYPE_COLOR.0 as u32,
                Width: 2,
                Height: 1,
                Pitch: 8,
                HotSpot: POINT { x: 1, y: 0 },
            },
            buf: vec![0, 0, 160, 255, 0, 0, 176, 255],
        };
        let mono = CachedCursorShape {
            info: DXGI_OUTDUPL_POINTER_SHAPE_INFO {
                Type: DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME.0 as u32,
                Width: 2,
                Height: 2,
                Pitch: 1,
                HotSpot: POINT { x: 1, y: 0 },
            },
            buf: vec![0, 0x40],
        };
        for (shape, first, second) in [(&color, 160, 176), (&mono, 0, 255)] {
            for (rotation, expected) in [
                (0, [1, 2, 3, first, second, 6]),
                (1, [1, 2, 3, first, second, 6]),
                (2, [4, 1, first, second, 6, 3]),
                (3, [6, 5, 4, first, second, 1]),
                (4, [3, 6, first, second, 1, 4]),
            ] {
                let (data, _, _) =
                    prepare_frame((pixels.clone(), 3, 2), rotation, Some(shape), Some(pointer));
                let reds: Vec<u8> = data.chunks_exact(4).map(|pixel| pixel[0]).collect();
                assert_eq!(
                    reds, expected,
                    "rotation {rotation}, shape {}",
                    shape.info.Type
                );
            }
        }
    }

    #[test]
    fn build_outputs_empty_returns_no_outputs() {
        assert!(matches!(build_outputs(&[]), Err(Error::NoOutputs)));
    }

    #[test]
    fn adapter_initialization_keeps_trying_and_returns_the_first_device_error() {
        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.unwrap();
        let adapter = (0u32..)
            .map_while(|index| unsafe { factory.EnumAdapters1(index) }.ok())
            .find(|adapter| unsafe { adapter.GetDesc1() }.unwrap().Flags & 1 == 0)
            .expect("a local DXGI adapter");
        let candidates = [(0, adapter.clone()), (1, adapter)];
        let mut attempts = 0;
        let result = WindowsCapture::initialize_adapters(candidates.into_iter(), |_| {
            attempts += 1;
            Err(Error::DirectXError(format!("device error {attempts}")))
        });
        assert_eq!(attempts, 2);
        assert!(matches!(result, Err(Error::DirectXError(message)) if message == "device error 1"));
    }

    #[test]
    fn duplication_uses_the_original_dxgi_adapter_index() {
        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.unwrap();
        assert!(unsafe { factory.EnumAdapters1(0) }.is_ok());

        // Simulate a compact slot of 0 with a different native index. An unavailable
        // native index also verifies the lookup's error without starting duplication.
        let dxgi_index = u32::MAX;
        let info = OutputInfo::from_desc(&DXGI_OUTPUT_DESC::default(), 0, dxgi_index, 0);
        let expected = unsafe { factory.EnumAdapters1(dxgi_index) }.unwrap_err();
        assert!(matches!(
            info.dxgi_adapter(&factory),
            Err(Error::DirectXError(message)) if message == format!("EnumAdapters1: {expected}")
        ));
    }

    #[test]
    fn output_capture_rejects_out_of_bounds_regions_before_duplication() {
        let desc = DXGI_OUTPUT_DESC {
            DesktopCoordinates: RECT {
                right: 16,
                bottom: 8,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut info = OutputInfo::from_desc(&desc, 0, 0, 0);
        info.name = "DISPLAY1".to_string();
        let mut capture = WindowsCapture {
            // Invalid regions must be rejected before a D3D device is needed.
            adapters: Vec::new(),
            output_info: vec![info],
            dup_cache: HashMap::new(),
            factory: unsafe { CreateDXGIFactory1() }.unwrap(),
        };
        for region in [
            Region::new(15, 0, 2, 1),
            Region::new(0, 7, 1, 2),
            Region::new(16, 0, 1, 1),
            Region::new(i32::MAX, 0, 2, 1),
        ] {
            assert!(matches!(
                capture.capture_output_raw("DISPLAY1", region, false),
                Err(Error::InvalidRegion(_))
            ));
        }
    }

    #[test]
    fn build_outputs_maps_fields() {
        let info = vec![OutputInfo {
            name: "DISPLAY1".to_string(),
            x: 0,
            y: 0,
            width: 3440,
            height: 1440,
            rotation: 1,
            description: String::new(),
            adapter_slot: 0,
            dxgi_adapter_idx: 0,
            output_idx: 0,
        }];
        let outputs = build_outputs(&info).expect("non-empty list");
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].name, "DISPLAY1");
        assert_eq!(outputs[0].scale, 1);
        assert!(outputs[0].description.is_none());
    }
}
