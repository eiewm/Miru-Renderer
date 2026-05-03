use super::super::gpu::GpuError;
use std::fmt;
#[derive(Debug)]
pub enum RendererError {
    NotInitialized,
    GpuError(GpuError),
    TextureLoad(String),
}
impl fmt::Display for RendererError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInitialized => write!(f, "GPU pipeline not initialized"),
            Self::GpuError(e) => write!(f, "GPU error: {}", e),
            Self::TextureLoad(s) => write!(f, "Texture load failed: {}", s),
        }
    }
}
impl std::error::Error for RendererError {}
impl From<GpuError> for RendererError {
    fn from(e: GpuError) -> Self {
        Self::GpuError(e)
    }
}
