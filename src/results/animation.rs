#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct AnimatedElement {
    pub(crate) alpha: f32,
    pub(crate) offset_x: f32,
    pub(crate) offset_y: f32,
}
#[derive(Debug, Clone)]
pub(crate) struct ResultsAnimationState {
    pub(crate) title: AnimatedElement,
    pub(crate) panel: AnimatedElement,
    pub(crate) score: AnimatedElement,
    pub(crate) grade: AnimatedElement,
    pub(crate) judgments: [AnimatedElement; 6],
    pub(crate) graph_frame: AnimatedElement,
    pub(crate) graph_line_progress: f32,
    pub(crate) timing: AnimatedElement,
    pub(crate) accuracy: AnimatedElement,
    pub(crate) combo: AnimatedElement,
    pub(crate) perfect: AnimatedElement,
}
fn window(progress: f32, start: f32, end: f32) -> f32 {
    if end <= start {
        return (progress >= end) as i32 as f32;
    }
    ((progress - start) / (end - start)).clamp(0.0, 1.0)
}
fn ease_out_cubic(t: f32) -> f32 {
    let inv = 1.0 - t.clamp(0.0, 1.0);
    1.0 - inv * inv * inv
}
fn staged(progress: f32, start: f32, end: f32, offset_x: f32, offset_y: f32) -> AnimatedElement {
    let alpha = ease_out_cubic(window(progress, start, end));
    AnimatedElement {
        alpha,
        offset_x: offset_x * (1.0 - alpha),
        offset_y: offset_y * (1.0 - alpha),
    }
}
pub(crate) fn results_animation_state(progress: f32) -> ResultsAnimationState {
    // Progress is normalized over the results intro, so all timings below are fractions of that reveal.
    let progress = progress.clamp(0.0, 1.0);
    let mut judgments = [AnimatedElement::default(); 6];
    for (index, judgment) in judgments.iter_mut().enumerate() {
        let start = 0.20 + index as f32 * 0.035;
        let end = start + 0.10;
        // Judgment rows enter from opposite sides to match the two-column layout.
        let offset_x = if index < 3 { -10.0 } else { 10.0 };
        *judgment = staged(progress, start, end, offset_x, 0.0);
    }
    ResultsAnimationState {
        title: staged(progress, 0.00, 0.14, 0.0, -8.0),
        panel: staged(progress, 0.04, 0.18, -14.0, 0.0),
        score: staged(progress, 0.08, 0.20, 0.0, 8.0),
        grade: staged(progress, 0.14, 0.28, 18.0, 0.0),
        judgments,
        graph_frame: staged(progress, 0.34, 0.48, 0.0, 8.0),
        graph_line_progress: ease_out_cubic(window(progress, 0.40, 0.62)),
        timing: staged(progress, 0.42, 0.54, 0.0, 8.0),
        accuracy: staged(progress, 0.42, 0.54, 0.0, 8.0),
        combo: staged(progress, 0.42, 0.54, 0.0, 8.0),
        perfect: staged(progress, 0.54, 0.66, 0.0, 6.0),
    }
}
