// Results coordinates are authored against osu!'s 1024x768-era skin layout.
const REFERENCE_WIDTH: f32 = 1024.0;
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
    /// Bottom of the 1024x768 reference block, in canvas pixels.
    pub(crate) block_bottom: i32,
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
/// Where the reference canvas sits inside the real one.
#[derive(Debug, Clone, Copy)]
struct Frame {
    scale: f32,
    top: i32,
}
impl Frame {
    fn point(self, x: f32, y: f32) -> LayoutPoint {
        LayoutPoint {
            x: scaled(self.scale, x),
            y: self.top + scaled(self.scale, y),
        }
    }
    fn rect(self, x: f32, y: f32, w: f32, h: f32) -> LayoutRect {
        LayoutRect {
            x: scaled(self.scale, x),
            y: self.top + scaled(self.scale, y),
            w: scaled_u32(self.scale, w),
            h: scaled_u32(self.scale, h),
        }
    }
    fn judgment_row(self, label_center_x: f32, row_center_y: f32) -> JudgmentRowLayout {
        JudgmentRowLayout {
            label_center: self.point(label_center_x, row_center_y),
            count_origin: self.point(label_center_x + 64.0, row_center_y - 26.0),
            max_icon_size: (scaled_u32(self.scale, 108.0), scaled_u32(self.scale, 46.0)),
        }
    }
}
pub(crate) fn compute_results_layout(output_w: u32, output_h: u32) -> ResultsLayout {
    // Scaling by height alone holds while the canvas is at least as wide as the
    // 4:3 reference. In 9:16 it makes the screen more than twice as wide as the
    // canvas, and everything to the right of the score falls off the edge.
    let scale = (output_h.max(1) as f32 / REFERENCE_HEIGHT)
        .min(output_w.max(1) as f32 / REFERENCE_WIDTH)
        .max(f32::EPSILON);
    // Leftover height is split above and below, so on a tall canvas the block sits
    // centred instead of hanging off the top edge. In 16:9 nothing is left over,
    // the offset is zero and the layout is exactly what it was.
    let frame = Frame {
        scale,
        top: (((output_h as f32 - REFERENCE_HEIGHT * scale) / 2.0).max(0.0)).round() as i32,
    };
    ResultsLayout {
        scale,
        title_line_origins: [
            frame.point(5.0, 27.0),
            frame.point(5.0, 52.0),
            frame.point(5.0, 74.0),
        ],
        panel_anchor: frame.point(0.0, 102.0),
        score_origin: frame.point(160.0, 126.0),
        grade_center: LayoutPoint {
            // The rank sprite stays pinned to the right edge on ultrawide outputs.
            x: output_w as i32 - scaled(scale, 192.0),
            y: frame.top + scaled(scale, 320.0),
        },
        title_right_inset: scaled(scale, 32.0),
        title_top: frame.top,
        block_bottom: frame.top + scaled(scale, REFERENCE_HEIGHT),
        mod_badges_first_center: LayoutPoint {
            x: output_w as i32 - scaled(scale, 64.0),
            y: frame.top + scaled(scale, 416.0),
        },
        mod_badges_step_x: -scaled(scale, 32.0),
        judgment_rows: [
            frame.judgment_row(64.0, 256.0),
            frame.judgment_row(64.0, 352.0),
            frame.judgment_row(64.0, 448.0),
            frame.judgment_row(384.0, 256.0),
            frame.judgment_row(384.0, 352.0),
            frame.judgment_row(384.0, 448.0),
        ],
        graph_rect: frame.rect(256.0, 608.0, 308.0, 156.0),
        timing_box_origin: frame.point(582.0, 662.0),
        timing_line_gap: scaled(scale, 14.0),
        accuracy_label_anchor: frame.point(291.0, 480.0),
        accuracy_value_origin: frame.point(310.0, 528.0),
        combo_label_anchor: frame.point(8.0, 480.0),
        combo_value_origin: frame.point(24.0, 528.0),
        perfect_center: frame.point(416.0, 688.0),
    }
}
#[cfg(test)]
mod tests;
