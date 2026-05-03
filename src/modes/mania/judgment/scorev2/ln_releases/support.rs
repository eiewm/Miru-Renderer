use crate::modes::mania::judgment::scorev2::claims;
use crate::modes::mania::judgment::{
    calc_hit_kind, steals_next_ln_head, InternalJudgment, KeyEvent, ReleaseKind,
};
use crate::types::JudgmentKind;
pub(super) fn calc_rel_kind(abs_diff: i32, w: &crate::types::Windows, scale: f32) -> ReleaseKind {
    let safe_scale = if scale > 0.0 { scale } else { 1.0 };
    let scaled_diff = (abs_diff as f32) / safe_scale;
    if scaled_diff <= w.max as f32 {
        ReleaseKind::Max
    } else if scaled_diff <= w.hit300 as f32 {
        ReleaseKind::Hit300
    } else if scaled_diff <= w.hit200 as f32 {
        ReleaseKind::Hit200
    } else if scaled_diff <= w.hit100 as f32 {
        ReleaseKind::Hit100
    } else if scaled_diff <= w.hit50 as f32 {
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
        .filter(|(next_idx, next_ho)| {
            let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
            let next_duration = next_end_time - next_ho.time;
            let next_tail_start =
                next_end_time - ((w.hit50 as f32) * tail_window_scale).round() as i32;
            let next_tail_end =
                next_end_time + ((w.hit100 as f32) * tail_window_scale).round() as i32;
            let next_head_kind = calc_hit_kind((candidate_press_time - next_ho.time).abs(), w);
            let claimed_by_imm_next = judgments.iter().any(|jj| {
                jj.index == *next_idx
                    && jj.column == current_col
                    && jj.press_time == Some(candidate_press_time)
            });
            let weak_next_ln_no_repl = claimed_by_imm_next
                && next_duration <= w.hit50 + w.max
                && candidate_press_time >= next_ho.time - w.hit100
                && candidate_press_time < next_ho.time
                && matches!(
                    next_head_kind,
                    JudgmentKind::Hit200 | JudgmentKind::Hit100 | JudgmentKind::Hit50
                )
                && candidate_rel_time > next_ho.time
                && candidate_rel_time < next_end_time
                && candidate_rel_time <= next_ho.time + w.hit300
                && next_ho.time - candidate_press_time > candidate_rel_time - next_ho.time
                && candidate_rel_time >= next_tail_start
                && candidate_rel_time < next_tail_end;
            steals_next_ln_head(
                judgments,
                map,
                current_col,
                next_same_col_idx,
                candidate_press_time,
                candidate_rel_time,
                w,
                tail_window_scale,
            ) || weak_next_ln_no_repl
        })
        .map(|(next_idx, next_ho)| {
            let replacement_press =
                claims::find_repl_pt(judgments, map, events, next_idx, candidate_press_time, w);
            let rep_cla_by_fol_str_ln = replacement_press
                .and_then(|replacement_press| {
                    let replacement_kind =
                        calc_hit_kind((replacement_press - next_ho.time).abs(), w);
                    if matches!(replacement_kind, JudgmentKind::Max | JudgmentKind::Hit300) {
                        return None;
                    }
                    let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                    let next_next_idx = map.hit_objects[(next_idx + 1)..]
                        .iter()
                        .enumerate()
                        .find(|(_, next_next_ho)| next_next_ho.column == current_col)
                        .map(|(offset, _)| next_idx + 1 + offset)?;
                    let next_next_ho = map.hit_objects.get(next_next_idx)?;
                    if !next_next_ho.is_long_note() {
                        return None;
                    }
                    let next_next_end_time = next_next_ho.end_time.unwrap_or(next_next_ho.time);
                    let next_next_tail_start =
                        next_next_end_time - ((w.hit50 as f32) * tail_window_scale).round() as i32;
                    let next2_tail_end =
                        next_next_end_time + ((w.hit100 as f32) * tail_window_scale).round() as i32;
                    let replacement_rel_time = events
                        .iter()
                        .find(|ev| ev.time > replacement_press && !ev.pressed)
                        .map(|ev| ev.time)?;
                    Some(
                        replacement_press > next_end_time
                            && judgments.iter().any(|jj| {
                                jj.index == next_next_idx
                                    && jj.column == current_col
                                    && jj.press_time == Some(replacement_press)
                                    && matches!(jj.kind, JudgmentKind::Max | JudgmentKind::Hit300)
                            })
                            && replacement_rel_time >= next_next_tail_start
                            && replacement_rel_time < next2_tail_end,
                    )
                })
                .unwrap_or(false);
            replacement_press.is_none() || rep_cla_by_fol_str_ln
        })
        .unwrap_or(false)
}
