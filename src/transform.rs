/// Platform-independent output transform representation.
///
/// DXGI provides 4 rotation modes (Normal, Rotate90, Rotate180, Rotate270).
/// Wayland adds 4 flipped variants on top of those.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputTransform {
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
    #[cfg(target_os = "linux")]
    Flipped,
    #[cfg(target_os = "linux")]
    Flipped90,
    #[cfg(target_os = "linux")]
    Flipped180,
    #[cfg(target_os = "linux")]
    Flipped270,
}

#[cfg(target_os = "linux")]
impl From<wayland_client::protocol::wl_output::Transform> for OutputTransform {
    fn from(t: wayland_client::protocol::wl_output::Transform) -> Self {
        use wayland_client::protocol::wl_output::Transform;
        match t {
            Transform::Normal => OutputTransform::Normal,
            Transform::_90 => OutputTransform::Rotate90,
            Transform::_180 => OutputTransform::Rotate180,
            Transform::_270 => OutputTransform::Rotate270,
            Transform::Flipped => OutputTransform::Flipped,
            Transform::Flipped90 => OutputTransform::Flipped90,
            Transform::Flipped180 => OutputTransform::Flipped180,
            Transform::Flipped270 => OutputTransform::Flipped270,
            _ => OutputTransform::Normal,
        }
    }
}

/// Swap width/height for 90°/270° rotations (including flipped variants).
#[cfg(target_os = "linux")]
pub(crate) fn apply_output_transform(
    transform: OutputTransform,
    width: &mut i32,
    height: &mut i32,
) {
    match transform {
        OutputTransform::Rotate90
        | OutputTransform::Rotate270
        | OutputTransform::Flipped90
        | OutputTransform::Flipped270 => {
            std::mem::swap(width, height);
        }
        _ => {}
    }
}

/// Apply the given transform to image data, returning new pixel buffer and dimensions.
///
/// Logs an error and returns `(Vec::new(), 0, 0)` if the RGBA buffer is too short,
/// the dimensions exceed the global pixel limit, or allocation fails.
pub fn apply_image_transform(
    data: &[u8],
    width: u32,
    height: u32,
    transform: OutputTransform,
) -> (Vec<u8>, u32, u32) {
    transform_rgba(data, width, height, transform)
}

/// Rotate image 90 degrees clockwise.
///
/// Invalid input or allocation failure is logged and returns `(Vec::new(), 0, 0)`.
pub fn rotate_90(data: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    transform_rgba(data, width, height, OutputTransform::Rotate90)
}

/// Rotate image 180 degrees.
///
/// Invalid input or allocation failure is logged and returns `(Vec::new(), 0, 0)`.
pub fn rotate_180(data: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    transform_rgba(data, width, height, OutputTransform::Rotate180)
}

/// Rotate image 270 degrees clockwise.
///
/// Invalid input or allocation failure is logged and returns `(Vec::new(), 0, 0)`.
pub fn rotate_270(data: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    transform_rgba(data, width, height, OutputTransform::Rotate270)
}

/// Flip image vertically.
#[cfg(target_os = "linux")]
pub(crate) fn flip_vertical(data: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    transform_rgba(data, width, height, OutputTransform::Flipped180)
}

fn transform_rgba(
    data: &[u8],
    width: u32,
    height: u32,
    transform: OutputTransform,
) -> (Vec<u8>, u32, u32) {
    let result = (|| -> crate::Result<_> {
        use crate::Error;
        let expected = crate::buffer::checked_buffer_size(width, height, 4, None)?;
        if data.len() < expected {
            return Err(Error::InvalidData(format!(
                "expected {expected} bytes for {width}x{height} RGBA, got {}",
                data.len()
            )));
        }
        let swaps_dimensions = match transform {
            OutputTransform::Rotate90 | OutputTransform::Rotate270 => true,
            #[cfg(target_os = "linux")]
            OutputTransform::Flipped90 | OutputTransform::Flipped270 => true,
            _ => false,
        };
        let (new_width, new_height) = if swaps_dimensions {
            (height, width)
        } else {
            (width, height)
        };
        let mut out = Vec::new();
        let len = if transform == OutputTransform::Normal {
            data.len()
        } else {
            expected
        };
        if let Err(error) = out.try_reserve_exact(len) {
            log::error!("Image buffer allocation failed: {error}");
            return Ok((Vec::new(), 0, 0));
        }
        if transform == OutputTransform::Normal {
            out.extend_from_slice(data);
            return Ok((out, new_width, new_height));
        }
        if expected == 0 {
            return Ok((out, new_width, new_height));
        }
        out.resize(len, 0);
        let (w, h) = (u64::from(width), u64::from(height));
        let row_bytes = crate::buffer::checked_buffer_size(width, 1, 4, None)?;
        for (y, row) in data[..expected].chunks_exact(row_bytes).enumerate() {
            for (x, pixel) in row.chunks_exact(4).enumerate() {
                let (x, y) = (x as u64, y as u64);
                let (new_x, new_y) = match transform {
                    OutputTransform::Normal => (x, y),
                    OutputTransform::Rotate90 => (h - 1 - y, x),
                    OutputTransform::Rotate180 => (w - 1 - x, h - 1 - y),
                    OutputTransform::Rotate270 => (y, w - 1 - x),
                    #[cfg(target_os = "linux")]
                    OutputTransform::Flipped => (w - 1 - x, y),
                    #[cfg(target_os = "linux")]
                    OutputTransform::Flipped90 => (h - 1 - y, w - 1 - x),
                    #[cfg(target_os = "linux")]
                    OutputTransform::Flipped180 => (x, h - 1 - y),
                    #[cfg(target_os = "linux")]
                    OutputTransform::Flipped270 => (y, x),
                };
                let dst = new_y
                    .checked_mul(u64::from(new_width))
                    .and_then(|row| row.checked_add(new_x))
                    .and_then(|index| index.checked_mul(4))
                    .and_then(|index| usize::try_from(index).ok())
                    .and_then(|index| out.get_mut(index..))
                    .and_then(|tail| tail.get_mut(..4))
                    .ok_or_else(|| {
                        Error::InvalidData("transformed pixel index exceeds buffer".to_string())
                    })?;
                dst.copy_from_slice(pixel);
            }
        }
        Ok((out, new_width, new_height))
    })();
    match result {
        Ok(image) => image,
        Err(error) => {
            log::error!("{error}");
            (Vec::new(), 0, 0)
        }
    }
}
