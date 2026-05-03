// Gameplay coordinates use osu!'s 640x480 skin space and scale from height.
pub const GP_BASE_HEIGHT: f32 = 480.0;
#[inline]
pub fn get_gp_scale(canvas_h: u32) -> f32 {
    if canvas_h == 0 {
        return 0.0;
    }
    canvas_h as f32 / GP_BASE_HEIGHT
}
#[inline]
pub fn to_px_gp(v_gp: f32, scale: f32) -> i32 {
    (v_gp * scale).round() as i32
}
#[inline]
pub fn gp_point_to_px(x_gp: f32, y_gp: f32, scale: f32) -> (i32, i32) {
    (to_px_gp(x_gp, scale), to_px_gp(y_gp, scale))
}
