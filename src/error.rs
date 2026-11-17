use std::fmt;

#[derive(Debug)]
pub enum Error {
    InvalidGeometry(String),
    NoOutputs,
    OutputNotFound(String),
    InvalidRegion(String),
    CaptureFailed,
    BufferCreation(String),
    /// Operation is not available in this build (e.g. a disabled cargo feature).
    Unsupported(String),
    /// Caller-provided pixel data does not match the declared dimensions.
    InvalidData(String),
    Io(std::io::Error),
    IoWithContext {
        operation: String,
        source: std::io::Error,
    },
    UnsupportedProtocol(String),
    WaylandConnection(String),
    FrameCapture(String),
    TransformNotSupported(String),
    InvertFailed(String),
    ScalingFailed(String),
    DirectXError(String),
    ProtectedContent(String),
    NoGpuAdapter,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidGeometry(msg) => write!(f, "Invalid geometry format: {msg}"),
            Error::NoOutputs => write!(f, "No outputs available"),
            Error::OutputNotFound(name) => write!(f, "Output not found: {name}"),
            Error::InvalidRegion(msg) => write!(f, "Invalid capture region: {msg}"),
            Error::CaptureFailed => write!(f, "Screenshot capture failed"),
            Error::BufferCreation(msg) => write!(f, "Buffer creation failed: {msg}"),
            Error::Unsupported(msg) => write!(f, "Operation not supported: {msg}"),
            Error::InvalidData(msg) => write!(f, "Data does not match dimensions: {msg}"),
            Error::Io(err) => write!(f, "IO error: {err}"),
            Error::IoWithContext { operation, source } => {
                write!(f, "IO error during {operation}: {source}")
            }
            Error::UnsupportedProtocol(name) => {
                write!(f, "Compositor doesn't support required protocol: {name}")
            }
            Error::WaylandConnection(msg) => write!(f, "Wayland connection error: {msg}"),
            Error::FrameCapture(msg) => write!(f, "Frame capture failed: {msg}"),
            Error::TransformNotSupported(msg) => {
                write!(f, "Output transform not supported: {msg}")
            }
            Error::InvertFailed(msg) => write!(f, "Failed to apply Y-invert transformation: {msg}"),
            Error::ScalingFailed(msg) => write!(f, "Image scaling failed: {msg}"),
            Error::DirectXError(msg) => write!(f, "DirectX error: {msg}"),
            Error::ProtectedContent(msg) => {
                write!(f, "Protected content cannot be captured: {msg}")
            }
            Error::NoGpuAdapter => write!(f, "No suitable GPU adapter found"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            Error::IoWithContext { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
