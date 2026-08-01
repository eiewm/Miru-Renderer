use super::config::{HudElementConfig, HudElements};
use super::HudConfig;
/// Height of the HUD strip in 9:16, as a fraction of the canvas.
///
/// A 16:9 HUD does not translate to a phone canvas: the score sits over the lane
/// where the notes come in and the combo floats in the middle of the playfield,
/// so both collide with the chart whenever it gets dense. In vertical there is
/// height to spare, so the HUD gets a strip of its own at the top and the stage
/// starts below it. It costs under a tenth of the track and nothing ever overlaps.
const BAND_FRACTION: f32 = 0.085;
/// Share of the strip given to the score.
/// A size, not a reservation: the scale divides this by the sprite height.
const SCORE_SHARE: f32 = 0.33;
/// Gap between the two, in canvas pixels.
const STACK_GAP: f32 = 8.0;
/// Height of that strip in pixels, or `None` when the canvas is not vertical.
pub fn vertical_hud_band(canvas_w: u32, canvas_h: u32) -> Option<u32> {
    (canvas_w < canvas_h).then(|| (canvas_h as f32 * BAND_FRACTION).round().max(1.0) as u32)
}
/// Solo posicion vertical.
/// `x` is left alone so the progress circle keeps clear of the first digit.
///
fn element(y: f32, height: Option<f32>) -> HudElementConfig {
    HudElementConfig {
        y: Some(y),
        height,
        ..Default::default()
    }
}
/// Where each piece sits inside the strip, already in canvas pixels.
/// Only the score and the accuracy: combo and judgement belong to the skin.
fn defaults(canvas_w: u32, canvas_h: u32) -> Option<HudElements> {
    let band = vertical_hud_band(canvas_w, canvas_h)? as f32;
    let score_h = (band * SCORE_SHARE).round();
    let top = (band * 0.10).round().max(2.0);
    Some(HudElements {
        score: Some(element(top, Some(score_h))),
        accuracy: Some(element(top + score_h + STACK_GAP, None)),
        ..Default::default()
    })
}
/// Field-by-field merge, with whatever the user authored winning.
fn merge(user: Option<&HudElementConfig>, base: Option<HudElementConfig>) -> Option<HudElementConfig> {
    match (user, base) {
        (None, base) => base,
        (Some(user), None) => Some(user.clone()),
        (Some(user), Some(base)) => Some(HudElementConfig {
            x: user.x.or(base.x),
            y: user.y.or(base.y),
            width: user.width.or(base.width),
            height: user.height.or(base.height),
            size: user.size.or(base.size),
            ..user.clone()
        }),
    }
}
/// Drops the vertical defaults under a resolved config, if there is one at all.
pub fn with_vertical_defaults(
    resolved: Option<HudConfig>,
    canvas_w: u32,
    canvas_h: u32,
) -> Option<HudConfig> {
    let Some(base) = defaults(canvas_w, canvas_h) else {
        return resolved;
    };
    let mut config = resolved.unwrap_or_default();
    config.elements = HudElements {
        score: merge(config.elements.score.as_ref(), base.score),
        accuracy: merge(config.elements.accuracy.as_ref(), base.accuracy),
        ..config.elements
    };
    Some(config)
}
#[cfg(test)]
mod tests;
