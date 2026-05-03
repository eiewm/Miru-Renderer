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
    let mut cleared_penalty_rule: Option<&'static str> = None;
    let mut retained_penalty_rule: Option<&'static str> = None;
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
        let prev_pen_to_iso = true
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
                let next_note_is_tap = col_notes
                    .get(note_pos + 1)
                    .map(|(_, next_ho)| !next_ho.is_long_note())
                    .unwrap_or(true);
                cand_pt >= ho.time
                    && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                    && events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| {
                            (ev.time <= ho.time + w.hit100
                                || (next_note_is_tap && ev.time <= ho.time + w.hit50))
                                && next_note_time
                                    .map(|next_t| ev.time < next_t)
                                    .unwrap_or(true)
                        })
                        .unwrap_or(false)
            };
        let prev_pen_to_post = true
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
        let repeat_prev_pen = prev_prev_was_miss
            && prev2_had_prewin_pen
            && prev_note_time
                .map(|prev_t| pt < prev_t && prev_t - pt <= w.max)
                .unwrap_or(false);
        let prev_pen_to_prehead = true
            && !ho.is_long_note()
            && prev_was_miss
            && prev_had_prewin_pen
            && (!(prev_prev_was_miss && prev2_had_prewin_pen) || repeat_prev_pen)
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
                    let next3_tap_head =
                        col_notes
                            .get(note_pos + 3)
                            .and_then(|(_, next_next_next_ho)| {
                                (!next_next_next_ho.is_long_note())
                                    .then_some(next_next_next_ho.time)
                            });
                    let nex_note_own_str_fol = presses
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
                            let next_followup_release = events
                                .iter()
                                .find(|ev| ev.time > next_followup_pt && !ev.pressed)
                                .map(|ev| ev.time);
                            (matches!(
                                calc_hit_kind((next_followup_pt - next_head_time).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300
                            ) && next_followup_release
                                .map(|rt| next_next_tap_head.map(|head| rt < head).unwrap_or(true))
                                .unwrap_or(false))
                            .then_some(next_followup_pt)
                        });
                    let next_has_chain = nex_note_own_str_fol
                        .zip(next_next_tap_head)
                        .map(|(next_followup_pt, next_next_head)| {
                            let next2_win_start = next_next_head - w.hit50;
                            let next2_win_end = next_next_head + w.hit100;
                            presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|cand| **cand < next2_win_end)
                                .find(|cand| {
                                    **cand > next_followup_pt
                                        && **cand >= next2_win_start
                                        && **cand < next_next_head
                                        && next3_tap_head.map(|head| **cand < head).unwrap_or(true)
                                        && !reserved_ln_repr.contains(cand)
                                })
                                .map(|cand| {
                                    let next_next_pt = *cand;
                                    matches!(
                                        calc_hit_kind((next_next_pt - next_next_head).abs(), w,),
                                        JudgmentKind::Max | JudgmentKind::Hit300
                                    ) && events
                                        .iter()
                                        .find(|ev| ev.time > next_next_pt && !ev.pressed)
                                        .map(|ev| {
                                            next3_tap_head
                                                .map(|head| ev.time < head)
                                                .unwrap_or(true)
                                        })
                                        .unwrap_or(false)
                                })
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    cand_pt < ho.time
                        && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                        && nex_note_own_str_fol.is_some()
                        && (next_next_tap_head
                            .map(|next_next_head| {
                                next_next_head - next_head_time > w.hit50 + w.hit300
                            })
                            .unwrap_or(true)
                            || next_has_chain)
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
        let pos_pre_head_to_prhd = true
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
                    calc_hit_kind((prev_pt - prev_t).abs(), w) == JudgmentKind::Max
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
                    let next3_tap_head =
                        col_notes
                            .get(note_pos + 3)
                            .and_then(|(_, next_next_next_ho)| {
                                (!next_next_next_ho.is_long_note())
                                    .then_some(next_next_next_ho.time)
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
                            let next_pt = *next_pt;
                            let next_kind = calc_hit_kind((next_pt - next_head_time).abs(), w);
                            let next_release = events
                                .iter()
                                .find(|ev| ev.time > next_pt && !ev.pressed)
                                .map(|ev| ev.time);
                            matches!(next_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                                && next_release
                                    .zip(next_next_tap_head)
                                    .map(|(rel_time, next_next_head)| rel_time < next_next_head)
                                    .unwrap_or(true)
                        })
                        .unwrap_or(false);
                    let next2_tap_strong = next_next_tap_head
                        .map(|next_next_head| {
                            let next2_win_start = next_next_head - w.hit50;
                            let next2_win_end = next_next_head + w.hit100;
                            presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|next_pt| **next_pt < next2_win_end)
                                .find(|next_pt| {
                                    **next_pt >= next2_win_start
                                        && next3_tap_head
                                            .map(|next3_head| **next_pt < next3_head)
                                            .unwrap_or(true)
                                        && !reserved_ln_repr.contains(next_pt)
                                })
                                .map(|next_pt| {
                                    let next_pt = *next_pt;
                                    let next_kind =
                                        calc_hit_kind((next_pt - next_next_head).abs(), w);
                                    let next_release = events
                                        .iter()
                                        .find(|ev| ev.time > next_pt && !ev.pressed)
                                        .map(|ev| ev.time);
                                    matches!(next_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                                        && next_release
                                            .zip(next3_tap_head)
                                            .map(|(rel_time, next3_head)| rel_time < next3_head)
                                            .unwrap_or(true)
                                })
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    cand_pt < ho.time
                        && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                        && next_note_str_fol
                        && (next_next_tap_head
                            .map(|next_next_head| {
                                next_next_head - next_head_time > w.hit50 + w.hit300
                            })
                            .unwrap_or(true)
                            || next_head_time - ho.time > w.hit50 * 4
                            || next2_tap_strong)
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
        let post_prev_head_pref = true
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
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_head_time, (_, next_ho))| {
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
                    let next_has_prewin = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_head_time)
                        .any(|cand| {
                            let next_pt = *cand;
                            next_pt >= next_window_start - early_penalty_window - 1
                                && next_pt < next_window_start
                                && !reserved_ln_repr.contains(cand)
                                && calc_hit_kind((next_pt - next_head_time).abs(), w)
                                    == JudgmentKind::Miss
                                && events
                                    .iter()
                                    .find(|ev| ev.time > next_pt && !ev.pressed)
                                    .map(|ev| ev.time < next_head_time)
                                    .unwrap_or(false)
                        });
                    next_head_time - ho.time > w.hit50 + w.hit300
                        && cand_pt >= ho.time
                        && cand_pt < next_head_time
                        && cand_kind == JudgmentKind::Max
                        && cand_pre_next
                        && next_has_prewin
                })
                .unwrap_or(false);
        let post_prev_head_chain = true
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
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_head_time, (_, next_ho))| {
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
                    let cand_next_miss = cand_pt >= next_window_start - early_penalty_window - 1
                        && cand_pt < next_window_start
                        && calc_hit_kind((cand_pt - next_head_time).abs(), w) == JudgmentKind::Miss;
                    let no_ext_pt_pre_nex_hea = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_head_time)
                        .next()
                        .is_none();
                    let nex_not_exa_head_max = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_win_end)
                        .find(|cand| {
                            let next_pt = **cand;
                            next_pt == next_head_time
                                && !reserved_ln_repr.contains(cand)
                                && calc_hit_kind((next_pt - next_head_time).abs(), w)
                                    == JudgmentKind::Max
                        })
                        .copied();
                    let next_note_pre_head = nex_not_exa_head_max
                        .zip(next_next_tap_head)
                        .map(|(next_pt, next_next_head)| {
                            events
                                .iter()
                                .find(|ev| ev.time > next_pt && !ev.pressed)
                                .map(|ev| ev.time < next_next_head)
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    let next_exact_is_nnext = nex_not_exa_head_max
                        .zip(next_next_tap_head)
                        .map(|(next_pt, next_next_head)| {
                            let next2_win_start = next_next_head - w.hit50;
                            next_next_head - next_head_time <= w.hit50 + w.hit300
                                && next_pt >= next2_win_start - early_penalty_window - 1
                                && next_pt < next2_win_start
                                && calc_hit_kind((next_pt - next_next_head).abs(), w)
                                    == JudgmentKind::Miss
                        })
                        .unwrap_or(false);
                    let nnext_has_max_cand = next_next_tap_head
                        .map(|next_next_head| {
                            let next2_win_end = next_next_head + w.hit100;
                            presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|cand| **cand < next2_win_end)
                                .any(|cand| {
                                    let next_next_pt = *cand;
                                    next_next_pt >= next_next_head - w.max
                                        && next_next_pt <= next_next_head + w.max
                                        && !reserved_ln_repr.contains(cand)
                                        && calc_hit_kind((next_next_pt - next_next_head).abs(), w)
                                            == JudgmentKind::Max
                                })
                        })
                        .unwrap_or(false);
                    next_head_time - ho.time > w.hit50
                        && next_head_time - ho.time <= w.hit50 + w.hit300
                        && cand_pt >= ho.time
                        && cand_pt < next_head_time
                        && cand_kind == JudgmentKind::Max
                        && cand_pre_next
                        && cand_next_miss
                        && no_ext_pt_pre_nex_hea
                        && next_note_pre_head
                        && next_exact_is_nnext
                        && nnext_has_max_cand
                })
                .unwrap_or(false);
        let pre_earl_to_post_h50 = true
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
        let pre_ear_to_post_chai = true
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
                    let next3_tap_head =
                        col_notes
                            .get(note_pos + 3)
                            .and_then(|(_, next_next_next_ho)| {
                                (!next_next_next_ho.is_long_note())
                                    .then_some(next_next_next_ho.time)
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
                    let post_cur_starts_dense = next_next_tap_head
                        .map(|next_next_head| {
                            let prewin_starts_dense = prev_col_pt
                                .zip(prev_note_time)
                                .map(|(prev_pt, prev_t)| {
                                    let prev_fragment_gap = prev_t - pt;
                                    let shl_pre_h50_bou_nois =
                                        calc_hit_kind((prev_pt - prev_t).abs(), w)
                                            == JudgmentKind::Hit50
                                            && prev_pt < prev_t
                                            && prev_fragment_gap > 0
                                            && prev_fragment_gap <= w.max / 2
                                            && prewindow_overflow < early_penalty_window - 1;
                                    prev_fragment_gap > w.max / 2 || shl_pre_h50_bou_nois
                                })
                                .unwrap_or(false);
                            let dense_current_to_next = next_head_time - ho.time <= w.hit50 * 2;
                            let dense_next_to_next2 =
                                next_next_head - next_head_time <= w.hit50 * 2;
                            let next_chain_follow = presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|cand| **cand < next_next_head + w.hit100)
                                .any(|cand| {
                                    let followup_pt = *cand;
                                    followup_pt >= next_next_head - w.hit50
                                        && !reserved_ln_repr.contains(cand)
                                        && calc_hit_kind((followup_pt - next_next_head).abs(), w)
                                            != JudgmentKind::Miss
                                        && events
                                            .iter()
                                            .find(|ev| ev.time > followup_pt && !ev.pressed)
                                            .map(|ev| {
                                                next3_tap_head
                                                    .map(|head| ev.time < head)
                                                    .unwrap_or(true)
                                            })
                                            .unwrap_or(false)
                                });
                            prewin_starts_dense
                                && dense_current_to_next
                                && dense_next_to_next2
                                && cand_pt >= next_head_time - w.hit50
                                && cand_pt < next_head_time
                                && next_chain_follow
                        })
                        .unwrap_or(false);
                    cand_pt >= ho.time
                        && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                        && next_kind == JudgmentKind::Hit50
                        && next2_has_cand
                        && !post_cur_starts_dense
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
        let prev_h100_dense_falls = deep_tap
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
                    calc_hit_kind((prev_pt - prev_t).abs(), w) == JudgmentKind::Hit100
                        && pt > prev_pt
                        && pt < prev_t
                        && prev_t - pt > 0
                        && prev_t - pt <= w.max / 2
                })
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
                    let next3_tap_head =
                        col_notes
                            .get(note_pos + 3)
                            .and_then(|(_, next_next_next_ho)| {
                                (!next_next_next_ho.is_long_note())
                                    .then_some(next_next_next_ho.time)
                            });
                    let next_note_strong_pre = next_next_tap_head
                        .and_then(|next_next_head| {
                            let next_gap = next_head_time - ho.time;
                            let next_next_gap = next_next_head - next_head_time;
                            ((next_next_gap - next_gap).abs() <= w.max * 2)
                                .then_some(next_next_head)
                        })
                        .and_then(|next_next_head| {
                            presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|cand| **cand < next_win_end)
                                .find(|cand| {
                                    **cand >= next_window_start
                                        && **cand < next_head_time
                                        && **cand < next_next_head
                                        && !reserved_ln_repr.contains(cand)
                                })
                                .and_then(|cand| {
                                    let next_pt = *cand;
                                    let next_release = events
                                        .iter()
                                        .find(|ev| ev.time > next_pt && !ev.pressed)
                                        .map(|ev| ev.time);
                                    (matches!(
                                        calc_hit_kind((next_pt - next_head_time).abs(), w),
                                        JudgmentKind::Max | JudgmentKind::Hit300
                                    ) && next_release
                                        .map(|rt| rt < next_next_head)
                                        .unwrap_or(false))
                                    .then_some(next_pt)
                                })
                        });
                    let next2_strong_pre = next_next_tap_head
                        .map(|next_next_head| {
                            let next_gap = next_head_time - ho.time;
                            let next_next_gap = next_next_head - next_head_time;
                            let next2_win_start = next_next_head - w.hit50;
                            let next2_win_end = next_next_head + w.hit100;
                            (next_next_gap - next_gap).abs() <= w.max * 2
                                    && presses
                                        .iter()
                                        .skip(press_idx + 1)
                                        .take_while(|cand| **cand < next2_win_end)
                                        .find(|cand| {
                                            **cand >= next2_win_start
                                                && **cand < next_next_head
                                                && next3_tap_head
                                                    .map(|head| **cand < head)
                                                    .unwrap_or(true)
                                                && !reserved_ln_repr.contains(cand)
                                        })
                                        .map(|cand| {
                                            let next_next_pt = *cand;
                                            matches!(
                                                calc_hit_kind(
                                                    (next_next_pt - next_next_head).abs(),
                                                    w,
                                                ),
                                                JudgmentKind::Max | JudgmentKind::Hit300
                                            ) && events
                                                .iter()
                                                .find(|ev| ev.time > next_next_pt && !ev.pressed)
                                                .map(|ev| {
                                                    next3_tap_head
                                                        .map(|head| ev.time < head)
                                                        .unwrap_or(true)
                                                })
                                                .unwrap_or(false)
                                        })
                                        .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    cand_pt >= ho.time - w.max
                        && cand_pt < ho.time
                        && cand_kind == JudgmentKind::Max
                        && next_note_strong_pre.is_some()
                        && next2_strong_pre
                        && candidate_release
                            .zip(next_note_strong_pre)
                            .map(|(rel_time, next_followup_pt)| {
                                rel_time > ho.time && rel_time < next_followup_pt
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let prev_early_to_iso = true
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
                    let pre_pt_is_lat_h10_ban = prev_pt < prev_t
                        && prev_t - prev_pt > w.hit300
                        && prev_t - prev_pt <= w.hit100
                        && pt > prev_pt
                        && pt < prev_t
                        && prev_t - pt > w.max;
                    let prev_press_h50_noise = calc_hit_kind((prev_pt - prev_t).abs(), w)
                        == JudgmentKind::Hit50
                        && prev_pt < prev_t
                        && pt > prev_pt
                        && pt < prev_t
                        && prev_t - pt >= 0
                        && prev_t - pt <= w.max / 2;
                    let prev_press_post_frag = prev_pt < prev_t
                        && (prev_t - prev_pt).abs() <= w.hit300
                        && pt > prev_t + w.max
                        && pt <= prev_t + w.hit100
                        && has_in_win_cand
                        && presses[press_idx] < ho.time
                        && early_rel_before_note
                        && prewindow_overflow == early_penalty_window - 1;
                    pre_pt_is_lat_h10_ban || prev_press_h50_noise || prev_press_post_frag
                })
                .unwrap_or(false)
            && prev_note_time
                .map(|prev_t| ho.time - prev_t > w.hit50)
                .unwrap_or(true)
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
                cand_pt >= ho.time - w.max
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
        let prev_early_to_cur_ln = true
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
                    let pre_pt_is_lat_h10_ban = prev_pt < prev_t
                        && prev_t - prev_pt > w.hit300
                        && prev_t - prev_pt <= w.hit100
                        && pt > prev_pt
                        && pt < prev_t
                        && prev_t - pt > w.max;
                    let prev_press_h50_noise = calc_hit_kind((prev_pt - prev_t).abs(), w)
                        == JudgmentKind::Hit50
                        && prev_pt < prev_t
                        && pt > prev_pt
                        && pt < prev_t
                        && prev_t - pt >= 0
                        && prev_t - pt <= w.max / 2;
                    pre_pt_is_lat_h10_ban || prev_press_h50_noise
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
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let candidate_release = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let next_window_start = next_head_time - w.hit50;
                    let next_next_note_time = col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                    let next_ln_late_end = next_next_note_time
                        .map(|next_time| next_time <= next_head_time + w.hit50)
                        .unwrap_or(false);
                    let next_lock_end =
                        next_head_time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                    let next_end_time = next_ho.end_time.unwrap_or(next_head_time);
                    let next_tail_start = next_end_time - w.hit50;
                    let next_tail_end = next_end_time + w.hit100;
                    let nex_ln_self_cont_fol = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|next_pt| **next_pt < next_lock_end)
                        .find(|next_pt| {
                            **next_pt >= next_window_start && !reserved_ln_repr.contains(next_pt)
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
                                        rel_time >= next_tail_start && rel_time < next_tail_end
                                    })
                                    .unwrap_or(false))
                            .then_some(next_followup_pt)
                        });
                    cand_pt >= ho.time
                        && cand_pt < next_head_time
                        && cand_kind == JudgmentKind::Max
                        && next_head_time - ho.time <= w.hit50 + w.hit300
                        && candidate_release
                            .map(|rel_time| rel_time > ho.time && rel_time < next_head_time)
                            .unwrap_or(false)
                        && nex_ln_self_cont_fol.is_some()
                })
                .unwrap_or(false);
        let prev_gap_early_pen = true
            && !ho.is_long_note()
            && (!prev_was_miss || prewindow_overflow <= early_penalty_window)
            && !prev_pen_to_iso
            && !prev_pen_to_post
            && !prev_pen_to_prehead
            && !pos_pre_head_to_prhd
            && !post_prev_head_pref
            && !prev_early_to_iso
            && !prev_early_to_cur_ln
            && !prev_h100_dense_falls
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
            !true && !ho.is_long_note() && w.hit300 <= 38 && prewindow_overflow == w.hit300 - 1;
        let exact_prev_h50_keep = !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && !has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && matches!((ho.time - pt).abs(), 160 | 161)
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    calc_hit_kind((prev_pt - prev_t).abs(), w) == JudgmentKind::Hit50
                        && pt > prev_t
                        && pt <= prev_t + w.hit100
                        && ho.time - prev_t <= w.hit50 * 2
                })
                .unwrap_or(false)
            && next_note_time
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_t, (_, next_ho))| {
                    if next_ho.is_long_note() || press_idx >= presses.len() {
                        return false;
                    }
                    let next_pt = presses[press_idx];
                    let next_window_start = next_t - w.hit50;
                    let next_win_end = next_t + w.hit100;
                    let next_kind = calc_hit_kind((next_pt - next_t).abs(), w);
                    next_pt >= next_window_start
                        && next_pt < next_win_end
                        && !reserved_ln_repr.contains(&next_pt)
                        && matches!(
                            next_kind,
                            JudgmentKind::Max
                                | JudgmentKind::Hit300
                                | JudgmentKind::Hit200
                                | JudgmentKind::Hit100
                        )
                        && early_press_rel_time
                            .map(|rt| {
                                rt < next_pt
                                    && (rt <= window_start + w.max / 2
                                        || (rt < ho.time && rt < next_window_start))
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let exact_prev_head_pen = (!ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && early_press_rel_time
                .map(|rt| rt < presses[press_idx])
                .unwrap_or(false)
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && matches!((ho.time - pt).abs(), 160 | 161)
            && prev_note_time.map(|prev_t| pt > prev_t).unwrap_or(false))
            || exact_prev_h50_keep;
        let far_tap_pen_base = true
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
                                    JudgmentKind::Max
                                        | JudgmentKind::Hit300
                                        | JudgmentKind::Hit200
                                        | JudgmentKind::Hit100
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
                                && matches!(
                                    next_kind,
                                    JudgmentKind::Max | JudgmentKind::Hit300 | JudgmentKind::Hit200
                                )
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
                    matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
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
        let far_pen_yield_exact = far_tap_pen_base
            && prewindow_overflow >= early_penalty_window
            && next_note_time
                .map(|next_head_time| {
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let next_window_start = next_head_time - w.hit50;
                    let cand_rel_pre_next_win = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time > ho.time && ev.time < next_window_start)
                        .unwrap_or(false);
                    cand_pt < ho.time && cand_kind == JudgmentKind::Max && cand_rel_pre_next_win
                })
                .unwrap_or(false);
        let far_pen_pref_next_ln = far_tap_pen_base
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if !next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let candidate_release = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let next_window_start = next_head_time - w.hit50;
                    let next_next_note_time = col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                    let next_ln_late_end = next_next_note_time
                        .map(|next_time| next_time <= next_head_time + w.hit50)
                        .unwrap_or(false);
                    let next_lock_end =
                        next_head_time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                    let next_end_time = next_ho.end_time.unwrap_or(next_head_time);
                    let next_duration = next_end_time - next_head_time;
                    let next_tail_start = next_end_time - w.hit50;
                    let next_tail_end = next_end_time + w.hit100;
                    let nex_ln_self_cont_fol = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|next_pt| **next_pt < next_lock_end)
                        .find(|next_pt| {
                            **next_pt >= next_window_start && !reserved_ln_repr.contains(next_pt)
                        })
                        .and_then(|next_pt| {
                            let next_followup_pt = *next_pt;
                            let next_kind =
                                calc_hit_kind((next_followup_pt - next_head_time).abs(), w);
                            let next_release = events
                                .iter()
                                .find(|ev| ev.time > next_followup_pt && !ev.pressed)
                                .map(|ev| ev.time);
                            (matches!(next_kind, JudgmentKind::Hit200 | JudgmentKind::Max)
                                && next_release
                                    .map(|rel_time| {
                                        rel_time >= next_tail_start && rel_time < next_tail_end
                                    })
                                    .unwrap_or(false))
                            .then_some((next_followup_pt, next_kind))
                        });
                    cand_pt < ho.time
                        && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                        && next_head_time - ho.time <= w.hit50 * 3
                        && next_duration > w.hit50
                        && candidate_release
                            .zip(nex_ln_self_cont_fol)
                            .map(|(rel_time, (next_followup_pt, next_kind))| {
                                (next_kind == JudgmentKind::Hit200
                                    && rel_time > ho.time
                                    && rel_time < next_window_start)
                                    || (cand_kind == JudgmentKind::Max
                                        && next_kind == JudgmentKind::Max
                                        && next_followup_pt == next_head_time
                                        && rel_time > ho.time
                                        && rel_time < next_followup_pt)
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let far_pen_next_chain = true
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && early_press_rel_time
                .map(|rt| rt <= ho.time - w.hit300)
                .unwrap_or(false)
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    calc_hit_kind((prev_pt - prev_t).abs(), w) == JudgmentKind::Max
                        && ho.time - prev_t > w.hit50 * 2
                        && pt > prev_t + w.hit100
                })
                .unwrap_or(false)
            && next_note_time
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_head_time, (_, next_ho))| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let cur_rel_pre_nex_head = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time > ho.time && ev.time < next_head_time)
                        .unwrap_or(false);
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let nex_not_exa_head_max = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_win_end)
                        .find(|cand| {
                            let next_pt = **cand;
                            next_pt == next_head_time
                                && !reserved_ln_repr.contains(cand)
                                && calc_hit_kind((next_pt - next_head_time).abs(), w)
                                    == JudgmentKind::Max
                        })
                        .copied();
                    let no_intrvn_prhd_pt = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_head_time)
                        .next()
                        .is_none();
                    let next_note_pre_head = nex_not_exa_head_max
                        .zip(next_next_tap_head)
                        .map(|(next_pt, next_next_head)| {
                            events
                                .iter()
                                .find(|ev| ev.time > next_pt && !ev.pressed)
                                .map(|ev| ev.time < next_next_head)
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    let nex_note_has_own_max = next_next_tap_head
                        .map(|next_next_head| {
                            let next2_win_end = next_next_head + w.hit100;
                            presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|cand| **cand < next2_win_end)
                                .any(|cand| {
                                    let next_next_pt = *cand;
                                    next_next_pt >= next_next_head - w.max
                                        && next_next_pt <= next_next_head + w.max
                                        && !reserved_ln_repr.contains(cand)
                                        && calc_hit_kind((next_next_pt - next_next_head).abs(), w)
                                            == JudgmentKind::Max
                                })
                        })
                        .unwrap_or(false);
                    next_head_time - ho.time > w.hit50 + w.hit300
                        && cand_pt >= ho.time
                        && cand_pt < next_head_time
                        && cand_kind == JudgmentKind::Hit300
                        && cur_rel_pre_nex_head
                        && no_intrvn_prhd_pt
                        && nex_not_exa_head_max.is_some()
                        && next_note_pre_head
                        && nex_note_has_own_max
                })
                .unwrap_or(false);
        let far_exact_next_chain = true
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && early_press_rel_time
                .map(|rt| rt < presses[press_idx] && rt <= ho.time - w.hit300)
                .unwrap_or(false)
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prewindow_overflow == early_penalty_window - 1
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    calc_hit_kind((prev_pt - prev_t).abs(), w) == JudgmentKind::Max
                        && ho.time - prev_t > w.hit50 * 2
                        && pt > prev_t + w.hit100
                })
                .unwrap_or(false)
            && next_note_time
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_head_time, (_, next_ho))| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let current_release = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let next_window_start = next_head_time - w.hit50;
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let next3_tap_head =
                        col_notes
                            .get(note_pos + 3)
                            .and_then(|(_, next_next_next_ho)| {
                                (!next_next_next_ho.is_long_note())
                                    .then_some(next_next_next_ho.time)
                            });
                    let next_strong_prehead = next_next_tap_head.and_then(|next_next_head| {
                        presses
                            .iter()
                            .skip(press_idx + 1)
                            .take_while(|cand| **cand < next_win_end)
                            .find(|cand| {
                                let next_pt = **cand;
                                next_pt >= next_window_start
                                    && next_pt < next_head_time
                                    && next_pt < next_next_head
                                    && !reserved_ln_repr.contains(cand)
                                    && calc_hit_kind((next_pt - next_head_time).abs(), w)
                                        == JudgmentKind::Max
                            })
                            .and_then(|cand| {
                                let next_pt = *cand;
                                let next_release = events
                                    .iter()
                                    .find(|ev| ev.time > next_pt && !ev.pressed)
                                    .map(|ev| ev.time);
                                next_release
                                    .map(|rt| rt < next_next_head)
                                    .unwrap_or(false)
                                    .then_some(next_pt)
                            })
                    });
                    let next2_note_strong = next_next_tap_head
                        .map(|next_next_head| {
                            let next2_win_start = next_next_head - w.hit50;
                            let next2_win_end = next_next_head + w.hit100;
                            presses
                                    .iter()
                                    .skip(press_idx + 1)
                                    .take_while(|cand| **cand < next2_win_end)
                                    .find(|cand| {
                                        let next_next_pt = **cand;
                                        next_next_pt > next_head_time
                                            && !reserved_ln_repr.contains(cand)
                                    })
                                    .map(|cand| {
                                        let next_next_pt = *cand;
                                        next_next_pt > next_head_time
                                            && next_next_pt >= next2_win_start
                                            && matches!(
                                                calc_hit_kind(
                                                    (next_next_pt - next_next_head).abs(),
                                                    w,
                                                ),
                                                JudgmentKind::Max | JudgmentKind::Hit300
                                            )
                                            && events
                                                .iter()
                                                .find(|ev| ev.time > next_next_pt && !ev.pressed)
                                                .map(|ev| {
                                                    next3_tap_head
                                                        .map(|head| ev.time < head)
                                                        .unwrap_or(true)
                                                })
                                                .unwrap_or(false)
                                    })
                                    .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    next_head_time - ho.time > w.hit50 + w.hit300
                        && next_head_time - ho.time <= w.hit50 * 3
                        && cand_pt >= ho.time
                        && cand_pt < next_head_time
                        && cand_kind == JudgmentKind::Hit300
                        && current_release
                            .zip(next_strong_prehead)
                            .map(|(rel_time, next_pt)| {
                                rel_time > ho.time
                                    && rel_time <= ho.time + w.hit100
                                    && rel_time < next_pt
                            })
                            .unwrap_or(false)
                        && next_strong_prehead.is_some()
                        && next2_note_strong
                })
                .unwrap_or(false);
        let far_pen_h300_chain = true
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && early_press_rel_time
                .map(|rt| rt <= ho.time - w.hit300)
                .unwrap_or(false)
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    calc_hit_kind((prev_pt - prev_t).abs(), w) == JudgmentKind::Max
                        && ho.time - prev_t > w.hit50 * 2
                        && pt > prev_t + w.hit100
                })
                .unwrap_or(false)
            && next_note_time
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_head_time, (_, next_ho))| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let cur_rel_pre_nex_head = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time > ho.time && ev.time < next_head_time)
                        .unwrap_or(false);
                    let next_window_start = next_head_time - w.hit50;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let next_head_boun_claim = cand_pt
                        >= next_window_start - early_penalty_window - 1
                        && cand_pt < next_window_start
                        && calc_hit_kind((cand_pt - next_head_time).abs(), w) == JudgmentKind::Miss;
                    let next_note_post_h30 = next_next_tap_head.and_then(|next_next_head| {
                        presses
                            .iter()
                            .skip(press_idx + 1)
                            .take_while(|cand| **cand < next_next_head)
                            .find(|cand| {
                                let next_pt = **cand;
                                next_pt >= next_head_time
                                    && next_pt < next_next_head
                                    && !reserved_ln_repr.contains(cand)
                                    && calc_hit_kind((next_pt - next_head_time).abs(), w)
                                        == JudgmentKind::Hit300
                            })
                            .copied()
                    });
                    let next_note_h30_pre = next_note_post_h30
                        .zip(next_next_tap_head)
                        .map(|(next_pt, next_next_head)| {
                            events
                                .iter()
                                .find(|ev| ev.time > next_pt && !ev.pressed)
                                .map(|ev| ev.time < next_next_head)
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    let next_note_h30_h50 = next_note_post_h30
                        .zip(next_next_tap_head)
                        .map(|(next_pt, next_next_head)| {
                            calc_hit_kind((next_pt - next_next_head).abs(), w)
                                == JudgmentKind::Hit50
                        })
                        .unwrap_or(false);
                    let nnext_has_max_cand = next_next_tap_head
                        .map(|next_next_head| {
                            let next2_win_end = next_next_head + w.hit100;
                            presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|cand| **cand < next2_win_end)
                                .any(|cand| {
                                    let next_next_pt = *cand;
                                    next_next_pt >= next_next_head - w.max
                                        && next_next_pt <= next_next_head + w.max
                                        && !reserved_ln_repr.contains(cand)
                                        && calc_hit_kind((next_next_pt - next_next_head).abs(), w)
                                            == JudgmentKind::Max
                                })
                        })
                        .unwrap_or(false);
                    next_head_time - ho.time > w.hit50
                        && next_head_time - ho.time <= w.hit50 + w.hit300
                        && cand_pt >= ho.time
                        && cand_pt < next_head_time
                        && cand_kind == JudgmentKind::Max
                        && cur_rel_pre_nex_head
                        && next_head_boun_claim
                        && next_note_h30_pre
                        && next_note_h30_h50
                        && nnext_has_max_cand
                })
                .unwrap_or(false);
        let far_tap_pen_keep = far_tap_pen_base
            && !far_pen_to_exact
            && !far_pen_to_prehead
            && !far_pen_to_post
            && !far_pen_to_iso
            && !far_pen_yield_exact
            && !far_pen_pref_next_ln;
        let prev_pen_h300_chain = !ho.is_long_note()
            && prev_was_miss
            && prev_had_prewin_pen
            && has_in_win_cand
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prev_note_time
                .map(|prev_t| pt == prev_t + w.hit300 && ho.time - prev_t > w.hit50 + w.hit300)
                .unwrap_or(false)
            && {
                let cand_pt = presses[press_idx];
                let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                    && cand_pt >= ho.time
                    && events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| {
                            ev.time > ho.time
                                && ev.time <= ho.time + w.hit100
                                && next_note_time
                                    .map(|next_t| ev.time < next_t)
                                    .unwrap_or(true)
                        })
                        .unwrap_or(false)
            }
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if next_ho.is_long_note() || next_head_time - ho.time <= w.hit50 + w.hit300 {
                        return false;
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
                            let follow_pt = *cand;
                            follow_pt >= next_window_start
                                && next_next_tap_head
                                    .map(|head| follow_pt < head)
                                    .unwrap_or(true)
                                && !reserved_ln_repr.contains(cand)
                                && matches!(
                                    calc_hit_kind((follow_pt - next_head_time).abs(), w),
                                    JudgmentKind::Max | JudgmentKind::Hit300
                                )
                        })
                })
                .unwrap_or(false);
        let exact_prev_pen_chain = !ho.is_long_note()
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
        let prev_pen_keep_chain = !ho.is_long_note()
            && !prev_note_is_ln
            && prev_was_miss
            && prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && early_press_rel_time
                .map(|rt| rt < presses[press_idx])
                .unwrap_or(false)
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prewindow_overflow == early_penalty_window - 1
            && prev_note_time
                .map(|prev_t| {
                    pt < prev_t
                        && prev_t - pt >= w.max - 1
                        && prev_t - pt <= w.max
                        && (147..=148).contains(&(ho.time - prev_t))
                })
                .unwrap_or(false)
            && next_note_time
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_head_time, (_, next_ho))| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let current_release = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let next_window_start = next_head_time - w.hit50;
                    let next_win_end = next_head_time + w.hit100;
                    let next_has_strong_cand = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_win_end)
                        .any(|cand| {
                            let next_pt = *cand;
                            next_pt >= next_window_start
                                && !reserved_ln_repr.contains(cand)
                                && matches!(
                                    calc_hit_kind((next_pt - next_head_time).abs(), w),
                                    JudgmentKind::Max | JudgmentKind::Hit300
                                )
                        });
                    cand_pt < ho.time
                        && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                        && current_release
                            .map(|rt| {
                                rt > ho.time && rt <= ho.time + w.hit100 && rt < next_head_time
                            })
                            .unwrap_or(false)
                        && next_has_strong_cand
                })
                .unwrap_or(false);
        let prssls_prev_keep_pen = !ho.is_long_note()
            && !prev_note_is_ln
            && prev_was_miss
            && !prev_had_prewin_pen
            && prev_col_pt.is_none()
            && has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prewindow_overflow == early_penalty_window - 1
            && prev_note_time
                .map(|prev_t| pt > prev_t && pt < ho.time && ho.time - prev_t > w.hit50 + w.hit300)
                .unwrap_or(false)
            && prev_note_time
                .map(|prev_t| {
                    events
                        .iter()
                        .rev()
                        .find(|ev| ev.pressed && ev.time < prev_t)
                        .map(|ev| ev.time)
                        .zip(
                            events
                                .iter()
                                .find(|ev| ev.time > pt && !ev.pressed)
                                .map(|ev| ev.time),
                        )
                        .map(|(prev_boundary_pt, cur_frag_rel)| {
                            calc_hit_kind((prev_boundary_pt - prev_t).abs(), w)
                                == JudgmentKind::Miss
                                && cur_frag_rel < ho.time
                        })
                        .unwrap_or(false)
                })
                .unwrap_or(false)
            && next_note_time
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_head_time, (_, next_ho))| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let current_release = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let next_window_start = next_head_time - w.hit50;
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let next_has_strong_cand = next_next_tap_head
                        .map(|next_next_head| {
                            presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|cand| **cand < next_win_end)
                                .find(|cand| {
                                    let next_pt = **cand;
                                    next_pt >= next_window_start
                                        && next_pt < next_next_head
                                        && !reserved_ln_repr.contains(cand)
                                })
                                .map(|cand| {
                                    let next_pt = *cand;
                                    matches!(
                                        calc_hit_kind((next_pt - next_head_time).abs(), w),
                                        JudgmentKind::Max | JudgmentKind::Hit300
                                    ) && events
                                        .iter()
                                        .find(|ev| ev.time > next_pt && !ev.pressed)
                                        .map(|ev| ev.time < next_next_head)
                                        .unwrap_or(false)
                                })
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    cand_pt < ho.time
                        && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                        && current_release
                            .map(|rt| rt > ho.time && rt < next_head_time)
                            .unwrap_or(false)
                        && next_has_strong_cand
                })
                .unwrap_or(false);
        let deep_tap = deep_tap
            && !prev_gap_early_pen
            && !strict_od_tap_keep
            && !exact_prev_head_pen
            && !prssls_prev_keep_pen
            && !post_prev_head_chain
            && !far_pen_next_chain
            && !far_pen_h300_chain
            && !far_tap_pen_keep;
        let deep_tap_chain = deep_tap_chain
            && !prev_gap_early_pen
            && !prev_pen_h300_chain
            && !exact_prev_pen_chain
            && !prev_pen_keep_chain;
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
                                    || (!true
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
                || (true && current_ln_duration <= w.hit100 && presses[press_idx] >= ho.time))
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
        let ln_pos_pre_shor_inwi = true
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
        let ln_pre_tai_pref_h100 = true
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
        let sho_ln_sta_post_head = !true
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
        let prev_break_to_next = true
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
        let pos_pre_bod_keep_pen = true
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
        let tap_prev_keeps_pair = true
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
        let deep_hold_keeps_prhd = !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && !has_in_win_cand
            && !early_rel_before_note
            && prewindow_overflow == early_penalty_window - 1
            && next_note_time
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_t, (_, next_ho))| {
                    if next_ho.is_long_note() || press_idx >= presses.len() {
                        return false;
                    }
                    let next_pt = presses[press_idx];
                    let next_window_start = next_t - w.hit50;
                    next_pt >= next_window_start
                        && next_pt < next_t
                        && !reserved_ln_repr.contains(&next_pt)
                        && matches!(
                            calc_hit_kind((next_pt - next_t).abs(), w),
                            JudgmentKind::Max | JudgmentKind::Hit300 | JudgmentKind::Hit200
                        )
                        && early_press_rel_time
                            .map(|rt| rt > ho.time && rt < next_t)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let dee_hol_prwn_no_cand = !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && !has_in_win_cand
            && !early_rel_before_note
            && prewindow_overflow >= early_penalty_window - 1
            && !deep_hold_keeps_prhd;
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
        let _tap_exact_no_cand = !ho.is_long_note()
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
        let post_prev_frag = !true
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
        let post_prev_frag_next = true
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && !has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prewindow_overflow >= early_penalty_window - 3
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
                    if next_ho.is_long_note() || press_idx >= presses.len() {
                        return false;
                    }
                    let next_pt = presses[press_idx];
                    let next_window_start = next_t - w.hit50;
                    let next_win_end = next_t + w.hit100;
                    let next_kind = calc_hit_kind((next_pt - next_t).abs(), w);
                    let nex_tap_is_nonm_prhd = next_pt >= next_window_start
                        && next_pt < next_t
                        && matches!(
                            next_kind,
                            JudgmentKind::Max
                                | JudgmentKind::Hit300
                                | JudgmentKind::Hit200
                                | JudgmentKind::Hit100
                        );
                    nex_tap_is_nonm_prhd
                        && next_pt < next_win_end
                        && !reserved_ln_repr.contains(&next_pt)
                        && early_press_rel_time
                            .map(|rt| {
                                rt <= window_start + w.max / 2
                                    || (rt < ho.time && rt < next_window_start)
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let post_h50_strong_pre = true
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prewindow_overflow >= early_penalty_window - 3
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    calc_hit_kind((prev_pt - prev_t).abs(), w) == JudgmentKind::Hit50
                        && pt > prev_t
                        && pt <= prev_t + w.hit100
                })
                .unwrap_or(false)
            && next_note_time
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_head_time, (_, next_ho))| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let next_window_start = next_head_time - w.hit50;
                    let can_rel_pre_nex_head = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time < next_head_time)
                        .unwrap_or(false);
                    let rel_sup_pos_hea_path = early_press_rel_time
                        .map(|rt| rt <= window_start + w.max / 2)
                        .unwrap_or(false);
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let next_win_end = next_head_time + w.hit100;
                    let next_note_strong_pre = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_head_time)
                        .find(|cand| {
                            let next_pt = **cand;
                            next_pt >= next_window_start
                                && !reserved_ln_repr.contains(cand)
                                && matches!(
                                    calc_hit_kind((next_pt - next_head_time).abs(), w),
                                    JudgmentKind::Max | JudgmentKind::Hit300 | JudgmentKind::Hit200
                                )
                        })
                        .copied();
                    let next_has_prehead = next_note_strong_pre.is_some();
                    let next_note_pre_max = next_note_strong_pre
                        .map(|next_pt| {
                            calc_hit_kind((next_pt - next_head_time).abs(), w) == JudgmentKind::Max
                        })
                        .unwrap_or(false);
                    let next_strong_h100 = next_note_strong_pre
                        .map(|next_pt| {
                            next_next_tap_head
                                .map(|next_next_head| next_pt < next_next_head - w.hit100)
                                .unwrap_or(true)
                        })
                        .unwrap_or(false);
                    let next_has_strong_cross = next_next_tap_head
                        .map(|next_next_head| {
                            presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|cand| **cand < next_win_end)
                                .any(|cand| {
                                    let next_pt = *cand;
                                    let next_release = events
                                        .iter()
                                        .find(|ev| ev.time > next_pt && !ev.pressed)
                                        .map(|ev| ev.time);
                                    next_pt >= next_head_time
                                        && matches!(
                                            calc_hit_kind((next_pt - next_head_time).abs(), w),
                                            JudgmentKind::Max | JudgmentKind::Hit300
                                        )
                                        && next_release
                                            .map(|rt| {
                                                rt >= next_next_head && rt <= next_next_head + w.max
                                            })
                                            .unwrap_or(false)
                                        && !reserved_ln_repr.contains(cand)
                                })
                        })
                        .unwrap_or(false);
                    let cur_can_pre_pen_miss = ((next_head_time - ho.time <= w.hit50
                        && cand_pt >= ho.time
                        && cand_pt < next_head_time
                        && matches!(
                            cand_kind,
                            JudgmentKind::Max | JudgmentKind::Hit300 | JudgmentKind::Hit200
                        )
                        && rel_sup_pos_hea_path)
                        || (next_head_time - ho.time > w.hit50 + w.hit300
                            && next_head_time - ho.time <= w.hit50 * 2
                            && cand_pt >= ho.time - w.max
                            && cand_pt < ho.time
                            && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300))
                        || (next_head_time - ho.time > w.hit50
                            && next_head_time - ho.time <= w.hit50 + w.hit300
                            && cand_pt >= ho.time
                            && cand_pt < next_head_time
                            && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                            && next_has_strong_cross)
                        || (next_head_time - ho.time > w.hit50 * 2
                            && cand_pt >= ho.time - w.hit300
                            && cand_pt < ho.time
                            && cand_kind == JudgmentKind::Hit300
                            && next_note_pre_max
                            && next_strong_h100)
                        || (next_head_time - ho.time > w.hit50 * 2
                            && cand_pt >= ho.time
                            && cand_pt < next_head_time
                            && cand_kind == JudgmentKind::Hit200
                            && next_has_prehead
                            && next_strong_h100))
                        && can_rel_pre_nex_head;
                    cur_can_pre_pen_miss && (next_has_prehead || next_has_strong_cross)
                })
                .unwrap_or(false);
        let post_h50_prehead_max = true
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prewindow_overflow >= early_penalty_window - 3
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    calc_hit_kind((prev_pt - prev_t).abs(), w) == JudgmentKind::Hit50
                        && pt > prev_t
                        && pt <= prev_t + w.hit100
                })
                .unwrap_or(false)
            && next_note_time
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_head_time, (_, next_ho))| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let next_window_start = next_head_time - w.hit50;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let cur_rel_pre_nex_head = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time < next_head_time)
                        .unwrap_or(false);
                    let early_rel_before_cur =
                        early_press_rel_time.map(|rt| rt < cand_pt).unwrap_or(false);
                    let next_note_pre_cand = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_head_time)
                        .find(|cand| {
                            let next_pt = **cand;
                            next_pt >= next_window_start
                                && !reserved_ln_repr.contains(cand)
                                && calc_hit_kind((next_pt - next_head_time).abs(), w)
                                    == JudgmentKind::Max
                        })
                        .copied();
                    next_head_time - ho.time > w.hit50 + w.hit300
                        && next_head_time - ho.time <= w.hit50 * 2
                        && cand_pt >= ho.time - w.max
                        && cand_pt < ho.time
                        && cand_kind == JudgmentKind::Max
                        && cand_pt < next_head_time - w.hit100
                        && cur_rel_pre_nex_head
                        && early_rel_before_cur
                        && next_note_pre_cand
                            .map(|next_pt| {
                                next_pt > cand_pt
                                    && next_next_tap_head
                                        .map(|next_next_head| next_pt < next_next_head - w.hit100)
                                        .unwrap_or(true)
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let post_h300_cross_fol = true
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prewindow_overflow >= early_penalty_window - 1
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    calc_hit_kind((prev_pt - prev_t).abs(), w) == JudgmentKind::Hit300
                        && pt > prev_t + w.max
                        && pt <= prev_t + w.hit300
                })
                .unwrap_or(false)
            && next_note_time
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_head_time, (_, next_ho))| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let cur_rel_pre_nex_head = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time > ho.time && ev.time < next_head_time)
                        .unwrap_or(false);
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let next_has_h300_cross = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_win_end)
                        .any(|cand| {
                            let next_pt = *cand;
                            let next_release = events
                                .iter()
                                .find(|ev| ev.time > next_pt && !ev.pressed)
                                .map(|ev| ev.time);
                            next_next_tap_head
                                .zip(next_release)
                                .map(|(next_next_head, next_rel_time)| {
                                    next_pt >= next_head_time
                                        && calc_hit_kind((next_pt - next_head_time).abs(), w)
                                            == JudgmentKind::Hit300
                                        && next_rel_time >= next_next_head
                                        && next_rel_time <= next_next_head + w.max
                                        && !reserved_ln_repr.contains(cand)
                                })
                                .unwrap_or(false)
                        });
                    next_head_time - ho.time > w.hit50 + w.hit300
                        && next_head_time - ho.time <= w.hit50 * 2
                        && cand_pt >= ho.time
                        && cand_pt < next_head_time
                        && cand_kind == JudgmentKind::Hit300
                        && cur_rel_pre_nex_head
                        && next_has_h300_cross
                })
                .unwrap_or(false);
        let post_h300_dense_chain = true
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prewindow_overflow >= early_penalty_window - 1
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    calc_hit_kind((prev_pt - prev_t).abs(), w) == JudgmentKind::Hit300
                        && pt > prev_t + w.max
                        && pt <= prev_t + w.hit100
                })
                .unwrap_or(false)
            && next_note_time
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_head_time, (_, next_ho))| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let cur_rel_pre_nex_head = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time > ho.time && ev.time < next_head_time)
                        .unwrap_or(false);
                    let next_window_start = next_head_time - w.hit50;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let next3_tap_head =
                        col_notes
                            .get(note_pos + 3)
                            .and_then(|(_, next_next_next_ho)| {
                                (!next_next_next_ho.is_long_note())
                                    .then_some(next_next_next_ho.time)
                            });
                    let next_note_strong_pre = next_next_tap_head.and_then(|next_next_head| {
                        presses
                            .iter()
                            .skip(press_idx + 1)
                            .take_while(|cand| **cand < next_head_time)
                            .find(|cand| {
                                let next_pt = **cand;
                                next_pt >= next_window_start
                                    && next_pt < next_next_head
                                    && !reserved_ln_repr.contains(cand)
                                    && matches!(
                                        calc_hit_kind((next_pt - next_head_time).abs(), w),
                                        JudgmentKind::Max | JudgmentKind::Hit300
                                    )
                            })
                            .and_then(|cand| {
                                let next_pt = *cand;
                                let next_release = events
                                    .iter()
                                    .find(|ev| ev.time > next_pt && !ev.pressed)
                                    .map(|ev| ev.time);
                                next_release
                                    .map(|rt| rt <= next_next_head)
                                    .unwrap_or(false)
                                    .then_some(next_pt)
                            })
                    });
                    let next2_note_strong = next_next_tap_head
                        .map(|next_next_head| {
                            let next2_win_start = next_next_head - w.hit50;
                            let next2_win_end = next_next_head + w.hit100;
                            presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|cand| **cand < next2_win_end)
                                .find(|cand| {
                                    let next_next_pt = **cand;
                                    next_next_pt > next_head_time
                                        && next_next_pt >= next2_win_start
                                        && !reserved_ln_repr.contains(cand)
                                })
                                .map(|cand| {
                                    let next_next_pt = *cand;
                                    matches!(
                                        calc_hit_kind((next_next_pt - next_next_head).abs(), w),
                                        JudgmentKind::Max | JudgmentKind::Hit300
                                    ) && events
                                        .iter()
                                        .find(|ev| ev.time > next_next_pt && !ev.pressed)
                                        .map(|ev| {
                                            next3_tap_head
                                                .map(|head| ev.time < head)
                                                .unwrap_or(true)
                                        })
                                        .unwrap_or(false)
                                })
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    next_head_time - ho.time <= w.hit50
                        && cand_pt >= ho.time - w.hit300
                        && cand_pt < ho.time
                        && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                        && cur_rel_pre_nex_head
                        && next_note_strong_pre.is_some()
                        && next2_note_strong
                })
                .unwrap_or(false);
        let post_h100_dense_fol = true
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prewindow_overflow >= early_penalty_window - 1
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    calc_hit_kind((prev_pt - prev_t).abs(), w) == JudgmentKind::Hit100
                        && pt > prev_t + w.max
                })
                .unwrap_or(false)
            && next_note_time
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_head_time, (_, next_ho))| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let next_kind = calc_hit_kind((cand_pt - next_head_time).abs(), w);
                    let current_release = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let nex_note_has_own_fol = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_win_end)
                        .find(|cand| {
                            let next_pt = **cand;
                            next_pt >= next_head_time
                                && next_next_tap_head
                                    .map(|next_next_head| next_pt < next_next_head)
                                    .unwrap_or(false)
                                && !reserved_ln_repr.contains(cand)
                        })
                        .map(|cand| {
                            let next_pt = *cand;
                            let next_followup_kind =
                                calc_hit_kind((next_pt - next_head_time).abs(), w);
                            let next_followup_release = events
                                .iter()
                                .find(|ev| ev.time > next_pt && !ev.pressed)
                                .map(|ev| ev.time);
                            matches!(
                                next_followup_kind,
                                JudgmentKind::Hit300 | JudgmentKind::Hit200 | JudgmentKind::Hit100
                            ) && next_followup_release
                                .zip(next_next_tap_head)
                                .map(|(rel_time, next_next_head)| rel_time < next_next_head)
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    next_head_time - ho.time <= w.hit50
                        && cand_pt >= ho.time
                        && cand_pt < next_head_time
                        && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                        && next_kind == JudgmentKind::Hit100
                        && current_release
                            .zip(presses.get(press_idx + 1).copied())
                            .map(|(rel_time, next_followup_pt)| {
                                rel_time > next_head_time && rel_time < next_followup_pt
                            })
                            .unwrap_or(false)
                        && nex_note_has_own_fol
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
            && !(true
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
        let prev_cross_h200 = !true
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
        let prev_near_h200 = !true
            && presses[press_idx] >= ho.time - w.hit200
            && presses[press_idx] < ho.time - w.hit300
            && calc_hit_kind((presses[press_idx] - ho.time).abs(), w) == JudgmentKind::Hit200
            && prewindow_overflow < early_penalty_window - 1
            && early_press_rel_time
                .zip(prev_note_time)
                .map(|(rt, prev_t)| {
                    rt <= prev_t
                        && prev_t - rt <= w.max
                        && next_note_time
                            .map(|next_t| next_t - ho.time > w.hit50 + w.hit300)
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
        let near_head_to_tap_h50 = !ho.is_long_note()
            && has_in_win_cand
            && calc_hit_kind((pt - ho.time).abs(), w) == JudgmentKind::Miss
            && matches!(
                calc_hit_kind((presses[press_idx] - ho.time).abs(), w),
                JudgmentKind::Max | JudgmentKind::Hit300
            )
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head_time)| {
                    if next_ho.is_long_note() || next_head_time - ho.time > w.hit50 + w.max {
                        return false;
                    }
                    let cand_pt = presses[press_idx];
                    let next_window_start = next_head_time - w.hit50;
                    let next_win_end = next_head_time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    cand_pt >= ho.time
                        && cand_pt >= next_window_start
                        && cand_pt < next_head_time
                        && calc_hit_kind((cand_pt - next_head_time).abs(), w) == JudgmentKind::Hit50
                        && events
                            .iter()
                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                            .map(|ev| ev.time < next_head_time)
                            .unwrap_or(false)
                        && !presses
                            .iter()
                            .skip(press_idx + 1)
                            .take_while(|cand| **cand < next_win_end)
                            .any(|cand| {
                                let follow_pt = *cand;
                                follow_pt >= next_window_start
                                    && next_next_tap_head
                                        .map(|head| follow_pt < head)
                                        .unwrap_or(true)
                                    && !reserved_ln_repr.contains(cand)
                            })
                })
                .unwrap_or(false);
        let prev_h50_noise_keep = true
            && !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && (early_rel_before_note || early_rel_same_ms)
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    calc_hit_kind((prev_pt - prev_t).abs(), w) == JudgmentKind::Hit50
                        && pt > prev_pt
                        && pt < prev_t
                        && prev_t - pt <= w.max
                })
                .unwrap_or(false)
            && {
                let cand_pt = presses[press_idx];
                let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                let candidate_rel_time = events
                    .iter()
                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let cand_pre_next = candidate_rel_time
                    .map(|rt| next_note_time.map(|next_t| rt < next_t).unwrap_or(true))
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
                                let next_pt = *cand;
                                next_pt >= next_window_start
                                    && next_next_tap_head
                                        .map(|next_next_head| next_pt < next_next_head)
                                        .unwrap_or(true)
                                    && !reserved_ln_repr.contains(cand)
                            })
                    })
                    .unwrap_or(false);
                let next_cand_is_far = next_note_has_cand
                    && candidate_rel_time
                        .zip(next_note_time)
                        .map(|(rt, next_t)| {
                            next_t - ho.time > w.hit50 + w.hit300 && rt <= next_t - w.hit100
                        })
                        .unwrap_or(false);
                cand_pt >= ho.time
                    && cand_pt <= ho.time + w.max
                    && cand_kind == JudgmentKind::Max
                    && cand_pre_next
                    && (!next_note_has_cand || next_cand_is_far)
            };
        let prewin_prev_near_head = !ho.is_long_note()
            && !prev_note_is_ln
            && !prev_was_miss
            && !prev_had_prewin_pen
            && has_in_win_cand
            && (early_rel_before_note || early_rel_same_ms)
            && (!true || presses[press_idx] >= ho.time)
            && (presses[press_idx] >= ho.time - w.hit300 || prev_near_h200)
            && (presses[press_idx] <= ho.time + w.hit300 || prev_cross_h200)
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| pt > prev_pt && pt < prev_t)
                .unwrap_or(false)
            && !prev_h50_noise_keep
            && !prev_gap_early_pen
            && !near_head_to_tap_h50;
        let short_ln_prewin = ho.is_long_note()
            && (!true || prev_was_miss)
            && current_ln_duration <= w.hit100
            && prev_note_is_ln
            && has_in_win_cand
            && early_rel_before_note
            && (!true
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
        let short_ln_carry_hless = true
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
                    let shor_prev_strts_miss = prev_note_duration
                        .map(|d| d <= w.hit100)
                        .unwrap_or(false)
                        && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
                        && early_press_rel_time
                            .map(|rt| {
                                rt > ho.time
                                    && next_note_time
                                        .map(|next_time| rt < next_time)
                                        .unwrap_or(false)
                            })
                            .unwrap_or(false)
                        && next_note_time
                            .map(|next_time| {
                                presses
                                    .iter()
                                    .skip(press_idx)
                                    .take_while(|cand| **cand < next_time)
                                    .any(|cand| *cand > ho.time && !reserved_ln_repr.contains(cand))
                            })
                            .unwrap_or(false);
                    pt < prev_end
                        && !shor_prev_strts_miss
                        && (early_rel_before_note
                            || (true
                                && prev_note_duration.map(|d| d <= w.hit100).unwrap_or(false)
                                && !early_rel_before_note
                                && early_press_rel_time
                                    .map(|rt| rt > ho.time && rt <= prev_end + w.hit50 + w.hit100)
                                    .unwrap_or(false)))
                })
                .unwrap_or(false);
        let post_prev_break = true
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
        let prev_pen_near_no_cand = !ho.is_long_note()
            && !prev_note_is_ln
            && prev_was_miss
            && prev_had_prewin_pen
            && !(prev_prev_was_miss && prev2_had_prewin_pen)
            && !has_in_win_cand
            && early_rel_before_note
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && prev_col_pt
                .zip(prev_note_time)
                .map(|(prev_pt, prev_t)| {
                    prev_pt < prev_t
                        && prev_t - prev_pt > w.hit50 + w.max
                        && pt > prev_pt
                        && pt < prev_t
                        && prev_t - pt <= w.max
                        && ho.time - prev_t <= w.hit50 + w.hit300
                })
                .unwrap_or(false)
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| !next_ho.is_long_note())
                .unwrap_or(true)
            && next_note_time
                .map(|next_t| next_t - ho.time > w.hit50 * 2)
                .unwrap_or(true);
        let prev_pen_next_ln = true
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
                    let next_ln_weak_cur = matches!(
                        calc_hit_kind((cand_pt - next_head_time).abs(), w),
                        JudgmentKind::Hit50 | JudgmentKind::Hit100
                    );
                    let next_ln_from_cur = matches!(
                        calc_hit_kind((cand_pt - next_head_time).abs(), w),
                        JudgmentKind::Hit50 | JudgmentKind::Hit100 | JudgmentKind::Hit200
                    );
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
                    let cur_cand_pre_next = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time > ho.time && ev.time < next_head_time)
                        .unwrap_or(false);
                    let short_next_stays_pen = next_duration <= w.hit50 + w.max
                        && next_ln_from_cur
                        && cur_cand_pre_next
                        && (next_ln_pre_follow || next_ln_self_fol);
                    cand_pt >= ho.time
                        && cand_pt < next_head_time
                        && next_head_time - ho.time <= w.hit50 + w.hit300
                        && matches!(
                            cand_kind,
                            JudgmentKind::Max | JudgmentKind::Hit300 | JudgmentKind::Hit200
                        )
                        && has_next_ln_follow
                        && !(cur_is_next_ln_prewin
                            && (next_ln_pre_follow
                                || (next_duration <= w.hit50 + w.hit100 + w.max
                                    && next_ln_self_fol)))
                        && !short_next_stays_pen
                        && !(next_duration > w.hit100 && next_ln_weak_cur && next_ln_self_fol)
                        && cur_cand_pre_next
                })
                .unwrap_or(false);
        let prev_pen_next_tap = true
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
                .map(|prev_t| pt < prev_t && ho.time - prev_t <= w.hit50 + w.hit300)
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
                    let next3_tap_head =
                        col_notes
                            .get(note_pos + 3)
                            .and_then(|(_, next_next_next_ho)| {
                                (!next_next_next_ho.is_long_note())
                                    .then_some(next_next_next_ho.time)
                            });
                    let next_gap_is_far_engh = next_next_tap_head
                        .map(|next_next_head| next_next_head - next_head_time > w.hit50 + w.hit300)
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
                    let next_tap_strong = presses
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
                            *cand >= next_head_time
                                && matches!(
                                    calc_hit_kind((*cand - next_head_time).abs(), w),
                                    JudgmentKind::Max | JudgmentKind::Hit300
                                )
                        })
                        .unwrap_or(false);
                    let next2_tap_strong = next_next_tap_head
                        .map(|next_next_head| {
                            let next2_win_start = next_next_head - w.hit50;
                            let next2_win_end = next_next_head + w.hit100;
                            presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|cand| **cand < next2_win_end)
                                .find(|cand| {
                                    **cand >= next2_win_start
                                        && next3_tap_head
                                            .map(|next_next_next_head| **cand < next_next_next_head)
                                            .unwrap_or(true)
                                        && !reserved_ln_repr.contains(cand)
                                })
                                .map(|cand| {
                                    *cand >= next_next_head
                                        && matches!(
                                            calc_hit_kind((*cand - next_next_head).abs(), w),
                                            JudgmentKind::Max | JudgmentKind::Hit300
                                        )
                                        && events
                                            .iter()
                                            .find(|ev| ev.time > *cand && !ev.pressed)
                                            .map(|ev| {
                                                next3_tap_head
                                                    .map(|head| ev.time < head)
                                                    .unwrap_or(true)
                                            })
                                            .unwrap_or(false)
                                })
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    let rel_post_cur_cand = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let near_prev_stays_pen = prev_note_time
                        .map(|prev_t| {
                            let prev_fragment_gap = prev_t - pt;
                            prev_fragment_gap >= w.max - 1
                                && prev_fragment_gap <= w.max
                                && (147..=148).contains(&(ho.time - prev_t))
                                && prewindow_overflow == early_penalty_window - 1
                                && next_head_time - ho.time > w.hit50 + w.hit300
                        })
                        .unwrap_or(false);
                    let chain_bound_pre = prev_note_time
                        .map(|prev_t| {
                            let prev_fragment_gap = prev_t - pt;
                            let exact_dense_follow = prev_fragment_gap <= 1
                                && next_next_tap_head
                                    .map(|next_next_head| {
                                        next_next_head - next_head_time <= w.hit50 + w.hit300
                                    })
                                    .unwrap_or(false)
                                && next2_tap_strong;
                            prev_fragment_gap > 0
                                && prev_fragment_gap <= w.max / 2
                                && next_head_time - ho.time <= w.hit50 + w.hit300
                                && (next_gap_is_far_engh || exact_dense_follow)
                                && cand_pt < ho.time
                                && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                                && next_tap_strong
                                && rel_post_cur_cand
                                    .map(|rt| rt > ho.time && rt < next_head_time)
                                    .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    chain_bound_pre
                        || (cand_pt >= ho.time - w.max
                            && cand_pt < ho.time
                            && cand_pt < next_head_time
                            && prev_note_time
                                .map(|prev_t| prev_t - pt > w.max / 2)
                                .unwrap_or(false)
                            && cand_kind == JudgmentKind::Max
                            && prewindow_overflow >= early_penalty_window - 1
                            && !near_prev_stays_pen
                            && has_next_tap_follow
                            && rel_post_cur_cand
                                .map(|rt| rt > ho.time && rt < next_head_time - w.hit50)
                                .unwrap_or(false))
                })
                .unwrap_or(false);
        let pre_mis_pen_next_tap = true
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
                    let next_tap_has_str_fol = presses
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
                            matches!(
                                calc_hit_kind((*cand - next_head_time).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300
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
                        || (cand_pt < ho.time
                            && matches!(cand_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                            && next_tap_has_str_fol
                            && events
                                .iter()
                                .find(|ev| ev.time > cand_pt && !ev.pressed)
                                .map(|ev| {
                                    ev.time > ho.time
                                        && ev.time <= ho.time + w.hit100
                                        && ev.time < next_head_time
                                })
                                .unwrap_or(false))
                })
                .unwrap_or(false);
        let prev_miss_pen_iso = true
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
                let candidate_rel_time = events
                    .iter()
                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let cand_pre_next = candidate_rel_time
                    .map(|rt| next_note_time.map(|nt| rt < nt).unwrap_or(true))
                    .unwrap_or(false);
                let next_note_own_cand = col_notes
                    .get(note_pos + 1)
                    .zip(next_note_time)
                    .and_then(|((_, next_ho), next_head_time)| {
                        if next_ho.is_long_note() {
                            return None;
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
                            .find(|cand| {
                                **cand >= next_window_start
                                    && next_next_tap_head
                                        .map(|next_next_head| **cand < next_next_head)
                                        .unwrap_or(true)
                                    && !reserved_ln_repr.contains(cand)
                            })
                    })
                    .copied();
                let next_note_has_cand = col_notes
                    .get(note_pos + 1)
                    .map(|(_, next_ho)| next_ho.is_long_note())
                    .unwrap_or(false)
                    || next_note_own_cand.is_some();
                let cand_dense_no_own = candidate_rel_time
                    .zip(next_note_time)
                    .map(|(rt, next_t)| {
                        cand_pt >= ho.time
                            && next_t - ho.time <= w.hit50
                            && rt >= next_t
                            && rt <= next_t + w.max
                    })
                    .unwrap_or(false);
                let next_cand_is_far = next_note_has_cand
                    && candidate_rel_time
                        .zip(next_note_time)
                        .map(|(rt, next_t)| {
                            next_t - ho.time > w.hit50 + w.hit300 && rt <= next_t - w.hit100
                        })
                        .unwrap_or(false);
                let same_col_follow_dense = col_notes
                    .get(note_pos + 2)
                    .zip(next_note_time)
                    .map(|((_, next_next_ho), next_t)| {
                        !next_next_ho.is_long_note()
                            && next_next_ho.time - next_t <= w.hit50 + w.hit300
                    })
                    .unwrap_or(false);
                let post_cur_rel_pre_fol = cand_pt >= ho.time
                    && candidate_rel_time
                        .zip(next_note_own_cand)
                        .map(|(rt, next_pt)| rt < next_pt)
                        .unwrap_or(false);
                matches!(
                    cand_kind,
                    JudgmentKind::Max
                        | JudgmentKind::Hit300
                        | JudgmentKind::Hit200
                        | JudgmentKind::Hit100
                ) && (cand_pre_next || (cand_dense_no_own && !next_note_has_cand))
                    && (!next_note_has_cand
                        || next_cand_is_far
                        || (post_cur_rel_pre_fol && same_col_follow_dense))
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
        let sho_ln_pre_frag_clai = true
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
        let prev_prwn_keeps_repr = true
            && (ln_prev_tap_near_head || ln_prewin_near_head)
            && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
            && early_press_rel_time.map(|rt| rt < ho.time).unwrap_or(false)
            && {
                let end_time = ho.end_time.unwrap_or(ho.time);
                let tail_start = end_time - w.hit50;
                let tail_end_exclusive = end_time + w.hit100;
                let candidate_pt = presses[press_idx];
                events
                    .iter()
                    .find(|ev| ev.time > candidate_pt && !ev.pressed)
                    .map(|ev| {
                        let rel_time = ev.time;
                        let post_head_tail_rec = candidate_pt > ho.time
                            && candidate_pt <= end_time
                            && rel_time >= tail_start;
                        let prehead_claims_late = candidate_pt >= ho.time - w.hit300
                            && candidate_pt < ho.time
                            && matches!(
                                calc_hit_kind((candidate_pt - ho.time).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300
                            )
                            && rel_time > end_time;
                        let prehead_claims_end = current_ln_duration > w.hit100
                            && candidate_pt >= ho.time - w.hit300
                            && candidate_pt < ho.time
                            && matches!(
                                calc_hit_kind((candidate_pt - ho.time).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300
                            )
                            && rel_time >= tail_start
                            && rel_time < end_time;
                        (post_head_tail_rec || prehead_claims_late || prehead_claims_end)
                            && rel_time < tail_end_exclusive
                            && rel_time > candidate_pt
                    })
                    .unwrap_or(false)
            };
        let prev_noise_keeps_repr = true
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
        let ln_prewin_near_head = ln_prewin_near_head && prev_frag_blocked;
        let ln_near_deep_late = ln_near_deep_late && prev_frag_blocked;
        let deep_tap_blocked = post_prev_head_pref
            || exact_prev_head_pen
            || post_prev_head_chain
            || far_exact_next_chain
            || far_pen_h300_chain
            || post_h50_strong_pre
            || post_h300_cross_fol
            || post_h300_dense_chain
            || post_h100_dense_fol
            || pos_pre_bod_keep_pen;
        let deep_tap = deep_tap && !deep_tap_blocked;
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
            far_pen_pref_next_ln,
            far_pen_yield_exact,
            far_pen_next_chain,
            far_exact_next_chain,
            far_pen_h300_chain,
            exact_prev_head_pen,
            exact_prev_pen_chain,
            prssls_prev_keep_pen,
            prev_pen_keep_chain,
            deep_tap,
            deep_tap_chain,
            stale_chain_prewin,
            prev_head_noise_prwn,
            prev_h50_noise_keep,
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
            post_prev_head_pref,
            post_prev_head_chain,
            post_h50_prehead_max,
            post_h50_strong_pre,
            post_h300_cross_fol,
            post_h300_dense_chain,
            post_h100_dense_fol,
            post_prev_frag,
            post_prev_frag_next,
        };
        let active_penalty_rule = flags.active_rule();
        if flags.clears_inwin_pen() && has_in_win_cand && !flags.keeps_exact_pen() {
            early_pen_pt = None;
            cleared_penalty_rule = active_penalty_rule;
        } else if dee_hol_prwn_no_cand {
            early_pen_pt = None;
            cleared_penalty_rule = Some("dee_hol_prwn_no_cand");
        } else if prewin_edge_auto_miss {
            early_pen_pt = None;
            cleared_penalty_rule = Some("prewin_edge_auto_miss");
        } else if far_note_no_inwin {
            early_pen_pt = None;
            cleared_penalty_rule = Some("far_note_no_inwin");
        } else if exa_pen_hold_no_inwi {
            early_pen_pt = None;
            cleared_penalty_rule = Some("exa_pen_hold_no_inwi");
        } else if pos_pre_head_to_prhd {
            early_pen_pt = None;
            cleared_penalty_rule = Some("pos_pre_head_to_prhd");
        } else if post_prev_frag || post_prev_frag_next {
            if !exact_prev_head_pen {
                early_pen_pt = None;
                cleared_penalty_rule = Some("post_prev_frag");
            }
        } else if stale_prev_ln_pen {
            early_pen_pt = None;
            cleared_penalty_rule = Some("sta_res_pre_ln_pt_pen");
        } else if stale_prev_ln_no_repr {
            early_pen_pt = None;
            cleared_penalty_rule = Some("stale_prev_ln_no_repr");
        } else if short_ln_carry_hless {
            early_pen_pt = None;
            cleared_penalty_rule = Some("short_ln_carry_hless");
        } else if post_prev_ln_no_inwin {
            early_pen_pt = None;
            cleared_penalty_rule = Some("post_prev_ln_no_inwin");
        } else if prev_pen_near_no_cand {
            early_pen_pt = None;
            cleared_penalty_rule = Some("prev_pen_near_no_cand");
        } else if pos_prev_tap_no_inwi {
            early_pen_pt = None;
            cleared_penalty_rule = Some("pos_prev_tap_no_inwi");
        }
        if early_pen_pt.is_some() && cleared_penalty_rule.is_none() && prev_gap_early_pen {
            retained_penalty_rule = Some("prev_gap_early_pen");
        }
        state.penalty_flags = flags;
    }
    state.rules.early_pen = early_pen_pt;
    state.rules.pen = retained_penalty_rule;
}
