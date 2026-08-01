use super::*;
fn reference_block_width(layout: &ResultsLayout) -> f32 {
    REFERENCE_WIDTH * layout.scale
}
#[test]
fn horizontal_layouts_are_unchanged() {
    for (w, h) in [(1280u32, 720u32), (1920, 1080)] {
        let layout = compute_results_layout(w, h);
        // Scale still comes from the height and nothing is pushed down.
        assert!((layout.scale - h as f32 / REFERENCE_HEIGHT).abs() < f32::EPSILON);
        assert_eq!(layout.title_top, 0);
    }
}
#[test]
fn vertical_layout_fits_the_canvas() {
    for (w, h) in [(1080u32, 1920u32), (720, 1280)] {
        let layout = compute_results_layout(w, h);
        assert!(reference_block_width(&layout) <= w as f32 + 0.5);
    }
}
#[test]
fn vertical_layout_is_centred() {
    let (w, h) = (1080u32, 1920u32);
    let layout = compute_results_layout(w, h);
    let block = REFERENCE_HEIGHT * layout.scale;
    let below = h as f32 - block - layout.title_top as f32;
    assert!((below - layout.title_top as f32).abs() <= 1.0);
}
