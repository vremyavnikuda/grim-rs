/// Pixel format variants for 32-bit RGBA-like layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PixelFormat {
    /// A, R, G, B — alpha in byte 0 (most significant).
    Argb8888 = 0,
    /// X, R, G, B — alpha unused, byte 0 ignored.
    Xrgb8888 = 1,
    /// A, B, G, R — alpha in byte 0.
    Abgr8888 = 2,
    /// X, B, G, R — alpha unused.
    Xbgr8888 = 3,
}

/// Map a DRM fourcc code to a [`PixelFormat`].
pub fn fourcc_to_format(fourcc: u32) -> Option<PixelFormat> {
    match fourcc {
        0x34325241 => Some(PixelFormat::Argb8888), // DRM_FORMAT_ARGB8888
        0x34325258 => Some(PixelFormat::Xrgb8888), // DRM_FORMAT_XRGB8888
        0x34324241 => Some(PixelFormat::Abgr8888), // DRM_FORMAT_ABGR8888
        0x34324258 => Some(PixelFormat::Xbgr8888), // DRM_FORMAT_XBGR8888
        _ => None,
    }
}

/// Convert 32-bit pixel data from the given [`PixelFormat`] to RGBA byte order.
///
/// Conversion is done in-place on the slice. Unrecognized formats are left
/// unchanged.
pub fn convert_to_rgba(data: &mut [u8], format: PixelFormat) {
    match format {
        PixelFormat::Xrgb8888 => {
            for chunk in data.as_chunks_mut::<4>().0 {
                chunk.swap(0, 2);
                chunk[3] = 255;
            }
        }
        PixelFormat::Argb8888 => {
            for chunk in data.as_chunks_mut::<4>().0 {
                chunk.swap(0, 2);
            }
        }
        PixelFormat::Xbgr8888 => {
            for chunk in data.as_chunks_mut::<4>().0 {
                chunk[3] = 255;
            }
        }
        PixelFormat::Abgr8888 => {}
    }
}
