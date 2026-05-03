use crate::types::JudgmentKind;
pub fn judgment_sprite_base(kind: JudgmentKind) -> &'static str {
    match kind {
        JudgmentKind::Miss => "mania-hit0",
        JudgmentKind::Hit50 => "mania-hit50",
        JudgmentKind::Hit100 => "mania-hit100",
        JudgmentKind::Hit200 => "mania-hit200",
        JudgmentKind::Hit300 => "mania-hit300",
        JudgmentKind::Max => "mania-hit300g",
    }
}
#[derive(Debug, Clone)]
pub struct JudgmentAnimRenderData {
    pub sprite_name: String,
    pub sprite_base: String,
    pub frame_index: u32,
    pub total_frames: u32,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub opacity: f32,
    pub width: f32,
    pub height: f32,
}
pub fn compute_judgment_anim(
    kind: JudgmentKind,
    age: i32,
    center_x: f32,
    center_y: f32,
    base_width: f32,
    base_height: f32,
    animation_fps: f32,
    total_frames: u32,
    override_x: Option<f32>,
    override_y: Option<f32>,
    override_scale: Option<f32>,
) -> Option<JudgmentAnimRenderData> {
    let sprite_base = judgment_sprite_base(kind);
    let frame_time = 1000.0 / animation_fps.max(1.0);
    let max_age = if total_frames > 1 {
        (total_frames as f32 * frame_time) as i32
    } else {
        200
    };
    if age < 0 || age >= max_age {
        return None;
    }
    let age_f = age as f32;
    let max_age_f = max_age as f32;
    let frame_index = if total_frames > 1 {
        ((age_f / frame_time) as u32).min(total_frames - 1)
    } else {
        0
    };
    // Animated judgment skins use base-0.png, base-1.png, ...; static skins use base.png.
    let sprite_name = if total_frames > 1 {
        format!("{}-{}.png", sprite_base, frame_index)
    } else {
        format!("{}.png", sprite_base)
    };
    let opacity = (1.0 - age_f / max_age_f).max(0.0);
    let half = max_age_f / 2.0;
    let scale_anim = if age_f < half {
        1.0 + 0.15 * (age_f / half)
    } else {
        1.15 - 0.15 * ((age_f - half) / half)
    };
    let base_scale = override_scale.unwrap_or(1.0);
    let scale = scale_anim * base_scale;
    let width = base_width * scale;
    let height = base_height * scale;
    let x = override_x.unwrap_or(center_x);
    let y = override_y.unwrap_or(center_y);
    Some(JudgmentAnimRenderData {
        sprite_name,
        sprite_base: sprite_base.to_string(),
        frame_index,
        total_frames,
        x,
        y,
        scale,
        opacity,
        width,
        height,
    })
}
