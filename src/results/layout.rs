// Results coordinates are authored against osu!'s 1024x768-era skin layout and scale by height.
const REFERENCE_HEIGHT: f32 = 768.0;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LayoutPoint {
    pub(crate) x: i32,
    pub(crate) y: i32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LayoutRect {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) w: u32,
    pub(crate) h: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JudgmentRowLayout {
    pub(crate) label_center: LayoutPoint,
    pub(crate) count_origin: LayoutPoint,
    pub(crate) max_icon_size: (u32, u32),
}
#[derive(Debug, Clone)]
pub(crate) struct ResultsLayout {
    pub(crate) scale: f32,
    pub(crate) title_line_origins: [LayoutPoint; 3],
    pub(crate) panel_anchor: LayoutPoint,
    pub(crate) score_origin: LayoutPoint,
    pub(crate) grade_center: LayoutPoint,
    pub(crate) title_right_inset: i32,
    pub(crate) title_top: i32,
    pub(crate) mod_badges_first_center: LayoutPoint,
    pub(crate) mod_badges_step_x: i32,
    pub(crate) judgment_rows: [JudgmentRowLayout; 6],
    pub(crate) graph_rect: LayoutRect,
    pub(crate) timing_box_origin: LayoutPoint,
    pub(crate) timing_line_gap: i32,
    pub(crate) accuracy_label_anchor: LayoutPoint,
    pub(crate) accuracy_value_origin: LayoutPoint,
    pub(crate) combo_label_anchor: LayoutPoint,
    pub(crate) combo_value_origin: LayoutPoint,
    pub(crate) perfect_center: LayoutPoint,
}
fn scaled(scale: f32, value: f32) -> i32 {
    (value * scale).round() as i32
}
fn scaled_u32(scale: f32, value: f32) -> u32 {
    (value * scale).round().max(1.0) as u32
}
fn point(scale: f32, x: f32, y: f32) -> LayoutPoint {
    LayoutPoint {
        x: scaled(scale, x),
        y: scaled(scale, y),
    }
}
fn rect(scale: f32, x: f32, y: f32, w: f32, h: f32) -> LayoutRect {
    LayoutRect {
        x: scaled(scale, x),
        y: scaled(scale, y),
        w: scaled_u32(scale, w),
        h: scaled_u32(scale, h),
    }
}
fn judgment_row(scale: f32, label_center_x: f32, row_center_y: f32) -> JudgmentRowLayout {
    JudgmentRowLayout {
        label_center: point(scale, label_center_x, row_center_y),
        count_origin: point(scale, label_center_x + 64.0, row_center_y - 26.0),
        max_icon_size: (scaled_u32(scale, 108.0), scaled_u32(scale, 46.0)),
    }
}
pub(crate) fn compute_results_layout(output_w: u32, output_h: u32) -> ResultsLayout {
    let scale = (output_h.max(1) as f32 / REFERENCE_HEIGHT).max(f32::EPSILON);
    ResultsLayout {
        scale,
        title_line_origins: [
            point(scale, 5.0, 27.0),
            point(scale, 5.0, 52.0),
            point(scale, 5.0, 74.0),
        ],
        panel_anchor: point(scale, 0.0, 102.0),
        score_origin: point(scale, 160.0, 126.0),
        grade_center: LayoutPoint {
            // The rank sprite stays pinned to the right edge on ultrawide outputs.
            x: output_w as i32 - scaled(scale, 192.0),
            y: scaled(scale, 320.0),
        },
        title_right_inset: scaled(scale, 32.0),
        title_top: 0,
        mod_badges_first_center: LayoutPoint {
            x: output_w as i32 - scaled(scale, 64.0),
            y: scaled(scale, 416.0),
        },
        mod_badges_step_x: -scaled(scale, 32.0),
        judgment_rows: [
            judgment_row(scale, 64.0, 256.0),
            judgment_row(scale, 64.0, 352.0),
            judgment_row(scale, 64.0, 448.0),
            judgment_row(scale, 384.0, 256.0),
            judgment_row(scale, 384.0, 352.0),
            judgment_row(scale, 384.0, 448.0),
        ],
        graph_rect: rect(scale, 256.0, 608.0, 308.0, 156.0),
        timing_box_origin: point(scale, 582.0, 662.0),
        timing_line_gap: scaled(scale, 14.0),
        accuracy_label_anchor: point(scale, 291.0, 480.0),
        accuracy_value_origin: point(scale, 310.0, 528.0),
        combo_label_anchor: point(scale, 8.0, 480.0),
        combo_value_origin: point(scale, 24.0, 528.0),
        perfect_center: point(scale, 416.0, 688.0),
    }
}
