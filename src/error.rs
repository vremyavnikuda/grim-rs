use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid geometry format: {0}")]
    InvalidGeometry(String),
    
    #[error("No outputs available")]
    NoOutputs,
    
    #[error("Output not found: {0}")]
    OutputNotFound(String),
    
    #[error("Screenshot capture failed")]
    CaptureFailed,
    
    #[error("Buffer creation failed")]
    BufferCreation,
    
    #[error("Image processing error: {0}")]
    ImageProcessing(#[from] image::ImageError),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Compositor doesn't support required protocol: {0}")]
    UnsupportedProtocol(String),
    
    #[error("Wayland connection error: {0}")]
    WaylandConnection(String),
    
    #[error("Frame capture failed: {0}")]
    FrameCapture(String),
}

pub type Result<T> = std::result::Result<T, Error>;