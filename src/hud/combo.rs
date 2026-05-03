#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ComboAnimType {
    None,
    Increment,
}
#[derive(Debug, Clone)]
pub struct NormalComboRenderData {
    pub value: u32,
    pub x: f32,
    pub y: f32,
    pub base_height: f32,
    pub height_scale: f32,
}
#[derive(Debug, Clone)]
pub struct CountdownComboRenderData {
    pub value: u32,
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub opacity: f32,
}
#[derive(Debug, Clone)]
pub struct RedPopupRenderData {
    pub value: u32,
    pub x: f32,
    pub y: f32,
    pub base_height: f32,
    pub scale: f32,
    pub opacity: f32,
    pub color: (u8, u8, u8),
}
#[derive(Debug, Clone, Default)]
pub struct ComboRenderData {
    pub normal: Option<NormalComboRenderData>,
    pub countdown: Option<CountdownComboRenderData>,
    pub red_popup: Option<RedPopupRenderData>,
}
const STRETCH_ANIM_DURATION: f32 = 120.0;
const STRETCH_PEAK_TIME: f32 = 0.2;
const STRETCH_AMOUNT: f32 = 0.10;
const POPUP_ANIM_DURATION: f32 = 270.0;
const POPUP_PEAK_TIME: f32 = 0.35;
const POPUP_MAX_SCALE: f32 = 2.0;
const POPUP_MIN_SCALE: f32 = 1.0;
const POPUP_START_OPACITY: f32 = 0.5;
const COUNTDOWN_FADE_MS: f32 = 3000.0;
fn calc_stretch_height_scale(anim_age: Option<i32>) -> f32 {
    let age = match anim_age {
        Some(a) if a >= 0 && (a as f32) < STRETCH_ANIM_DURATION => a as f32,
        _ => return 1.0,
    };
    let t = age / STRETCH_ANIM_DURATION;
    if t < STRETCH_PEAK_TIME {
        let progress = t / STRETCH_PEAK_TIME;
        let ease = 1.0 - (1.0 - progress).powi(2);
        1.0 + STRETCH_AMOUNT * ease
    } else {
        let progress = (t - STRETCH_PEAK_TIME) / (1.0 - STRETCH_PEAK_TIME);
        let ease = (1.0 - progress).powi(2);
        1.0 + STRETCH_AMOUNT * ease
    }
}
fn calc_popup_animation(elapsed: f32) -> Option<(f32, f32)> {
    if !(0.0..POPUP_ANIM_DURATION).contains(&elapsed) {
        return None;
    }
    let progress = elapsed / POPUP_ANIM_DURATION;
    let scale = if progress < POPUP_PEAK_TIME {
        let expand_progress = progress / POPUP_PEAK_TIME;
        let ease = if expand_progress < 0.5 {
            2.0 * expand_progress * expand_progress
        } else {
            1.0 - (-2.0 * expand_progress + 2.0).powi(2) / 2.0
        };
        POPUP_MIN_SCALE + (POPUP_MAX_SCALE - POPUP_MIN_SCALE) * ease
    } else {
        let shrink_progress = (progress - POPUP_PEAK_TIME) / (1.0 - POPUP_PEAK_TIME);
        let ease = shrink_progress * shrink_progress;
        POPUP_MAX_SCALE - (POPUP_MAX_SCALE - POPUP_MIN_SCALE) * ease
    };
    let opacity = (POPUP_START_OPACITY * (1.0 - progress)).max(0.0);
    Some((scale, opacity))
}
fn calc_countdown_opacity(current_value: u32, from_combo: u32, elapsed: f32) -> f32 {
    let fade_by_value = current_value as f32 / from_combo.max(1) as f32;
    let fade_by_time = (1.0 - elapsed / COUNTDOWN_FADE_MS).max(0.0);
    fade_by_value.min(fade_by_time)
}
pub fn compute_combo_render(
    combo: u32,
    combo_event_age: Option<i32>,
    combo_event_is_inc: bool,
    countdown: Option<(u32, u32, i32)>,
    break_anim: Option<(u32, i32)>,
    pos_x: f32,
    pos_y: f32,
    base_height: f32,
    visible: bool,
) -> ComboRenderData {
    if !visible {
        return ComboRenderData::default();
    }
    let mut data = ComboRenderData::default();
    if combo > 0 {
        let height_scale = if combo_event_is_inc {
            calc_stretch_height_scale(combo_event_age)
        } else {
            1.0
        };
        data.normal = Some(NormalComboRenderData {
            value: combo,
            x: pos_x,
            y: pos_y,
            base_height,
            height_scale,
        });
    }
    if combo == 0 {
        if let Some((current_value, from_combo, elapsed)) = countdown {
            if current_value > 0 {
                let opacity = calc_countdown_opacity(current_value, from_combo, elapsed as f32);
                if opacity > 0.05 {
                    data.countdown = Some(CountdownComboRenderData {
                        value: current_value,
                        x: pos_x,
                        y: pos_y,
                        height: base_height,
                        opacity,
                    });
                }
            }
        }
    }
    if let Some((start_combo, elapsed)) = break_anim {
        if let Some((scale, opacity)) = calc_popup_animation(elapsed as f32) {
            if opacity > 0.01 {
                data.red_popup = Some(RedPopupRenderData {
                    value: start_combo,
                    x: pos_x,
                    y: pos_y,
                    base_height,
                    scale,
                    opacity,
                    color: (255, 50, 50),
                });
            }
        }
    }
    data
}
