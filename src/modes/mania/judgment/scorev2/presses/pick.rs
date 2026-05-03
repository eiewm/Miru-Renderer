use super::note::{PressNoteCtx, PressState};
use crate::modes::mania::judgment::calc_hit_kind;
use crate::types::JudgmentKind;
pub(super) fn resolve_primary(ctx: &PressNoteCtx<'_>, state: &mut PressState) {
    let _idx = ctx.idx;
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
    let _next_window_start = note_window.next_window_start;
    let early_penalty_window = note_window.early_penalty_window;
    let next_early_pen = note_window.next_early_pen;
    let legacy_early_win = w.max + 4;
    let _last_note_idx_overall = ctx.last_note_idx_overall;
    let _terminal_extreme_ln_end_times = ctx.extreme_ln_ends;
    let _initial_press_time = state.pick.press;
    let _initial_tail_only_pt = state.pick.tail;
    let mut press_idx = state.press_idx;
    let prev_had_prewin_pen = state.prev.had_prewin_pen;
    let _prev_body_break_pre_tail = state.prev.body_break_pre_tail;
    let prev_was_miss = state.prev.was_miss;
    let _prev_prev_prewin_pen = state.prev.prev2_had_prewin_pen;
    let _prev_prev_was_miss = state.prev.prev2_was_miss;
    let prev_col_pt = state.prev.col_pt;
    let _skipped_stale_prev = state.prev.skipped_stale;
    let reserved_ln_repr = &state.prev.reserved_ln_repr;
    let mut early_pen_pt = state.rules.early_pen;
    let mut selected_pt = 0;
    let mut selected_idx = state.press_idx;
    let mut has_sel_cand = false;
    let mut steals_next_ex = false;
    let mut ln_claim_fallback = false;
    let mut prev_miss_pen_prewin = false;
    let mut pre_ear_pen_pos_h200 = false;
    let mut press_time: Option<i32> = None;
    let tail_only_pt: Option<i32> = None;
    if let Some(pt) = early_pen_pt {
        let prev_miss_pen_cur_ln = if ho.is_long_note() && press_idx < presses.len() {
            let current_pt = presses[press_idx];
            let cur_rel_in_body = ho.end_time.and_then(|end_time| {
                events
                    .iter()
                    .find(|ev| ev.time > current_pt && !ev.pressed)
                    .map(|ev| ev.time)
                    .filter(|rt| {
                        *rt > ho.time
                            && *rt < end_time
                            && next_note_time
                                .map(|next_time| *rt < next_time)
                                .unwrap_or(true)
                    })
            });
            let early_rel_pre_cur = events
                .iter()
                .find(|ev| ev.time > pt && !ev.pressed)
                .map(|ev| ev.time)
                .filter(|rt| *rt < current_pt && *rt < ho.time);
            prev_was_miss
                && !prev_had_prewin_pen
                && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
                && note_pos
                    .checked_sub(1)
                    .and_then(|p| col_notes.get(p))
                    .map(|(_, prev_ho)| !prev_ho.is_long_note())
                    .unwrap_or(false)
                && note_pos
                    .checked_sub(1)
                    .and_then(|p| col_notes.get(p))
                    .map(|(_, prev_ho)| prev_ho.time)
                    .map(|prev_t| {
                        let prev_press_is_stale =
                            prev_col_pt.map(|prev_pt| prev_pt < prev_t).unwrap_or(true);
                        prev_press_is_stale
                            && pt == prev_t + w.hit100
                            && ho.time - prev_t <= w.hit50 * 2
                    })
                    .unwrap_or(false)
                && current_pt >= window_start
                && current_pt < ho.time
                && next_note_time
                    .map(|next_time| current_pt < next_time - w.hit50)
                    .unwrap_or(true)
                && !reserved_ln_repr.contains(&current_pt)
                && matches!(
                    calc_hit_kind((current_pt - ho.time).abs(), w),
                    JudgmentKind::Max | JudgmentKind::Hit300
                )
                && early_rel_pre_cur.is_some()
                && cur_rel_in_body.is_some()
        } else {
            false
        };
        let short_overlap_miss = if true && ho.is_long_note() {
            let end_time = ho.end_time.unwrap_or(ho.time);
            let current_ln_duration = end_time - ho.time;
            let prev_note = note_pos.checked_sub(1).and_then(|p| col_notes.get(p));
            let prev_note_is_ln = prev_note
                .map(|(_, prev_ho)| prev_ho.is_long_note())
                .unwrap_or(false);
            let prev_note_end_time =
                prev_note.map(|(_, prev_ho)| prev_ho.end_time.unwrap_or(prev_ho.time));
            let has_in_win_cand = press_idx < presses.len()
                && presses[press_idx] >= window_start
                && presses[press_idx] < lock_end_exclusive
                && !reserved_ln_repr.contains(&presses[press_idx]);
            let early_press_rel_time = events
                .iter()
                .find(|ev| ev.time > pt && !ev.pressed)
                .map(|ev| ev.time);
            current_ln_duration <= w.hit100
                && prev_note_is_ln
                && prev_was_miss
                && !prev_had_prewin_pen
                && !has_in_win_cand
                && prev_note_end_time
                    .map(|prev_end| pt < prev_end)
                    .unwrap_or(false)
                && early_press_rel_time
                    .map(|rt| rt > ho.time && rt <= end_time)
                    .unwrap_or(false)
        } else {
            false
        };
        if prev_miss_pen_cur_ln {
            selected_pt = presses[press_idx];
            selected_idx = press_idx;
            has_sel_cand = true;
            early_pen_pt = None;
            prev_miss_pen_prewin = true;
        } else if !short_overlap_miss {
            press_time = Some(pt);
            let cont_prewin_chain = ho.is_long_note() || prev_had_prewin_pen;
            if !cont_prewin_chain {
                let prewindow_overflow = (ho.time - pt).abs() - w.hit50;
                let near_hit50_edge = prewindow_overflow <= 4;
                let mut consume_until = if near_hit50_edge {
                    ho.time
                } else {
                    lock_end_exclusive
                };
                if !near_hit50_edge
                    && (prewindow_overflow > legacy_early_win || prewindow_overflow > 4)
                {
                    if let Some(next_start) = next_early_pen {
                        consume_until = consume_until.min(next_start);
                    }
                }
                let prhd_inwin_next_tap = true
                    && !ho.is_long_note()
                    && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
                    && next_note_time
                        .map(|next_time| {
                            next_time - ho.time <= w.hit50 && press_idx < presses.len() && {
                                let next_window_start = next_time - w.hit50;
                                let next_pt = presses[press_idx];
                                next_pt >= next_window_start
                                    && next_pt < ho.time
                                    && !reserved_ln_repr.contains(&next_pt)
                            }
                        })
                        .unwrap_or(false);
                let prewin_pen_next_tap = true
                    && !ho.is_long_note()
                    && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
                    && next_note_time
                        .map(|next_time| {
                            next_time - ho.time <= w.hit50 + w.hit300
                                && press_idx < presses.len()
                                && {
                                    let next_window_start = next_time - w.hit50;
                                    let next_prewin_start =
                                        next_window_start - early_penalty_window - 1;
                                    let next_pt = presses[press_idx];
                                    let tig_nex_tap_h20_frag = next_time - ho.time <= w.hit50
                                        && calc_hit_kind((next_pt - ho.time).abs(), w)
                                            == JudgmentKind::Hit200;
                                    (next_pt > ho.time - w.hit300 || tig_nex_tap_h20_frag)
                                        && next_pt >= next_prewin_start
                                        && next_pt < next_window_start
                                        && next_pt < ho.time
                                        && !reserved_ln_repr.contains(&next_pt)
                                        && events
                                            .iter()
                                            .find(|ev| ev.time > next_pt && !ev.pressed)
                                            .map(|ev| ev.time < next_time + w.hit300)
                                            .unwrap_or(false)
                                }
                        })
                        .unwrap_or(false);
                let prewin_pen_next_ln = true
                    && !ho.is_long_note()
                    && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
                    && col_notes
                        .get(note_pos + 1)
                        .map(|(_, next_ho)| next_ho.is_long_note())
                        .unwrap_or(false)
                    && next_note_time
                        .map(|next_time| {
                            next_time - ho.time <= w.hit50 + w.hit300
                                && press_idx < presses.len()
                                && {
                                    let next_window_start = next_time - w.hit50;
                                    let next_prewin_start =
                                        next_window_start - early_penalty_window - 1;
                                    let next_pt = presses[press_idx];
                                    let next_cand_rel_gap = events
                                        .iter()
                                        .find(|ev| ev.time > next_pt && !ev.pressed)
                                        .map(|ev| ev.time > ho.time && ev.time < next_time)
                                        .unwrap_or(false);
                                    let next_cand_post_head = col_notes
                                        .get(note_pos + 1)
                                        .map(|(_, next_ho)| {
                                            let next_end_time =
                                                next_ho.end_time.unwrap_or(next_ho.time);
                                            let next_tail_start = next_end_time - w.hit50;
                                            let next_tail_end = next_end_time + w.hit100;
                                            let next_next_note_time =
                                                col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                                            let next_ln_late_end = next_next_note_time
                                                .map(|next_next_time| {
                                                    next_next_time <= next_ho.time + w.hit50
                                                })
                                                .unwrap_or(false);
                                            let next_lock_end = next_ho.time
                                                + w.hit50
                                                + if next_ln_late_end { 1 } else { 0 };
                                            next_pt >= ho.time
                                                && next_cand_rel_gap
                                                && presses
                                                    .iter()
                                                    .skip(press_idx + 1)
                                                    .take_while(|cand| **cand < next_lock_end)
                                                    .any(|cand| {
                                                        let followup_pt = *cand;
                                                        followup_pt >= next_window_start
                                                            && !reserved_ln_repr.contains(cand)
                                                            && events
                                                                .iter()
                                                                .find(|ev| {
                                                                    ev.time > followup_pt
                                                                        && !ev.pressed
                                                                })
                                                                .map(|ev| {
                                                                    ev.time >= next_tail_start
                                                                        && ev.time < next_tail_end
                                                                })
                                                                .unwrap_or(false)
                                                    })
                                        })
                                        .unwrap_or(false);
                                    next_pt >= next_prewin_start
                                        && next_pt < next_window_start
                                        && (next_pt < ho.time || next_cand_post_head)
                                        && !reserved_ln_repr.contains(&next_pt)
                                        && next_cand_rel_gap
                                }
                        })
                        .unwrap_or(false);
                if prhd_inwin_next_tap {
                    consume_until = consume_until.min(window_start);
                } else if prewin_pen_next_tap {
                    if let Some(next_start) = next_early_pen {
                        consume_until = consume_until.min(next_start);
                    }
                } else if prewin_pen_next_ln {
                    if let Some(next_start) = next_early_pen {
                        consume_until = consume_until.min(next_start);
                    }
                }
                let prev_early_pen_h200 = !ho.is_long_note()
                    && !prev_was_miss
                    && !prev_had_prewin_pen
                    && matches!(state.rules.pen, Some("prev_gap_early_pen"))
                    && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
                    && note_pos
                        .checked_sub(1)
                        .and_then(|p| col_notes.get(p))
                        .zip(col_notes.get(note_pos + 1))
                        .zip(col_notes.get(note_pos + 2))
                        .map(|(((_, prev_ho), (_, next_ho)), (_, following_ho))| {
                            if prev_ho.is_long_note()
                                || next_ho.is_long_note()
                                || following_ho.is_long_note()
                                || press_idx >= presses.len()
                            {
                                return false;
                            }
                            let current_pt = presses[press_idx];
                            let current_release = events
                                .iter()
                                .find(|ev| ev.time > current_pt && !ev.pressed)
                                .map(|ev| ev.time);
                            let next_head_time = next_ho.time;
                            let following_head_time = following_ho.time;
                            let has_cur_pre_head = presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|cand| **cand < ho.time)
                                .any(|cand| !reserved_ln_repr.contains(cand));
                            let next_tap_nonmiss = presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|cand| **cand < following_head_time)
                                .any(|cand| {
                                    let next_pt = *cand;
                                    next_pt >= next_head_time - w.hit50
                                        && next_pt < following_head_time
                                        && !reserved_ln_repr.contains(cand)
                                        && calc_hit_kind((next_pt - next_head_time).abs(), w)
                                            != JudgmentKind::Miss
                                });
                            let follow_tap_strong = presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|cand| **cand < following_head_time)
                                .any(|cand| {
                                    let following_pt = *cand;
                                    following_pt >= following_head_time - w.hit50
                                        && following_pt < following_head_time
                                        && !reserved_ln_repr.contains(cand)
                                        && matches!(
                                            calc_hit_kind(
                                                (following_pt - following_head_time).abs(),
                                                w
                                            ),
                                            JudgmentKind::Max | JudgmentKind::Hit300
                                        )
                                });
                            current_pt > prev_ho.time
                                && current_pt >= window_start
                                && current_pt < ho.time
                                && current_pt < next_head_time - w.hit50
                                && !reserved_ln_repr.contains(&current_pt)
                                && !has_cur_pre_head
                                && calc_hit_kind((current_pt - ho.time).abs(), w)
                                    == JudgmentKind::Hit200
                                && current_release
                                    .map(|rt| {
                                        rt > ho.time
                                            && rt == next_head_time + w.hit100
                                            && rt < following_head_time
                                    })
                                    .unwrap_or(false)
                                && !next_tap_nonmiss
                                && follow_tap_strong
                        })
                        .unwrap_or(false);
                if prev_early_pen_h200 {
                    let current_pt = presses[press_idx];
                    press_time = Some(current_pt);
                    early_pen_pt = None;
                    consume_until = consume_until.max(current_pt.saturating_add(1));
                    pre_ear_pen_pos_h200 = true;
                }
                let pen_prev_miss_strong = !ho.is_long_note()
                    && prev_was_miss
                    && note_pos
                        .checked_sub(1)
                        .and_then(|p| col_notes.get(p))
                        .map(|(_, prev_ho)| !prev_ho.is_long_note() && pt < prev_ho.time)
                        .unwrap_or(false)
                    && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
                    && press_idx < presses.len()
                    && {
                        let current_pt = presses[press_idx];
                        current_pt >= ho.time
                            && current_pt < lock_end_exclusive
                            && next_note_time
                                .map(|next_time| current_pt < next_time - w.hit50)
                                .unwrap_or(true)
                            && !reserved_ln_repr.contains(&current_pt)
                            && matches!(
                                calc_hit_kind((current_pt - ho.time).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300
                            )
                            && events
                                .iter()
                                .find(|ev| ev.time > current_pt && !ev.pressed)
                                .map(|ev| {
                                    next_note_time
                                        .map(|next_time| ev.time < next_time)
                                        .unwrap_or(true)
                                })
                                .unwrap_or(true)
                    };
                if pen_prev_miss_strong {
                    let current_pt = presses[press_idx];
                    press_time = Some(current_pt);
                    early_pen_pt = None;
                    consume_until = consume_until.max(current_pt.saturating_add(1));
                }
                let post_h300_to_h50 = !ho.is_long_note()
                    && !prev_was_miss
                    && !prev_had_prewin_pen
                    && calc_hit_kind((ho.time - pt).abs(), w) == JudgmentKind::Miss
                    && press_idx < presses.len()
                    && note_pos
                        .checked_sub(1)
                        .and_then(|p| col_notes.get(p))
                        .map(|(_, prev_ho)| !prev_ho.is_long_note())
                        .unwrap_or(false)
                    && prev_col_pt
                        .zip(
                            note_pos
                                .checked_sub(1)
                                .and_then(|p| col_notes.get(p))
                                .map(|(_, prev_ho)| prev_ho.time),
                        )
                        .map(|(prev_pt, prev_t)| {
                            calc_hit_kind((prev_pt - prev_t).abs(), w) == JudgmentKind::Hit300
                                && pt > prev_t + w.max
                                && pt <= prev_t + w.hit300
                        })
                        .unwrap_or(false)
                    && {
                        let current_pt = presses[press_idx];
                        let cur_rel_pre_note = events
                            .iter()
                            .find(|ev| ev.time > current_pt && !ev.pressed)
                            .map(|ev| ev.time < ho.time)
                            .unwrap_or(false);
                        let early_rel_before_cur = events
                            .iter()
                            .find(|ev| ev.time > pt && !ev.pressed)
                            .map(|ev| ev.time < current_pt)
                            .unwrap_or(false);
                        current_pt >= window_start
                            && current_pt < ho.time
                            && next_note_time
                                .map(|next_time| current_pt < next_time - w.hit50)
                                .unwrap_or(true)
                            && !reserved_ln_repr.contains(&current_pt)
                            && calc_hit_kind((current_pt - ho.time).abs(), w) == JudgmentKind::Hit50
                            && cur_rel_pre_note
                            && early_rel_before_cur
                            && next_note_time
                                .zip(col_notes.get(note_pos + 1))
                                .map(|(next_head_time, (_, next_ho))| {
                                    if next_ho.is_long_note() {
                                        return false;
                                    }
                                    let next_window_start = next_head_time - w.hit50;
                                    presses
                                        .iter()
                                        .skip(press_idx + 1)
                                        .take_while(|cand| **cand < next_head_time + w.hit100)
                                        .any(|cand| {
                                            let next_pt = *cand;
                                            next_pt >= next_window_start
                                                && next_pt < next_head_time
                                                && !reserved_ln_repr.contains(cand)
                                                && matches!(
                                                    calc_hit_kind(
                                                        (next_pt - next_head_time).abs(),
                                                        w
                                                    ),
                                                    JudgmentKind::Max
                                                        | JudgmentKind::Hit300
                                                        | JudgmentKind::Hit200
                                                        | JudgmentKind::Hit100
                                                )
                                        })
                                })
                                .unwrap_or(false)
                    };
                if post_h300_to_h50 {
                    let current_pt = presses[press_idx];
                    press_time = Some(current_pt);
                    early_pen_pt = None;
                    consume_until = consume_until.max(current_pt.saturating_add(1));
                }
                let mut late_follow_post_head: Option<i32> = None;
                let pre_frag_late_follow = !true
                    && !ho.is_long_note()
                    && press_idx < presses.len()
                    && presses[press_idx] >= window_start
                    && presses[press_idx] < lock_end_exclusive
                    && !reserved_ln_repr.contains(&presses[press_idx])
                    && {
                        let first_candidate = presses[press_idx];
                        let fir_can_rels_by_head = events
                            .iter()
                            .find(|ev| ev.time > first_candidate && !ev.pressed)
                            .map(|ev| ev.time <= ho.time)
                            .unwrap_or(false);
                        late_follow_post_head = presses
                            .iter()
                            .skip(press_idx + 1)
                            .take_while(|next_pt| **next_pt < lock_end_exclusive)
                            .find(|next_pt| {
                                **next_pt >= ho.time && !reserved_ln_repr.contains(next_pt)
                            })
                            .copied();
                        first_candidate < ho.time
                            && fir_can_rels_by_head
                            && late_follow_post_head.is_some()
                    };
                if pre_frag_late_follow {
                    let pre_not_is_ln_for_prm = note_pos
                        .checked_sub(1)
                        .and_then(|p| col_notes.get(p))
                        .map(|(_, prev_ho)| prev_ho.is_long_note())
                        .unwrap_or(false);
                    let pre_not_tim_for_prmt = note_pos
                        .checked_sub(1)
                        .and_then(|p| col_notes.get(p))
                        .map(|(_, prev_ho)| prev_ho.time);
                    let promote_late_fol = !pre_not_is_ln_for_prm
                        && !prev_was_miss
                        && !prev_had_prewin_pen
                        && prev_col_pt
                            .zip(pre_not_tim_for_prmt)
                            .map(|(prev_pt, prev_t)| pt > prev_pt && pt < prev_t)
                            .unwrap_or(false);
                    if promote_late_fol {
                        if let Some(late_pt) = late_follow_post_head {
                            press_time = Some(late_pt);
                            early_pen_pt = None;
                            consume_until = consume_until.max(late_pt.saturating_add(1));
                        }
                    } else {
                        consume_until = consume_until.max(ho.time);
                    }
                }
                while press_idx < presses.len() && presses[press_idx] < consume_until {
                    press_idx += 1;
                }
            }
        } else {
            early_pen_pt = None;
        }
    } else if press_idx < presses.len() {
        let pt = presses[press_idx];
        let ln_duration = ho.end_time.unwrap_or(ho.time) - ho.time;
        let exact_next_head_late = ho.is_long_note()
            && next_note_time
                .map(|next_time| pt == next_time && pt >= ho.time + w.hit50)
                .unwrap_or(false);
        let short_exact_no_follow = if exact_next_head_late {
            if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
                if next_ho.is_long_note() && ln_duration <= w.hit100 && ln_duration > w.hit300 {
                    let next_window_start = next_ho.time - w.hit50;
                    let next_next_note_time = col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                    let next_ln_late_end = next_next_note_time
                        .map(|next_time| next_time <= next_ho.time + w.hit50)
                        .unwrap_or(false);
                    let next_lock_end =
                        next_ho.time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                    let has_next_pt_fol = press_idx + 1 < presses.len() && {
                        let next_pt = presses[press_idx + 1];
                        next_pt >= next_window_start
                            && next_pt < next_lock_end
                            && !reserved_ln_repr.contains(&next_pt)
                    };
                    !has_next_pt_fol
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };
        let short_h50_pref_tap =
            if ho.is_long_note() && ln_duration <= w.hit100 && pt == ho.time + w.hit50 {
                if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
                    let end_time = ho.end_time.unwrap_or(ho.time);
                    !next_ho.is_long_note()
                        && next_ho.time <= pt
                        && pt > end_time
                        && events
                            .iter()
                            .any(|ev| !ev.pressed && ev.time > end_time && ev.time < pt)
                } else {
                    false
                }
            } else {
                false
            };
        let short_post_end_tap = if true && ho.is_long_note() && ln_duration <= w.hit100 {
            if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
                if next_ho.is_long_note() {
                    false
                } else {
                    let end_time = ho.end_time.unwrap_or(ho.time);
                    let next_tap_window_start = next_ho.time - w.hit50;
                    let next_tap_end = next_ho.time + w.hit100;
                    let next_next_tap_head =
                        col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                            (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                        });
                    let has_next_tap_follow = press_idx + 1 < presses.len() && {
                        let next_pt = presses[press_idx + 1];
                        let next_pt_is_next2 = next_next_tap_head
                            .map(|next_next_head| {
                                let next2_win_start = next_next_head - w.hit50;
                                let next2_win_end = next_next_head + w.hit100;
                                next_pt >= next2_win_start
                                    && next_pt < next2_win_end
                                    && calc_hit_kind((next_pt - next_next_head).abs(), w)
                                        != JudgmentKind::Miss
                            })
                            .unwrap_or(false);
                        next_pt >= next_tap_window_start
                            && next_pt < next_tap_end
                            && !reserved_ln_repr.contains(&next_pt)
                            && !next_pt_is_next2
                    };
                    pt > end_time && pt >= next_ho.time && pt < next_tap_end && !has_next_tap_follow
                }
            } else {
                false
            }
        } else {
            false
        };
        steals_next_ex = (exact_next_head_late && !short_exact_no_follow)
            || short_h50_pref_tap
            || short_post_end_tap;
        ln_claim_fallback = if ho.is_long_note() && pt == ho.time + w.hit50 {
            if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
                if !next_ho.is_long_note() {
                    false
                } else if ln_duration > w.hit100 {
                    let next_window_start = next_ho.time - w.hit50;
                    let next_duration = next_ho.end_time.unwrap_or(next_ho.time) - next_ho.time;
                    let next_next_note_time = col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                    let next_ln_late_end = next_next_note_time
                        .map(|next_time| next_time <= next_ho.time + w.hit50)
                        .unwrap_or(false);
                    let next_lock_end =
                        next_ho.time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                    let has_next_pt_fol = press_idx + 1 < presses.len() && {
                        let next_pt = presses[press_idx + 1];
                        next_pt >= next_window_start
                            && next_pt < next_lock_end
                            && !reserved_ln_repr.contains(&next_pt)
                    };
                    let rel_after_bound = events
                        .iter()
                        .find(|ev| ev.time > pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let bound_press_next = rel_after_bound
                        .map(|rt| rt >= next_ho.time)
                        .unwrap_or(false);
                    let bound_press_to_short = !true
                        && ln_duration <= w.hit50 + w.max
                        && next_duration <= w.hit200
                        && pt >= next_window_start
                        && pt < next_ho.time
                        && rel_after_bound
                            .map(|rt| {
                                let end_time = ho.end_time.unwrap_or(ho.time);
                                rt > end_time && rt < next_ho.time
                            })
                            .unwrap_or(false)
                        && next_next_note_time
                            .zip(col_notes.get(note_pos + 2))
                            .map(|(next_next_head, (_, next_next_ho))| {
                                if !next_next_ho.is_long_note() || !has_next_pt_fol {
                                    return false;
                                }
                                let next_followup_pt = presses[press_idx + 1];
                                let next2_win_start = next_next_head - w.hit50;
                                next_followup_pt >= next2_win_start
                                    && next_followup_pt < next_next_head
                            })
                            .unwrap_or(false);
                    let has_follow_claim_cur = if true {
                        has_next_pt_fol
                    } else {
                        has_next_pt_fol && !bound_press_next && !bound_press_to_short
                    };
                    let bound_hold_cross_next = !true
                        && !has_next_pt_fol
                        && pt >= next_window_start
                        && pt < next_ho.time
                        && rel_after_bound.map(|rt| rt > next_ho.time).unwrap_or(true);
                    has_follow_claim_cur || bound_hold_cross_next
                } else if ln_duration <= w.hit100 {
                    let next_window_start = next_ho.time - w.hit50;
                    let next_ln_duration = next_ho.end_time.unwrap_or(next_ho.time) - next_ho.time;
                    let next_next_note_time = col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                    let next_ln_late_end = next_next_note_time
                        .map(|next_time| next_time <= next_ho.time + w.hit50)
                        .unwrap_or(false);
                    let next_lock_end =
                        next_ho.time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                    let has_next_pt_fol = press_idx + 1 < presses.len() && {
                        let next_pt = presses[press_idx + 1];
                        next_pt >= next_window_start
                            && next_pt < next_lock_end
                            && !reserved_ln_repr.contains(&next_pt)
                    };
                    let first_rel_pt = events
                        .iter()
                        .find(|ev| ev.time > pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let bound_press_next =
                        first_rel_pt.map(|rt| rt >= next_ho.time).unwrap_or(false);
                    let bound_rel_before_next = first_rel_pt
                        .map(|rt| rt <= next_ho.end_time.unwrap_or(next_ho.time))
                        .unwrap_or(false);
                    let rel_after_bound_near =
                        first_rel_pt.map(|rt| rt - pt <= w.hit50).unwrap_or(false);
                    let next_pt_bound_sparse = press_idx + 1 < presses.len() && {
                        let next_pt = presses[press_idx + 1];
                        next_pt - pt > w.hit50 + w.max * 2 + 2
                    };
                    let cur_ln_pre_bound = events
                        .iter()
                        .any(|ev| !ev.pressed && ev.time > ho.time && ev.time < pt);
                    let exact_post_end_pref = !true
                        && pt > ho.end_time.unwrap_or(ho.time)
                        && pt < next_window_start
                        && first_rel_pt
                            .map(|rt| rt < next_window_start)
                            .unwrap_or(false);
                    let next_long_bridge = !has_next_pt_fol
                        && next_ln_duration > w.hit100
                        && pt >= next_window_start
                        && pt < next_ho.time
                        && next_ho.time - pt <= w.max / 2
                        && bound_press_next
                        && bound_rel_before_next;
                    let short_ln_bound_pref = !bound_rel_before_next && cur_ln_pre_bound;
                    let bound_swap_next_ln = next_long_bridge
                        || (!has_next_pt_fol
                            && rel_after_bound_near
                            && !short_ln_bound_pref
                            && next_pt_bound_sparse);
                    exact_post_end_pref
                        || (next_ho.time > ho.time + w.hit50 + w.max
                            && pt >= next_window_start
                            && pt < next_ho.time
                            && !bound_swap_next_ln)
                        || (pt >= next_window_start
                            && pt < next_ho.time
                            && !has_next_pt_fol
                            && !bound_swap_next_ln)
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };
        selected_pt = pt;
        selected_idx = press_idx;
        has_sel_cand = true;
    }
    state.press_idx = press_idx;
    state.rules.early_pen = early_pen_pt;
    state.pick.press = press_time;
    state.pick.tail = tail_only_pt;
    state.head_candidate.has_candidate = has_sel_cand;
    state.head_candidate.selected_pt = selected_pt;
    state.head_candidate.selected_idx = selected_idx;
    state.head_candidate.steals_next_ex = steals_next_ex;
    state.head_candidate.ln_claim_fallback = ln_claim_fallback;
    state.head_candidate.prev_miss_pen_prewin = prev_miss_pen_prewin;
    state.head_candidate.pre_ear_pen_pos_h200 = pre_ear_pen_pos_h200;
}
