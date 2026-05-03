use super::super::{calc_hit_kind, InternalJudgment, KeyEvent};
use crate::types::{Beatmap, JudgmentKind, Windows};
#[derive(Debug, Clone, Copy, Default)]
pub struct PostTailRescueConflictResult {
    pub allowed: bool,
    pub consumed_by_other: bool,
}
fn press_window_for_note(map: &Beatmap, note_idx: usize, w: &Windows) -> Option<(u8, i32, i32)> {
    let ho = map.hit_objects.get(note_idx)?;
    let col = ho.column;
    let window_start = ho.time - w.hit50;
    let next_same_col = map
        .hit_objects
        .iter()
        .enumerate()
        .filter(|(candidate_idx, candidate)| {
            *candidate_idx != note_idx && candidate.column == col && candidate.time > ho.time
        })
        .min_by_key(|(_, candidate)| candidate.time);
    let next_same_col_time = next_same_col.map(|(_, candidate)| candidate.time);
    let ln_late_end = ho.is_long_note()
        && next_same_col_time
            .map(|t| t <= ho.time + w.hit50)
            .unwrap_or(false);
    let ln_to_tap_post_end = ho.is_long_note()
        && next_same_col
            .map(|(_, candidate)| {
                !candidate.is_long_note()
                    && candidate.time > ho.time + w.hit50
                    && candidate.time <= ho.end_time.unwrap_or(ho.time) + w.hit50
            })
            .unwrap_or(false);
    let window_end_exclusive = if ho.is_long_note() {
        ho.time
            + w.hit50
            + if ln_late_end || ln_to_tap_post_end {
                1
            } else {
                0
            }
    } else {
        ho.time + w.hit100
    };
    Some((col, window_start, window_end_exclusive))
}
pub(crate) fn find_repl_pt(
    judgments: &[InternalJudgment],
    map: &Beatmap,
    events: &[KeyEvent],
    note_idx: usize,
    min_time_exclusive: i32,
    w: &Windows,
) -> Option<i32> {
    let ho = map.hit_objects.get(note_idx)?;
    let (note_col, window_start, window_end_exclusive) = press_window_for_note(map, note_idx, w)?;
    let taken_by_earlier = |candidate_time: i32| {
        judgments.iter().any(|jj| {
            jj.column == note_col
                && jj.index != note_idx
                && jj.press_time == Some(candidate_time)
                && jj.index < note_idx
        })
    };
    if let Some(standard_candidate) = events
        .iter()
        .filter(|ev| ev.pressed)
        .map(|ev| ev.time)
        .find(|candidate_time| {
            if *candidate_time <= min_time_exclusive
                || *candidate_time < window_start
                || *candidate_time >= window_end_exclusive
            {
                return false;
            }
            !taken_by_earlier(*candidate_time)
        })
    {
        return Some(standard_candidate);
    }
    if !ho.is_long_note() {
        return None;
    }
    let end_time = ho.end_time.unwrap_or(ho.time);
    let ln_duration = end_time - ho.time;
    if ln_duration < w.hit50 * 2 {
        return None;
    }
    let tail_start = end_time - w.hit50;
    let tail_end_exclusive = end_time + w.hit100;
    let late_hless_start = ho.time + w.hit50;
    let late_hless_end_incls = (tail_start + w.hit100).max(end_time + w.hit50);
    let next_same_col_time = map
        .hit_objects
        .iter()
        .enumerate()
        .filter(|(candidate_idx, candidate)| {
            *candidate_idx != note_idx && candidate.column == note_col && candidate.time > ho.time
        })
        .min_by_key(|(_, candidate)| candidate.time)
        .map(|(_, candidate)| candidate.time);
    events
        .iter()
        .filter(|ev| ev.pressed)
        .map(|ev| ev.time)
        .find(|candidate_time| {
            if *candidate_time <= min_time_exclusive
                || *candidate_time <= late_hless_start
                || *candidate_time > late_hless_end_incls
            {
                return false;
            }
            let overlaps_next_prewin = next_same_col_time
                .map(|next_t| *candidate_time >= next_t - w.hit50)
                .unwrap_or(false);
            if overlaps_next_prewin {
                let allo_post_end_no_fol = next_same_col_time
                    .map(|next_t| {
                        let next_window_start = next_t - w.hit50;
                        let next_win_end = next_t + w.hit50;
                        let has_next_pt_fol = events.iter().any(|ev| {
                            ev.pressed
                                && ev.time > *candidate_time
                                && ev.time >= next_window_start
                                && ev.time < next_win_end
                        });
                        let prehead_press_time = events
                            .iter()
                            .rev()
                            .find(|ev| ev.pressed && ev.time <= ho.time)
                            .map(|ev| ev.time);
                        let pre_break_near_tail = prehead_press_time
                            .and_then(|prev_pt| {
                                events
                                    .iter()
                                    .find(|ev| ev.time > prev_pt && !ev.pressed)
                                    .map(|ev| ev.time)
                            })
                            .map(|release_t| release_t >= ho.time - 1 && release_t < tail_start)
                            .unwrap_or(false);
                        *candidate_time > end_time
                            && *candidate_time <= end_time + w.hit50
                            && pre_break_near_tail
                            && !has_next_pt_fol
                    })
                    .unwrap_or(false);
                if !allo_post_end_no_fol {
                    return false;
                }
            }
            if taken_by_earlier(*candidate_time) {
                return false;
            }
            let first_rel_after_cand = events
                .iter()
                .find(|ev| ev.time > *candidate_time && !ev.pressed)
                .map(|ev| ev.time);
            let closes_in_tail_win = first_rel_after_cand
                .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
                .unwrap_or(false);
            let holds_thru_tail_win = first_rel_after_cand
                .map(|rt| rt >= tail_end_exclusive)
                .unwrap_or(true);
            closes_in_tail_win || holds_thru_tail_win
        })
}
fn cascade_reassign_pts(
    judgments: &mut [InternalJudgment],
    map: &Beatmap,
    events: &[KeyEvent],
    column: u8,
    owner_idx: usize,
    claimed_press: i32,
    w: &Windows,
) {
    let mut queue: Vec<(usize, i32)> = vec![(owner_idx, claimed_press)];
    while let Some((current_owner_idx, current_press)) = queue.pop() {
        let mut victim_positions: Vec<usize> = judgments
            .iter()
            .enumerate()
            .filter(|(_, j)| {
                j.column == column
                    && j.index > current_owner_idx
                    && j.press_time == Some(current_press)
            })
            .map(|(pos, _)| pos)
            .collect();
        victim_positions.sort_by_key(|pos| judgments[*pos].index);
        for victim_pos in victim_positions {
            let victim_idx = match judgments.get(victim_pos) {
                Some(victim) => victim.index,
                None => continue,
            };
            let replacement = find_repl_pt(judgments, map, events, victim_idx, current_press, w);
            if let Some(new_press) = replacement {
                let victim_time = map.hit_objects.get(victim_idx).map(|ho| ho.time);
                let reassigned_head_kind = victim_time
                    .map(|t| calc_hit_kind((new_press - t).abs(), w))
                    .unwrap_or(JudgmentKind::Miss);
                if let Some(victim) = judgments.get_mut(victim_pos) {
                    victim.press_time = Some(new_press);
                    victim.kind = reassigned_head_kind;
                }
                queue.push((victim_idx, new_press));
            } else if let Some(victim) = judgments.get_mut(victim_pos) {
                victim.press_time = None;
                victim.kind = JudgmentKind::Miss;
            }
        }
    }
}
pub fn reconcile_tail_rescue(
    judgments: &mut [InternalJudgment],
    map: &Beatmap,
    events: &[KeyEvent],
    current_idx: usize,
    current_col: u8,
    rescue_press_time: i32,
    ln_duration: i32,
    tail_start: i32,
    end_time: i32,
    w: &Windows,
) -> PostTailRescueConflictResult {
    let mut conflicting_judgments: Vec<usize> = Vec::new();
    for (other_pos, other) in judgments.iter().enumerate() {
        if other.index == current_idx || other.column != current_col {
            continue;
        }
        if other.press_time == Some(rescue_press_time) && other.index > current_idx {
            conflicting_judgments.push(other_pos);
        }
    }
    let consumed_by_other = !conflicting_judgments.is_empty();
    let post_tail_delta = rescue_press_time - end_time;
    let short_ln_late_repr =
        ln_duration <= w.hit50 + w.hit100 + w.max && rescue_press_time > tail_start;
    let all_nea_pos_tai_resc = post_tail_delta <= w.max
        && !short_ln_late_repr
        && (!consumed_by_other || ln_duration >= (w.hit50 * 2));
    let allow_post_tail_resc = consumed_by_other
        && !short_ln_late_repr
        && post_tail_delta < w.hit100
        && ln_duration > (w.hit50 * 2);
    if !(all_nea_pos_tai_resc || allow_post_tail_resc) {
        return PostTailRescueConflictResult {
            allowed: false,
            consumed_by_other,
        };
    }
    if consumed_by_other {
        for other_pos in conflicting_judgments {
            let (other_idx, other_col, still_conflicting) = match judgments.get(other_pos) {
                Some(other) => (
                    other.index,
                    other.column,
                    other.press_time == Some(rescue_press_time),
                ),
                None => continue,
            };
            if !still_conflicting {
                continue;
            }
            let other_ho = match map.hit_objects.get(other_idx) {
                Some(v) => v,
                None => continue,
            };
            let replacement_press =
                find_repl_pt(judgments, map, events, other_idx, rescue_press_time, w);
            if let Some(new_press) = replacement_press {
                if let Some(other) = judgments.get_mut(other_pos) {
                    other.press_time = Some(new_press);
                    other.kind = calc_hit_kind((new_press - other_ho.time).abs(), w);
                }
                cascade_reassign_pts(judgments, map, events, other_col, other_idx, new_press, w);
            } else if let Some(other) = judgments.get_mut(other_pos) {
                other.press_time = None;
                other.kind = JudgmentKind::Miss;
            }
        }
    }
    PostTailRescueConflictResult {
        allowed: true,
        consumed_by_other,
    }
}
pub fn reclaim_pt_conflict(
    judgments: &mut [InternalJudgment],
    map: &Beatmap,
    events: &[KeyEvent],
    current_idx: usize,
    current_col: u8,
    claimed_press_time: i32,
    w: &Windows,
) {
    let mut conflicting_judgments: Vec<usize> = judgments
        .iter()
        .enumerate()
        .filter(|(_, other)| {
            other.index != current_idx
                && other.column == current_col
                && other.index > current_idx
                && other.press_time == Some(claimed_press_time)
        })
        .map(|(pos, _)| pos)
        .collect();
    conflicting_judgments.sort_unstable();
    for other_pos in conflicting_judgments {
        let (other_idx, other_col, still_conflicting) = match judgments.get(other_pos) {
            Some(other) => (
                other.index,
                other.column,
                other.press_time == Some(claimed_press_time),
            ),
            None => continue,
        };
        if !still_conflicting {
            continue;
        }
        let other_ho = match map.hit_objects.get(other_idx) {
            Some(v) => v,
            None => continue,
        };
        let replacement_press =
            find_repl_pt(judgments, map, events, other_idx, claimed_press_time, w);
        if let Some(new_press) = replacement_press {
            let reassigned_head_kind = calc_hit_kind((new_press - other_ho.time).abs(), w);
            if let Some(other) = judgments.get_mut(other_pos) {
                other.press_time = Some(new_press);
                other.kind = reassigned_head_kind;
            }
            cascade_reassign_pts(judgments, map, events, other_col, other_idx, new_press, w);
        } else if let Some(other) = judgments.get_mut(other_pos) {
            other.press_time = None;
            other.kind = JudgmentKind::Miss;
        }
    }
}
