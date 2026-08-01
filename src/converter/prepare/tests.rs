use super::*;
use crate::types::SkinAssets;

fn layout_for(width: u32, height: u32, keys: u8) -> ManiaLayoutInfo {
    let settings = ConverterSettings {
        width,
        height,
        ..Default::default()
    };
    ManiaVideoConverter::new(settings).build_layout(keys, &SkinAssets::new(), None)
}

/// The bug this covers: an 8K chart rendered in 9:16 read its hit position as
/// 828 in a 768-tall space, clamped, and showed every note for a quarter of
/// the time the same replay got in 16:9.
#[test]
fn a_chart_stays_on_screen_as_long_in_both_video_shapes() {
    for keys in [4u8, 7, 8, 10] {
        let wide = layout_for(1920, 1080, keys);
        let tall = layout_for(1080, 1920, keys);
        let wide_ms = scroll_time_range_ms(25.0, wide.hit_position);
        let tall_ms = scroll_time_range_ms(25.0, tall.hit_position);
        assert!(
            (wide_ms - tall_ms).abs() / wide_ms < 0.02,
            "{keys}K: {wide_ms:.1}ms in 16:9 against {tall_ms:.1}ms in 9:16"
        );
    }
}

/// A default skin at speed 25 is the stable number, and doubling the speed
/// halves the time. Neither depends on the canvas.
#[test]
fn the_default_skin_lands_on_the_stable_number() {
    assert!((scroll_time_range_ms(25.0, 402.0) - 11_485.0 / 25.0).abs() < 0.001);
    assert!(
        (scroll_time_range_ms(50.0, 402.0) - scroll_time_range_ms(25.0, 402.0) / 2.0).abs()
            < 0.001
    );
}
