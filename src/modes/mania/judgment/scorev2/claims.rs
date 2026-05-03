use super::super::{calc_hit_kind, InternalJudgment, KeyEvent};
use crate::types::{Beatmap, HitObject, JudgmentKind, Windows};
#[derive(Debug, Clone, Copy, Default)]
pub struct PostTailRescueConflictResult {
    pub allowed: bool,
    pub consumed_by_other: bool,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaimedPressConflictResult {
    pub claimed_press_time: Option<i32>,
    pub competing_note_idx: Option<usize>,
    pub competing_note_time: Option<i32>,
    pub followup_press_time: Option<i32>,
    pub next_window_start: Option<i32>,
    pub competing_note_kind: Option<JudgmentKind>,
    pub press_first_rel: bool,
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
fn reclmd_to_tail_only(
    ho: &HitObject,
    _prior_kind: JudgmentKind,
    prior_press_time: Option<i32>,
    new_press: i32,
    reassigned_head_kind: JudgmentKind,
    events: &[KeyEvent],
    w: &Windows,
) -> bool {
    if !ho.is_long_note() {
        return false;
    }
    let end_time = ho.end_time.unwrap_or(ho.time);
    let ln_duration = end_time - ho.time;
    let tail_start = end_time - w.hit50;
    let tail_end_exclusive = end_time + w.hit100;
    let replacement_release = events
        .iter()
        .find(|ev| ev.time > new_press && !ev.pressed)
        .map(|ev| ev.time);
    let short_post_tail_only = ln_duration <= w.hit100
        && new_press > end_time
        && new_press <= end_time + w.hit100
        && matches!(
            reassigned_head_kind,
            JudgmentKind::Hit50 | JudgmentKind::Miss
        );
    let late_inbody_headless = prior_press_time
        .and_then(|pt| {
            events
                .iter()
                .find(|ev| ev.time > pt && !ev.pressed)
                .map(|ev| ev.time)
        })
        .map(|rt| rt < ho.time)
        .unwrap_or(false)
        && new_press > ho.time + w.hit100
        && new_press <= end_time
        && replacement_release
            .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
            .unwrap_or(false);
    short_post_tail_only || late_inbody_headless
}
fn hless_yields_next_ln(
    map: &Beatmap,
    events: &[KeyEvent],
    current_idx: usize,
    claimed_press_time: i32,
    victim_idx: usize,
    w: &Windows,
) -> bool {
    let Some(current_ho) = map.hit_objects.get(current_idx) else {
        return false;
    };
    if !current_ho.is_long_note() {
        return false;
    }
    let current_end = current_ho.end_time.unwrap_or(current_ho.time);
    if current_end - current_ho.time > w.hit100 || claimed_press_time <= current_end {
        return false;
    }
    let next_same_col_idx = map.hit_objects[(current_idx + 1)..]
        .iter()
        .enumerate()
        .find(|(_, next_ho)| next_ho.column == current_ho.column)
        .map(|(offset, _)| current_idx + 1 + offset);
    if next_same_col_idx != Some(victim_idx) {
        return false;
    }
    let Some(victim_ho) = map.hit_objects.get(victim_idx) else {
        return false;
    };
    if !victim_ho.is_long_note() {
        return false;
    }
    let victim_end = victim_ho.end_time.unwrap_or(victim_ho.time);
    if victim_end - victim_ho.time > w.hit100 {
        return false;
    }
    let victim_head_kind = calc_hit_kind((claimed_press_time - victim_ho.time).abs(), w);
    if claimed_press_time < victim_ho.time
        || claimed_press_time > victim_ho.time + w.max
        || !matches!(victim_head_kind, JudgmentKind::Max | JudgmentKind::Hit300)
    {
        return false;
    }
    let release_after_claim = events
        .iter()
        .find(|ev| ev.time > claimed_press_time && !ev.pressed)
        .map(|ev| ev.time);
    let next2_same_col_time = map.hit_objects[(victim_idx + 1)..]
        .iter()
        .find(|next_next_ho| next_next_ho.column == current_ho.column)
        .map(|next_next_ho| next_next_ho.time);
    release_after_claim
        .map(|rel_time| {
            rel_time >= victim_end - w.hit50
                && rel_time < victim_end + w.hit100
                && next2_same_col_time
                    .map(|next_next_time| rel_time < next_next_time)
                    .unwrap_or(true)
        })
        .unwrap_or(false)
}
fn weak_post_to_strong(
    map: &Beatmap,
    events: &[KeyEvent],
    note_idx: usize,
    replacement_press: i32,
    replacement_kind: JudgmentKind,
    w: &Windows,
) -> bool {
    if matches!(replacement_kind, JudgmentKind::Max | JudgmentKind::Hit300) {
        return false;
    }
    let Some(current_ho) = map.hit_objects.get(note_idx) else {
        return false;
    };
    map.hit_objects[(note_idx + 1)..]
        .iter()
        .enumerate()
        .find(|(_, next_ho)| next_ho.column == current_ho.column)
        .map(|(offset, _)| note_idx + 1 + offset)
        .map(|next_idx| hless_yields_next_ln(map, events, note_idx, replacement_press, next_idx, w))
        .unwrap_or(false)
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
            let owner_headless_yields = judgments
                .iter()
                .find(|judgment| judgment.index == current_owner_idx && judgment.column == column)
                .filter(|owner| {
                    owner.kind == JudgmentKind::Miss
                        && owner.press_time.is_none()
                        && owner.early_press_idx == Some(current_press)
                })
                .is_some()
                && hless_yields_next_ln(
                    map,
                    events,
                    current_owner_idx,
                    current_press,
                    victim_idx,
                    w,
                );
            if owner_headless_yields {
                continue;
            }
            let replacement = find_repl_pt(judgments, map, events, victim_idx, current_press, w);
            if let Some(new_press) = replacement {
                let victim_ho = match map.hit_objects.get(victim_idx) {
                    Some(v) => v,
                    None => continue,
                };
                let (victim_prior_kind, victim_prior_pt_time) = judgments
                    .get(victim_pos)
                    .map(|victim| (victim.kind, victim.press_time))
                    .unwrap_or((JudgmentKind::Miss, None));
                let reassigned_head_kind = calc_hit_kind((new_press - victim_ho.time).abs(), w);
                let weak_post_end_yield = weak_post_to_strong(
                    map,
                    events,
                    victim_idx,
                    new_press,
                    reassigned_head_kind,
                    w,
                );
                let reclmd_to_tail_only = reclmd_to_tail_only(
                    victim_ho,
                    victim_prior_kind,
                    victim_prior_pt_time,
                    new_press,
                    reassigned_head_kind,
                    events,
                    w,
                );
                if let Some(victim) = judgments.get_mut(victim_pos) {
                    if weak_post_end_yield {
                        victim.press_time = None;
                        victim.kind = JudgmentKind::Miss;
                        victim.delta = 0;
                    } else if reclmd_to_tail_only {
                        victim.press_time = None;
                        victim.kind = JudgmentKind::Miss;
                        victim.delta = 0;
                        victim.early_press_idx = Some(new_press);
                    } else {
                        victim.press_time = Some(new_press);
                        victim.kind = reassigned_head_kind;
                    }
                }
                if !weak_post_end_yield {
                    queue.push((victim_idx, new_press));
                }
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
) -> ClaimedPressConflictResult {
    let mut result = ClaimedPressConflictResult::default();
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
    let current_ho = map.hit_objects.get(current_idx);
    let current_judgment = judgments
        .iter()
        .find(|jj| jj.index == current_idx && jj.column == current_col)
        .map(|jj| (jj.kind, jj.press_time));
    let imm_nex_same_col_idx = map.hit_objects[(current_idx + 1)..]
        .iter()
        .enumerate()
        .find(|(_, next_ho)| next_ho.column == current_col)
        .map(|(offset, _)| current_idx + 1 + offset);
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
        if hless_yields_next_ln(map, events, current_idx, claimed_press_time, other_idx, w) {
            continue;
        }
        let replacement_press =
            find_repl_pt(judgments, map, events, other_idx, claimed_press_time, w);
        if let Some(new_press) = replacement_press {
            let (other_prior_kind, other_prior_pt_time) = judgments
                .get(other_pos)
                .map(|other| (other.kind, other.press_time))
                .unwrap_or((JudgmentKind::Miss, None));
            if result.competing_note_idx.is_none() {
                result.claimed_press_time = Some(claimed_press_time);
                result.competing_note_idx = Some(other_idx);
                result.competing_note_time = Some(other_ho.time);
                result.followup_press_time = Some(new_press);
                result.next_window_start = Some(other_ho.time - w.hit50);
                result.competing_note_kind = Some(other_prior_kind);
            }
            let cur_ln_prehead_h100 = current_ho
                .zip(current_judgment)
                .map(|(current_ho, (current_kind, current_press_time))| {
                    if !current_ho.is_long_note()
                        || current_kind != JudgmentKind::Hit100
                        || imm_nex_same_col_idx != Some(other_idx)
                    {
                        return false;
                    }
                    let Some(current_press_time) = current_press_time else {
                        return false;
                    };
                    if current_press_time >= current_ho.time {
                        return false;
                    }
                    let current_end_time = current_ho.end_time.unwrap_or(current_ho.time);
                    let next_window_start = other_ho.time - w.hit50;
                    let next_win_end = other_ho.time + w.hit100;
                    let firs_rel_post_cur_pt = events
                        .iter()
                        .find(|ev| ev.time > current_press_time && !ev.pressed)
                        .map(|ev| ev.time);
                    let rel_post_claimed_pt = events
                        .iter()
                        .find(|ev| ev.time > claimed_press_time && !ev.pressed)
                        .map(|ev| ev.time);
                    other_prior_kind == JudgmentKind::Hit50
                        && other_prior_pt_time == Some(claimed_press_time)
                        && claimed_press_time >= next_window_start
                        && claimed_press_time < next_win_end
                        && claimed_press_time < current_end_time
                        && new_press > claimed_press_time
                        && new_press >= next_window_start
                        && new_press < next_win_end
                        && firs_rel_post_cur_pt
                            .map(|rt| rt >= current_ho.time && rt < next_window_start)
                            .unwrap_or(false)
                        && rel_post_claimed_pt
                            .map(|rt| {
                                rt > claimed_press_time
                                    && rt >= current_end_time
                                    && rt < current_end_time + w.hit100
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
            if cur_ln_prehead_h100 {
                result.press_first_rel = true;
                continue;
            }
            let reassigned_head_kind = calc_hit_kind((new_press - other_ho.time).abs(), w);
            let weak_post_end_yield =
                weak_post_to_strong(map, events, other_idx, new_press, reassigned_head_kind, w);
            let reclmd_to_tail_only = reclmd_to_tail_only(
                other_ho,
                other_prior_kind,
                other_prior_pt_time,
                new_press,
                reassigned_head_kind,
                events,
                w,
            );
            if let Some(other) = judgments.get_mut(other_pos) {
                if weak_post_end_yield {
                    other.press_time = None;
                    other.kind = JudgmentKind::Miss;
                    other.delta = 0;
                } else if reclmd_to_tail_only {
                    other.press_time = None;
                    other.kind = JudgmentKind::Miss;
                    other.delta = 0;
                    other.early_press_idx = Some(new_press);
                } else {
                    other.press_time = Some(new_press);
                    other.kind = reassigned_head_kind;
                }
            }
            if !weak_post_end_yield {
                cascade_reassign_pts(judgments, map, events, other_col, other_idx, new_press, w);
            }
        } else if let Some(other) = judgments.get_mut(other_pos) {
            other.press_time = None;
            other.kind = JudgmentKind::Miss;
        }
    }
    result
}
