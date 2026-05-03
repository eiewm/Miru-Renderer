#[derive(Debug, Clone, Copy)]
pub struct RendererConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub scroll_speed: f32,
}
impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 60,
            scroll_speed: 0.0,
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    pub start_seconds: Option<f32>,
    pub end_seconds: Option<f32>,
    pub intro_config: Option<IntroConfig>,
}
#[derive(Debug, Clone, Default)]
pub struct IntroConfig {
    pub title: String,
    pub artist: String,
    pub mapper: String,
    pub diff_name: String,
    pub player: String,
    pub duration_ms: u32,
}
