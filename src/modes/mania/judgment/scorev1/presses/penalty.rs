use super::note::{PenaltyFlags, PressNoteCtx, PressState};
use crate::modes::mania::judgment::calc_hit_kind;
use crate::types::JudgmentKind;
pub(super) fn evaluate(ctx: &PressNoteCtx<'_>, state: &mut PressState) {
    let note_pos = ctx.note_pos;
    let ho = ctx.ho;
    let col_notes = ctx.col_notes;
    let presses = ctx.presses;
    let events = ctx.events;
    let w = ctx.windows;
    let next_note_time = ctx.next_note_time;
    let note_window = ctx.note_window;
    let window_start = note_window.window_start;
    let lock_end_exclusive = note_window.lock_end_exclusive;
    let early_penalty_window = note_window.early_penalty_window;
    let next_early_pen = note_window.next_early_pen;
    let press_idx = state.press_idx;
    let prev_had_prewin_pen = state.prev.had_prewin_pen;
    let prev_break_pre = state.prev.body_break_pre_tail;
    let prev_was_miss = state.prev.was_miss;
    let prev2_had_prewin_pen = state.prev.prev2_had_prewin_pen;
    let prev_prev_was_miss = state.prev.prev2_was_miss;
    let prev_col_pt = state.prev.col_pt;
    let reserved_ln_repr = &state.prev.reserved_ln_repr;
    let _penalty_before = state.rules.early_pen;
    let mut early_pen_pt = state.rules.early_pen;
    let mut retained_penalty_rule: Option<&'static str> = None;
    if early_pen_pt.is_some() && press_idx >= presses.len() {
        // No press remains to claim or clear this carried early-press penalty.
        // Drop it here instead of letting stale penalty rules index past the input.
        state.penalty_flags = PenaltyFlags::default();
        state.rules.early_pen = None;
        state.rules.pen = None;
        return;
    }
    if let Some(pt) = early_pen_pt {
        let prewindow_overflow = (ho.time - pt).abs() - w.hit50;
        let deep_ln = ho.is_long_note() && prewindow_overflow >= w.hit300 - 2;
        let deep_ln_chain = ho.is_long_note()
            && prev_had_prewin_pen
            && prewindow_overflow >= early_penalty_window - 1;
        let deep_tap = !ho.is_long_note()
            && prewindow_overflow >= early_penalty_window - 1
            && !prev_had_prewin_pen;
        let deep_tap_chain = !ho.is_long_note()
            && prev_had_prewin_pen
            && prewindow_overflow >= early_penalty_window - 1;
        let has_in_win_cand = press_idx < presses.len()
            && presses[press_idx] >= window_start
            && presses[press_idx] < lock_end_exclusive
            && !reserved_ln_repr.contains(&presses[press_idx]);
        let early_press_rel_time = events
            .iter()
            .find(|ev| ev.time > pt && !ev.pressed)
            .map(|ev| ev.time);
        let early_rel_before_note = early_press_rel_time.map(|rt| rt < ho.time).unwrap_or(false);
        let early_rel_same_ms = events
            .iter()
            .any(|ev| !ev.pressed && ev.time == pt && ev.time < ho.time);
        let prev_note_is_ln = note_pos
            .checked_sub(1)
            .and_then(|p| col_notes.get(p))
            .map(|(_, prev_ho)| prev_ho.is_long_note())
            .unwrap_or(false);
        let prev_note_time = note_pos
            .checked_sub(1)
            .and_then(|p| col_notes.get(p))
            .map(|(_, prev_ho)| prev_ho.time);
        let prev_prev_note_time = note_pos
            .checked_sub(2)
            .and_then(|p| col_notes.get(p))
            .map(|(_, prev_prev_ho)| prev_prev_ho.time);
        let prev_note_duration = note_pos
            .checked_sub(1)
            .and_then(|p| col_notes.get(p))
            .map(|(_, prev_ho)| prev_ho.end_time.unwrap_or(prev_ho.time) - prev_ho.time);
        let prev_note_end_time = note_pos
            .checked_sub(1)
            .and_then(|p| col_notes.get(p))
            .map(|(_, prev_ho)| prev_ho.end_time.unwrap_or(prev_ho.time));
        let current_ln_duration = ho.end_time.unwrap_or(ho.time) - ho.time;
        let prev_pen_to_iso = false
            && !ho.is_long_note()
            && prev_was_miss
            && prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| prev_pt < prev_t && pt > prev_pt && pt < prev_t)
                .unwrap_or(false)
            && next_note_time
                .map(|next_t| next_t - ho.time > w.hit50 + w.hit300)
                .unwrap_or(true)
            && {
                let cand_pt = presses[press_idx];
                let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                cand_pt >= ho.time
                    && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                    && events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| {
                            ev.time <= ho.time + w.hit100
                                && next_note_time
                                    .map(|next_t| ev.time < next_t)
                                    .unwrap_or(true)
                        })
                        .unwrap_or(false)
            };
        let prev_pen_to_post = false
            && !ho.is_long_note()
            && prev_was_miss
            && prev_had_prewin_pen
            && !(prev_prev_was_miss && prev2_had_prewin_pen)
            && has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| prev_pt < prev_t && pt > prev_pt && pt < prev_t)
                .unwrap_or(false)
            && prev_note_time
                .map(|prev_t| {
                    pt < prev_t && prev_t - pt > w.max / 2 && ho.time - prev_t <= w.hit50 + w.hit300
                })
                .unwrap_or(false)
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let next_window_start = next_head_time - w.hit50;
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let next_has_post_follow = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|next_pt| **next_pt < next_win_end)
                        .find(|next_pt| {
                            **next_pt >= next_window_start
                                && next_next_tap_head
                                    .map(|next_next_head| **next_pt < next_next_head)
                                    .unwrap_or(true)
                                && !reserved_ln_repr.contains(next_pt)
                        })
                        .map(|next_pt| {
                            *next_pt >= next_head_time
                                && matches!(
                                    calc_hit_kind((*next_pt - next_head_time).abs(), w),
                                    JudgmentKind::Max | JudgmentKind::Hit300
                                )
                        })
                        .unwrap_or(false);
                    cand_pt >= ho.time
                        && cand_pt < next_head_time
                        && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                        && next_has_post_follow
                        && events
                            .iter()
                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                            .map(|ev| ev.time > ho.time && ev.time < next_head_time)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let prev_pen_to_prehead = false
            && !ho.is_long_note()
            && prev_was_miss
            && prev_had_prewin_pen
            && !(prev_prev_was_miss && prev2_had_prewin_pen)
            && has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| prev_pt < prev_t && pt > prev_pt && pt < prev_t)
                .unwrap_or(false)
            && next_note_time
                .map(|next_t| next_t - ho.time > w.hit50 + w.hit300)
                .unwrap_or(false)
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let next_window_start = next_head_time - w.hit50;
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let next_note_str_fol = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|next_pt| **next_pt < next_win_end)
                        .find(|next_pt| {
                            **next_pt >= next_window_start
                                && next_next_tap_head
                                    .map(|next_next_head| **next_pt < next_next_head)
                                    .unwrap_or(true)
                                && !reserved_ln_repr.contains(next_pt)
                        })
                        .map(|next_pt| {
                            matches!(
                                calc_hit_kind((*next_pt - next_head_time).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300
                            )
                        })
                        .unwrap_or(false);
                    cand_pt < ho.time
                        && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                        && next_note_str_fol
                        && next_next_tap_head
                            .map(|next_next_head| {
                                next_next_head - next_head_time > w.hit50 + w.hit300
                            })
                            .unwrap_or(true)
                        && events
                            .iter()
                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                            .map(|ev| {
                                ev.time > ho.time
                                    && ev.time <= ho.time + w.hit100
                                    && ev.time < next_head_time
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let pos_pre_head_to_prhd = false
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prewindow_overflow == early_penalty_window - 1
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    (prev_t - prev_pt).abs() <= w.hit300
                        && pt > prev_t + w.max
                        && pt <= prev_t + w.hit100
                        && ho.time - prev_t <= w.hit50 * 2
                })
                .unwrap_or(false)
            && next_note_time
                .map(|next_t| next_t - ho.time > w.hit50 + w.hit300)
                .unwrap_or(false)
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let next_window_start = next_head_time - w.hit50;
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let next_note_str_fol = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|next_pt| **next_pt < next_win_end)
                        .find(|next_pt| {
                            **next_pt >= next_window_start
                                && next_next_tap_head
                                    .map(|next_next_head| **next_pt < next_next_head)
                                    .unwrap_or(true)
                                && !reserved_ln_repr.contains(next_pt)
                        })
                        .map(|next_pt| {
                            *next_pt < next_head_time
                                && matches!(
                                    calc_hit_kind((*next_pt - next_head_time).abs(), w),
                                    JudgmentKind::Max | JudgmentKind::Hit300
                                )
                        })
                        .unwrap_or(false);
                    cand_pt < ho.time
                        && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                        && next_note_str_fol
                        && next_next_tap_head
                            .map(|next_next_head| {
                                next_next_head - next_head_time > w.hit50 + w.hit300
                            })
                            .unwrap_or(true)
                        && events
                            .iter()
                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                            .map(|ev| {
                                ev.time > ho.time
                                    && ev.time <= ho.time + w.hit100
                                    && ev.time < next_head_time
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let pre_earl_to_post_h50 = false
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| pt > prev_pt && pt < prev_t)
                .unwrap_or(false)
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let next_kind = calc_hit_kind((cand_pt - next_head_time).abs(), w);
                    let next_window_start = next_head_time - w.hit50;
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let next_note_has_fol = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|next_pt| **next_pt < next_win_end)
                        .any(|next_pt| {
                            *next_pt >= next_window_start
                                && next_next_tap_head
                                    .map(|next_next_head| *next_pt < next_next_head)
                                    .unwrap_or(true)
                                && !reserved_ln_repr.contains(next_pt)
                        });
                    cand_pt >= ho.time
                        && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                        && next_kind == JudgmentKind::Hit50
                        && !next_note_has_fol
                        && next_next_tap_head
                            .map(|next_next_head| {
                                next_next_head - next_head_time > w.hit50 + w.hit300
                            })
                            .unwrap_or(true)
                        && events
                            .iter()
                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                            .map(|ev| ev.time > ho.time && ev.time < next_head_time)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let pre_ear_to_post_chai = false
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| pt > prev_pt && pt < prev_t && prev_t - prev_pt > w.hit100)
                .unwrap_or(false)
            && next_note_time
                .map(|next_t| next_t - ho.time <= w.hit50 + w.hit300)
                .unwrap_or(false)
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let next_kind = calc_hit_kind((cand_pt - next_head_time).abs(), w);
                    let candidate_release = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let next_window_start = next_head_time - w.hit50;
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let next_note_post_follow = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|next_pt| **next_pt < next_win_end)
                        .find(|next_pt| {
                            **next_pt >= next_window_start
                                && next_next_tap_head
                                    .map(|next_next_head| **next_pt < next_next_head)
                                    .unwrap_or(false)
                                && !reserved_ln_repr.contains(next_pt)
                        })
                        .and_then(|next_pt| {
                            let next_followup_pt = *next_pt;
                            let next_followup_kind =
                                calc_hit_kind((next_followup_pt - next_head_time).abs(), w);
                            let next_followup_release = events
                                .iter()
                                .find(|ev| ev.time > next_followup_pt && !ev.pressed)
                                .map(|ev| ev.time)?;
                            (next_followup_pt >= next_head_time
                                && matches!(
                                    next_followup_kind,
                                    JudgmentKind::Max | JudgmentKind::Hit300
                                )
                                && next_next_tap_head
                                    .map(|next_next_head| {
                                        next_followup_release > next_head_time
                                            && next_followup_release < next_next_head
                                            && next_next_head - next_head_time <= w.hit50 + w.hit300
                                    })
                                    .unwrap_or(false))
                            .then_some((next_followup_pt, next_followup_release))
                        });
                    let next2_has_cand = next_note_post_follow
                        .zip(next_next_tap_head)
                        .map(|((next_followup_pt, _), next_next_head)| {
                            let next2_win_start = next_next_head - w.hit50;
                            let next2_win_end = next_next_head + w.hit100;
                            presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|cand| **cand < next2_win_end)
                                .any(|cand| {
                                    *cand > next_followup_pt
                                        && *cand >= next2_win_start
                                        && !reserved_ln_repr.contains(cand)
                                        && !matches!(
                                            calc_hit_kind((*cand - next_next_head).abs(), w,),
                                            JudgmentKind::Miss
                                        )
                                })
                        })
                        .unwrap_or(false);
                    cand_pt >= ho.time
                        && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                        && next_kind == JudgmentKind::Hit50
                        && next2_has_cand
                        && candidate_release
                            .zip(next_note_post_follow)
                            .map(|(rel_time, (next_followup_pt, _))| {
                                rel_time > ho.time
                                    && rel_time < next_head_time
                                    && rel_time < next_followup_pt
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let prev_early_to_iso = false
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    prev_pt < prev_t
                        && prev_t - prev_pt > w.hit300
                        && prev_t - prev_pt <= w.hit100
                        && pt > prev_pt
                        && pt < prev_t
                        && prev_t - pt > w.max
                })
                .unwrap_or(false)
            && next_note_time
                .map(|next_t| {
                    let next_gap = next_t - ho.time;
                    next_gap > w.hit50 + w.hit300
                        && prev_note_time
                            .zip(prev_prev_note_time)
                            .map(|(prev_t, prev_prev_t)| {
                                next_gap > (ho.time - prev_t) + (prev_t - prev_prev_t)
                            })
                            .unwrap_or(true)
                })
                .unwrap_or(true)
            && {
                let cand_pt = presses[press_idx];
                let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                cand_pt >= ho.time
                    && cand_kind == JudgmentKind::Max
                    && events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| {
                            next_note_time
                                .map(|next_t| ev.time < next_t)
                                .unwrap_or(true)
                        })
                        .unwrap_or(false)
            };
        let prev_gap_early_pen = false
            && !ho.is_long_note()
            && (!prev_was_miss || prewindow_overflow <= early_penalty_window)
            && !prev_pen_to_iso
            && !prev_pen_to_post
            && !prev_pen_to_prehead
            && !pos_pre_head_to_prhd
            && !prev_early_to_iso
            && !pre_earl_to_post_h50
            && !pre_ear_to_post_chai
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    let severe_prev_early_pt = prev_pt < prev_t
                        && prev_t - prev_pt > w.hit300
                        && pt > prev_pt
                        && pt < prev_t
                        && prewindow_overflow < early_penalty_window;
                    let medium_prev_gap_frag = prev_pt < prev_t
                        && prev_t - prev_pt > w.max
                        && prev_t - prev_pt <= w.hit300
                        && pt > prev_pt
                        && pt < prev_t;
                    let pos_pre_hea_cha_frag = prev_pt < prev_t
                        && prev_t - prev_pt > w.hit300
                        && prev_t - prev_pt <= w.hit100
                        && pt >= prev_t
                        && pt <= prev_t + w.hit50
                        && early_rel_before_note
                        && prewindow_overflow == early_penalty_window - 1
                        && prewindow_overflow <= 37
                        && ho.time - prev_t <= w.hit50 * 2;
                    let post_prev_head_nonsv = (prev_t - prev_pt).abs() <= w.hit300
                        && pt > prev_t + w.max
                        && pt <= prev_t + w.hit100
                        && has_in_win_cand
                        && presses[press_idx] < ho.time
                        && early_rel_before_note
                        && prewindow_overflow == early_penalty_window - 1
                        && ho.time - prev_t <= w.hit50 * 2;
                    severe_prev_early_pt
                        || medium_prev_gap_frag
                        || pos_pre_hea_cha_frag
                        || post_prev_head_nonsv
                })
                .unwrap_or(false);
        let strict_od_tap_keep =
            !false && !ho.is_long_note() && w.hit300 <= 38 && prewindow_overflow == w.hit300 - 1;
        let prev_early_gap_pen = !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    let severe_prev_early_pt = prev_pt < prev_t
                        && prev_t - prev_pt > w.hit300
                        && pt > prev_pt
                        && pt < prev_t
                        && prewindow_overflow < early_penalty_window;
                    let medium_prev_gap_frag = prev_pt < prev_t
                        && prev_t - prev_pt > w.max
                        && prev_t - prev_pt <= w.hit300
                        && pt > prev_pt
                        && pt < prev_t;
                    severe_prev_early_pt || medium_prev_gap_frag
                })
                .unwrap_or(false);
        let next_tap_lock_end = next_note_time
            .zip(col_notes.get(note_pos + 1))
            .map(|(next_t, (_, next_ho))| {
                !next_ho.is_long_note()
                    && press_idx < presses.len()
                    && presses[press_idx] >= lock_end_exclusive
                    && presses[press_idx] >= next_t - w.hit50
                    && presses[press_idx] < next_t + w.hit100
            })
            .unwrap_or(false);
        let exa_pre_hea_keep_pen = !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && (has_in_win_cand || next_tap_lock_end)
            && early_rel_before_note
            && early_press_rel_time
                .map(|rt| rt < presses[press_idx])
                .unwrap_or(false)
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && matches!((ho.time - pt).abs(), 160 | 161)
            && prev_note_time.map(|prev_t| pt > prev_t).unwrap_or(false);
        let exac_prev_dens_chain = !ho.is_long_note()
            && !prev_note_is_ln
            && has_in_win_cand
            && early_rel_before_note
            && early_press_rel_time
                .map(|rt| rt < presses[press_idx])
                .unwrap_or(false)
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prev_note_time
                .map(|prev_t| {
                    let current_gap = ho.time - prev_t;
                    let prehead_gap = prev_t - pt;
                    (prev_was_miss
                        && prev_had_prewin_pen
                        && pt > prev_t
                        && matches!((ho.time - pt).abs(), 160 | 161))
                        || (deep_tap_chain
                            && pt < prev_t
                            && (17..=24).contains(&prehead_gap)
                            && (136..=137).contains(&current_gap))
                })
                .unwrap_or(false);
        let exa_pre_edg_keep_pen = !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && early_press_rel_time
                .map(|rt| rt < presses[press_idx])
                .unwrap_or(false)
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && (ho.time - pt).abs() == 161
            && prev_col_pt
                .zip(prev_note_time)
                .zip(next_note_time)
                .map(|((prev_pt, prev_t), next_t)| {
                    prev_pt > prev_t
                        && prev_pt - prev_t <= w.hit300
                        && pt >= prev_t + w.hit50 - w.max
                        && pt <= prev_t + w.hit50
                        && next_t - ho.time > w.hit50 * 2
                })
                .unwrap_or(false);
        let far_tap_pen_base = false
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && presses[press_idx] < ho.time
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    let prev_delta = (prev_t - prev_pt).abs();
                    prev_delta > w.max && prev_delta <= w.hit300
                })
                .unwrap_or(false)
            && prev_note_time
                .map(|prev_t| ho.time - prev_t >= w.hit50 * 2)
                .unwrap_or(false)
            && prev_note_time
                .map(|prev_t| pt > prev_t + w.hit50 + w.max)
                .unwrap_or(false);
        let far_pen_to_exact = far_tap_pen_base
            && next_early_pen
                .zip(next_note_time)
                .map(|(next_penalty_start, next_head_time)| {
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let cand_pre_next = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time < next_head_time)
                        .unwrap_or(false);
                    let next_has_post_follow = col_notes
                        .get(note_pos + 1)
                        .map(|(_, next_ho)| {
                            if next_ho.is_long_note() {
                                return false;
                            }
                            let next_window_start = next_head_time - w.hit50;
                            let next_win_end = next_head_time + w.hit100;
                            presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|next_pt| **next_pt < next_win_end)
                                .find(|next_pt| {
                                    **next_pt >= next_window_start
                                        && !reserved_ln_repr.contains(next_pt)
                                })
                                .map(|next_pt| {
                                    *next_pt >= next_head_time
                                        && matches!(
                                            calc_hit_kind((*next_pt - next_head_time).abs(), w,),
                                            JudgmentKind::Max
                                                | JudgmentKind::Hit300
                                                | JudgmentKind::Hit200
                                        )
                                })
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    cand_pt == next_penalty_start
                        && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                        && cand_pre_next
                        && next_has_post_follow
                })
                .unwrap_or(false);
        let far_pen_to_prehead = far_tap_pen_base
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let next_window_start = next_head_time - w.hit50;
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let next_note_pre_follow = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|next_pt| **next_pt < next_win_end)
                        .find(|next_pt| {
                            **next_pt >= next_head_time - w.hit50
                                && next_next_tap_head
                                    .map(|next_next_head| **next_pt < next_next_head)
                                    .unwrap_or(true)
                                && !reserved_ln_repr.contains(next_pt)
                        })
                        .and_then(|next_pt| {
                            let next_followup_pt = *next_pt;
                            (next_followup_pt < next_head_time
                                && matches!(
                                    calc_hit_kind((next_followup_pt - next_head_time).abs(), w),
                                    JudgmentKind::Max | JudgmentKind::Hit300
                                ))
                            .then_some(next_followup_pt)
                        });
                    let candidate_release = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                        && next_note_pre_follow.is_some()
                        && candidate_release
                            .zip(next_note_pre_follow)
                            .map(|(rel_time, next_followup_pt)| {
                                (rel_time > ho.time && rel_time < next_window_start)
                                    || (cand_kind == JudgmentKind::Max
                                        && rel_time >= next_window_start
                                        && rel_time < next_followup_pt)
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let far_pen_to_post = far_tap_pen_base
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let candidate_release = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let next_window_start = next_head_time - w.hit50;
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let next_note_post_follow = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|next_pt| **next_pt < next_win_end)
                        .find(|next_pt| {
                            **next_pt >= next_window_start
                                && next_next_tap_head
                                    .map(|next_next_head| **next_pt < next_next_head)
                                    .unwrap_or(true)
                                && !reserved_ln_repr.contains(next_pt)
                        })
                        .and_then(|next_pt| {
                            let next_followup_pt = *next_pt;
                            let next_kind =
                                calc_hit_kind((next_followup_pt - next_head_time).abs(), w);
                            let next_release = events
                                .iter()
                                .find(|ev| ev.time > next_followup_pt && !ev.pressed)
                                .map(|ev| ev.time);
                            (next_followup_pt >= next_head_time
                                && matches!(next_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                                && next_release
                                    .map(|rel_time| {
                                        rel_time > next_head_time
                                            && next_next_tap_head
                                                .map(|next_next_head| rel_time < next_next_head)
                                                .unwrap_or(true)
                                    })
                                    .unwrap_or(false))
                            .then_some(next_followup_pt)
                        });
                    cand_kind == JudgmentKind::Max
                        && next_note_post_follow.is_some()
                        && candidate_release
                            .zip(next_note_post_follow)
                            .map(|(rel_time, next_followup_pt)| {
                                rel_time > ho.time
                                    && rel_time < next_head_time
                                    && rel_time < next_followup_pt
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let far_pen_to_iso = far_tap_pen_base
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let cand_pre_next = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time > ho.time && ev.time < next_head_time)
                        .unwrap_or(false);
                    let next_window_start = next_head_time - w.hit50;
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let next_note_has_cand = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|next_pt| **next_pt < next_win_end)
                        .any(|next_pt| {
                            *next_pt >= next_window_start
                                && next_next_tap_head
                                    .map(|next_next_head| *next_pt < next_next_head)
                                    .unwrap_or(true)
                                && !reserved_ln_repr.contains(next_pt)
                        });
                    cand_pt < ho.time
                        && cand_kind == JudgmentKind::Hit100
                        && next_head_time - ho.time > w.hit50 + w.hit300
                        && cand_pre_next
                        && !next_note_has_cand
                })
                .unwrap_or(false);
        let far_tap_pen_keep = far_tap_pen_base
            && !far_pen_to_exact
            && !far_pen_to_prehead
            && !far_pen_to_post
            && !far_pen_to_iso;
        let deep_tap = deep_tap
            && !prev_gap_early_pen
            && !strict_od_tap_keep
            && !exa_pre_hea_keep_pen
            && !exac_prev_dens_chain
            && !exa_pre_edg_keep_pen
            && !far_tap_pen_keep;
        let deep_tap_chain = deep_tap_chain
            && !prev_gap_early_pen
            && !strict_od_tap_keep
            && !exa_pre_hea_keep_pen
            && !exac_prev_dens_chain
            && !exa_pre_edg_keep_pen;
        let ln_near_deep_late = ho.is_long_note()
            && prewindow_overflow >= early_penalty_window - 1
            && has_in_win_cand
            && (presses[press_idx] >= ho.time || early_rel_before_note);
        let short_ln_prewin_claim = ho.is_long_note()
            && current_ln_duration <= w.hit100
            && prev_note_is_ln
            && prev_break_pre
            && !prev_was_miss
            && prewindow_overflow > 4
            && prewindow_overflow <= w.max / 2
            && has_in_win_cand
            && early_rel_before_note
            && {
                if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
                    if next_ho.is_long_note() {
                        false
                    } else {
                        let cand_pt = presses[press_idx];
                        let end_time = ho.end_time.unwrap_or(ho.time);
                        let tail_start = end_time - w.hit50;
                        let tail_end_exclusive = end_time + w.hit100;
                        let next_tap_window_start = next_ho.time - w.hit50;
                        let next_tap_end = next_ho.time + w.hit100;
                        let next_tap_left_end = next_tap_window_start + w.max + 2;
                        let has_next_tap_follow = press_idx + 1 < presses.len() && {
                            let next_pt = presses[press_idx + 1];
                            next_pt >= next_tap_window_start
                                && next_pt < next_tap_end
                                && !reserved_ln_repr.contains(&next_pt)
                        };
                        let cand_rel_in_tail_win2 = events
                            .iter()
                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                            .map(|ev| ev.time >= tail_start && ev.time < tail_end_exclusive)
                            .unwrap_or(false);
                        cand_pt >= ho.time
                            && cand_pt < lock_end_exclusive
                            && cand_pt >= next_tap_window_start
                            && cand_pt <= next_tap_left_end
                            && has_next_tap_follow
                            && cand_rel_in_tail_win2
                    }
                } else {
                    false
                }
            };
        let short_ln_prev_early = ho.is_long_note()
            && current_ln_duration <= w.hit100
            && prev_note_is_ln
            && !prev_was_miss
            && prev_break_pre
            && prev_note_duration
                .map(|d| d >= w.hit50 * 2)
                .unwrap_or(false)
            && prewindow_overflow >= w.max + 7
            && has_in_win_cand
            && early_rel_before_note
            && presses[press_idx] >= ho.time - w.max
            && presses[press_idx] < ho.time - 4
            && early_press_rel_time
                .zip(prev_note_end_time)
                .map(|(rt, prev_end)| rt <= prev_end)
                .unwrap_or(false)
            && prev_note_time
                .map(|prev_t| pt > prev_t + w.hit100)
                .unwrap_or(false);
        let short_ln_post_long = ho.is_long_note()
            && current_ln_duration <= w.hit100
            && prev_note_is_ln
            && prev_note_duration
                .map(|d| d >= w.hit50 * 2)
                .unwrap_or(false)
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| next_ho.is_long_note())
                .unwrap_or(false)
            && next_note_time
                .map(|next_t| next_t - ho.end_time.unwrap_or(ho.time) <= w.hit100)
                .unwrap_or(false)
            && prewindow_overflow > 4
            && prewindow_overflow <= w.max
            && has_in_win_cand
            && early_rel_before_note
            && early_press_rel_time
                .zip(prev_note_end_time)
                .map(|(rt, prev_end)| rt <= prev_end)
                .unwrap_or(false)
            && presses[press_idx] >= ho.time
            && {
                let cand_pt = presses[press_idx];
                let rel_after_cand = events
                    .iter()
                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let has_next_ln_follow = if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
                    let next_window_start = next_ho.time - w.hit50;
                    let next_next_note_time = col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                    let next_ln_late_end = next_next_note_time
                        .map(|next_time| next_time <= next_ho.time + w.hit50)
                        .unwrap_or(false);
                    let next_lock_end =
                        next_ho.time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                    press_idx + 1 < presses.len() && {
                        let next_pt = presses[press_idx + 1];
                        next_pt >= next_window_start
                            && next_pt < next_lock_end
                            && !reserved_ln_repr.contains(&next_pt)
                    }
                } else {
                    false
                };
                rel_after_cand
                    .map(|rt| {
                        next_note_time
                            .map(|next_t| {
                                rt <= next_t - w.hit50 + 1
                                    || (!false
                                        && prewindow_overflow <= w.hit50.min(w.max / 2 + 1)
                                        && has_next_ln_follow
                                        && rt <= next_t
                                        && rt <= ho.end_time.unwrap_or(ho.time) + w.max)
                            })
                            .unwrap_or(true)
                    })
                    .unwrap_or(false)
            };
        let ln_post_body_near = ho.is_long_note()
            && (current_ln_duration > w.hit100
                || (false && current_ln_duration <= w.hit100 && presses[press_idx] >= ho.time))
            && prev_note_is_ln
            && !prev_was_miss
            && prev_break_pre
            && prev_note_duration
                .map(|d| d >= w.hit50 * 2)
                .unwrap_or(false)
            && prewindow_overflow > w.max + 8
            && prewindow_overflow < early_penalty_window
            && has_in_win_cand
            && early_rel_before_note
            && presses[press_idx] >= ho.time - w.max
            && presses[press_idx] <= ho.time + w.hit300
            && early_press_rel_time
                .zip(prev_note_end_time)
                .map(|(rt, prev_end)| rt <= prev_end)
                .unwrap_or(false)
            && {
                let cand_pt = presses[press_idx];
                let end_time = ho.end_time.unwrap_or(ho.time);
                let tail_start = end_time - w.hit50;
                let tail_end_exclusive = end_time + w.hit100;
                let rel_after_cand = events
                    .iter()
                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let rel_in_tail_win = rel_after_cand
                    .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
                    .unwrap_or(false);
                let rel_pre_next = rel_after_cand
                    .zip(next_note_time)
                    .map(|(rt, next_t)| rt <= next_t)
                    .unwrap_or(true);
                rel_in_tail_win && rel_pre_next
            };
        let ln_pos_pre_shor_inwi = false
            && ho.is_long_note()
            && current_ln_duration > w.hit100
            && prev_note_is_ln
            && prev_was_miss
            && prev_break_pre
            && prev_note_duration.map(|d| d <= w.hit100).unwrap_or(false)
            && has_in_win_cand
            && early_rel_before_note
            && prewindow_overflow > w.max + 8
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && presses[press_idx] >= ho.time
            && presses[press_idx] <= ho.time + w.hit300
            && early_press_rel_time
                .zip(prev_note_end_time)
                .map(|(rt, prev_end)| rt > prev_end && rt <= prev_end + w.hit50 + w.hit100)
                .unwrap_or(false)
            && {
                let cand_pt = presses[press_idx];
                let end_time = ho.end_time.unwrap_or(ho.time);
                let tail_start = end_time - w.hit50;
                let tail_end_exclusive = end_time + w.hit100;
                let rel_after_cand = events
                    .iter()
                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let rel_in_tail_win = rel_after_cand
                    .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
                    .unwrap_or(false);
                let rel_pre_next = rel_after_cand
                    .zip(next_note_time)
                    .map(|(rt, next_t)| rt <= next_t)
                    .unwrap_or(true);
                rel_in_tail_win && rel_pre_next
            };
        let ln_pre_tai_pref_h100 = false
            && ho.is_long_note()
            && current_ln_duration > w.hit100
            && prev_note_is_ln
            && prev_was_miss
            && !prev_had_prewin_pen
            && prev_col_pt.is_none()
            && prev_note_duration.map(|d| d <= w.hit100).unwrap_or(false)
            && has_in_win_cand
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && pt < window_start
            && pt >= window_start - (w.max + 1)
            && presses[press_idx] > ho.time
            && presses[press_idx] <= ho.time + w.hit100
            && prev_note_end_time
                .map(|prev_end| pt > prev_end)
                .unwrap_or(false)
            && early_press_rel_time
                .zip(prev_note_end_time)
                .map(|(rt, prev_end)| {
                    rt > ho.time
                        && rt < presses[press_idx]
                        && rt > prev_end
                        && rt <= prev_end + w.hit50 + w.hit100
                })
                .unwrap_or(false)
            && {
                let cand_pt = presses[press_idx];
                let end_time = ho.end_time.unwrap_or(ho.time);
                let tail_start = end_time - w.hit50;
                let tail_end_exclusive = end_time + w.hit100;
                let rel_after_cand = events
                    .iter()
                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let rel_in_tail_win = rel_after_cand
                    .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
                    .unwrap_or(false);
                let rel_pre_next = rel_after_cand
                    .zip(next_note_time)
                    .map(|(rt, next_t)| rt <= next_t)
                    .unwrap_or(true);
                rel_in_tail_win && rel_pre_next
            };
        let sho_ln_pre_post_head = ho.is_long_note()
            && current_ln_duration <= w.hit100
            && prev_note_is_ln
            && prev_break_pre
            && prewindow_overflow > 4
            && prewindow_overflow <= w.max
            && has_in_win_cand
            && early_rel_before_note
            && presses[press_idx] >= ho.time
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| !next_ho.is_long_note())
                .unwrap_or(false)
            && {
                let cand_pt = presses[press_idx];
                let rel_after_cand = events
                    .iter()
                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let has_next_tap_follow = next_note_time
                    .map(|next_t| {
                        let next_tap_window_start = next_t - w.hit50;
                        let next_tap_end = next_t + w.hit100;
                        press_idx + 1 < presses.len() && {
                            let next_pt = presses[press_idx + 1];
                            next_pt >= next_tap_window_start
                                && next_pt < next_tap_end
                                && !reserved_ln_repr.contains(&next_pt)
                        }
                    })
                    .unwrap_or(false);
                rel_after_cand
                    .zip(next_note_time)
                    .map(|(rt, next_t)| rt >= next_t && has_next_tap_follow)
                    .unwrap_or(false)
            };
        let sho_ln_sta_post_head = !false
            && ho.is_long_note()
            && current_ln_duration <= w.hit100
            && prev_note_is_ln
            && !prev_was_miss
            && prev_break_pre
            && prev_note_duration
                .map(|d| d >= w.hit50 * 2)
                .unwrap_or(false)
            && prewindow_overflow <= w.hit300 - 7
            && has_in_win_cand
            && early_rel_before_note
            && presses[press_idx] >= ho.time
            && prev_note_end_time
                .map(|prev_end| pt <= prev_end)
                .unwrap_or(false);
        let prev_break_to_next = false
            && !ho.is_long_note()
            && prev_note_is_ln
            && prev_was_miss
            && prev_break_pre
            && has_in_win_cand
            && early_rel_before_note
            && prewindow_overflow <= w.max
            && prev_note_end_time
                .map(|prev_end| {
                    pt <= prev_end
                        && early_press_rel_time
                            .map(|rt| rt >= prev_end && rt < prev_end + w.hit100)
                            .unwrap_or(false)
                })
                .unwrap_or(false)
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let next_window_start = next_head_time - w.hit50;
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let has_next_tap_follow = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_win_end)
                        .any(|cand| {
                            *cand >= next_window_start
                                && next_next_tap_head
                                    .map(|next_next_head| *cand < next_next_head)
                                    .unwrap_or(true)
                                && !reserved_ln_repr.contains(cand)
                        });
                    cand_pt >= next_window_start && cand_pt < next_win_end && !has_next_tap_follow
                })
                .unwrap_or(false);
        let pos_pre_bod_keep_pen = false
            && !ho.is_long_note()
            && prev_note_is_ln
            && !prev_was_miss
            && prev_break_pre
            && has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prewindow_overflow >= early_penalty_window - 1
            && prev_note_end_time
                .map(|prev_end| {
                    pt <= prev_end
                        && early_press_rel_time
                            .map(|rt| rt >= prev_end && rt < ho.time)
                            .unwrap_or(false)
                })
                .unwrap_or(false)
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if !next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let next_window_start = next_head_time - w.hit50;
                    let next_prewin_start = next_window_start - early_penalty_window - 1;
                    cand_pt >= next_prewin_start
                        && cand_pt < next_window_start
                        && cand_pt < ho.time
                        && events
                            .iter()
                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                            .map(|ev| ev.time > ho.time && ev.time < next_head_time)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let tap_prev_keeps_pair = false
            && !ho.is_long_note()
            && prev_note_is_ln
            && prev_was_miss
            && prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && next_note_time
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_time, (_, next_ho))| {
                    if !next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_release = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                    let next_duration = next_end_time - next_ho.time;
                    let next_window_start = next_time - w.hit50;
                    let next_tail_start = next_end_time - w.hit50;
                    let next_tail_end = next_end_time + w.hit100;
                    let next_next_note_time = col_notes.get(note_pos + 2).map(|(_, ho)| ho.time);
                    next_time - ho.time <= w.hit50 + w.hit300
                        && cand_pt >= next_window_start
                        && cand_pt < ho.time
                        && next_duration <= w.hit100
                        && cand_release
                            .map(|rt| {
                                rt >= next_tail_start
                                    && rt < next_tail_end
                                    && rt > cand_pt
                                    && next_next_note_time
                                        .map(|next_time| rt < next_time)
                                        .unwrap_or(true)
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let post_ln_body_late = !ho.is_long_note()
            && prev_note_is_ln
            && early_rel_before_note
            && prewindow_overflow >= early_penalty_window - 1
            && has_in_win_cand
            && presses[press_idx] >= ho.time - w.max
            && !prev_break_to_next
            && !pos_pre_bod_keep_pen
            && !tap_prev_keeps_pair;
        let held_prev_ln_no_repr =
            !ho.is_long_note() && prev_note_is_ln && !early_rel_before_note && !has_in_win_cand;
        let tap_post_prev_no_cand = !ho.is_long_note()
            && prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && !has_in_win_cand
            && early_rel_before_note
            && prewindow_overflow >= early_penalty_window - 1
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prev_note_end_time.map(|end| pt > end).unwrap_or(false);
        let deep_prewin_no_cand = !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && !has_in_win_cand
            && prewindow_overflow >= early_penalty_window - 1
            && !strict_od_tap_keep;
        let prewin_edge_auto_miss = !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && !has_in_win_cand
            && early_rel_before_note
            && prewindow_overflow == early_penalty_window + 1;
        let far_note_no_inwin = !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_had_prewin_pen
            && !has_in_win_cand
            && prewindow_overflow >= early_penalty_window - 1
            && prev_note_time
                .map(|prev_t| pt > prev_t + w.hit50)
                .unwrap_or(false);
        let tap_exact_no_cand = !ho.is_long_note()
            && !prev_note_is_ln
            && !has_in_win_cand
            && early_rel_before_note
            && prewindow_overflow == early_penalty_window
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prev_note_time
                .map(|prev_t| pt > prev_t && pt <= prev_t + w.hit50 + w.max)
                .unwrap_or(false);
        let exa_pen_hold_no_inwi = !ho.is_long_note()
            && !prev_note_is_ln
            && prev_was_miss
            && prev_had_prewin_pen
            && !has_in_win_cand
            && !early_rel_before_note
            && prewindow_overflow == early_penalty_window
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prev_note_time
                .map(|prev_t| pt > prev_t && pt <= prev_t + w.hit50 + w.max)
                .unwrap_or(false);
        let post_prev_frag = !false
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && !has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prewindow_overflow >= early_penalty_window - 3
            && early_press_rel_time
                .map(|rt| rt <= window_start + w.max / 2)
                .unwrap_or(false)
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    prev_pt < prev_t
                        && prev_t - prev_pt > w.hit300
                        && pt > prev_t
                        && pt <= prev_t + w.hit100
                        && ho.time - prev_t <= w.hit50 * 2
                })
                .unwrap_or(false);
        let post_prev_frag_next = false
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && !has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prewindow_overflow >= early_penalty_window - 3
            && early_press_rel_time
                .map(|rt| rt <= window_start + w.max / 2)
                .unwrap_or(false)
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    prev_pt < prev_t
                        && prev_t - prev_pt > w.hit300
                        && pt > prev_t
                        && pt <= prev_t + w.hit100
                        && ho.time - prev_t <= w.hit50 * 2
                })
                .unwrap_or(false)
            && next_note_time
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_t, (_, next_ho))| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let next_window_start = next_t - w.hit50;
                    let next_win_end = next_t + w.hit100;
                    press_idx < presses.len()
                        && presses[press_idx] >= next_window_start
                        && presses[press_idx] < next_win_end
                        && !reserved_ln_repr.contains(&presses[press_idx])
                })
                .unwrap_or(false);
        let pos_pre_prwn_next_ln = !ho.is_long_note()
            && prev_note_is_ln
            && prev_had_prewin_pen
            && prewindow_overflow > 4
            && prewindow_overflow <= w.max
            && early_rel_before_note
            && has_in_win_cand
            && presses[press_idx] >= ho.time
            && prev_note_time
                .map(|prev_t| pt > prev_t + w.hit50)
                .unwrap_or(false)
            && {
                if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
                    if !next_ho.is_long_note() {
                        false
                    } else {
                        let cand_pt = presses[press_idx];
                        let next_ln_duration =
                            next_ho.end_time.unwrap_or(next_ho.time) - next_ho.time;
                        let next_window_start = next_ho.time - w.hit50;
                        let next_next_note_time = col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                        let next_ln_late_end = next_next_note_time
                            .map(|next_time| next_time <= next_ho.time + w.hit50)
                            .unwrap_or(false);
                        let next_lock_end =
                            next_ho.time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                        let has_next_ln_follow = press_idx + 1 < presses.len() && {
                            let next_pt = presses[press_idx + 1];
                            next_pt >= next_window_start
                                && next_pt < next_lock_end
                                && !reserved_ln_repr.contains(&next_pt)
                        };
                        next_ln_duration > w.hit200
                            && next_ln_duration <= w.hit100
                            && cand_pt >= next_window_start
                            && cand_pt < next_lock_end
                            && has_next_ln_follow
                    }
                } else {
                    false
                }
            };
        let stale_chain_prewin = !ho.is_long_note()
            && prev_had_prewin_pen
            && has_in_win_cand
            && prev_note_time.map(|prev_t| pt < prev_t).unwrap_or(false)
            && !(false
                && prev_col_pt
                    .zip(prev_note_time)
                    .map(|(prev_pt, prev_t)| pt > prev_pt && pt < prev_t)
                    .unwrap_or(false));
        let prev_head_noise_prwn = !ho.is_long_note()
            && !prev_note_is_ln
            && has_in_win_cand
            && prewindow_overflow >= early_penalty_window - 1
            && early_rel_before_note
            && prev_note_time
                .map(|prev_t| pt >= prev_t && pt <= prev_t + w.max)
                .unwrap_or(false);
        let prev_cross_h200 = !false
            && presses[press_idx] <= ho.time + w.hit200
            && prewindow_overflow <= w.max
            && early_press_rel_time
                .zip(prev_note_time)
                .map(|(rt, prev_t)| rt > prev_t && rt < ho.time)
                .unwrap_or(false)
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| prev_t - prev_pt > w.hit300 && pt > prev_pt && pt < prev_t)
                .unwrap_or(false);
        let prev_near_h200 = !false
            && presses[press_idx] >= ho.time - w.hit200
            && presses[press_idx] < ho.time - w.hit300
            && calc_hit_kind((presses[press_idx] - ho.time).abs(), w) == JudgmentKind::Hit200
            && prewindow_overflow < early_penalty_window - 1
            && early_press_rel_time
                .zip(prev_note_time)
                .map(|(rt, prev_t)| {
                    let dense_gap_keeps_tap = col_notes
                        .get(note_pos + 1)
                        .zip(next_note_time)
                        .map(|((_, next_ho), next_t)| {
                            if next_ho.is_long_note() || next_t - ho.time > w.hit100 {
                                return false;
                            }
                            let next_window_start = next_t - w.hit50;
                            let next_win_end = next_t + w.hit100;
                            presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|cand| **cand < next_win_end)
                                .any(|cand| {
                                    *cand >= next_window_start && !reserved_ln_repr.contains(cand)
                                })
                        })
                        .unwrap_or(false);
                    rt <= prev_t + w.max
                        && prev_t - rt <= w.hit300
                        && next_note_time
                            .map(|next_t| {
                                next_t - ho.time > w.hit50 + w.hit300 || dense_gap_keeps_tap
                            })
                            .unwrap_or(true)
                })
                .unwrap_or(false)
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| prev_t - prev_pt > w.hit300 && pt > prev_pt && pt < prev_t)
                .unwrap_or(false)
            && events
                .iter()
                .find(|ev| ev.time > presses[press_idx] && !ev.pressed)
                .map(|ev| {
                    next_note_time
                        .map(|next_t| ev.time < next_t)
                        .unwrap_or(true)
                })
                .unwrap_or(false);
        let prev_near_h100 = !false
            && presses[press_idx] >= ho.time - w.hit100
            && presses[press_idx] < ho.time - w.hit200
            && calc_hit_kind((presses[press_idx] - ho.time).abs(), w) == JudgmentKind::Hit100
            && prewindow_overflow < early_penalty_window - 1
            && early_press_rel_time
                .zip(prev_note_time)
                .map(|(rt, prev_t)| {
                    rt <= prev_t
                        && prev_t - rt <= w.hit300
                        && next_note_time
                            .map(|next_t| next_t - ho.time <= w.hit50 + w.hit300)
                            .unwrap_or(false)
                })
                .unwrap_or(false)
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| prev_t - prev_pt > w.hit300 && pt > prev_pt && pt < prev_t)
                .unwrap_or(false)
            && events
                .iter()
                .find(|ev| ev.time > presses[press_idx] && !ev.pressed)
                .map(|ev| {
                    next_note_time
                        .map(|next_t| ev.time < next_t)
                        .unwrap_or(true)
                })
                .unwrap_or(false);
        let prewin_prev_near_head = !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && (early_rel_before_note || early_rel_same_ms)
            && (!false || presses[press_idx] >= ho.time)
            && (presses[press_idx] >= ho.time - w.hit300 || prev_near_h200 || prev_near_h100)
            && (presses[press_idx] <= ho.time + w.hit300 || prev_cross_h200)
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| pt > prev_pt && pt < prev_t)
                .unwrap_or(false)
            && !prev_gap_early_pen;
        let short_ln_prewin = ho.is_long_note()
            && (!false || prev_was_miss)
            && current_ln_duration <= w.hit100
            && prev_note_is_ln
            && has_in_win_cand
            && early_rel_before_note
            && (!false
                || early_press_rel_time
                    .zip(prev_note_end_time)
                    .map(|(rt, prev_end)| rt <= prev_end)
                    .unwrap_or(false))
            && prewindow_overflow >= w.max + 8
            && prewindow_overflow < early_penalty_window
            && presses[press_idx] >= ho.time - w.hit300
            && presses[press_idx] <= ho.time + w.hit300
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| pt > prev_pt && pt < prev_t)
                .unwrap_or(false);
        let stale_prev_ln_pen = ho.is_long_note()
            && prev_note_is_ln
            && has_in_win_cand
            && presses[press_idx] >= ho.time
            && prev_col_pt.map(|prev_pt| prev_pt == pt).unwrap_or(false)
            && prev_note_time
                .map(|prev_t| pt > prev_t + w.hit100)
                .unwrap_or(false);
        let stale_prev_ln_no_repr = ho.is_long_note()
            && prev_note_is_ln
            && prev_col_pt.map(|prev_pt| prev_pt == pt).unwrap_or(false)
            && !early_rel_before_note;
        let short_ln_carry_hless = false
            && ho.is_long_note()
            && current_ln_duration <= w.hit100
            && prev_note_is_ln
            && prev_was_miss
            && prev_had_prewin_pen
            && prev_break_pre
            && prev_note_duration
                .map(|dur| dur <= w.hit100)
                .unwrap_or(false)
            && !has_in_win_cand
            && pt < ho.time
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prewindow_overflow <= w.max
            && prev_col_pt
                .zip(prev_note_end_time.zip(prev_note_time))
                .map(|(prev_pt, (prev_end, prev_time))| {
                    pt > prev_pt
                        && pt <= prev_end
                        && ho.time - prev_end <= w.hit50
                        && prev_end - prev_time <= w.hit100
                })
                .unwrap_or(false)
            && early_press_rel_time
                .map(|rt| {
                    let end_time = ho.end_time.unwrap_or(ho.time);
                    rt > end_time
                        && next_note_time
                            .map(|next_time| {
                                rt < next_time && next_time - ho.time <= w.hit50 + w.max
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let post_prev_ln_no_inwin = !ho.is_long_note()
            && prev_note_is_ln
            && prev_was_miss
            && prev_break_pre
            && !has_in_win_cand
            && prewindow_overflow > 4
            && prev_note_end_time
                .map(|prev_end| {
                    pt < prev_end
                        && (early_rel_before_note
                            || (false
                                && prev_note_duration.map(|d| d <= w.hit100).unwrap_or(false)
                                && !early_rel_before_note
                                && early_press_rel_time
                                    .map(|rt| rt > ho.time && rt <= prev_end + w.hit50 + w.hit100)
                                    .unwrap_or(false)))
                })
                .unwrap_or(false);
        let post_prev_break = false
            && !ho.is_long_note()
            && prev_note_is_ln
            && prev_was_miss
            && prev_break_pre
            && has_in_win_cand
            && early_rel_before_note
            && {
                let cand_pt = presses[press_idx];
                let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                (!prev_had_prewin_pen && prewindow_overflow <= w.max)
                    || (prev_had_prewin_pen
                        && prewindow_overflow > w.max
                        && prewindow_overflow < early_penalty_window - 1
                        && cand_pt >= ho.time - w.max
                        && matches!(
                            cand_kind,
                            JudgmentKind::Max | JudgmentKind::Hit300 | JudgmentKind::Hit200
                        )
                        && col_notes
                            .get(note_pos + 1)
                            .zip(next_note_time)
                            .map(|((_, next_ho), next_head_time)| {
                                if !next_ho.is_long_note() {
                                    return false;
                                }
                                let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                                let next_duration = next_end_time - next_ho.time;
                                let next_window_start = next_head_time - w.hit50;
                                let next_next_note_time =
                                    col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                                let next_ln_late_end = next_next_note_time
                                    .map(|next_time| next_time <= next_ho.time + w.hit50)
                                    .unwrap_or(false);
                                let next_lock_end =
                                    next_ho.time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                                let next_tail_start = next_end_time - w.hit50;
                                let next_tail_end = next_end_time + w.hit100;
                                let next_ln_follow = presses
                                    .iter()
                                    .copied()
                                    .skip(press_idx + 1)
                                    .take_while(|cand| *cand < next_lock_end)
                                    .find(|cand| {
                                        *cand >= next_window_start
                                            && !reserved_ln_repr.contains(cand)
                                    });
                                let next_ln_self_fol = next_ln_follow
                                    .and_then(|followup_pt| {
                                        events
                                            .iter()
                                            .find(|ev| ev.time > followup_pt && !ev.pressed)
                                            .map(|ev| ev.time)
                                    })
                                    .map(|rt| rt >= next_tail_start && rt < next_tail_end)
                                    .unwrap_or(false);
                                next_head_time - ho.time <= w.hit50 + w.max
                                    && next_duration <= w.hit100
                                    && cand_pt < next_window_start
                                    && next_ln_self_fol
                            })
                            .unwrap_or(false))
            }
            && prev_note_end_time
                .map(|prev_end| {
                    pt <= prev_end
                        && early_press_rel_time
                            .map(|rt| rt >= prev_end && rt < prev_end + w.hit100)
                            .unwrap_or(false)
                })
                .unwrap_or(false)
            && !prev_break_to_next
            && !col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if !next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let next_window_start = next_head_time - w.hit50;
                    let next_next_note_time = col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                    let next_ln_late_end = next_next_note_time
                        .map(|next_time| next_time <= next_ho.time + w.hit50)
                        .unwrap_or(false);
                    let next_lock_end =
                        next_ho.time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                    let has_next_ln_follow = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_lock_end)
                        .any(|cand| *cand >= next_window_start && !reserved_ln_repr.contains(cand));
                    let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                    let next_duration = next_end_time - next_ho.time;
                    let next_tail_start = next_end_time - w.hit50;
                    let next_tail_end = next_end_time + w.hit100;
                    let rel_after_cand = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let rel_in_next_tail = rel_after_cand
                        .map(|rt| rt >= next_tail_start && rt < next_tail_end)
                        .unwrap_or(false);
                    let next_ln_follow = presses
                        .iter()
                        .copied()
                        .skip(press_idx + 1)
                        .take_while(|cand| *cand < next_lock_end)
                        .find(|cand| {
                            *cand >= next_window_start && !reserved_ln_repr.contains(cand)
                        });
                    let next_ln_self_fol = next_ln_follow
                        .and_then(|followup_pt| {
                            events
                                .iter()
                                .find(|ev| ev.time > followup_pt && !ev.pressed)
                                .map(|ev| ev.time)
                        })
                        .map(|rt| rt >= next_tail_start && rt < next_tail_end)
                        .unwrap_or(false);
                    cand_pt >= next_window_start
                        && cand_pt < next_lock_end
                        && ((!has_next_ln_follow && rel_in_next_tail)
                            || (next_duration <= w.hit100 && rel_in_next_tail && next_ln_self_fol))
                })
                .unwrap_or(false);
        let pos_prev_tap_no_inwi = !ho.is_long_note()
            && !prev_note_is_ln
            && prev_was_miss
            && !prev_had_prewin_pen
            && !has_in_win_cand
            && early_rel_before_note
            && prewindow_overflow >= early_penalty_window - 1
            && prev_note_time
                .map(|prev_t| {
                    let near_prev_miss = pt <= prev_t + w.hit50 + w.max;
                    let short_prev_gap = ho.time - prev_t <= w.hit50 * 3;
                    pt > prev_t && near_prev_miss && short_prev_gap
                })
                .unwrap_or(false);
        let prev_pen_next_ln = false
            && !ho.is_long_note()
            && !prev_note_is_ln
            && prev_was_miss
            && prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| pt > prev_pt && pt < prev_t)
                .unwrap_or(false)
            && prev_note_time
                .map(|prev_t| pt < prev_t && ho.time - prev_t <= w.hit50 + w.hit300)
                .unwrap_or(false)
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if !next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let next_duration = next_ho.end_time.unwrap_or(next_ho.time) - next_ho.time;
                    let next_next_note_time = col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                    let next_ln_late_end = next_next_note_time
                        .map(|next_time| next_time <= next_ho.time + w.hit50)
                        .unwrap_or(false);
                    let next_lock_end =
                        next_ho.time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                    let next_ln_follow = presses
                        .iter()
                        .copied()
                        .skip(press_idx + 1)
                        .take_while(|cand| *cand < next_lock_end)
                        .find(|cand| {
                            *cand >= next_ho.time - w.hit50 && !reserved_ln_repr.contains(cand)
                        });
                    let has_next_ln_follow = next_ln_follow.is_some();
                    let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                    let next_tail_start = next_end_time - w.hit50;
                    let next_tail_end = next_end_time + w.hit100;
                    let next_ln_h50_cur =
                        calc_hit_kind((cand_pt - next_head_time).abs(), w) == JudgmentKind::Hit50;
                    let cur_is_next_ln_prewin = calc_hit_kind((cand_pt - next_head_time).abs(), w)
                        == JudgmentKind::Miss
                        && cand_pt >= next_head_time - w.hit50 - early_penalty_window - 1
                        && cand_pt < next_head_time - w.hit50;
                    let next_ln_pre_follow = next_ln_follow
                        .map(|followup_pt| followup_pt < next_head_time)
                        .unwrap_or(false);
                    let next_ln_self_fol = next_ln_follow
                        .and_then(|followup_pt| {
                            events
                                .iter()
                                .find(|ev| ev.time > followup_pt && !ev.pressed)
                                .map(|ev| ev.time)
                        })
                        .map(|rt| rt >= next_tail_start && rt < next_tail_end)
                        .unwrap_or(false);
                    cand_pt >= ho.time
                        && cand_pt < next_head_time
                        && next_head_time - ho.time <= w.hit50 + w.hit300
                        && matches!(
                            cand_kind,
                            JudgmentKind::Max | JudgmentKind::Hit300 | JudgmentKind::Hit200
                        )
                        && has_next_ln_follow
                        && !(cur_is_next_ln_prewin && next_ln_pre_follow)
                        && !(next_duration > w.hit100 && next_ln_h50_cur && next_ln_self_fol)
                        && events
                            .iter()
                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                            .map(|ev| ev.time > ho.time && ev.time < next_head_time)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let prev_pen_next_tap = false
            && !ho.is_long_note()
            && !prev_note_is_ln
            && prev_was_miss
            && prev_had_prewin_pen
            && prev_prev_was_miss
            && prev2_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| pt > prev_pt && pt < prev_t)
                .unwrap_or(false)
            && prev_note_time
                .map(|prev_t| {
                    pt < prev_t && prev_t - pt > w.max / 2 && ho.time - prev_t <= w.hit50 + w.hit300
                })
                .unwrap_or(false)
            && prewindow_overflow > 4
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let next_window_start = next_head_time - w.hit50;
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let next2_gap_flat = next_next_tap_head
                        .map(|next_next_head| {
                            next_next_head - next_head_time >= next_head_time - ho.time
                        })
                        .unwrap_or(true);
                    let has_next_tap_follow = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_win_end)
                        .find(|cand| {
                            **cand >= next_window_start
                                && next_next_tap_head
                                    .map(|next_next_head| **cand < next_next_head)
                                    .unwrap_or(true)
                                && !reserved_ln_repr.contains(cand)
                        })
                        .map(|cand| {
                            *cand < next_head_time
                                && matches!(
                                    calc_hit_kind((*cand - next_head_time).abs(), w),
                                    JudgmentKind::Max | JudgmentKind::Hit300 | JudgmentKind::Hit200
                                )
                        })
                        .unwrap_or(false);
                    cand_pt >= ho.time - w.max
                        && cand_pt < ho.time
                        && cand_pt < next_head_time
                        && cand_kind == JudgmentKind::Max
                        && (next2_gap_flat || prewindow_overflow >= early_penalty_window - 1)
                        && has_next_tap_follow
                        && events
                            .iter()
                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                            .map(|ev| ev.time > ho.time && ev.time < next_head_time - w.hit50)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let pre_mis_pen_next_tap = false
            && !ho.is_long_note()
            && !prev_note_is_ln
            && prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| prev_pt < prev_t)
                .unwrap_or(true)
            && prev_note_time
                .map(|prev_t| {
                    pt == prev_t + w.hit100
                        && ho.time - prev_t > w.hit50 + w.hit300
                        && ho.time - prev_t <= w.hit50 * 2
                })
                .unwrap_or(false)
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let next_window_start = next_head_time - w.hit50;
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let has_next_tap_follow = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_win_end)
                        .find(|cand| {
                            **cand >= next_window_start
                                && next_next_tap_head
                                    .map(|next_next_head| **cand < next_next_head)
                                    .unwrap_or(true)
                                && !reserved_ln_repr.contains(cand)
                        })
                        .map(|cand| {
                            !matches!(
                                calc_hit_kind((*cand - next_head_time).abs(), w),
                                JudgmentKind::Miss
                            )
                        })
                        .unwrap_or(false);
                    cand_pt >= ho.time
                        && cand_pt < next_head_time
                        && matches!(
                            cand_kind,
                            JudgmentKind::Max | JudgmentKind::Hit300 | JudgmentKind::Hit200
                        )
                        && has_next_tap_follow
                        && events
                            .iter()
                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                            .map(|ev| {
                                ev.time >= next_head_time && ev.time < next_head_time + w.hit100
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let prev_miss_pen_iso = false
            && !ho.is_long_note()
            && !prev_note_is_ln
            && prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| prev_pt < prev_t)
                .unwrap_or(true)
            && prev_note_time
                .map(|prev_t| pt == prev_t + w.hit100 && ho.time - prev_t > w.hit50 + w.hit300)
                .unwrap_or(false)
            && {
                let cand_pt = presses[press_idx];
                let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                let cand_pre_next = events
                    .iter()
                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                    .map(|ev| next_note_time.map(|nt| ev.time < nt).unwrap_or(true))
                    .unwrap_or(false);
                let next_note_has_cand = col_notes
                    .get(note_pos + 1)
                    .zip(next_note_time)
                    .map(|((_, next_ho), next_head_time)| {
                        if next_ho.is_long_note() {
                            return true;
                        }
                        let next_window_start = next_head_time - w.hit50;
                        let next_win_end = next_head_time + w.hit100;
                        let next_next_tap_head =
                            col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                                (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                            });
                        presses
                            .iter()
                            .skip(press_idx + 1)
                            .take_while(|cand| **cand < next_win_end)
                            .any(|cand| {
                                *cand >= next_window_start
                                    && next_next_tap_head
                                        .map(|next_next_head| *cand < next_next_head)
                                        .unwrap_or(true)
                                    && !reserved_ln_repr.contains(cand)
                            })
                    })
                    .unwrap_or(false);
                matches!(
                    cand_kind,
                    JudgmentKind::Max
                        | JudgmentKind::Hit300
                        | JudgmentKind::Hit200
                        | JudgmentKind::Hit100
                ) && cand_pre_next
                    && !next_note_has_cand
            };
        let ln_pos_prev_tap_inwi = ho.is_long_note()
            && !prev_note_is_ln
            && prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && prewindow_overflow > 4
            && prewindow_overflow <= w.max * 2
            && prev_col_pt.map(|prev_pt| pt > prev_pt).unwrap_or(false)
            && presses[press_idx] >= ho.time - w.max
            && presses[press_idx] <= ho.time + w.max
            && prev_note_time
                .map(|prev_t| {
                    let near_prev_miss = pt <= prev_t + w.hit50 + w.max;
                    let short_prev_gap = ho.time - prev_t <= w.hit50 * 3;
                    pt > prev_t && near_prev_miss && short_prev_gap
                })
                .unwrap_or(false);
        let ln_prewin_near_head = ho.is_long_note()
            && !prev_note_is_ln
            && prev_was_miss
            && prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && prewindow_overflow > w.max + 8
            && prewindow_overflow <= w.hit300 + w.max
            && presses[press_idx] >= ho.time - w.max
            && presses[press_idx] <= ho.time + w.max
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    pt > prev_pt
                        && pt < prev_t
                        && prev_t - prev_pt > w.hit50
                        && ho.time - prev_t <= w.hit50
                })
                .unwrap_or(false);
        let ln_prev_tap_near_head = ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && prewindow_overflow > 4
            && prewindow_overflow <= w.max * 2
            && presses[press_idx] >= ho.time - w.hit300
            && presses[press_idx] <= ho.time + w.hit300
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| pt > prev_pt && pt < prev_t)
                .unwrap_or(false);
        let sho_ln_pre_frag_clai = false
            && ho.is_long_note()
            && current_ln_duration <= w.hit100
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && prewindow_overflow > 4
            && prewindow_overflow <= w.max * 2
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && presses[press_idx] >= ho.time - w.hit300
            && presses[press_idx] <= ho.time + w.hit300
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| pt > prev_pt && pt < prev_t)
                .unwrap_or(false)
            && early_press_rel_time
                .map(|rt| {
                    let end_time = ho.end_time.unwrap_or(ho.time);
                    let tail_only_start = end_time - ((w.hit50 as f32) * 1.5).round() as i32;
                    rt >= tail_only_start && rt < ho.time
                })
                .unwrap_or(false);
        let prev_prwn_keeps_repr = false
            && ln_prev_tap_near_head
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && early_press_rel_time.map(|rt| rt < ho.time).unwrap_or(false)
            && {
                let end_time = ho.end_time.unwrap_or(ho.time);
                let tail_start = end_time - w.hit50;
                let tail_end_exclusive = end_time + w.hit100;
                let candidate_pt = presses[press_idx];
                candidate_pt > ho.time
                    && candidate_pt <= end_time
                    && events
                        .iter()
                        .find(|ev| ev.time > candidate_pt && !ev.pressed)
                        .map(|ev| {
                            ev.time >= tail_start
                                && ev.time < tail_end_exclusive
                                && ev.time > candidate_pt
                        })
                        .unwrap_or(false)
            };
        let prev_noise_keeps_repr = false
            && ho.is_long_note()
            && current_ln_duration > w.hit100
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| pt > prev_pt && pt > prev_t)
                .unwrap_or(false)
            && early_press_rel_time
                .map(|rt| rt > pt && rt - pt <= w.hit300)
                .unwrap_or(false)
            && {
                let end_time = ho.end_time.unwrap_or(ho.time);
                let tail_start = end_time - w.hit50;
                let tail_end_exclusive = end_time + w.hit100;
                let candidate_pt = presses[press_idx];
                candidate_pt >= ho.time
                    && candidate_pt <= end_time.min(ho.time + w.max)
                    && events
                        .iter()
                        .find(|ev| ev.time > candidate_pt && !ev.pressed)
                        .map(|ev| {
                            ev.time >= tail_start
                                && ev.time < tail_end_exclusive
                                && ev.time > candidate_pt
                        })
                        .unwrap_or(false)
            };
        let prev_frag_keeps_tail = prev_prwn_keeps_repr || prev_noise_keeps_repr;
        let prev_frag_blocked = !prev_frag_keeps_tail;
        let deep_ln = deep_ln && !sho_ln_pre_frag_clai && prev_frag_blocked;
        let ln_prev_tap_near_head =
            ln_prev_tap_near_head && !sho_ln_pre_frag_clai && prev_frag_blocked;
        let ln_near_deep_late = ln_near_deep_late && prev_frag_blocked;
        let deep_tap = deep_tap && !pos_pre_bod_keep_pen;
        let deep_ln_no_head = deep_ln
            && !has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss;
        let flags = PenaltyFlags {
            deep_ln,
            deep_ln_chain,
            ln_near_deep_late,
            short_ln_prewin_claim,
            short_ln_prev_early,
            short_ln_post_long,
            sho_ln_sta_post_head,
            ln_post_body_near,
            ln_pos_pre_shor_inwi,
            ln_pre_tai_pref_h100,
            sho_ln_pre_post_head,
            post_ln_body_late,
            held_prev_ln_no_repr,
            pos_pre_prwn_next_ln,
            exa_pre_hea_keep_pen,
            exac_prev_dens_chain,
            exa_pre_edg_keep_pen,
            deep_tap,
            deep_tap_chain,
            stale_chain_prewin,
            prev_head_noise_prwn,
            prewin_prev_near_head,
            short_ln_prewin,
            ln_prewin_near_head,
            ln_prev_tap_near_head,
            ln_pos_prev_tap_inwi,
            prev_pen_next_ln,
            prev_pen_next_tap,
            pre_mis_pen_next_tap,
            prev_miss_pen_iso,
            post_prev_break,
            post_prev_frag,
            post_prev_frag_next,
        };
        if flags.keeps_exact_pen() {
        } else if flags.clears_inwin_pen() && has_in_win_cand {
            early_pen_pt = None;
        } else if deep_ln_no_head {
            early_pen_pt = None;
        } else if tap_post_prev_no_cand {
            early_pen_pt = None;
        } else if deep_prewin_no_cand {
            early_pen_pt = None;
        } else if prewin_edge_auto_miss {
            early_pen_pt = None;
        } else if far_note_no_inwin {
            early_pen_pt = None;
        } else if tap_exact_no_cand {
            early_pen_pt = None;
        } else if exa_pen_hold_no_inwi {
            early_pen_pt = None;
        } else if post_prev_frag || post_prev_frag_next {
            early_pen_pt = None;
        } else if stale_prev_ln_pen {
            early_pen_pt = None;
        } else if stale_prev_ln_no_repr {
            early_pen_pt = None;
        } else if short_ln_carry_hless {
            early_pen_pt = None;
        } else if post_prev_ln_no_inwin {
            early_pen_pt = None;
        } else if pos_prev_tap_no_inwi {
            early_pen_pt = None;
        }
        if early_pen_pt.is_some() && flags.active_rule().is_none() && prev_early_gap_pen {
            retained_penalty_rule = Some("prev_gap_early_pen");
        }
        state.penalty_flags = flags;
    }
    state.rules.early_pen = early_pen_pt;
    state.rules.pen = retained_penalty_rule;
}
