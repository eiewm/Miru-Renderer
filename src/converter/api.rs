use super::*;
use serde::Serialize;
#[derive(Debug, Clone)]
pub struct ConverterSettings {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub scroll_speed: f32,
    pub lead_in_ms: i32,
    pub preset: String,
    pub crf: u8,
    pub encoder: VideoEncoder,
    pub motion_blur_percent: u8,
    pub enable_hud: bool,
    pub enable_lighting: bool,
    pub enable_barlines: bool,
    pub intro_enabled: bool,
    pub sv_enabled: bool,
    pub skin_animations_enabled: bool,
    pub combo_images_enabled: bool,
    pub ln_debug: bool,
    pub background_dim: f32,
    pub background_blur_percent: Option<u8>,
    pub background_offset_x: f32,
    pub background_offset_y: f32,
    pub background_video_enabled: bool,
    pub storyboard_enabled: bool,
    pub hud_config: Option<HudConfig>,
    pub note_debug: bool,
    pub all_presses: bool,
    pub ffmpeg_threads: Option<u32>,
    pub bg_compose_mode: BgComposeMode,
    pub gpu_preference: GpuPreference,
    pub music_volume_percent: f32,
    pub hitsound_volume_percent: f32,
}
impl Default for ConverterSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 60,
            scroll_speed: 30.0,
            lead_in_ms: 1500,
            preset: "veryfast".into(),
            crf: 18,
            encoder: VideoEncoder::X264,
            motion_blur_percent: 0,
            enable_hud: true,
            enable_lighting: true,
            enable_barlines: false,
            intro_enabled: true,
            sv_enabled: true,
            skin_animations_enabled: true,
            combo_images_enabled: true,
            ln_debug: false,
            background_dim: 1.0,
            background_blur_percent: None,
            background_offset_x: 0.0,
            background_offset_y: 0.0,
            background_video_enabled: true,
            storyboard_enabled: true,
            hud_config: None,
            note_debug: false,
            all_presses: false,
            ffmpeg_threads: None,
            bg_compose_mode: BgComposeMode::Auto,
            gpu_preference: GpuPreference::High,
            music_volume_percent: 100.0,
            hitsound_volume_percent: 100.0,
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct ResolveOpts {
    pub osu: Option<PathBuf>,
    pub start_seconds: Option<f32>,
    pub end_seconds: Option<f32>,
    pub out_width: Option<u32>,
    pub out_height: Option<u32>,
    pub audio_local_only: bool,
    pub autoplay_mods: Option<crate::utils::AutoplayModsConfig>,
    pub intro_user_data: Option<crate::utils::IntroUserData>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReplayIntegrityReport {
    pub has_summary_mismatch: bool,
}
#[derive(Debug)]
pub struct ConvertResult {
    pub output_path: PathBuf,
    pub elapsed_ms: u64,
    pub frames_rendered: u64,
    pub replay_integrity: Option<ReplayIntegrityReport>,
}
#[derive(Debug)]
pub enum ConvertError {
    Parse(String),
    Resolve(String),
    Render(String),
    Compose(ComposeError),
    Io(std::io::Error),
}
impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "parse: {e}"),
            Self::Resolve(e) => write!(f, "resolve: {e}"),
            Self::Render(e) => write!(f, "render: {e}"),
            Self::Compose(e) => write!(f, "compose: {e}"),
            Self::Io(e) => write!(f, "io: {e}"),
        }
    }
}
impl std::error::Error for ConvertError {}
impl From<std::io::Error> for ConvertError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<ComposeError> for ConvertError {
    fn from(e: ComposeError) -> Self {
        Self::Compose(e)
    }
}
#[derive(Debug, Clone)]
pub struct BackgroundSource {
    pub kind: BackgroundKind,
    pub path: PathBuf,
    pub start_time_ms: i32,
    pub dim: f32,
}
#[derive(Debug, Clone, Copy)]
pub enum BackgroundKind {
    Image,
    Video,
}
