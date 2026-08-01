use super::HudAssetRefConfig;
use std::path::{Path, PathBuf};

// Custom HUD assets are user-supplied, so every decode path enforces file, dimension, and frame caps.
pub const HUD_ASSET_MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
pub const HUD_ASSET_MAX_DIMENSION: u32 = 4096;
pub const HUD_ASSET_MAX_PIXELS: u64 = 8_294_400;
pub const HUD_GIF_MAX_FRAMES: usize = 120;
pub const HUD_GIF_MAX_TOTAL_PIXELS: u64 = 60_000_000;

pub fn hud_asset_path(asset: &HudAssetRefConfig) -> Option<PathBuf> {
    asset
        .path
        .as_deref()
        .map(Path::new)
        .filter(|path| path.is_file())
        .map(Path::to_path_buf)
}

pub fn hud_asset_file_within_limits(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.len() > 0 && metadata.len() <= HUD_ASSET_MAX_FILE_BYTES)
        .unwrap_or(false)
}

pub fn hud_asset_dimensions_within_limits(width: u32, height: u32) -> bool {
    if width == 0 || height == 0 {
        return false;
    }
    if width > HUD_ASSET_MAX_DIMENSION || height > HUD_ASSET_MAX_DIMENSION {
        return false;
    }
    (width as u64).saturating_mul(height as u64) <= HUD_ASSET_MAX_PIXELS
}
