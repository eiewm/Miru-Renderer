use super::super::{InternalJudgment, KeyEvent};
use crate::types::{Beatmap, Windows};
pub fn seg_hits_win(
    seg_start: i32,
    seg_end: Option<i32>,
    win_start: i32,
    win_end_exclusive: i32,
) -> bool {
    let seg_end = seg_end.unwrap_or(i32::MAX);
    seg_start < win_end_exclusive && seg_end >= win_start
}
pub fn steals_next_ln_head(
    judgments: &[InternalJudgment],
    map: &Beatmap,
    current_col: u8,
    next_same_col_idx: Option<usize>,
    candidate_press_time: i32,
    candidate_rel_time: i32,
    windows: &Windows,
    tail_window_scale: f32,
) -> bool {
    next_same_col_idx
        .and_then(|next_idx| map.hit_objects.get(next_idx).map(|next| (next_idx, next)))
        .map(|(next_idx, next_ho)| {
            if !next_ho.is_long_note() {
                return false;
            }
            let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
            let next_tail_start =
                next_end_time - ((windows.hit50 as f32) * tail_window_scale).round() as i32;
            let next_tail_end =
                next_end_time + ((windows.hit100 as f32) * tail_window_scale).round() as i32;
            let next_head_start = next_ho.time - windows.hit50;
            let nex_hea_win_end_incl = next_ho.time + windows.hit50;
            let consumed_by_imm_next = judgments.iter().any(|jj| {
                jj.index == next_idx
                    && jj.column == current_col
                    && jj.press_time == Some(candidate_press_time)
            });
            consumed_by_imm_next
                && candidate_press_time >= next_head_start
                && candidate_press_time <= nex_hea_win_end_incl
                && candidate_rel_time > candidate_press_time
                && candidate_rel_time >= next_tail_start
                && candidate_rel_time < next_tail_end
        })
        .unwrap_or(false)
}
pub fn steals_next_tap_head(
    judgments: &[InternalJudgment],
    map: &Beatmap,
    events: &[KeyEvent],
    current_col: u8,
    next_same_col_idx: Option<usize>,
    candidate_press_time: i32,
    windows: &Windows,
) -> bool {
    next_same_col_idx
        .and_then(|next_idx| map.hit_objects.get(next_idx).map(|next| (next_idx, next)))
        .map(|(next_idx, next_ho)| {
            if next_ho.is_long_note() {
                return false;
            }
            let next_head_start = next_ho.time - windows.hit50;
            let next_head_win_end = next_ho.time + windows.hit100;
            let consumed_by_imm_next = judgments.iter().any(|jj| {
                jj.index == next_idx
                    && jj.column == current_col
                    && jj.press_time == Some(candidate_press_time)
            });
            let has_next_tap_follow = events.iter().any(|ev| {
                ev.pressed
                    && ev.time > candidate_press_time
                    && ev.time >= next_head_start
                    && ev.time < next_head_win_end
            });
            consumed_by_imm_next
                && candidate_press_time >= next_head_start
                && candidate_press_time < next_head_win_end
                && candidate_press_time <= next_ho.time + windows.max
                && !has_next_tap_follow
        })
        .unwrap_or(false)
}
