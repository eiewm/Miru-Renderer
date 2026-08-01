use super::*;
use crate::hud::{HudLayerConfig, HudLayerTransformConfig, HudSpaceConfig};
const W: u32 = 640;
const H: u32 = 480;
fn opaque_background() -> Vec<u8> {
    (0..W * H * 4)
        .map(|index| match index % 4 {
            0 => 40,
            1 => 30,
            2 => 60,
            _ => 255,
        })
        .collect()
}
fn renderer(hud: Option<&HudConfig>) -> ResultsSceneRenderer {
    ResultsSceneRenderer::new(
        &opaque_background(),
        W,
        H,
        &SkinAssets::new(),
        &super::super::sample_results_screen_data(),
        hud,
    )
}
fn scene_of(sprites: &[ResultsElementSprite], skip: &[&str]) -> HudConfig {
    scene_from(sprites, skip, |rect| rect)
}
/// `warp` fakes an editor that saw the piece at another size, which is what
/// happens whenever the preview and the render are not the same play.
fn scene_from(
    sprites: &[ResultsElementSprite],
    skip: &[&str],
    warp: impl Fn([f32; 4]) -> [f32; 4],
) -> HudConfig {
    let layers = sprites
        .iter()
        .filter(|sprite| !skip.contains(&sprite.element.id()))
        .enumerate()
        .map(|(index, sprite)| {
            let [x, y, width, height] = warp([
                sprite.x as f32,
                sprite.y as f32,
                sprite.image.width() as f32,
                sprite.image.height() as f32,
            ]);
            HudLayerConfig {
                layer_type: sprite.element.id().to_string(),
                visible: true,
                z_index: index as i32,
                transform: HudLayerTransformConfig {
                    x,
                    y,
                    width,
                    height,
                    rotation: 0.0,
                    opacity: 1.0,
                },
                props: serde_json::json!({
                    "baseX": x,
                    "baseY": y,
                    "baseWidth": width,
                    "baseHeight": height,
                }),
                ..Default::default()
            }
        })
        .collect();
    HudConfig {
        results: Some(HudResultsSceneConfig {
            space: Some(HudSpaceConfig {
                width: Some(W as f32),
                height: Some(H as f32),
            }),
            layers,
            mode: None,
        }),
        ..Default::default()
    }
}
/// Skin art is missing here, so the pieces that survive are the ones the
/// painter draws itself.
#[test]
fn every_piece_of_the_screen_becomes_a_sprite() {
    let sprites = renderer(None).element_sprites();
    let ids: Vec<&str> = sprites.iter().map(|sprite| sprite.element.id()).collect();
    for expected in [
        "results.background",
        "results.titleBar",
        "results.title",
        "results.mapper",
        "results.player",
        "results.score",
        "results.rank",
        "results.judgment300",
        "results.judgment200",
        "results.judgment50",
        "results.judgmentMax",
        "results.judgment100",
        "results.judgmentMiss",
        "results.timing",
        "results.accuracy",
        "results.combo",
        // Drawn even on a play that dropped combo, or it could never be placed.
        "results.perfect",
    ] {
        assert!(ids.contains(&expected), "missing {expected}");
    }
}
#[test]
fn seeding_every_piece_draws_the_very_same_screen() {
    let default_screen = renderer(None);
    let sprites = default_screen.element_sprites();
    let hud = scene_of(&sprites, &[]);
    let expected = default_screen.render_with_progress(1.0);
    let composed = renderer(Some(&hud)).render_with_progress(1.0);
    assert_eq!(expected.len(), composed.len());
    // Off by one at most: a translucent piece rounds to 8 bits once on its
    // own sprite and once on the canvas, which is a difference nobody sees.
    let worst = expected
        .iter()
        .zip(composed.iter())
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);
    assert!(worst <= 1, "channel drifted by {worst}");
}
/// The editor sizes the pieces from a preview, and the preview is rarely the
/// same play as the video: a longer title, another rank, a life graph that
/// never dips. Whatever box it recorded, an untouched piece has to come out
/// at its own size, or the title arrives squashed and the graph line turns
/// into a block.
#[test]
fn a_piece_the_editor_measured_wrong_still_comes_out_right() {
    let default_screen = renderer(None);
    let sprites = default_screen.element_sprites();
    let expected = default_screen.render_with_progress(1.0);
    for warp in [
        // Narrow and short, like a full combo graph.
        |[x, y, w, h]: [f32; 4]| [x, y, w / 3.0, h / 8.0],
        // Much wider, like a genuinely long title.
        |[x, y, w, h]: [f32; 4]| [x, y, w * 3.0, h],
    ] {
        let hud = scene_from(&sprites, &[], warp);
        let composed = renderer(Some(&hud)).render_with_progress(1.0);
        let worst = expected
            .iter()
            .zip(composed.iter())
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        assert!(worst <= 1, "channel drifted by {worst}");
    }
}
#[test]
fn resizing_a_piece_scales_it_from_its_own_size() {
    let default_screen = renderer(None);
    let sprites = default_screen.element_sprites();
    let rank = sprites
        .iter()
        .find(|sprite| sprite.element == ResultsElement::Rank)
        .expect("rank sprite");
    // Base halves it and the node doubles it: four times larger.
    let hud = scene_from(&sprites, &[], |[x, y, w, h]| [x, y, w / 2.0, h / 2.0]);
    let mut layers = hud.results.clone().expect("scene").layers;
    for layer in &mut layers {
        if layer.layer_type == ResultsElement::Rank.id() {
            layer.transform.width *= 4.0;
            layer.transform.height *= 4.0;
        }
    }
    let hud = HudConfig {
        results: Some(HudResultsSceneConfig {
            layers,
            ..hud.results.unwrap()
        }),
        ..Default::default()
    };
    let with_rank = renderer(Some(&hud)).render_with_progress(1.0);
    // Removing it changes the range and nothing else.
    let without_rank = renderer(Some(&scene_from(
        &sprites,
        &[ResultsElement::Rank.id()],
        |[x, y, w, h]| [x, y, w / 2.0, h / 2.0],
    )))
    .render_with_progress(1.0);
    let columns: Vec<u32> = (0..W)
        .filter(|x| {
            (0..H).any(|y| {
                let index = ((y * W + x) * 4) as usize;
                with_rank[index..index + 4] != without_rank[index..index + 4]
            })
        })
        .collect();
    let width = columns.last().unwrap() - columns.first().unwrap() + 1;
    // Four times the sprite width, not four times the base.
    let expected = rank.image.width() * 4;
    assert!(
        width.abs_diff(expected) <= 2,
        "rank came out {width} wide, expected about {expected}"
    );
}
#[test]
fn dropping_a_piece_takes_it_off_the_screen() {
    let default_screen = renderer(None);
    let sprites = default_screen.element_sprites();
    let whole = scene_of(&sprites, &[]);
    let without_rank = scene_of(&sprites, &["results.rank"]);
    let whole_frame = renderer(Some(&whole)).render_with_progress(1.0);
    let cut_frame = renderer(Some(&without_rank)).render_with_progress(1.0);
    assert_ne!(whole_frame, cut_frame);
    // What is gone is the rank and nothing else: the rest still matches.
    let rank = sprites
        .iter()
        .find(|sprite| sprite.element == ResultsElement::Rank)
        .expect("rank sprite");
    let mut differing_rows = 0usize;
    for y in 0..H {
        let row = (y * W * 4) as usize..((y + 1) * W * 4) as usize;
        if whole_frame[row.clone()] != cut_frame[row] {
            differing_rows += 1;
            assert!(
                (y as i32) >= rank.y && (y as i32) < rank.y + rank.image.height() as i32,
                "row {y} changed outside the rank"
            );
        }
    }
    assert!(differing_rows > 0);
}
#[test]
fn a_design_with_no_pieces_still_draws_the_screen_underneath() {
    let mut hud = HudConfig::default();
    hud.results = Some(HudResultsSceneConfig {
        space: None,
        layers: vec![HudLayerConfig {
            layer_type: "text.static".to_string(),
            visible: true,
            ..Default::default()
        }],
        mode: None,
    });
    // An old design overlays; the built-in screen has to stay behind it.
    assert_eq!(
        renderer(Some(&hud)).render_with_progress(1.0),
        renderer(None).render_with_progress(1.0)
    );
}
