pub fn format_score(n: u32) -> String {
    let capped = n.min(1_000_000);
    format!("{:07}", capped)
}
#[derive(Debug, Clone)]
pub struct ScoreRenderData {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub height: f32,
}
pub fn compute_score_render(
    score: u32,
    canvas_width: f32,
    _canvas_height: f32,
    scale_y: f32,
    native_digit_height: f32,
    override_x: Option<f32>,
    override_y: Option<f32>,
    override_scale: Option<f32>,
) -> ScoreRenderData {
    let base_height = native_digit_height * scale_y;
    let height = override_scale
        .map(|s| base_height * s)
        .unwrap_or(base_height);
    let default_x = canvas_width - 10.0;
    let default_y = height / 2.0 + 5.0;
    let x = override_x.unwrap_or(default_x);
    let y = override_y.unwrap_or(default_y);
    ScoreRenderData {
        text: format_score(score),
        x,
        y,
        height,
    }
}
