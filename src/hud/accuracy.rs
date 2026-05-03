use crate::types::JudgmentKind;
#[derive(Debug, Clone)]
pub struct AccuracyRenderData {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub value: f32,
}
#[derive(Debug, Clone)]
pub struct ProgressCircleRenderData {
    pub progress: f32,
    pub x: f32,
    pub y: f32,
    pub size: f32,
}
pub fn format_accuracy(acc: f32) -> String {
    let pct = (acc * 100.0).clamp(0.0, 100.0);
    format!("{:.2}%", pct)
}
pub fn calc_accuracy(weighted_sum: u32, judged_count: u32, is_score_v2: bool) -> f32 {
    if judged_count == 0 {
        return 1.0;
    }
    let max_per_note = if is_score_v2 { 305 } else { 300 };
    weighted_sum as f32 / (judged_count * max_per_note) as f32
}
pub fn acc_weight(kind: JudgmentKind, is_score_v2: bool) -> u32 {
    match kind {
        JudgmentKind::Max => {
            if is_score_v2 {
                305
            } else {
                300
            }
        }
        JudgmentKind::Hit300 => 300,
        JudgmentKind::Hit200 => 200,
        JudgmentKind::Hit100 => 100,
        JudgmentKind::Hit50 => 50,
        JudgmentKind::Miss => 0,
    }
}
pub fn compute_accuracy_render(
    accuracy: f32,
    canvas_width: f32,
    _canvas_height: f32,
    scale_y: f32,
    native_digit_height: f32,
    override_x: Option<f32>,
    override_y: Option<f32>,
    override_scale: Option<f32>,
) -> AccuracyRenderData {
    let base_height = native_digit_height * scale_y * 0.6;
    let height = override_scale
        .map(|s| base_height * s)
        .unwrap_or(base_height);
    let default_x = canvas_width - 10.0;
    let default_y = native_digit_height * scale_y + height / 2.0 + 10.0;
    let x = override_x.unwrap_or(default_x);
    let y = override_y.unwrap_or(default_y);
    AccuracyRenderData {
        text: format_accuracy(accuracy),
        x,
        y,
        height,
        value: accuracy,
    }
}
pub fn compute_progress_circle_render(
    progress: f32,
    canvas_width: f32,
    canvas_height: f32,
    override_x: Option<f32>,
    override_y: Option<f32>,
    override_size: Option<f32>,
) -> ProgressCircleRenderData {
    let default_size = 50.0;
    let size = override_size.unwrap_or(default_size);
    let default_x = canvas_width - size / 2.0 - 10.0;
    let default_y = canvas_height / 2.0;
    let x = override_x.unwrap_or(default_x);
    let y = override_y.unwrap_or(default_y);
    ProgressCircleRenderData {
        progress: progress.clamp(0.0, 1.0),
        x,
        y,
        size,
    }
}
