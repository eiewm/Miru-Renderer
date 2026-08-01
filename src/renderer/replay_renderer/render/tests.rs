use super::*;
#[test]
fn widescreen_still_scales_by_height() {
    // 16:9 is wider than the 4:3 reference, so the width cap never bites here.
    for (w, h) in [(1280u32, 720u32), (1920, 1080)] {
        let expected = h as f32 / SCREEN_RIGHT_HUD_REFERENCE_HEIGHT;
        assert!(
            (screen_right_hud_scale(w, h) - expected).abs() < f32::EPSILON,
            "{w}x{h} deberia seguir saliendo del alto"
        );
    }
}
#[test]
fn canvases_narrower_than_four_thirds_are_bounded_by_width() {
    // Versus halves are near square, so width is what keeps the HUD in proportion.
    for (w, h) in [(1080u32, 960u32), (1080, 864)] {
        let expected = w as f32 / SCREEN_RIGHT_HUD_REFERENCE_WIDTH;
        assert!((screen_right_hud_scale(w, h) - expected).abs() < f32::EPSILON);
    }
}
#[test]
fn vertical_sits_between_the_two_bounds() {
    // Geometric mean: height alone oversizes the circle, width alone shrinks accuracy.
    for (w, h) in [(1080u32, 1920u32), (720, 1280)] {
        let by_width = w as f32 / SCREEN_RIGHT_HUD_REFERENCE_WIDTH;
        let by_height = h as f32 / SCREEN_RIGHT_HUD_REFERENCE_HEIGHT;
        let scale = screen_right_hud_scale(w, h);
        assert!(scale > by_width, "{w}x{h}: {scale} no crecio sobre {by_width}");
        assert!(scale < by_height, "{w}x{h}: {scale} no quedo bajo {by_height}");
        assert!((scale - (by_width * by_height).sqrt()).abs() < 1e-5);
    }
}
