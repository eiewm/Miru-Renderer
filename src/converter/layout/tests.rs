use super::*;
use crate::types::SkinAssets;

fn layout_for(width: u32, height: u32) -> ManiaLayoutInfo {
    layout_for_keys(width, height, 4)
}

fn layout_for_keys(width: u32, height: u32, keys: u8) -> ManiaLayoutInfo {
    let settings = ConverterSettings {
        width,
        height,
        ..Default::default()
    };
    ManiaVideoConverter::new(settings).build_layout(keys, &SkinAssets::new(), None)
}

/// The 9:16 playfield used to be a rectangle floating over the map art: the
/// HUD band pushed its top down and the skin's proportion left it short of
/// the bottom edge, so notes slid in above it and a strip of background was
/// left over below.
#[test]
fn the_vertical_playfield_reaches_both_edges() {
    let layout = layout_for(720, 1280);
    assert_eq!(layout.stage.view_top_y, 0);
    assert_eq!(layout.stage.view_bottom_y, 1280);
    assert_eq!(layout.view_height(), 1280);
}

/// The receptors sit on the far edge of the frame in 16:9, and used to float
/// well above it in 9:16. Same fraction of the canvas in both, or the same
/// chart reads as a different game depending on the shape of the video.
#[test]
fn the_hit_line_sits_at_the_same_height_in_both_shapes() {
    let wide = layout_for(1280, 720);
    let tall = layout_for(720, 1280);
    let wide_fraction = wide.stage.hit_y as f32 / 720.0;
    let tall_fraction = tall.stage.hit_y as f32 / 1280.0;
    assert!(
        (wide_fraction - tall_fraction).abs() < 0.005,
        "hit line at {wide_fraction:.4} of the frame in 16:9 and {tall_fraction:.4} in 9:16"
    );
}

/// The receptors and the life bar anchor to the stage's far edge, so it has
/// to be the edge of the frame. Anything else leaves both hanging in mid-air
/// with playfield underneath them.
#[test]
fn the_vertical_stage_ends_on_the_frame_edge() {
    let layout = layout_for(720, 1280);
    assert_eq!(layout.stage.bottom_y, 1280);
    assert!(
        layout.stage.top_y > 0,
        "the HUD band still keeps the notes out of the score"
    );
}

/// `mania-stage-bottom` and the side rails are authored against the skin's
/// 480-tall space, so they have to be sized by the drawn playfield. Sizing
/// them by `scale_y`, which in 9:16 comes from the width budget, put this
/// skin's top bar a tenth of the frame lower than the same skin puts it in
/// 16:9.
#[test]
fn stage_graphics_scale_by_the_drawn_playfield() {
    let tall = layout_for(720, 1280);
    assert!(
        (tall.furniture_scale() - 1280.0 / 480.0).abs() < 0.001,
        "9:16 furniture scale was {}",
        tall.furniture_scale()
    );
    for (w, h) in [(1280, 720), (1920, 1080), (1080, 960)] {
        let wide = layout_for(w, h);
        assert!(
            (wide.furniture_scale() - wide.scale_y).abs() < 0.001,
            "{w}x{h}: furniture scale {} left scale_y {}",
            wide.furniture_scale(),
            wide.scale_y
        );
    }
}

/// The number the scroll calibration reads. It used to be recovered by
/// dividing `hit_y` by `scale_y`, which only holds where both come from the
/// height: an 8K chart in 9:16 gave 828 against a 768-tall legacy space, the
/// calibration clamped, and the notes went out four times too fast.
#[test]
fn the_hit_position_is_the_skins_own_number_on_every_canvas() {
    for keys in [4u8, 7, 8, 10] {
        for (w, h) in [(1920, 1080), (1280, 720), (1080, 1920), (720, 1280)] {
            let layout = layout_for_keys(w, h, keys);
            assert!(
                (layout.hit_position - 402.0).abs() < 1.0,
                "{keys}K at {w}x{h} calibrated against {}",
                layout.hit_position
            );
        }
    }
}

/// Where the receptors hang from. In 16:9 the stage edge and the hit line are
/// `key_area_px` apart by construction, and reading the edge instead worked;
/// in 9:16 the edge is the frame and the columns keep their own scale, so the
/// same reading put the hit line at the very top of the receptor sprite
/// instead of halfway down it.
#[test]
fn the_receptors_hang_from_the_hit_line_by_the_same_gap() {
    for keys in [4u8, 7, 8, 10] {
        for (w, h) in [(1920, 1080), (1280, 720), (1080, 960)] {
            let wide = layout_for_keys(w, h, keys);
            assert_eq!(
                wide.stage.hit_y + wide.key_area_px,
                wide.stage.bottom_y,
                "{keys}K at {w}x{h}: nothing about a horizontal render may move",
            );
        }

        let tall = layout_for_keys(1080, 1920, keys);
        // The canvas edge sits well below the skin gap, which is what shifted receptors down.
        assert!(
            tall.stage.hit_y + tall.key_area_px < tall.stage.bottom_y,
            "{keys}K in 9:16 read the same edge as 16:9",
        );
    }
}

/// In 16:9 the box already filled the canvas, so the two have to be the same
/// pair of numbers and nothing about a horizontal render can move.
#[test]
fn the_horizontal_view_is_exactly_the_skin_box() {
    for (w, h) in [(1280, 720), (1920, 1080), (1080, 960)] {
        let layout = layout_for(w, h);
        assert_eq!(
            (layout.stage.view_top_y, layout.stage.view_bottom_y),
            (layout.stage.top_y, layout.stage.bottom_y),
            "{w}x{h} moved"
        );
        assert_eq!(layout.view_height(), layout.stage.height, "{w}x{h} moved");
    }
}
