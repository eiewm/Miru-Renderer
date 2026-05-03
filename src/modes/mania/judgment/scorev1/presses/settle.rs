use super::note::{PressNoteCtx, PressState};
use crate::modes::mania::judgment::calc_hit_kind;
use crate::types::JudgmentKind;
pub(super) fn finalize_candidate(ctx: &PressNoteCtx<'_>, state: &mut PressState) {
    if !state.head_candidate.has_candidate {
        let followup_press_time = ctx.presses.get(state.press_idx).copied();
        let followup_rel_time = followup_press_time.and_then(|pt| {
            ctx.events
                .iter()
                .find(|ev| ev.time > pt && !ev.pressed)
                .map(|ev| ev.time)
        });
        let rel_after_sel = state.pick.press.and_then(|pt| {
            ctx.events
                .iter()
                .find(|ev| ev.time > pt && !ev.pressed)
                .map(|ev| ev.time)
        });
        let _prev_end_time = ctx
            .note_pos
            .checked_sub(1)
            .and_then(|p| ctx.col_notes.get(p))
            .map(|(_, prev_ho)| prev_ho.end_time.unwrap_or(prev_ho.time));
        let _next_ln_post_follow = followup_press_time
            .zip(followup_rel_time)
            .and_then(|(_, followup_release)| {
                ctx.col_notes.get(ctx.note_pos + 1).map(|(_, next_ho)| {
                    if !next_ho.is_long_note() {
                        return false;
                    }
                    let next_head = next_ho.time;
                    let next_window_start = next_head - ctx.windows.hit50;
                    let next_win_end = next_head + ctx.windows.hit100;
                    ctx.presses
                        .iter()
                        .skip(state.press_idx + 1)
                        .take_while(|cand| **cand < next_win_end)
                        .any(|cand| {
                            let cand_pt = *cand;
                            cand_pt >= next_window_start
                                && cand_pt > followup_release
                                && !state.prev.reserved_ln_repr.contains(cand)
                                && calc_hit_kind((cand_pt - next_head).abs(), ctx.windows)
                                    != JudgmentKind::Miss
                        })
                })
            })
            .unwrap_or(false);
        if let Some(pt) = state.pick.press {
            let ho = ctx.ho;
            let w = ctx.windows;
            let end_time = ho.end_time.unwrap_or(ho.time);
            let ln_duration = end_time - ho.time;
            let prev_note = ctx
                .note_pos
                .checked_sub(1)
                .and_then(|p| ctx.col_notes.get(p));
            let prev_end_time =
                prev_note.map(|(_, prev_ho)| prev_ho.end_time.unwrap_or(prev_ho.time));
            let prev_duration = prev_note
                .map(|(_, prev_ho)| prev_ho.end_time.unwrap_or(prev_ho.time) - prev_ho.time);
            if let Some(followup_pt) = followup_press_time {
                let followup_release = ctx
                    .events
                    .iter()
                    .find(|ev| ev.time > followup_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let no_ext_cur_pt_pre_end = !ctx
                    .presses
                    .iter()
                    .skip(state.press_idx + 1)
                    .take_while(|cand| **cand <= end_time)
                    .any(|cand| !state.prev.reserved_ln_repr.contains(cand));
                let next_ln_cand_fol = ctx
                    .col_notes
                    .get(ctx.note_pos + 1)
                    .map(|(_, next_ho)| {
                        if !next_ho.is_long_note() {
                            return false;
                        }
                        let next_head = next_ho.time;
                        let next_window_start = next_head - w.hit50;
                        let next_win_end = next_head + w.hit100;
                        ctx.presses
                            .iter()
                            .skip(state.press_idx + 1)
                            .take_while(|cand| **cand < next_win_end)
                            .any(|cand| {
                                let cand_pt = *cand;
                                cand_pt >= next_window_start
                                    && followup_release.map(|rt| cand_pt > rt).unwrap_or(true)
                                    && !state.prev.reserved_ln_repr.contains(cand)
                                    && calc_hit_kind((cand_pt - next_head).abs(), w)
                                        != JudgmentKind::Miss
                            })
                    })
                    .unwrap_or(false);
                let should_promote_fol = ho.is_long_note()
                    && ln_duration <= w.hit100
                    && !state.prev.had_prewin_pen
                    && prev_note
                        .map(|(_, prev_ho)| prev_ho.is_long_note())
                        .unwrap_or(false)
                    && prev_duration.map(|dur| dur <= w.hit100).unwrap_or(false)
                    && calc_hit_kind((pt - ho.time).abs(), w) == JudgmentKind::Miss
                    && prev_note
                        .zip(prev_end_time)
                        .map(|((_, prev_ho), prev_end)| {
                            pt < prev_ho.time
                                && calc_hit_kind((pt - prev_ho.time).abs(), w) != JudgmentKind::Miss
                                && rel_after_sel
                                    .map(|rt| rt > prev_ho.time && rt <= prev_end && rt < ho.time)
                                    .unwrap_or(false)
                                && followup_pt > prev_end
                                && followup_pt - prev_end <= w.hit50
                        })
                        .unwrap_or(false)
                    && !state.prev.reserved_ln_repr.contains(&followup_pt)
                    && followup_pt < ho.time
                    && matches!(
                        calc_hit_kind((followup_pt - ho.time).abs(), w),
                        JudgmentKind::Max
                    )
                    && followup_release
                        .map(|rt| {
                            rt > ho.time
                                && rt <= end_time
                                && calc_hit_kind((rt - end_time).abs(), w) == JudgmentKind::Max
                                && ctx.next_note_time.map(|next_t| rt < next_t).unwrap_or(true)
                        })
                        .unwrap_or(false)
                    && no_ext_cur_pt_pre_end
                    && next_ln_cand_fol;
                if should_promote_fol {
                    state.pick.press = Some(followup_pt);
                    state.press_idx += 1;
                    return;
                }
            }
        }
        return;
    }
    let idx = ctx.idx;
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
    let last_note_idx_overall = ctx.last_note_idx_overall;
    let extreme_ln_ends = ctx.extreme_ln_ends;
    let mut press_idx = state.press_idx;
    let skipped_stale_prev = state.prev.skipped_stale;
    let prev_was_miss = state.prev.was_miss;
    let prev_had_prewin_pen = state.prev.had_prewin_pen;
    let prev_break_pre = state.prev.body_break_pre_tail;
    let prev_col_pt = state.prev.col_pt;
    let reserved_ln_repr = &mut state.prev.reserved_ln_repr;
    let prev_is_ln_stale = note_pos
        .checked_sub(1)
        .and_then(|p| col_notes.get(p))
        .map(|(_, prev_ho)| prev_ho.is_long_note())
        .unwrap_or(false);
    let prev_stale_time = note_pos
        .checked_sub(1)
        .and_then(|p| col_notes.get(p))
        .map(|(_, prev_ho)| prev_ho.time);
    let prev_end_stale = note_pos
        .checked_sub(1)
        .and_then(|p| col_notes.get(p))
        .map(|(_, prev_ho)| prev_ho.end_time.unwrap_or(prev_ho.time));
    let prev_dur_stale = note_pos
        .checked_sub(1)
        .and_then(|p| col_notes.get(p))
        .map(|(_, prev_ho)| prev_ho.end_time.unwrap_or(prev_ho.time) - prev_ho.time);
    let prev_note_miss_time = note_pos
        .checked_sub(1)
        .and_then(|p| col_notes.get(p))
        .map(|(_, prev_ho)| prev_ho.time);
    let _original_selected_pt = state.head_candidate.selected_pt;
    let _original_selected_idx = state.head_candidate.selected_idx;
    let mut selected_pt = state.head_candidate.selected_pt;
    let mut selected_idx = state.head_candidate.selected_idx;
    let steals_next_ex = state.head_candidate.steals_next_ex;
    let ln_claim_fallback = state.head_candidate.ln_claim_fallback;
    let tap_micro_keep_idx = state.head_candidate.tap_micro_keeps_idx;
    let mut ghost_prehead = state.head_candidate.ghost_prehead;
    let late_tap_cross_tap = state.head_candidate.late_tap_cross_tap;
    let late_tap_dense_chain = state.head_candidate.late_tap_dense_chain;
    let late_tap_iso_head = state.head_candidate.late_tap_iso_head;
    let late_tap_cross_ln = state.head_candidate.late_tap_cross_ln;
    let prev_miss_keeps_hless = state.head_candidate.prev_miss_keeps_hless;
    let mut press_time = state.pick.press;
    let mut tail_only_pt = state.pick.tail;
    let pt = selected_pt;
    let ln_duration = ho.end_time.unwrap_or(ho.time) - ho.time;
    let pre_miss_ghost_late = ghost_prehead
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale && selected_pt == prev_t + w.hit100
            })
            .unwrap_or(false);
    let pre_miss_fallback = !ghost_prehead
        && false
        && !ho.is_long_note()
        && prev_was_miss
        && !prev_had_prewin_pen
        && selected_pt < ho.time - w.hit100
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale && selected_pt == prev_t + w.hit100
            })
            .unwrap_or(false)
        && events
            .iter()
            .find(|ev| ev.time > selected_pt && !ev.pressed)
            .map(|ev| ev.time < ho.time)
            .unwrap_or(false);
    let pre_miss_ln_fallback = !ghost_prehead
        && false
        && ho.is_long_note()
        && prev_was_miss
        && !prev_had_prewin_pen
        && selected_pt < ho.time - w.hit100
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale && selected_pt == prev_t + w.hit100
            })
            .unwrap_or(false)
        && events
            .iter()
            .find(|ev| ev.time > selected_pt && !ev.pressed)
            .map(|ev| ev.time < ho.time)
            .unwrap_or(false);
    let pre_prev_short_fallb = false
        && ho.is_long_note()
        && ln_duration > w.hit100
        && prev_is_ln_stale
        && prev_was_miss
        && prev_break_pre
        && prev_dur_stale.map(|d| d <= w.hit100).unwrap_or(false)
        && calc_hit_kind((selected_pt - ho.time).abs(), w) == JudgmentKind::Miss
        && selected_pt < ho.time - w.hit100
        && prev_stale_time
            .zip(prev_end_stale)
            .map(|(prev_t, prev_end)| selected_pt >= prev_t && selected_pt < prev_end)
            .unwrap_or(false)
        && events
            .iter()
            .find(|ev| ev.time > selected_pt && !ev.pressed)
            .zip(prev_end_stale)
            .map(|(ev, prev_end)| {
                ev.time > prev_end && ev.time < ho.time && ev.time <= prev_end + w.hit300
            })
            .unwrap_or(false);
    let pre_strong_fallback = !ghost_prehead
        && false
        && ho.is_long_note()
        && ln_duration <= w.hit100
        && prev_was_miss
        && !prev_had_prewin_pen
        && selected_pt < ho.time
        && selected_pt >= ho.time - w.hit100
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale && selected_pt == prev_t + w.hit100
            })
            .unwrap_or(false)
        && matches!(
            calc_hit_kind((selected_pt - ho.time).abs(), w),
            JudgmentKind::Max | JudgmentKind::Hit300
        )
        && events
            .iter()
            .find(|ev| ev.time > selected_pt && !ev.pressed)
            .map(|ev| ev.time < ho.time)
            .unwrap_or(false)
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .map(|((_, next_ho), next_head_time)| {
                next_ho.is_long_note() && next_head_time - ho.time <= w.hit50 * 2
            })
            .unwrap_or(false);
    let pre_mis_h10_next_tap = !ghost_prehead
        && false
        && !ho.is_long_note()
        && prev_was_miss
        && !prev_had_prewin_pen
        && selected_pt < ho.time
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale && selected_pt == prev_t + w.hit100
            })
            .unwrap_or(false)
        && calc_hit_kind((selected_pt - ho.time).abs(), w) == JudgmentKind::Hit100
        && events
            .iter()
            .find(|ev| ev.time > selected_pt && !ev.pressed)
            .map(|ev| ev.time < ho.time)
            .unwrap_or(false)
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .map(|((_, next_ho), next_head_time)| {
                if next_ho.is_long_note() {
                    return false;
                }
                if next_head_time - ho.time > w.hit50 * 2 {
                    return false;
                }
                let Some(next_next_tap_head) =
                    col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                        (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                    })
                else {
                    return false;
                };
                presses
                    .iter()
                    .enumerate()
                    .skip(selected_idx + 1)
                    .take_while(|(_, cand)| **cand < next_head_time)
                    .find(|(_, cand)| {
                        let cand_pt = **cand;
                        cand_pt >= ho.time
                            && !reserved_ln_repr.contains(cand)
                            && matches!(
                                calc_hit_kind((cand_pt - ho.time).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300
                            )
                            && events
                                .iter()
                                .find(|ev| ev.time > cand_pt && !ev.pressed)
                                .map(|ev| ev.time < next_head_time)
                                .unwrap_or(false)
                    })
                    .map(|(fallback_idx, _)| {
                        let next_window_start = next_head_time - w.hit50;
                        let next_win_end = next_head_time + w.hit100;
                        presses
                            .iter()
                            .skip(fallback_idx + 1)
                            .take_while(|cand| **cand < next_win_end)
                            .any(|cand| {
                                let cand_pt = *cand;
                                cand_pt >= next_window_start
                                    && cand_pt < next_next_tap_head
                                    && !reserved_ln_repr.contains(cand)
                                    && events
                                        .iter()
                                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                                        .map(|ev| ev.time < next_next_tap_head + w.hit100)
                                        .unwrap_or(false)
                            })
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false);
    let pre_miss_h100_next_ln = !ghost_prehead
        && false
        && !ho.is_long_note()
        && prev_was_miss
        && !prev_had_prewin_pen
        && selected_pt < ho.time
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale && selected_pt == prev_t + w.hit100
            })
            .unwrap_or(false)
        && calc_hit_kind((selected_pt - ho.time).abs(), w) == JudgmentKind::Hit100
        && events
            .iter()
            .find(|ev| ev.time > selected_pt && !ev.pressed)
            .map(|ev| ev.time < ho.time)
            .unwrap_or(false)
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .map(|((_, next_ho), next_head_time)| {
                if !next_ho.is_long_note() {
                    return false;
                }
                if next_head_time - ho.time > w.hit50 * 2 {
                    return false;
                }
                presses
                    .iter()
                    .enumerate()
                    .skip(selected_idx + 1)
                    .take_while(|(_, cand)| **cand < next_head_time)
                    .find(|(_, cand)| {
                        let cand_pt = **cand;
                        cand_pt >= ho.time
                            && !reserved_ln_repr.contains(cand)
                            && matches!(
                                calc_hit_kind((cand_pt - ho.time).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300
                            )
                            && events
                                .iter()
                                .find(|ev| ev.time > cand_pt && !ev.pressed)
                                .map(|ev| ev.time < next_head_time)
                                .unwrap_or(false)
                    })
                    .map(|(fallback_idx, _)| {
                        let next_window_start = next_head_time - w.hit50;
                        let next_win_end = next_head_time + w.hit100;
                        let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                        let next_tail_start = next_end_time - w.hit50;
                        let next_tail_end = next_end_time + w.hit100;
                        presses
                            .iter()
                            .skip(fallback_idx + 1)
                            .take_while(|cand| **cand < next_win_end)
                            .any(|cand| {
                                let cand_pt = *cand;
                                cand_pt >= next_window_start
                                    && !reserved_ln_repr.contains(cand)
                                    && events
                                        .iter()
                                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                                        .map(|ev| {
                                            ev.time >= next_tail_start && ev.time < next_tail_end
                                        })
                                        .unwrap_or(false)
                            })
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false);
    let pre_miss_fallback = pre_miss_fallback || pre_mis_h10_next_tap || pre_miss_h100_next_ln;
    let pre_miss_pref_fallbc = !ghost_prehead
        && false
        && !ho.is_long_note()
        && prev_was_miss
        && !prev_had_prewin_pen
        && selected_pt < ho.time
        && selected_pt >= ho.time - w.hit100
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale && selected_pt == prev_t + w.hit100
            })
            .unwrap_or(false)
        && events
            .iter()
            .find(|ev| ev.time > selected_pt && !ev.pressed)
            .map(|ev| ev.time < ho.time)
            .unwrap_or(false);
    if pre_miss_pref_fallbc {
        let selected_kind = calc_hit_kind((selected_pt - ho.time).abs(), w);
        if let Some((fallback_idx, fallback_pt)) = presses
            .iter()
            .enumerate()
            .skip(selected_idx + 1)
            .take_while(|(_, cand)| **cand < ho.time)
            .find(|(_, cand)| {
                let cand_pt = **cand;
                let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                cand_kind.score_value() > selected_kind.score_value()
                    && next_note_time
                        .map(|nt| cand_pt < nt - w.hit50)
                        .unwrap_or(true)
                    && !reserved_ln_repr.contains(cand)
                    && events
                        .iter()
                        .find(|ev| ev.time > selected_pt && !ev.pressed)
                        .map(|ev| ev.time < cand_pt)
                        .unwrap_or(false)
                    && events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| {
                            next_note_time
                                .map(|nt| ev.time < nt + w.hit100)
                                .unwrap_or(true)
                        })
                        .unwrap_or(false)
            })
            .map(|(i, cand)| (i, *cand))
        {
            selected_pt = fallback_pt;
            selected_idx = fallback_idx;
        }
    }
    if pre_miss_fallback {
        if let Some((fallback_idx, fallback_pt)) = presses
            .iter()
            .enumerate()
            .skip(selected_idx + 1)
            .take_while(|(_, cand)| **cand < lock_end_exclusive)
            .find(|(_, cand)| {
                let cand_pt = **cand;
                cand_pt >= ho.time
                    && next_note_time.map(|nt| cand_pt < nt).unwrap_or(true)
                    && !reserved_ln_repr.contains(cand)
                    && events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| {
                            next_note_time
                                .map(|nt| ev.time < nt + w.hit100)
                                .unwrap_or(true)
                        })
                        .unwrap_or(false)
            })
            .map(|(i, cand)| (i, *cand))
        {
            selected_pt = fallback_pt;
            selected_idx = fallback_idx;
        }
    }
    if pre_miss_ln_fallback || pre_prev_short_fallb || pre_strong_fallback {
        let end_time = ho.end_time.unwrap_or(ho.time);
        let tail_start = end_time - w.hit50;
        let tail_end_exclusive = end_time + w.hit100;
        let selected_kind = calc_hit_kind((selected_pt - ho.time).abs(), w);
        if let Some((fallback_idx, fallback_pt)) = presses
            .iter()
            .enumerate()
            .skip(selected_idx + 1)
            .take_while(|(_, cand)| **cand < lock_end_exclusive)
            .find(|(fallback_idx, cand)| {
                let cand_pt = **cand;
                let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                let same_kind_short_pair = pre_strong_fallback
                    && cand_kind.score_value() == selected_kind.score_value()
                    && next_note_time
                        .zip(col_notes.get(note_pos + 1))
                        .map(|(next_head_time, (_, next_ho))| {
                            if !next_ho.is_long_note() {
                                return false;
                            }
                            let next_window_start = next_head_time - w.hit50;
                            let next_win_end = next_head_time + w.hit100;
                            let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                            let next_tail_start = next_end_time - w.hit50;
                            let next_tail_end = next_end_time + w.hit100;
                            presses
                                .iter()
                                .skip(*fallback_idx + 1)
                                .take_while(|cand| **cand < next_win_end)
                                .any(|cand| {
                                    let cand_pt = *cand;
                                    cand_pt >= next_window_start
                                        && !reserved_ln_repr.contains(cand)
                                        && events
                                            .iter()
                                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                                            .map(|ev| {
                                                ev.time >= next_tail_start
                                                    && ev.time < next_tail_end
                                            })
                                            .unwrap_or(false)
                                })
                        })
                        .unwrap_or(false);
                cand_pt >= ho.time
                    && cand_pt <= end_time
                    && (cand_kind.score_value() > selected_kind.score_value()
                        || same_kind_short_pair)
                    && next_note_time.map(|nt| cand_pt < nt).unwrap_or(true)
                    && !reserved_ln_repr.contains(cand)
                    && events
                        .iter()
                        .find(|ev| ev.time > selected_pt && !ev.pressed)
                        .map(|ev| ev.time < cand_pt)
                        .unwrap_or(false)
                    && events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| {
                            ev.time >= tail_start
                                && ev.time < tail_end_exclusive
                                && (!pre_strong_fallback
                                    || next_note_time.map(|nt| ev.time < nt).unwrap_or(true))
                                && next_note_time
                                    .map(|nt| ev.time < nt + w.hit100)
                                    .unwrap_or(true)
                        })
                        .unwrap_or(false)
            })
            .map(|(i, cand)| (i, *cand))
        {
            selected_pt = fallback_pt;
            selected_idx = fallback_idx;
        }
    }
    if pre_miss_ghost_late {
        if let Some((fallback_idx, fallback_pt)) = presses
            .iter()
            .enumerate()
            .skip(selected_idx + 1)
            .take_while(|(_, cand)| **cand < lock_end_exclusive)
            .find(|(fallback_idx, cand)| {
                let cand_pt = **cand;
                let rel_post_fallback = events
                    .iter()
                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let rel_pre_next_win_over = rel_post_fallback
                    .map(|rt| next_note_time.map(|nt| rt < nt + w.hit100).unwrap_or(true))
                    .unwrap_or(false);
                let rel_in_next_ln_tail = rel_post_fallback
                    .zip(col_notes.get(note_pos + 1))
                    .zip(next_note_time)
                    .map(|((rt, (_, next_ho)), next_head_time)| {
                        if !next_ho.is_long_note() || cand_pt >= next_head_time {
                            return false;
                        }
                        if next_head_time - ho.time > w.hit50 * 2 {
                            return false;
                        }
                        let bound_break_ln = events
                            .iter()
                            .find(|ev| ev.time > selected_pt && !ev.pressed)
                            .map(|ev| ev.time >= ho.time && ev.time < next_head_time)
                            .unwrap_or(false);
                        let next_next_note_time = col_notes
                            .get(note_pos + 2)
                            .map(|(_, next_next_ho)| next_next_ho.time);
                        let next_ln_late_end = next_next_note_time
                            .map(|next_time| next_time <= next_ho.time + w.hit50)
                            .unwrap_or(false);
                        let next_lock_end =
                            next_ho.time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                        let has_next_ln_follow = presses
                            .iter()
                            .skip(*fallback_idx + 1)
                            .take_while(|next_cand| **next_cand < next_lock_end)
                            .any(|next_cand| {
                                *next_cand >= next_head_time - w.hit50
                                    && !reserved_ln_repr.contains(next_cand)
                            });
                        let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                        let next_tail_start = next_end_time - w.hit50;
                        let next_tail_end = next_end_time + w.hit100;
                        bound_break_ln
                            && !has_next_ln_follow
                            && rt >= next_tail_start
                            && rt < next_tail_end
                    })
                    .unwrap_or(false);
                cand_pt >= ho.time + w.hit300
                    && next_note_time.map(|nt| cand_pt < nt).unwrap_or(true)
                    && !reserved_ln_repr.contains(cand)
                    && (rel_pre_next_win_over || rel_in_next_ln_tail)
            })
            .map(|(i, cand)| (i, *cand))
        {
            selected_pt = fallback_pt;
            selected_idx = fallback_idx;
            ghost_prehead = false;
        }
    }
    let shortish_h50_tail = ho.is_long_note()
        && (ln_duration <= w.hit50 + w.hit100
            || selected_pt < ho.end_time.unwrap_or(ho.time) - w.hit50)
        && selected_pt == ho.time + w.hit50
        && selected_pt <= ho.end_time.unwrap_or(ho.time)
        && next_note_time
            .map(|next_time| selected_pt < next_time - w.hit50)
            .unwrap_or(true)
        && events
            .iter()
            .find(|ev| ev.time > selected_pt && !ev.pressed)
            .map(|ev| {
                let end_time = ho.end_time.unwrap_or(ho.time);
                let tail_start = end_time - w.hit50;
                let tail_end_exclusive = end_time + w.hit100;
                ev.time >= tail_start && ev.time < tail_end_exclusive
            })
            .unwrap_or(false);
    let h50_nln = ho.is_long_note()
        && ln_duration > w.hit100
        && selected_pt == ho.time + w.hit50
        && selected_pt <= ho.end_time.unwrap_or(ho.time)
        && note_pos
            .checked_sub(1)
            .and_then(|p| col_notes.get(p))
            .map(|(_, p)| p.is_long_note())
            .unwrap_or(false)
        && col_notes
            .get(note_pos + 1)
            .map(|(_, n)| {
                let ne = n.end_time.unwrap_or(n.time);
                let nd = ne - n.time;
                let rt = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let mut fol = false;
                let mut post = false;
                for p in presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|p| **p < n.time + w.hit50)
                {
                    if reserved_ln_repr.contains(p) {
                        continue;
                    }
                    fol = true;
                    if *p > ne {
                        post = true;
                    }
                }
                let lon =
                    nd > w.hit100 && !fol && rt.map(|r| r > n.time && r <= ne).unwrap_or(false);
                n.is_long_note()
                    && selected_pt >= n.time - w.hit50
                    && selected_pt < n.time
                    && (post || lon)
            })
            .unwrap_or(false);
    let short_post_h50_stays = ho.is_long_note()
        && ln_duration <= w.hit100
        && calc_hit_kind((selected_pt - ho.time).abs(), w) == JudgmentKind::Hit50
        && col_notes
            .get(note_pos + 1)
            .map(|(_, next_ho)| {
                if !next_ho.is_long_note() {
                    return false;
                }
                let end_time = ho.end_time.unwrap_or(ho.time);
                let next_head = next_ho.time;
                let next_next_note_time = col_notes.get(note_pos + 2).map(|(_, ho)| ho.time);
                let next_late_end = next_next_note_time
                    .map(|next_time| next_time <= next_head + w.hit50)
                    .unwrap_or(false);
                let next_window_start = next_head - w.hit50;
                let next_win_end = next_head + w.hit50 + if next_late_end { 1 } else { 0 };
                let has_next_ln_follow = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next_win_end)
                    .any(|cand| *cand >= next_window_start && !reserved_ln_repr.contains(cand));
                selected_pt > end_time
                    && selected_pt <= end_time + w.hit50
                    && selected_pt >= next_window_start
                    && selected_pt < next_head
                    && !has_next_ln_follow
                    && events
                        .iter()
                        .find(|ev| ev.time > selected_pt && !ev.pressed)
                        .map(|ev| ev.time > end_time && ev.time < next_head)
                        .unwrap_or(false)
            })
            .unwrap_or(false);
    let exact_ln_end_stays = ho.is_long_note()
        && calc_hit_kind((selected_pt - ho.time).abs(), w) == JudgmentKind::Miss
        && col_notes
            .get(note_pos + 1)
            .map(|(_, next_ho)| {
                if !next_ho.is_long_note() {
                    return false;
                }
                let end_time = ho.end_time.unwrap_or(ho.time);
                let next_head = next_ho.time;
                let next_end = next_ho.end_time.unwrap_or(next_head);
                let next_next_note_time = col_notes.get(note_pos + 2).map(|(_, ho)| ho.time);
                let next_late_end = next_next_note_time
                    .map(|next_time| next_time <= next_head + w.hit50)
                    .unwrap_or(false);
                let next_window_start = next_head - w.hit50;
                let next_win_end = next_head + w.hit50 + if next_late_end { 1 } else { 0 };
                let has_next_ln_follow = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next_win_end)
                    .any(|cand| *cand >= next_window_start && !reserved_ln_repr.contains(cand));
                let ln_duration = end_time - ho.time;
                let rel_after_sel = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let had_earl_rel_pre_sel = events
                    .iter()
                    .any(|ev| !ev.pressed && ev.time > ho.time && ev.time < selected_pt);
                let exa_end_cla_stay_cur = selected_pt == end_time
                    && rel_after_sel.map(|rt| rt > next_end).unwrap_or(false);
                let near_end_repr_cur = ln_duration >= w.hit50 * 2
                    && selected_pt >= end_time - w.max
                    && selected_pt < end_time
                    && had_earl_rel_pre_sel
                    && rel_after_sel
                        .map(|rt| rt > next_head && rt <= next_end)
                        .unwrap_or(false);
                (exa_end_cla_stay_cur || near_end_repr_cur)
                    && selected_pt >= next_window_start
                    && selected_pt < next_head
                    && !has_next_ln_follow
            })
            .unwrap_or(false);
    if selected_pt >= window_start
        && (selected_pt < lock_end_exclusive
            || ln_claim_fallback
            || shortish_h50_tail
            || h50_nln
            || short_post_h50_stays
            || exact_ln_end_stays)
        && (!steals_next_ex || h50_nln)
        && !late_tap_cross_tap
        && !late_tap_dense_chain
        && !late_tap_iso_head
        && !late_tap_cross_ln
        && !ghost_prehead
        && !(!false
            && !ho.is_long_note()
            && Some(idx) == last_note_idx_overall
            && selected_pt > ho.time
            && extreme_ln_ends.contains(&ho.time))
        && !(false
            && !ho.is_long_note()
            && Some(idx) == last_note_idx_overall
            && selected_pt > ho.time
            && extreme_ln_ends.contains(&ho.time))
    {
        press_time = Some(selected_pt);
        if !tap_micro_keep_idx {
            press_idx = selected_idx + 1;
        }
    } else if ho.is_long_note() {
        if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
            if !next_ho.is_long_note() {
                let end_time = ho.end_time.unwrap_or(ho.time);
                let ln_duration = end_time - ho.time;
                let tail_start = end_time - w.hit50;
                let next_time = next_ho.time;
                let tail_bridge_end = end_time + w.hit50;
                let next_tap_window_start = next_time - w.hit50;
                let next_tap_end = next_time + w.hit100;
                let had_early_pre_tail = events
                    .iter()
                    .any(|ev| !ev.pressed && ev.time >= ho.time && ev.time < tail_start);
                let has_late_rel_in_tail = events
                    .iter()
                    .any(|ev| !ev.pressed && ev.time >= tail_start && ev.time <= end_time);
                let prehead_press_time = events
                    .iter()
                    .rev()
                    .find(|ev| ev.pressed && ev.time <= ho.time)
                    .map(|ev| ev.time);
                let prehead_rel_time = prehead_press_time.and_then(|prev_pt| {
                    events
                        .iter()
                        .find(|ev| ev.time > prev_pt && !ev.pressed)
                        .map(|ev| ev.time)
                });
                let pre_break_near_tail = prehead_rel_time
                    .map(|release_t| release_t > ho.time && release_t < tail_start)
                    .unwrap_or(false);
                let prehead_hold_ended = prehead_rel_time
                    .map(|release_t| release_t <= ho.time)
                    .unwrap_or(true);
                let first_rel_pt = events
                    .iter()
                    .find(|ev| ev.time > pt && !ev.pressed)
                    .map(|ev| ev.time);
                let med_lon_pos_end_tail = first_rel_pt
                    .map(|rt| rt > end_time && rt < end_time + w.hit100)
                    .unwrap_or(false);
                let has_rel_end_pt = events
                    .iter()
                    .any(|ev| !ev.pressed && ev.time > end_time && ev.time < pt);
                let fol_next_tap_pt = if press_idx + 1 < presses.len() {
                    let next_pt = presses[press_idx + 1];
                    (next_pt >= next_tap_window_start
                        && next_pt < next_tap_end
                        && !reserved_ln_repr.contains(&next_pt))
                    .then_some(next_pt)
                } else {
                    None
                };
                let has_next_tap_follow = fol_next_tap_pt.is_some();
                let short_ln_h50_claim = pt == ho.time + w.hit50;
                let shortish_h50_tap = short_ln_h50_claim
                    && pt < next_time
                    && first_rel_pt
                        .zip(fol_next_tap_pt)
                        .map(|(release_t, followup_pt)| {
                            release_t >= next_time && release_t < followup_pt
                        })
                        .unwrap_or(false);
                let sh_tap_fol = short_ln_h50_claim
                    && pt < next_time
                    && first_rel_pt
                        .map(|rt| rt > end_time && rt < next_time)
                        .unwrap_or(false)
                    && fol_next_tap_pt
                        .map(|fp| {
                            fp > next_time
                                && calc_hit_kind((fp - next_time).abs(), w) != JudgmentKind::Miss
                        })
                        .unwrap_or(false);
                let med_long_claim = ln_duration >= w.hit50 * 2
                    && ln_duration <= w.hit50 * 3 + w.max
                    && pt >= next_time - w.max
                    && pt <= next_time + w.max
                    && !has_rel_end_pt;
                let med_long_no_late =
                    med_long_claim && !has_late_rel_in_tail && (!false || med_lon_pos_end_tail);
                let post_end_notlc_claim = next_time <= tail_bridge_end
                    && pt > end_time
                    && pt <= tail_bridge_end
                    && pt >= next_tap_window_start
                    && pt < next_tap_end
                    && (short_ln_h50_claim || med_long_no_late)
                    && (!has_next_tap_follow || shortish_h50_tap || sh_tap_fol);
                let pos_end_ln_to_tap_cla = (had_early_pre_tail
                    && ln_duration >= w.hit50 * 2
                    && next_time <= tail_bridge_end
                    && pt >= end_time
                    && pt < next_time)
                    && (!false || !pre_break_near_tail)
                    || post_end_notlc_claim;
                let tail_rel_bridge = ln_duration >= w.hit50 * 2
                    && prehead_hold_ended
                    && calc_hit_kind((pt - ho.time).abs(), w) == JudgmentKind::Miss
                    && calc_hit_kind((pt - next_time).abs(), w) == JudgmentKind::Miss
                    && pt >= tail_start
                    && pt < end_time
                    && pt > tail_start + w.max + 8
                    && pt < next_tap_window_start
                    && next_tap_window_start - pt <= w.hit300
                    && next_time - end_time <= w.hit100
                    && first_rel_pt
                        .map(|release_t| {
                            release_t > end_time
                                && release_t < next_time
                                && release_t <= tail_bridge_end
                        })
                        .unwrap_or(false)
                    && fol_next_tap_pt
                        .zip(first_rel_pt)
                        .and_then(|(followup_pt, current_release)| {
                            if followup_pt <= current_release
                                || calc_hit_kind((followup_pt - next_time).abs(), w).score_value()
                                    < JudgmentKind::Hit300.score_value()
                            {
                                return None;
                            }
                            let followup_release = events
                                .iter()
                                .find(|ev| ev.time > followup_pt && !ev.pressed)
                                .map(|ev| ev.time)?;
                            let next_next_head =
                                col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                                    (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                                })?;
                            if followup_release >= next_next_head {
                                return None;
                            }
                            let next2_win_start = next_next_head - w.hit50;
                            let next2_win_end = next_next_head + w.hit100;
                            presses
                                .iter()
                                .skip(press_idx + 2)
                                .take_while(|cand| **cand < next2_win_end)
                                .find(|cand| {
                                    let cand_pt = **cand;
                                    cand_pt >= next2_win_start
                                        && cand_pt > followup_release
                                        && !reserved_ln_repr.contains(cand)
                                        && calc_hit_kind((cand_pt - next_next_head).abs(), w)
                                            .score_value()
                                            >= JudgmentKind::Hit300.score_value()
                                })
                                .map(|_| ())
                        })
                        .is_some();
                if pos_end_ln_to_tap_cla || tail_rel_bridge {
                    press_time = Some(pt);
                    press_idx += 1;
                }
            }
        }
        if press_time.is_none() {
            let end_time = ho.end_time.unwrap_or(ho.time);
            let ln_duration = end_time - ho.time;
            let tail_start = end_time - w.hit50;
            let tail_end_exclusive = end_time + w.hit100;
            let late_near_tail_start = pt >= tail_start && pt <= tail_start + w.max;
            let lat_nea_tai_sta_extn = !false
                && ln_duration >= w.hit50 * 2
                && pt >= tail_start
                && pt <= tail_start + w.max + 8;
            let ext_lat_nea_tai_star =
                !false && skipped_stale_prev && pt >= tail_start && pt <= tail_start + w.hit100;
            let nex_note_guar_ok_for = |cand_pt: i32| {
                next_note_time
                    .map(|next_time| cand_pt < next_time - w.hit50)
                    .unwrap_or(true)
            };
            let nex_not_guar_ok_base = nex_note_guar_ok_for(pt);
            let release_after_pt_time = events
                .iter()
                .find(|ev| ev.time > pt && !ev.pressed)
                .map(|ev| ev.time);
            let rel_in_tail_win = release_after_pt_time
                .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
                .unwrap_or(false);
            let ope_hol_thr_tail_win = !false
                && ln_duration >= w.hit50 * 2
                && pt > ho.time + w.hit50
                && pt < tail_start
                && release_after_pt_time
                    .map(|rt| rt >= tail_end_exclusive)
                    .unwrap_or(true);
            let rel_before_tail = release_after_pt_time
                .map(|rt| rt < tail_start)
                .unwrap_or(false);
            let has_late_body_tail = if rel_before_tail && ln_duration >= w.hit50 * 2 {
                let mut found = false;
                for next_i in (press_idx + 1)..presses.len() {
                    let cand_pt = presses[next_i];
                    if reserved_ln_repr.contains(&cand_pt) {
                        continue;
                    }
                    if cand_pt >= tail_start {
                        break;
                    }
                    if cand_pt <= ho.time + w.hit50 {
                        continue;
                    }
                    if !nex_note_guar_ok_for(cand_pt) {
                        break;
                    }
                    let cand_rel_in_tail_win1 = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time)
                        .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
                        .unwrap_or(false);
                    let can_hol_thr_tail_win = !false
                        && events
                            .iter()
                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                            .map(|ev| ev.time)
                            .map(|rt| rt >= tail_end_exclusive)
                            .unwrap_or(true);
                    if cand_rel_in_tail_win1 || can_hol_thr_tail_win {
                        found = true;
                        break;
                    }
                }
                found
            } else {
                false
            };
            let follow_repr_next_ln = if !false
                && rel_before_tail
                && ln_duration >= w.hit50 * 2
                && col_notes
                    .get(note_pos + 1)
                    .map(|(_, next_ho)| next_ho.is_long_note())
                    .unwrap_or(false)
            {
                let mut found = false;
                for next_i in (press_idx + 1)..presses.len() {
                    let cand_pt = presses[next_i];
                    if reserved_ln_repr.contains(&cand_pt) {
                        continue;
                    }
                    if cand_pt <= tail_start {
                        continue;
                    }
                    if next_note_time
                        .map(|next_time| cand_pt > next_time + w.max)
                        .unwrap_or(false)
                    {
                        break;
                    }
                    let cand_rel_rechs_tail = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time > cand_pt && ev.time >= tail_start)
                        .unwrap_or(false);
                    if cand_rel_rechs_tail {
                        found = true;
                        break;
                    }
                }
                found
            } else {
                false
            };
            let prehead_press_time = events
                .iter()
                .rev()
                .find(|ev| ev.pressed && ev.time <= ho.time)
                .map(|ev| ev.time);
            let prehead_first_release = prehead_press_time.and_then(|prev_pt| {
                events
                    .iter()
                    .find(|ev| ev.time > prev_pt && !ev.pressed)
                    .map(|ev| ev.time)
            });
            let pre_break_near_tail = prehead_first_release
                .map(|release_t| release_t > ho.time && release_t < tail_start)
                .unwrap_or(false);
            let pre_break_tail_start = prehead_first_release
                .map(|release_t| release_t > ho.time && release_t <= tail_start + w.max)
                .unwrap_or(false);
            let prehead_hold_ended = prehead_first_release
                .map(|release_t| release_t <= ho.time)
                .unwrap_or(true);
            let prev_repr_cross_head = false
                && note_pos
                    .checked_sub(1)
                    .and_then(|prev_pos| col_notes.get(prev_pos))
                    .map(|(_, prev_ho)| {
                        let prev_end = prev_ho.end_time.unwrap_or(prev_ho.time);
                        prev_ho.is_long_note()
                            && prev_end <= ho.time
                            && prehead_press_time
                                .map(|press_t| press_t < prev_end)
                                .unwrap_or(false)
                            && prehead_first_release
                                .map(|release_t| release_t > ho.time)
                                .unwrap_or(false)
                            && prehead_press_time
                                .map(|press_t| {
                                    let prev_head_win_start = prev_ho.time - w.hit50;
                                    events.iter().any(|ev| {
                                        !ev.pressed
                                            && ev.time >= prev_head_win_start
                                            && ev.time < press_t
                                    })
                                })
                                .unwrap_or(false)
                    })
                    .unwrap_or(false);
            let prev_cros_allws_tail = false
                && pt >= tail_start
                && note_pos
                    .checked_sub(1)
                    .and_then(|prev_pos| col_notes.get(prev_pos))
                    .map(|(_, prev_ho)| {
                        prev_ho.is_long_note()
                            && prev_ho.end_time.unwrap_or(prev_ho.time) <= ho.time
                    })
                    .unwrap_or(false)
                && prehead_first_release
                    .map(|release_t| {
                        release_t > ho.time && release_t <= ho.time + w.hit300 && release_t < pt
                    })
                    .unwrap_or(false);
            let post_end_clai_no_fol = if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
                if next_ho.is_long_note() && ln_duration >= w.hit50 * 2 {
                    let next_window_start = next_ho.time - w.hit50;
                    let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
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
                    let follow_post_end_miss = has_next_pt_fol
                        && col_notes
                            .get(note_pos + 2)
                            .map(|(_, next_next_ho)| {
                                let next_pt = presses[press_idx + 1];
                                let next_duration = next_end_time - next_ho.time;
                                let next_tail_end = next_end_time + w.hit100;
                                let next_followup_release = events
                                    .iter()
                                    .find(|ev| ev.time > next_pt && !ev.pressed)
                                    .map(|ev| ev.time);
                                next_duration <= w.hit100
                                    && next_pt > next_end_time
                                    && next_pt < next_next_ho.time
                                    && next_followup_release
                                        .map(|rt| rt >= next_tail_end)
                                        .unwrap_or(true)
                            })
                            .unwrap_or(false);
                    let pt_in_post_miss = pt > end_time && pt <= end_time + w.hit50;
                    let next_note_auto_miss = next_ho.time <= end_time + w.hit50;
                    let pt_would_feed_next_ln = pt >= next_window_start && pt < next_lock_end;
                    let post_end_to_next_ln = false
                        && pt_would_feed_next_ln
                        && release_after_pt_time
                            .map(|rt| {
                                let next_tail_start = next_end_time - w.hit50;
                                let next_tail_end = next_end_time + w.hit100;
                                rt >= next_tail_start && rt < next_tail_end
                            })
                            .unwrap_or(false);
                    let next_short_parasitic = prehead_hold_ended
                        && !has_next_pt_fol
                        && next_end_time - next_ho.time <= w.hit50
                        && pt >= next_ho.time
                        && pt < next_end_time
                        && release_after_pt_time
                            .map(|rt| rt > next_ho.time && rt < next_end_time)
                            .unwrap_or(false)
                        && presses
                            .iter()
                            .skip(selected_idx + 1)
                            .find(|cand| !reserved_ln_repr.contains(cand))
                            .map(|cand| *cand > next_end_time)
                            .unwrap_or(false);
                    pt_in_post_miss
                        && next_note_auto_miss
                        && pt_would_feed_next_ln
                        && (pre_break_near_tail || next_short_parasitic)
                        && (!has_next_pt_fol || follow_post_end_miss)
                        && !post_end_to_next_ln
                } else {
                    false
                }
            } else {
                false
            };
            let short_ln_tail_end = end_time + w.hit50 + w.max;
            let tail_only_start = end_time - ((w.hit50 as f32) * 1.5).round() as i32;
            let tail_only_tail_end = end_time + ((w.hit100 as f32) * 1.5).round() as i32;
            let pre_break_before_cand = false
                && prehead_first_release
                    .map(|release_t| {
                        release_t > ho.time
                            && release_t >= tail_only_start
                            && release_t < pt
                            && pt - release_t <= w.hit50
                    })
                    .unwrap_or(false);
            let short_post_end_claim = false
                && ln_duration <= w.hit100
                && pt > end_time
                && pt <= short_ln_tail_end
                && release_after_pt_time
                    .map(|rt| rt > end_time && rt <= short_ln_tail_end)
                    .unwrap_or(false);
            let tail_claim_base = col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| {
                    if !next_ho.is_long_note() {
                        return false;
                    }
                    let next_duration = next_ho.end_time.unwrap_or(next_ho.time) - next_ho.time;
                    let next_window_start = next_ho.time - w.hit50;
                    next_duration <= w.hit50
                        && ln_duration >= w.hit50 + w.max
                        && pre_break_tail_start
                        && pt > ho.time + w.hit50
                        && release_after_pt_time
                            .map(|rt| rt >= next_window_start && rt < next_ho.time)
                            .unwrap_or(false)
                        && release_after_pt_time
                            .map(|rt| rt < end_time - w.max)
                            .unwrap_or(false)
                        && pt <= end_time
                })
                .unwrap_or(false);
            let headless_claim_h50 = tail_claim_base && prehead_hold_ended;
            let hless_tal100_next_ln = tail_claim_base
                && prehead_hold_ended
                && ln_duration <= w.hit50 + w.hit100
                && pt >= tail_start
                && col_notes
                    .get(note_pos + 1)
                    .map(|(_, next_ho)| {
                        if !next_ho.is_long_note() {
                            return false;
                        }
                        let next_head = next_ho.time;
                        let next_end = next_ho.end_time.unwrap_or(next_head);
                        let next_tail_start = next_end - w.hit50;
                        let next_window_start = next_head - w.hit50;
                        let next_next_note_time =
                            col_notes.get(note_pos + 2).map(|(_, ho)| ho.time);
                        let next_late_end = next_next_note_time
                            .map(|next_time| next_time <= next_head + w.hit50)
                            .unwrap_or(false);
                        let next_win_end = next_head + w.hit50 + if next_late_end { 1 } else { 0 };
                        let has_next_ln_follow = presses
                            .iter()
                            .skip(press_idx + 1)
                            .take_while(|cand| **cand < next_win_end)
                            .any(|cand| {
                                *cand >= next_window_start && !reserved_ln_repr.contains(cand)
                            });
                        next_head - end_time <= w.hit50
                            && pt < next_head
                            && release_after_pt_time
                                .map(|rt| {
                                    rt > next_head
                                        && rt < next_tail_start
                                        && rt <= tail_only_tail_end
                                })
                                .unwrap_or(false)
                            && !has_next_ln_follow
                    })
                    .unwrap_or(false);
            let tail_claim_post_end = tail_claim_base
                && !prehead_hold_ended
                && pre_break_near_tail
                && prehead_first_release
                    .map(|release_t| pt > release_t)
                    .unwrap_or(false)
                && pt < tail_start
                && release_after_pt_time
                    .map(|rt| {
                        rt > end_time
                            && rt <= end_time + w.hit50
                            && next_note_time
                                .map(|next_time| rt < next_time - w.hit50)
                                .unwrap_or(true)
                    })
                    .unwrap_or(false);
            let tail_claim_next_ln = tail_claim_base
                && pre_break_before_cand
                && pt == ho.time + w.hit50
                && !nex_not_guar_ok_base
                && col_notes
                    .get(note_pos + 1)
                    .map(|(_, next_ho)| {
                        if !next_ho.is_long_note() {
                            return false;
                        }
                        let next_head = next_ho.time;
                        let next_end = next_ho.end_time.unwrap_or(next_head);
                        let next_window_start = next_head - w.hit50;
                        let next_win_end = next_head + w.hit100;
                        let has_next_ln_follow = presses
                            .iter()
                            .skip(press_idx + 1)
                            .take_while(|cand| **cand < next_win_end)
                            .any(|cand| {
                                *cand >= next_window_start && !reserved_ln_repr.contains(cand)
                            });
                        pt >= next_window_start
                            && pt < next_head
                            && release_after_pt_time
                                .map(|rt| {
                                    rt > next_head && rt <= next_end && rt <= tail_only_tail_end
                                })
                                .unwrap_or(false)
                            && !has_next_ln_follow
                    })
                    .unwrap_or(false);
            let tail_rel_next_ln = tail_claim_base
                && !prev_cros_allws_tail
                && pt >= tail_start
                && col_notes
                    .get(note_pos + 1)
                    .map(|(_, next_ho)| {
                        if !next_ho.is_long_note() {
                            return false;
                        }
                        let next_head = next_ho.time;
                        let next_end = next_ho.end_time.unwrap_or(next_ho.time);
                        let next_ln_duration = next_end - next_head;
                        let next_window_start = next_head - w.hit50;
                        let next_win_end = next_head + w.hit100;
                        let next_tail_start = next_end - w.hit50;
                        let next_tail_end = next_end + w.hit100;
                        let next_ln_follow = presses
                            .iter()
                            .skip(press_idx + 1)
                            .take_while(|cand| **cand < next_win_end)
                            .find(|cand| {
                                **cand >= next_window_start && !reserved_ln_repr.contains(cand)
                            })
                            .copied();
                        release_after_pt_time
                            .map(|rt| {
                                let next_ln_self_fol = next_ln_follow
                                    .map(|followup_pt| {
                                        let fol_rel_in_tail = events
                                            .iter()
                                            .find(|ev| ev.time > followup_pt && !ev.pressed)
                                            .map(|ev| {
                                                ev.time >= next_tail_start
                                                    && ev.time < next_tail_end
                                            })
                                            .unwrap_or(false);
                                        fol_rel_in_tail
                                            && ((next_ln_duration > w.hit100 && followup_pt > rt)
                                                || (next_ln_duration <= w.hit100
                                                    && rt <= end_time
                                                    && rt < next_head
                                                    && followup_pt >= next_head))
                                    })
                                    .unwrap_or(false);
                                let next_short_has_prhd = presses
                                    .iter()
                                    .skip(press_idx + 1)
                                    .take_while(|cand| **cand < next_head)
                                    .any(|cand| *cand > pt && !reserved_ln_repr.contains(cand));
                                let nex_sho_ln_self_pair = next_ln_duration <= w.hit100
                                    && pt >= next_window_start - w.hit300
                                    && pt < next_head
                                    && rt > pt
                                    && rt < next_head
                                    && rt < end_time - w.max
                                    && next_ln_self_fol
                                    && !next_short_has_prhd;
                                (pt < next_window_start - w.hit300
                                    && rt > pt
                                    && rt > next_window_start
                                    && rt < next_head
                                    && next_ln_follow.is_some()
                                    && !next_ln_self_fol)
                                    || nex_sho_ln_self_pair
                            })
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
            let tail_rel_tap_pair = tail_claim_base
                && col_notes
                    .get(note_pos + 1)
                    .zip(next_note_time)
                    .map(|((_, next_ho), next_head)| {
                        if next_ho.is_long_note() {
                            return false;
                        }
                        let next_window_start = next_head - w.hit50;
                        let next_penalty_start = next_window_start - early_penalty_window - 1;
                        let next_win_end = next_head + w.hit100;
                        let pre_bre_int_next_tap = release_after_pt_time
                            .map(|rt| {
                                let has_nex_tap_post_rel = presses
                                    .iter()
                                    .skip(press_idx + 1)
                                    .take_while(|cand| **cand < next_win_end)
                                    .any(|cand| {
                                        *cand > rt
                                            && *cand >= next_window_start
                                            && !reserved_ln_repr.contains(cand)
                                    });
                                pre_break_before_cand
                                    && pt < next_penalty_start
                                    && rt > pt
                                    && rt <= end_time + w.max
                                    && rt >= next_window_start
                                    && rt < next_head
                                    && has_nex_tap_post_rel
                            })
                            .unwrap_or(false);
                        if pre_bre_int_next_tap {
                            return true;
                        }
                        let Some((_, next_next_ho)) = col_notes.get(note_pos + 2) else {
                            return false;
                        };
                        if !next_next_ho.is_long_note() {
                            return false;
                        }
                        let next_next_head = next_next_ho.time;
                        let next2_win_start = next_next_head - w.hit50;
                        let next2_win_end = next_next_head + w.hit100;
                        let next_next_end = next_next_ho.end_time.unwrap_or(next_next_head);
                        let next_next_duration = next_next_end - next_next_head;
                        let next_next_tail_start = next_next_end - w.hit50;
                        let next2_tail_end = next_next_end + w.hit100;
                        let next2_has_self_pair = next_next_duration <= w.hit100
                            && presses
                                .iter()
                                .skip(press_idx + 1)
                                .take_while(|cand| **cand < next2_win_end)
                                .any(|cand| {
                                    let cand_pt = *cand;
                                    cand_pt >= next2_win_start
                                        && !reserved_ln_repr.contains(cand)
                                        && events
                                            .iter()
                                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                                            .map(|ev| {
                                                ev.time >= next_next_tail_start
                                                    && ev.time < next2_tail_end
                                            })
                                            .unwrap_or(false)
                                });
                        pt >= next_penalty_start
                            && pt < next_window_start
                            && release_after_pt_time
                                .map(|rt| {
                                    let has_nex_tap_post_rel = presses
                                        .iter()
                                        .skip(press_idx + 1)
                                        .take_while(|cand| **cand < next_win_end)
                                        .any(|cand| {
                                            *cand > rt
                                                && *cand >= next_window_start
                                                && !reserved_ln_repr.contains(cand)
                                        });
                                    let tight_tap_short_gap =
                                        next_next_head - next_head <= w.hit100;
                                    rt > pt
                                        && rt < next_head
                                        && !(has_nex_tap_post_rel && tight_tap_short_gap)
                                })
                                .unwrap_or(false)
                            && next2_has_self_pair
                    })
                    .unwrap_or(false);
            let miss_body_tail_claim = tail_claim_base
                && !tail_rel_next_ln
                && !tail_rel_tap_pair
                && release_after_pt_time
                    .map(|rt| {
                        rt >= tail_only_start
                            && (rt <= end_time + w.hit100
                                || (headless_claim_h50 && rt <= end_time + w.hit50)
                                || hless_tal100_next_ln
                                || tail_claim_post_end
                                || tail_claim_next_ln)
                    })
                    .unwrap_or(false);
            let next_note_guard_ok = nex_not_guar_ok_base || tail_claim_next_ln;
            let tail_overlap_tap = if !false && ln_duration >= w.hit50 * 2 {
                if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
                    if next_ho.is_long_note() {
                        false
                    } else {
                        let next_tap_window_start = next_ho.time - w.hit50;
                        let next_tap_end = next_ho.time + w.hit100;
                        let has_next_tap_follow = press_idx + 1 < presses.len() && {
                            let next_pt = presses[press_idx + 1];
                            next_pt >= next_tap_window_start
                                && next_pt < next_tap_end
                                && !reserved_ln_repr.contains(&next_pt)
                        };
                        let near_next_tap_left = pt >= next_tap_window_start
                            || (pt < next_tap_window_start
                                && next_tap_window_start - pt <= w.hit300
                                && pt > tail_start + w.max + 8
                                && pre_break_near_tail);
                        let rel_before_next_head = release_after_pt_time
                            .map(|rt| rt <= next_ho.time)
                            .unwrap_or(false);
                        pt > ho.time + w.hit50
                            && pt >= tail_start
                            && pt < end_time
                            && near_next_tap_left
                            && pt < next_ho.time
                            && rel_in_tail_win
                            && rel_before_next_head
                            && has_next_tap_follow
                    }
                } else {
                    false
                }
            } else {
                false
            };
            let tail_overlap_ln = if !false && ln_duration >= w.hit50 * 2 {
                if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
                    if !next_ho.is_long_note() {
                        false
                    } else {
                        let next_head = next_ho.time;
                        let next_end = next_ho.end_time.unwrap_or(next_head);
                        let next_window_start = next_head - w.hit50;
                        let next_next_note_time =
                            col_notes.get(note_pos + 2).map(|(_, ho)| ho.time);
                        let next_late_end = next_next_note_time
                            .map(|next_time| next_time <= next_head + w.hit50)
                            .unwrap_or(false);
                        let next_win_end = next_head + w.hit50 + if next_late_end { 1 } else { 0 };
                        let next_tail_start = next_end - w.hit50;
                        let next_tail_end = next_end + w.hit100;
                        let near_next_ln_left = pt >= next_window_start
                            || (pt < next_window_start
                                && next_window_start - pt <= w.hit300
                                && pt > tail_start + w.max + 8);
                        pre_break_near_tail
                            && pt >= tail_start
                            && pt <= end_time
                            && next_head - end_time <= w.hit50
                            && near_next_ln_left
                            && pt < next_head
                            && release_after_pt_time
                                .map(|rt| {
                                    let next_ln_follow = presses
                                        .iter()
                                        .skip(selected_idx + 1)
                                        .take_while(|cand| **cand < next_win_end)
                                        .find(|cand| {
                                            **cand > rt
                                                && **cand >= next_window_start
                                                && !reserved_ln_repr.contains(cand)
                                        })
                                        .copied();
                                    rt > pt
                                        && rt < next_head
                                        && next_ln_follow
                                            .map(|followup_pt| {
                                                events
                                                    .iter()
                                                    .find(|ev| ev.time > followup_pt && !ev.pressed)
                                                    .map(|ev| {
                                                        ev.time >= next_tail_start
                                                            && ev.time < next_tail_end
                                                    })
                                                    .unwrap_or(false)
                                            })
                                            .unwrap_or(false)
                                })
                                .unwrap_or(false)
                    }
                } else {
                    false
                }
            } else {
                false
            };
            let short_post_end_bridge = if !false && ln_duration <= w.hit100 + w.max {
                if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
                    pt > end_time
                        && pt <= end_time + w.max
                        && pt < next_ho.time
                        && next_ho.is_long_note()
                        && release_after_pt_time
                            .map(|rt| {
                                rt > end_time && rt <= tail_end_exclusive && rt < next_ho.time
                            })
                            .unwrap_or(false)
                } else {
                    false
                }
            } else {
                false
            };
            let sho_post_end_miss_ln = if !false && ln_duration <= w.hit100 {
                if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
                    if next_ho.is_long_note() && pt > end_time && pt <= end_time + w.hit50 {
                        let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                        let next_window_start = next_ho.time - w.hit50;
                        let next_win_end = next_ho.time + w.hit100;
                        let next_tail_start = next_end_time - w.hit50;
                        let next_tail_end = next_end_time + w.hit100;
                        let sta_rel_pre_nex_tail = release_after_pt_time
                            .map(|rt| rt > next_ho.time && rt < next_tail_start)
                            .unwrap_or(false);
                        let next_ln_head_tail = presses
                            .iter()
                            .skip(press_idx + 1)
                            .take_while(|cand| **cand < next_win_end)
                            .find(|cand| {
                                **cand >= next_window_start && !reserved_ln_repr.contains(cand)
                            })
                            .copied()
                            .and_then(|followup_pt| {
                                events
                                    .iter()
                                    .find(|ev| ev.time > followup_pt && !ev.pressed)
                                    .map(|ev| (followup_pt, ev.time))
                            })
                            .map(|(followup_pt, followup_release)| {
                                release_after_pt_time
                                    .map(|stale_release| stale_release < followup_pt)
                                    .unwrap_or(false)
                                    && followup_release >= next_tail_start
                                    && followup_release < next_tail_end
                            })
                            .unwrap_or(false);
                        pt < next_window_start && sta_rel_pre_nex_tail && next_ln_head_tail
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };
            let sho_pos_end_miss_win = if !false && ln_duration <= w.hit100 {
                if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
                    if next_ho.is_long_note() {
                        let next_window_start = next_ho.time - w.hit50;
                        let next_next_note_time = col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                        let next_ln_late_end = next_next_note_time
                            .map(|next_time| next_time <= next_ho.time + w.hit50)
                            .unwrap_or(false);
                        let next_lock_end =
                            next_ho.time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                        let next_ln_late_head = presses
                            .iter()
                            .skip(press_idx + 1)
                            .take_while(|cand| **cand < next_lock_end)
                            .any(|cand| {
                                *cand >= next_window_start && !reserved_ln_repr.contains(cand)
                            });
                        pt > end_time
                            && pt <= ho.time + w.hit50
                            && pt >= next_window_start
                            && pt < next_ho.time
                            && next_ln_late_head
                            && release_after_pt_time
                                .map(|rt| rt > next_ho.time)
                                .unwrap_or(false)
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };
            let tail_claim_exact_h50 = miss_body_tail_claim && pt == ho.time + w.hit50;
            let short_post_end_h50 = short_post_end_claim && pt == ho.time + w.hit50;
            let late_miss_keeps_meta = true
                && ln_duration >= w.hit50 * 2 + w.max
                && pt > ho.time + w.hit50
                && pt < tail_start
                && nex_not_guar_ok_base
                && !rel_in_tail_win
                && !has_late_body_tail
                && !follow_repr_next_ln
                && prehead_hold_ended
                && release_after_pt_time
                    .map(|rt| {
                        rt > pt
                            && rt < tail_start
                            && tail_start - rt <= w.max / 2
                            && next_note_time
                                .map(|next_time| rt < next_time - w.hit50)
                                .unwrap_or(true)
                    })
                    .unwrap_or(false);
            if ((pt > ho.time + w.hit50 || tail_claim_exact_h50 || short_post_end_h50)
                && (pt < tail_start
                    || (late_near_tail_start && ln_duration >= w.hit50 * 2)
                    || lat_nea_tai_sta_extn
                    || (ext_lat_nea_tai_star && ln_duration >= w.hit50 * 2)
                    || short_post_end_claim
                    || miss_body_tail_claim)
                && next_note_guard_ok
                && (rel_in_tail_win
                    || ope_hol_thr_tail_win
                    || has_late_body_tail
                    || follow_repr_next_ln
                    || short_post_end_claim
                    || miss_body_tail_claim))
                || post_end_clai_no_fol
                || tail_overlap_tap
                || tail_overlap_ln
                || short_post_end_bridge
                || sho_post_end_miss_ln
                || sho_pos_end_miss_win
                || late_miss_keeps_meta
            {
                press_time = Some(pt);
                press_idx += 1;
            }
            if false && press_time.is_none() && tail_only_pt.is_none() {
                let short_ln_dur = ln_duration <= w.hit50 + w.hit100;
                let mid_ln_dur =
                    ln_duration > w.hit50 + w.hit100 && ln_duration < w.hit50 * 2 + w.max;
                let post_end_tail_cap = end_time + w.hit50 + w.hit100;
                if short_ln_dur || mid_ln_dur {
                    let mut rec_tail_only_cand: Option<(usize, i32)> = None;
                    for cand_idx in press_idx..presses.len() {
                        let cand_pt = presses[cand_idx];
                        if reserved_ln_repr.contains(&cand_pt) {
                            continue;
                        }
                        let cand_release_after = events
                            .iter()
                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                            .map(|ev| ev.time);
                        let prev_miss_short_rec = prev_miss_keeps_hless
                            && cand_pt > ho.time + w.hit100
                            && cand_pt < end_time
                            && cand_release_after
                                .map(|rt| {
                                    rt > end_time
                                        && rt <= post_end_tail_cap
                                        && next_note_time
                                            .map(|next_time| rt < next_time)
                                            .unwrap_or(true)
                                })
                                .unwrap_or(false);
                        let hless_short_next_tap = false
                            && cand_pt > ho.time + w.hit100
                            && cand_pt < end_time
                            && col_notes
                                .get(note_pos + 1)
                                .map(|(_, next_ho)| {
                                    if next_ho.is_long_note() {
                                        return false;
                                    }
                                    let next_head = next_ho.time;
                                    let next_window_start = next_head - w.hit50;
                                    let next_win_end = next_head + w.hit100;
                                    let nex_gap_from_cur_end = next_head - end_time;
                                    let has_next_tap_follow = presses
                                        .iter()
                                        .skip(cand_idx + 1)
                                        .take_while(|cand| **cand < next_win_end)
                                        .any(|cand| {
                                            *cand >= next_window_start
                                                && !reserved_ln_repr.contains(cand)
                                        });
                                    cand_release_after
                                        .map(|rt| {
                                            rt > end_time
                                                && rt < next_head
                                                && rt <= post_end_tail_cap
                                        })
                                        .unwrap_or(false)
                                        && cand_pt >= end_time - w.hit300
                                        && cand_pt < next_window_start
                                        && nex_gap_from_cur_end <= w.hit50
                                        && has_next_tap_follow
                                })
                                .unwrap_or(false);
                        let headless_body_next_ln = false
                            && cand_pt > ho.time + w.hit100
                            && cand_pt <= end_time
                            && col_notes
                                .get(note_pos + 1)
                                .map(|(_, next_ho)| {
                                    if !next_ho.is_long_note() {
                                        return false;
                                    }
                                    let next_head = next_ho.time;
                                    let next_window_start = next_head - w.hit50;
                                    let next_prehead_cand = presses
                                        .iter()
                                        .skip(cand_idx + 1)
                                        .take_while(|cand| **cand < next_head)
                                        .find(|cand| {
                                            **cand >= next_window_start
                                                && !reserved_ln_repr.contains(cand)
                                        })
                                        .copied();
                                    cand_release_after
                                        .map(|rt| {
                                            rt > end_time
                                                && rt < next_head
                                                && rt <= post_end_tail_cap
                                                && next_prehead_cand
                                                    .map(|next_pt| {
                                                        next_pt > rt
                                                            && events
                                                                .iter()
                                                                .find(|ev| {
                                                                    ev.time > next_pt && !ev.pressed
                                                                })
                                                                .map(|ev| {
                                                                    ev.time > next_pt
                                                                        && ev.time < next_head
                                                                })
                                                                .unwrap_or(false)
                                                    })
                                                    .unwrap_or(false)
                                        })
                                        .unwrap_or(false)
                                })
                                .unwrap_or(false);
                        if cand_pt <= end_time
                            && !prev_miss_short_rec
                            && !hless_short_next_tap
                            && !headless_body_next_ln
                        {
                            continue;
                        }
                        if cand_pt > end_time + w.hit50 {
                            break;
                        }
                        let shortish_post_end_rec = short_ln_dur
                            && !prev_repr_cross_head
                            && events.iter().any(|ev| {
                                !ev.pressed
                                    && ev.time > ho.time
                                    && ev.time <= end_time
                                    && ev.time < cand_pt
                            });
                        let mid_post_end_pre_next = mid_ln_dur
                            && (prehead_hold_ended
                                || (pre_break_tail_start && !prev_repr_cross_head))
                            && prehead_first_release
                                .map(|release_t| cand_pt > release_t)
                                .unwrap_or(true)
                            && cand_release_after
                                .map(|rt| {
                                    next_note_time
                                        .map(|next_time| rt < next_time)
                                        .unwrap_or(true)
                                })
                                .unwrap_or(false);
                        let short_post_end_self = false
                            && short_ln_dur
                            && ln_duration <= w.hit100 + w.max
                            && prev_repr_cross_head
                            && prehead_first_release
                                .map(|release_t| release_t > ho.time && cand_pt > release_t)
                                .unwrap_or(false)
                            && col_notes
                                .get(note_pos + 1)
                                .map(|(_, next_ho)| {
                                    if !next_ho.is_long_note() {
                                        return false;
                                    }
                                    let next_head = next_ho.time;
                                    let next_end = next_ho.end_time.unwrap_or(next_ho.time);
                                    let next_window_start = next_head - w.hit50;
                                    let next_win_end = next_head + w.hit100;
                                    let next_tail_start = next_end - w.hit50;
                                    let next_tail_end = next_end + w.hit100;
                                    let next_ln_self_fol = presses
                                        .iter()
                                        .skip(cand_idx + 1)
                                        .take_while(|cand| **cand < next_win_end)
                                        .any(|cand| {
                                            *cand >= next_window_start
                                                && !reserved_ln_repr.contains(cand)
                                                && events
                                                    .iter()
                                                    .find(|ev| ev.time > *cand && !ev.pressed)
                                                    .map(|ev| {
                                                        ev.time >= next_tail_start
                                                            && ev.time < next_tail_end
                                                    })
                                                    .unwrap_or(false)
                                        });
                                    cand_release_after
                                        .map(|rt| {
                                            rt > end_time
                                                && rt < next_head
                                                && rt <= post_end_tail_cap
                                        })
                                        .unwrap_or(false)
                                        && next_ln_self_fol
                                })
                                .unwrap_or(false);
                        let hle_post_end_next_ln = false
                            && prehead_hold_ended
                            && col_notes
                                .get(note_pos + 1)
                                .map(|(_, next_ho)| {
                                    if !next_ho.is_long_note() {
                                        return false;
                                    }
                                    let next_head = next_ho.time;
                                    let next_end = next_ho.end_time.unwrap_or(next_ho.time);
                                    let next_ln_duration = next_end - next_ho.time;
                                    let next_window_start = next_head - w.hit50;
                                    let next_next_note_time =
                                        col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                                    let next_ln_late_end = next_next_note_time
                                        .map(|next_time| next_time <= next_head + w.hit50)
                                        .unwrap_or(false);
                                    let next_lock_end =
                                        next_head + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                                    let has_next_ln_follow = presses
                                        .iter()
                                        .skip(cand_idx + 1)
                                        .take_while(|cand| **cand < next_lock_end)
                                        .any(|cand| {
                                            *cand >= next_window_start
                                                && !reserved_ln_repr.contains(cand)
                                        });
                                    let rel_sta_pre_nex_head = cand_release_after
                                        .map(|rt| {
                                            rt > end_time
                                                && rt < next_head
                                                && rt <= post_end_tail_cap
                                        })
                                        .unwrap_or(false);
                                    let rel_shl_cro_nex_head = cand_release_after
                                        .map(|rt| {
                                            rt > next_head
                                                && rt < next_end
                                                && rt <= next_head + w.hit300
                                                && rt <= post_end_tail_cap
                                                && cand_pt <= end_time + w.hit300
                                                && next_head - cand_pt > rt - next_head
                                        })
                                        .unwrap_or(false);
                                    ln_duration > w.hit300 + w.max
                                        && next_ln_duration > w.hit300 + w.max
                                        && next_ln_duration <= w.hit50 + w.hit100
                                        && cand_pt >= next_head - w.hit100
                                        && cand_pt < next_head
                                        && (rel_sta_pre_nex_head || rel_shl_cro_nex_head)
                                        && !has_next_ln_follow
                                })
                                .unwrap_or(false);
                        let hless_post_end_miss = false
                            && short_ln_dur
                            && ln_duration <= w.hit100
                            && prehead_hold_ended
                            && !prev_repr_cross_head
                            && cand_pt == ho.time + w.hit50
                            && col_notes
                                .get(note_pos + 1)
                                .zip(col_notes.get(note_pos + 2))
                                .map(|((_, next_ho), (_, next_next_ho))| {
                                    if !next_ho.is_long_note() {
                                        return false;
                                    }
                                    let next_head = next_ho.time;
                                    let next_end = next_ho.end_time.unwrap_or(next_ho.time);
                                    let next_duration = next_end - next_head;
                                    let next_window_start = next_head - w.hit50;
                                    let next_window_end = next_head + w.hit100;
                                    let next_late_end = next_next_ho.time <= next_head + w.hit50;
                                    let next_lock_end =
                                        next_head + w.hit50 + if next_late_end { 1 } else { 0 };
                                    let next_tail_start = next_end - w.hit50;
                                    let next_tail_end = next_end + w.hit100;
                                    if next_duration > w.hit100
                                        || cand_pt < next_head - w.max
                                        || cand_pt >= next_head
                                    {
                                        return false;
                                    }
                                    cand_release_after
                                        .filter(|rt| {
                                            *rt > next_head
                                                && *rt < next_end
                                                && *rt < next_next_ho.time
                                                && next_end - *rt <= w.max
                                                && *rt <= post_end_tail_cap
                                        })
                                        .map(|rt| {
                                            let nex_has_stn_sel_pair = presses
                                                .iter()
                                                .skip(cand_idx + 1)
                                                .take_while(|cand| **cand < next_window_end)
                                                .any(|cand| {
                                                    let followup_pt = *cand;
                                                    followup_pt >= next_window_start
                                                        && !reserved_ln_repr.contains(cand)
                                                        && events
                                                            .iter()
                                                            .find(|ev| {
                                                                ev.time > followup_pt && !ev.pressed
                                                            })
                                                            .map(|ev| {
                                                                ev.time >= next_tail_start
                                                                    && ev.time < next_tail_end
                                                            })
                                                            .unwrap_or(false)
                                                });
                                            let next_has_late_press = presses
                                                .iter()
                                                .skip(cand_idx + 1)
                                                .take_while(|cand| **cand < next_lock_end)
                                                .find(|cand| {
                                                    let late_pt = **cand;
                                                    late_pt >= next_window_end
                                                        && late_pt < next_next_ho.time
                                                        && late_pt > rt
                                                        && !reserved_ln_repr.contains(cand)
                                                })
                                                .map(|cand| {
                                                    let late_pt = *cand;
                                                    events
                                                        .iter()
                                                        .find(|ev| ev.time > late_pt && !ev.pressed)
                                                        .map(|ev| {
                                                            ev.time < next_tail_start
                                                                || ev.time >= next_tail_end
                                                        })
                                                        .unwrap_or(true)
                                                })
                                                .unwrap_or(false);
                                            !nex_has_stn_sel_pair && next_has_late_press
                                        })
                                        .unwrap_or(false)
                                })
                                .unwrap_or(false);
                        let short_post_end_next = false
                            && short_ln_dur
                            && col_notes
                                .get(note_pos + 1)
                                .map(|(_, next_ho)| {
                                    if next_ho.is_long_note() {
                                        return false;
                                    }
                                    let next_head = next_ho.time;
                                    let next_window_start = next_head - w.hit50;
                                    let next_win_end = next_head + w.hit100;
                                    let has_next_tap_follow = presses
                                        .iter()
                                        .skip(cand_idx + 1)
                                        .take_while(|cand| **cand < next_win_end)
                                        .any(|cand| {
                                            *cand >= next_window_start
                                                && !reserved_ln_repr.contains(cand)
                                        });
                                    cand_pt == next_window_start
                                        && cand_pt > end_time
                                        && cand_release_after
                                            .map(|rt| {
                                                rt > end_time
                                                    && rt < next_head
                                                    && rt <= post_end_tail_cap
                                            })
                                            .unwrap_or(false)
                                        && has_next_tap_follow
                                })
                                .unwrap_or(false);
                        if !nex_note_guar_ok_for(cand_pt)
                            && !hle_post_end_next_ln
                            && !hless_post_end_miss
                            && !short_post_end_next
                            && !prev_miss_short_rec
                            && !hless_short_next_tap
                        {
                            break;
                        }
                        let rel_rechs_relxd_tail = cand_release_after
                            .map(|rt| rt > cand_pt && rt > end_time && rt <= post_end_tail_cap)
                            .unwrap_or(false);
                        if rel_rechs_relxd_tail
                            && (shortish_post_end_rec
                                || short_post_end_self
                                || mid_post_end_pre_next
                                || hle_post_end_next_ln
                                || hless_post_end_miss
                                || prev_miss_short_rec
                                || hless_short_next_tap
                                || headless_body_next_ln)
                        {
                            rec_tail_only_cand = Some((cand_idx, cand_pt));
                            break;
                        }
                    }
                    if let Some((cand_idx, cand_pt)) = rec_tail_only_cand {
                        tail_only_pt = Some(cand_pt);
                        press_idx = cand_idx + 1;
                    }
                }
            }
        }
    }
    state.press_idx = press_idx;
    state.pick.press = press_time;
    state.pick.tail = tail_only_pt;
    state.head_candidate.selected_pt = selected_pt;
    state.head_candidate.selected_idx = selected_idx;
    state.head_candidate.ghost_prehead = ghost_prehead;
}
