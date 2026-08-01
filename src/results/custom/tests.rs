use super::*;

#[test]
fn parses_every_hex_color_shape() {
    assert_eq!(parse_color(Some("#fff"), [0, 0, 0, 9]), [255, 255, 255, 9]);
    assert_eq!(parse_color(Some("#102030"), [0, 0, 0, 7]), [16, 32, 48, 7]);
    assert_eq!(parse_color(Some("#10203040"), [0; 4]), [16, 32, 48, 64]);
    // Junk falls back instead of painting black.
    assert_eq!(parse_color(Some("nope"), [1, 2, 3, 4]), [1, 2, 3, 4]);
    assert_eq!(parse_color(None, [1, 2, 3, 4]), [1, 2, 3, 4]);
}

#[test]
fn converts_windows_ticks_to_a_civil_date() {
    // 2023-08-08, the date on the test replay with the Japanese title.
    let ticks = (62_135_596_800i64 + 1_691_452_800) * 10_000_000;
    let days = (ticks / 10_000_000 - 62_135_596_800) / 86_400;
    assert_eq!(civil_from_days(days), (2023, 8, 8));
}

#[test]
fn maps_the_unix_epoch() {
    assert_eq!(civil_from_days(0), (1970, 1, 1));
}

#[test]
fn scales_type_by_the_smaller_axis() {
    // Scaling type per axis would stretch the letters.
    let scene = HudResultsSceneConfig {
        space: Some(crate::hud::HudSpaceConfig {
            width: Some(1280.0),
            height: Some(720.0),
        }),
        layers: Vec::new(),
        mode: None,
    };
    let space = Space::new(&scene, 2560, 1440);
    assert_eq!(space.font(10.0), 20.0);

    // A canvas wider than it is tall cannot grow type by its width.
    let space = Space::new(&scene, 2560, 720);
    assert_eq!(space.font(10.0), 10.0);
}

fn media_with(delays: &[u32]) -> ResultsMedia {
    let frames = delays
        .iter()
        .enumerate()
        .map(|(index, delay_ms)| MediaFrame {
            image: RgbaImage::from_pixel(1, 1, Rgba([index as u8, 0, 0, 255])),
            delay_ms: *delay_ms,
        })
        .collect();
    ResultsMedia {
        by_asset: HashMap::from([("logo".to_string(), frames)]),
    }
}

fn frame_index(media: &ResultsMedia, elapsed_ms: u32) -> u8 {
    media.frame_at("logo", elapsed_ms).unwrap().get_pixel(0, 0)[0]
}

#[test]
fn a_still_image_ignores_the_clock() {
    let media = media_with(&[0]);
    assert_eq!(frame_index(&media, 0), 0);
    assert_eq!(frame_index(&media, 4_800), 0);
}

#[test]
fn a_gif_walks_its_frames_and_loops() {
    let media = media_with(&[100, 100, 100]);
    assert_eq!(frame_index(&media, 0), 0);
    assert_eq!(frame_index(&media, 150), 1);
    assert_eq!(frame_index(&media, 250), 2);
    // 300 ms is a full turn, so it starts over.
    assert_eq!(frame_index(&media, 300), 0);
    assert_eq!(frame_index(&media, 350), 0);
    assert_eq!(frame_index(&media, 450), 1);
}

#[test]
fn a_zero_delay_gif_still_advances() {
    // Without the 20 ms floor every frame would land on the first one.
    let media = media_with(&[0, 0]);
    assert_eq!(frame_index(&media, 0), 0);
    assert_eq!(frame_index(&media, 25), 1);
}

#[test]
fn an_unknown_asset_draws_nothing() {
    assert!(media_with(&[0]).frame_at("missing", 0).is_none());
}

#[test]
fn reads_the_asset_id_from_either_spelling() {
    let mut layer = HudLayerConfig {
        layer_type: "media.image".to_string(),
        ..Default::default()
    };
    layer.props = serde_json::json!({ "assetId": "logo" });
    assert_eq!(layer_asset_id(&layer), Some("logo"));
    layer.props = serde_json::json!({ "asset_id": "logo" });
    assert_eq!(layer_asset_id(&layer), Some("logo"));
    layer.props = serde_json::json!({ "assetId": "   " });
    assert_eq!(layer_asset_id(&layer), None);
}

#[test]
fn a_turned_sprite_lands_where_the_editor_shows_it() {
    // A 2x1 red bar turned a quarter turn has to come out 1x2.
    let mut sprite = RgbaImage::from_pixel(4, 2, Rgba([255, 0, 0, 255]));
    sprite.put_pixel(0, 0, Rgba([0, 0, 255, 255]));
    let mut canvas = RgbaImage::from_pixel(12, 12, Rgba([0, 0, 0, 255]));
    blit_rotated(&mut canvas, &sprite, 4, 5, 90.0, 1.0);
    let painted: Vec<(u32, u32)> = canvas
        .enumerate_pixels()
        .filter(|(_, _, pixel)| pixel[0] > 0 || pixel[2] > 0)
        .map(|(x, y, _)| (x, y))
        .collect();
    let min_x = painted.iter().map(|(x, _)| *x).min().unwrap();
    let max_x = painted.iter().map(|(x, _)| *x).max().unwrap();
    let min_y = painted.iter().map(|(_, y)| *y).min().unwrap();
    let max_y = painted.iter().map(|(_, y)| *y).max().unwrap();
    assert_eq!((max_x - min_x + 1, max_y - min_y + 1), (2, 4));
    // Centred on the same point the straight blit would have used.
    assert_eq!(
        ((min_x + max_x).div_ceil(2), (min_y + max_y).div_ceil(2)),
        (6, 6)
    );
}

#[test]
fn no_turn_means_the_plain_blit() {
    let sprite = RgbaImage::from_pixel(3, 3, Rgba([10, 20, 30, 255]));
    let mut turned = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 255]));
    let mut straight = turned.clone();
    blit_placed(&mut turned, &sprite, 2, 2, 0.0, 1.0);
    blit(&mut straight, &sprite, 2, 2, 1.0);
    assert_eq!(turned, straight);
}

#[test]
fn spin_turns_the_layer_as_the_screen_runs() {
    let mut layer = HudLayerConfig::default();
    layer.transform.rotation = 10.0;
    assert_eq!(layer_rotation_degrees(&layer, 2_000), 10.0);
    layer.props = serde_json::json!({ "spinEnabled": true, "spinSpeed": 90.0 });
    assert_eq!(layer_rotation_degrees(&layer, 2_000), 190.0);
    layer.props = serde_json::json!({ "spinEnabled": true });
    assert_eq!(layer_rotation_degrees(&layer, 1_000), 100.0);
}

#[test]
fn a_piece_nested_in_a_group_still_counts() {
    let leaf = |layer_type: &str| HudLayerConfig {
        layer_type: layer_type.to_string(),
        ..Default::default()
    };
    let group = |child: HudLayerConfig| HudLayerConfig {
        layer_type: "group".to_string(),
        children: vec![child],
        ..Default::default()
    };
    let scene = |layers: Vec<HudLayerConfig>| HudResultsSceneConfig {
        space: None,
        layers,
        mode: None,
    };
    assert!(scene_uses_elements(&scene(vec![leaf("results.rank")])));
    assert!(scene_uses_elements(&scene(vec![group(leaf(
        "results.judgment300"
    ))])));
    assert!(!scene_uses_elements(&scene(vec![leaf("text.static")])));
    assert!(!scene_uses_elements(&scene(vec![group(leaf("media.image"))])));
    assert!(!scene_uses_elements(&scene(Vec::new())));
}

#[test]
fn only_replace_swaps_the_built_in_screen() {
    let scene = |mode: Option<&str>| HudResultsSceneConfig {
        space: None,
        layers: Vec::new(),
        mode: mode.map(str::to_string),
    };
    assert!(scene(Some("replace")).replaces_default_screen());
    assert!(scene(Some("REPLACE")).replaces_default_screen());
    assert!(!scene(Some("overlay")).replaces_default_screen());
    // An old design with no mode draws over the screen, which is what the
    // editor shows behind it.
    assert!(!scene(None).replaces_default_screen());
    assert!(!scene(Some("junk")).replaces_default_screen());
}

#[test]
fn falls_back_to_the_real_canvas_when_the_scene_has_no_space() {
    let scene = HudResultsSceneConfig {
        space: None,
        layers: Vec::new(),
        mode: None,
    };
    let space = Space::new(&scene, 1280, 720);
    assert_eq!(space.x(100.0), 100);
    assert_eq!(space.y(100.0), 100);
}
