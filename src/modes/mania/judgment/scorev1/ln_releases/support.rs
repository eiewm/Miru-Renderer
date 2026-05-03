use crate::modes::mania::judgment::scorev1::claims;
use crate::modes::mania::judgment::{steals_next_ln_head, InternalJudgment, KeyEvent, ReleaseKind};
pub(super) fn calc_rel_kind(abs_diff: i32, w: &crate::types::Windows, scale: f32) -> ReleaseKind {
    let max_w = ((w.max as f32) * scale).round() as i32;
    let hit300_w = ((w.hit300 as f32) * scale).round() as i32;
    let hit200_w = ((w.hit200 as f32) * scale).round() as i32;
    let hit100_w = ((w.hit100 as f32) * scale).round() as i32;
    let hit50_w = ((w.hit50 as f32) * scale).round() as i32;
    if abs_diff <= max_w {
        ReleaseKind::Max
    } else if abs_diff <= hit300_w {
        ReleaseKind::Hit300
    } else if abs_diff <= hit200_w {
        ReleaseKind::Hit200
    } else if abs_diff <= hit100_w {
        ReleaseKind::Hit100
    } else if abs_diff <= hit50_w {
        ReleaseKind::Hit50
    } else {
        ReleaseKind::Miss
    }
}
pub(super) fn next_ln_keeps(
    judgments: &[InternalJudgment],
    map: &crate::types::Beatmap,
    events: &[KeyEvent],
    current_col: u8,
    next_same_col_idx: Option<usize>,
    candidate_press_time: i32,
    candidate_rel_time: i32,
    w: &crate::types::Windows,
    tail_window_scale: f32,
) -> bool {
    next_same_col_idx
        .and_then(|next_idx| {
            map.hit_objects
                .get(next_idx)
                .map(|next_ho| (next_idx, next_ho))
        })
        .filter(|(_, next_ho)| next_ho.is_long_note())
        .filter(|_| {
            steals_next_ln_head(
                judgments,
                map,
                current_col,
                next_same_col_idx,
                candidate_press_time,
                candidate_rel_time,
                w,
                tail_window_scale,
            )
        })
        .map(|(next_idx, _)| {
            claims::find_repl_pt(judgments, map, events, next_idx, candidate_press_time, w)
                .is_none()
        })
        .unwrap_or(false)
}
