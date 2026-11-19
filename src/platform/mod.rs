#[cfg(target_os = "linux")]
pub(crate) mod wayland;

#[cfg(target_os = "linux")]
use wayland::WaylandCapture;

#[cfg(target_os = "windows")]
pub(crate) mod windows;

#[cfg(target_os = "windows")]
use windows::WindowsCapture;

use crate::{
    Backend, CaptureParameters, CaptureResult, MultiOutputCaptureResult, Output, Region, Result,
};

/// Platform abstraction — static enum, no allocation, no vtable.
pub(crate) enum Platform {
    #[cfg(target_os = "linux")]
    Wayland(WaylandCapture),
    #[cfg(target_os = "windows")]
    Windows(WindowsCapture),
}

/// Generate a match that delegates to all platform variants.
macro_rules! dispatch {
    ($self:expr, $method:ident $(, $arg:expr)*) => {
        match $self {
            #[cfg(target_os = "linux")]
            Platform::Wayland(w) => w.$method($($arg),*),
            #[cfg(target_os = "windows")]
            Platform::Windows(w) => w.$method($($arg),*),
        }
    };
}

impl Platform {
    pub(crate) fn new() -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            Ok(Platform::Wayland(WaylandCapture::new(Backend::Auto)?))
        }
        #[cfg(target_os = "windows")]
        {
            Ok(Platform::Windows(WindowsCapture::new(Backend::Auto)?))
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            Err(crate::Error::UnsupportedProtocol(
                "No platform backend available for this operating system".to_string(),
            ))
        }
    }

    pub(crate) fn new_with_backend(backend: Backend) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            Ok(Platform::Wayland(WaylandCapture::new(backend)?))
        }
        #[cfg(target_os = "windows")]
        {
            match backend {
                Backend::Auto => Ok(Platform::Windows(WindowsCapture::new(backend)?)),
                Backend::ExtImageCopyCapture | Backend::WlrScreencopy => {
                    Err(crate::Error::UnsupportedProtocol(
                        "ext-image-copy-capture-v1 and wlr-screencopy are Wayland protocols \
                         not available on Windows"
                            .to_string(),
                    ))
                }
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            let _ = backend;
            Err(crate::Error::UnsupportedProtocol(
                "No platform backend available for this operating system".to_string(),
            ))
        }
    }

    pub(crate) fn get_outputs(&mut self) -> Result<Vec<Output>> {
        dispatch!(self, get_outputs)
    }

    pub(crate) fn capture_all(&mut self) -> Result<CaptureResult> {
        dispatch!(self, capture_all)
    }

    pub(crate) fn capture_all_with_scale(&mut self, scale: f64) -> Result<CaptureResult> {
        dispatch!(self, capture_all_with_scale, scale)
    }

    pub(crate) fn capture_output(&mut self, output_name: &str) -> Result<CaptureResult> {
        dispatch!(self, capture_output, output_name)
    }

    pub(crate) fn capture_output_with_scale(
        &mut self,
        output_name: &str,
        scale: f64,
    ) -> Result<CaptureResult> {
        dispatch!(self, capture_output_with_scale, output_name, scale)
    }

    pub(crate) fn capture_region(&mut self, region: Region) -> Result<CaptureResult> {
        dispatch!(self, capture_region, region)
    }

    pub(crate) fn capture_region_with_scale(
        &mut self,
        region: Region,
        scale: f64,
    ) -> Result<CaptureResult> {
        dispatch!(self, capture_region_with_scale, region, scale)
    }

    pub(crate) fn capture_outputs(
        &mut self,
        parameters: Vec<CaptureParameters>,
    ) -> Result<MultiOutputCaptureResult> {
        dispatch!(self, capture_outputs, parameters)
    }

    pub(crate) fn capture_outputs_with_scale(
        &mut self,
        parameters: Vec<CaptureParameters>,
        default_scale: f64,
    ) -> Result<MultiOutputCaptureResult> {
        dispatch!(self, capture_outputs_with_scale, parameters, default_scale)
    }
}
