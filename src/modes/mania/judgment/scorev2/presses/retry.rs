use super::note::{PressNoteCtx, PressState};
use crate::modes::mania::judgment::{calc_hit_kind, InternalJudgment};
use crate::types::JudgmentKind;
pub(super) fn reselect(ctx: &PressNoteCtx<'_>, state: &mut PressState, out: &[InternalJudgment]) {
    if !state.head_candidate.has_candidate {
        return;
    }
    let note_pos = ctx.note_pos;
    let ho = ctx.ho;
    let col_notes = ctx.col_notes;
    let same_time_tap_count = ctx.same_time_tap_count;
    let presses = ctx.presses;
    let events = ctx.events;
    let w = ctx.windows;
    let next_note_time = ctx.next_note_time;
    let note_window = ctx.note_window;
    let window_start = note_window.window_start;
    let lock_end_exclusive = note_window.lock_end_exclusive;
    let early_penalty_window = note_window.early_penalty_window;
    let press_idx = state.press_idx;
    let prev_was_miss = state.prev.was_miss;
    let prev_had_prewin_pen = state.prev.had_prewin_pen;
    let prev_prev_was_miss = state.prev.prev2_was_miss;
    let _prev_prev_prewin_pen = state.prev.prev2_had_prewin_pen;
    let prev_col_pt = state.prev.col_pt;
    let reserved_ln_repr = &state.prev.reserved_ln_repr;
    let prev_same_col_kind = out.iter().rev().find(|jj| jj.column == ho.column);
    let _prev_note_is_ln_for_stale = note_pos
        .checked_sub(1)
        .and_then(|p| col_notes.get(p))
        .map(|(_, prev_ho)| prev_ho.is_long_note())
        .unwrap_or(false);
    let _prev_note_time_for_stale = note_pos
        .checked_sub(1)
        .and_then(|p| col_notes.get(p))
        .map(|(_, prev_ho)| prev_ho.time);
    let _prev_note_end_time_for_stale = note_pos
        .checked_sub(1)
        .and_then(|p| col_notes.get(p))
        .map(|(_, prev_ho)| prev_ho.end_time.unwrap_or(prev_ho.time));
    let _prev_note_duration_for_stale = note_pos
        .checked_sub(1)
        .and_then(|p| col_notes.get(p))
        .map(|(_, prev_ho)| prev_ho.end_time.unwrap_or(prev_ho.time) - prev_ho.time);
    let _ln_duration = ho.end_time.unwrap_or(ho.time) - ho.time;
    let _original_selected_pt = state.head_candidate.selected_pt;
    let original_selected_idx = state.head_candidate.selected_idx;
    let pt = state.head_candidate.selected_pt;
    let mut selected_pt = state.head_candidate.selected_pt;
    let mut selected_idx = state.head_candidate.selected_idx;
    let mut tap_micro_keep_idx = state.head_candidate.tap_micro_keeps_idx;
    let mut prewin_follow_next_ln = state.head_candidate.prewin_follow_next_ln;
    let mut pre_mis_pos_hea_prom = state.head_candidate.pre_mis_pos_hea_prom;
    let _ghost_prehead = state.head_candidate.ghost_prehead;
    let mut prev_miss_clear_rule: Option<&'static str> = None;
    let mut lat_tap_yild_next_ln = state.head_candidate.lat_tap_yild_next_ln;
    let prev_note_miss_time = note_pos
        .checked_sub(1)
        .and_then(|p| col_notes.get(p))
        .map(|(_, prev_ho)| prev_ho.time);
    if !ho.is_long_note()
        && prev_was_miss
        && prev_note_miss_time
            .map(|prev_t| pt <= prev_t)
            .unwrap_or(false)
        && pt >= window_start
        && pt < lock_end_exclusive
        && pt < ho.time
        && ho.time - pt >= w.hit50 - 1
        && calc_hit_kind((pt - ho.time).abs(), w) == JudgmentKind::Hit50
        && events
            .iter()
            .find(|ev| ev.time > pt && !ev.pressed)
            .map(|ev| ev.time < ho.time)
            .unwrap_or(false)
    {
        if let Some(next_head_time) = next_note_time {
            let next_window_start = next_head_time - w.hit50;
            if let Some((cand_idx, cand_pt)) = presses
                .iter()
                .enumerate()
                .skip(press_idx + 1)
                .take_while(|(_, cand)| **cand < lock_end_exclusive)
                .find(|(_, cand)| {
                    let cand_pt = **cand;
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    cand_pt >= next_window_start
                        && cand_pt < next_head_time
                        && !reserved_ln_repr.contains(cand)
                        && matches!(
                            cand_kind,
                            JudgmentKind::Max | JudgmentKind::Hit300 | JudgmentKind::Hit200
                        )
                        && events
                            .iter()
                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                            .map(|ev| ev.time > ho.time && ev.time < next_head_time)
                            .unwrap_or(false)
                })
                .map(|(i, cand)| (i, *cand))
            {
                let prev_h50_reselect = true
                    && col_notes
                        .get(note_pos + 1)
                        .zip(col_notes.get(note_pos + 2))
                        .map(|((_, next_ho), (_, next_next_ho))| {
                            if next_ho.is_long_note() || next_next_ho.is_long_note() {
                                return false;
                            }
                            let next_head = next_ho.time;
                            let next_next_head = next_next_ho.time;
                            let next3_tap_head =
                                col_notes
                                    .get(note_pos + 3)
                                    .and_then(|(_, next_next_next_ho)| {
                                        (!next_next_next_ho.is_long_note())
                                            .then_some(next_next_next_ho.time)
                                    });
                            let dense_current_to_next = next_head - ho.time <= w.hit50 * 2;
                            let dense_next_to_next2 = next_next_head - next_head <= w.hit50 * 2;
                            let next_chain_follow = presses
                                .iter()
                                .skip(cand_idx + 1)
                                .take_while(|cand| **cand < next_next_head + w.hit100)
                                .any(|cand| {
                                    let followup_pt = *cand;
                                    let follow_near_nnext = followup_pt < next_next_head
                                        && followup_pt >= next_next_head - w.hit50 - w.max
                                        && calc_hit_kind((followup_pt - next_next_head).abs(), w)
                                            == JudgmentKind::Miss;
                                    let follow_nnext_anchor = (followup_pt
                                        >= next_next_head - w.hit50
                                        && calc_hit_kind((followup_pt - next_next_head).abs(), w)
                                            != JudgmentKind::Miss)
                                        || follow_near_nnext;
                                    follow_nnext_anchor
                                        && !reserved_ln_repr.contains(cand)
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
                            dense_current_to_next
                                && dense_next_to_next2
                                && cand_pt >= next_head - w.hit50
                                && cand_pt < next_head
                                && next_chain_follow
                        })
                        .unwrap_or(false);
                if !prev_h50_reselect {
                    selected_pt = cand_pt;
                    selected_idx = cand_idx;
                }
            }
        }
    }
    let late_tap_cross_tap = true
        && !ho.is_long_note()
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .map(|((_, next_ho), next_head_time)| {
                if next_ho.is_long_note() || selected_pt < next_head_time {
                    return false;
                }
                let next_window_start = next_head_time - w.hit50;
                let next_win_end = next_head_time + w.hit100;
                let next_next_tap_head =
                    col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                        (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                    });
                let has_next_tap_follow = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next_win_end)
                    .any(|cand| {
                        *cand >= next_window_start
                            && next_next_tap_head
                                .map(|next_next_head| *cand < next_next_head)
                                .unwrap_or(true)
                            && !reserved_ln_repr.contains(cand)
                    });
                !has_next_tap_follow
            })
            .unwrap_or(false);
    let late_tap_dense_chain = true
        && !ho.is_long_note()
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .map(|((_, next_ho), next_head_time)| {
                let Some((_, next_next_ho)) = col_notes.get(note_pos + 2) else {
                    return false;
                };
                if next_ho.is_long_note()
                    || next_next_ho.is_long_note()
                    || selected_pt < next_head_time
                {
                    return false;
                }
                let next_next_head = next_next_ho.time;
                let next3_tap_head =
                    col_notes
                        .get(note_pos + 3)
                        .and_then(|(_, next_next_next_ho)| {
                            (!next_next_next_ho.is_long_note()).then_some(next_next_next_ho.time)
                        });
                let current_kind = calc_hit_kind((selected_pt - ho.time).abs(), w);
                let next_kind = calc_hit_kind((selected_pt - next_head_time).abs(), w);
                let rel_after_sel = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let next_chain_follow = presses
                    .iter()
                    .skip(selected_idx + 1)
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
                                .map(|ev| next3_tap_head.map(|head| ev.time < head).unwrap_or(true))
                                .unwrap_or(false)
                    });
                next_head_time - ho.time <= w.hit50 * 2
                    && next_next_head - next_head_time <= w.hit50 * 2
                    && selected_pt < next_next_head
                    && next_kind.score_value() > current_kind.score_value()
                    && rel_after_sel
                        .map(|rt| rt > next_head_time && rt < next_next_head)
                        .unwrap_or(false)
                    && next_chain_follow
            })
            .unwrap_or(false);
    let late_tap_iso_head = true
        && !ho.is_long_note()
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .map(|((_, next_ho), next_head_time)| {
                let Some((_, next_next_ho)) = col_notes.get(note_pos + 2) else {
                    return false;
                };
                if next_ho.is_long_note()
                    || next_next_ho.is_long_note()
                    || selected_pt < next_head_time
                {
                    return false;
                }
                let next_next_head = next_next_ho.time;
                let current_kind = calc_hit_kind((selected_pt - ho.time).abs(), w);
                let next_kind = calc_hit_kind((selected_pt - next_head_time).abs(), w);
                let rel_after_sel = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let next_tap_post_follow = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next_next_head)
                    .any(|cand| {
                        let cand_pt = *cand;
                        cand_pt >= next_head_time
                            && !reserved_ln_repr.contains(cand)
                            && calc_hit_kind((cand_pt - next_head_time).abs(), w)
                                != JudgmentKind::Miss
                            && events
                                .iter()
                                .find(|ev| ev.time > cand_pt && !ev.pressed)
                                .map(|ev| ev.time < next_next_head)
                                .unwrap_or(false)
                    });
                next_head_time - ho.time <= w.hit50
                    && next_next_head - next_head_time > w.hit50 + w.hit300
                    && selected_pt < next_next_head
                    && next_kind.score_value() > current_kind.score_value()
                    && rel_after_sel
                        .map(|rt| rt > next_head_time && rt < next_next_head)
                        .unwrap_or(false)
                    && next_tap_post_follow
            })
            .unwrap_or(false);
    let late_tap_cross_ln = true
        && !ho.is_long_note()
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .map(|((_, next_ho), next_head_time)| {
                if !next_ho.is_long_note() || selected_pt < next_head_time {
                    return false;
                }
                let next_window_start = next_head_time - w.hit50;
                let next_next_note_time = col_notes
                    .get(note_pos + 2)
                    .map(|(_, next_next_ho)| next_next_ho.time);
                let next_ln_late_end = next_next_note_time
                    .map(|next_time| next_time <= next_ho.time + w.hit50)
                    .unwrap_or(false);
                let next_lock_end = next_ho.time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                let next_tail_start = next_end_time - w.hit50;
                let next_tail_end = next_end_time + w.hit100;
                let next_tail_start_sc = next_end_time - ((w.hit50 as f32) * 1.5).round() as i32;
                let next_tail_end_sc = next_end_time + ((w.hit100 as f32) * 1.5).round() as i32;
                let fol_next_ln_pt = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next_lock_end)
                    .find(|cand| {
                        let followup_pt = **cand;
                        followup_pt >= next_window_start
                            && !reserved_ln_repr.contains(cand)
                            && events
                                .iter()
                                .find(|ev| ev.time > followup_pt && !ev.pressed)
                                .map(|ev| ev.time >= next_tail_start && ev.time < next_tail_end)
                                .unwrap_or(false)
                    })
                    .copied();
                let has_next_ln_follow = fol_next_ln_pt.is_some();
                let rel_after_sel = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let rel_in_next_tail = rel_after_sel
                    .map(|rt| rt >= next_tail_start && rt < next_tail_end)
                    .unwrap_or(false);
                let rel_in_next_ln_body = rel_after_sel
                    .map(|rt| rt > selected_pt && rt <= next_end_time)
                    .unwrap_or(false);
                let exa_nex_ln_head_clai = selected_pt == next_head_time
                    && rel_in_next_ln_body
                    && calc_hit_kind((selected_pt - ho.time).abs(), w) == JudgmentKind::Hit200;
                let sel_kind_for_cur = calc_hit_kind((selected_pt - ho.time).abs(), w);
                let sel_kind_for_next_ln = calc_hit_kind((selected_pt - next_head_time).abs(), w);
                let sel_str_next =
                    sel_kind_for_next_ln.score_value() > sel_kind_for_cur.score_value();
                let rel_in_next_tail_sc = rel_after_sel
                    .map(|rt| rt >= next_tail_start_sc && rt < next_tail_end_sc && sel_str_next)
                    .unwrap_or(false);
                let short_next_ln = next_end_time - next_head_time <= w.hit100
                    && selected_pt >= next_head_time
                    && selected_pt < next_lock_end
                    && !has_next_ln_follow
                    && sel_str_next
                    && rel_after_sel
                        .map(|rt| {
                            rt >= next_end_time
                                && next_next_note_time
                                    .map(|next_next_time| rt < next_next_time)
                                    .unwrap_or(true)
                        })
                        .unwrap_or(false);
                let fol_kind_for_next_ln = fol_next_ln_pt
                    .map(|followup_pt| calc_hit_kind((followup_pt - next_head_time).abs(), w));
                let fol_str_post_sel_rel = rel_after_sel
                    .zip(fol_next_ln_pt)
                    .map(|(rel_time, followup_pt)| rel_time < followup_pt)
                    .unwrap_or(false);
                let sel_pair_fits_next = selected_pt >= next_head_time
                    && (rel_in_next_tail || rel_in_next_tail_sc)
                    && sel_str_next
                    && fol_str_post_sel_rel
                    && fol_kind_for_next_ln
                        .map(|followup_kind| {
                            sel_kind_for_next_ln.score_value() > followup_kind.score_value()
                        })
                        .unwrap_or(false);
                if sel_pair_fits_next {
                    lat_tap_yild_next_ln = true;
                }
                selected_pt >= next_window_start
                    && selected_pt < next_lock_end
                    && ((!has_next_ln_follow && (rel_in_next_tail || rel_in_next_tail_sc))
                        || exa_nex_ln_head_clai
                        || short_next_ln)
            })
            .unwrap_or(false);
    let far_gap_prev_miss = true
        && !ho.is_long_note()
        && prev_was_miss
        && !prev_had_prewin_pen
        && selected_pt < ho.time
        && prev_note_miss_time
            .map(|prev_t| selected_pt == prev_t + w.hit100 && ho.time - prev_t > w.hit50 + w.hit300)
            .unwrap_or(false)
        && (presses
            .iter()
            .skip(selected_idx + 1)
            .take_while(|cand| **cand < lock_end_exclusive)
            .any(|cand| *cand >= ho.time + w.hit200 && !reserved_ln_repr.contains(cand))
            || next_note_time
                .map(|next_time| {
                    let next_window_start = next_time - w.hit50;
                    let next_lock_end = next_time + w.hit100;
                    presses
                        .iter()
                        .enumerate()
                        .skip(selected_idx + 1)
                        .take_while(|(_, cand)| **cand < lock_end_exclusive)
                        .filter(|(_, cand)| {
                            let cand_pt = **cand;
                            cand_pt >= ho.time + w.hit300
                                && cand_pt < ho.time + w.hit200
                                && cand_pt < next_time
                                && !reserved_ln_repr.contains(cand)
                        })
                        .any(|(fallback_idx, _)| {
                            presses
                                .iter()
                                .skip(fallback_idx + 1)
                                .take_while(|cand| **cand < next_lock_end)
                                .any(|cand| {
                                    *cand >= next_window_start && !reserved_ln_repr.contains(cand)
                                })
                        })
                })
                .unwrap_or(false));
    let prev_miss_micro_ghost = true
        && !ho.is_long_note()
        && prev_was_miss
        && !prev_had_prewin_pen
        && selected_pt < ho.time
        && calc_hit_kind((selected_pt - ho.time).abs(), w) == JudgmentKind::Hit50
        && prev_note_miss_time
            .map(|prev_t| selected_pt == prev_t + w.hit100)
            .unwrap_or(false)
        && !presses
            .iter()
            .skip(selected_idx + 1)
            .take_while(|cand| **cand < lock_end_exclusive)
            .any(|cand| !reserved_ln_repr.contains(cand))
        && events
            .iter()
            .find(|ev| ev.time > selected_pt && !ev.pressed)
            .map(|ev| ev.time < ho.time)
            .unwrap_or(false)
        && events
            .iter()
            .rev()
            .find(|ev| !ev.pressed && ev.time < selected_pt)
            .map(|ev| selected_pt - ev.time <= w.hit300)
            .unwrap_or(false);
    let prev_miss_pre_ghost = true
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
        && !presses
            .iter()
            .skip(selected_idx + 1)
            .take_while(|cand| **cand < lock_end_exclusive)
            .any(|cand| !reserved_ln_repr.contains(cand))
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
                let next_window_start = next_head_time - w.hit50;
                let next_win_end = next_head_time + w.hit100;
                let next_next_tap_head =
                    col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                        (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                    });
                presses
                    .iter()
                    .skip(selected_idx + 1)
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
    let prev_miss_late_ghost = true
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
        && events
            .iter()
            .find(|ev| ev.time > selected_pt && !ev.pressed)
            .map(|ev| ev.time < ho.time)
            .unwrap_or(false)
        && {
            let selected_kind = calc_hit_kind((selected_pt - ho.time).abs(), w);
            presses
                .iter()
                .enumerate()
                .skip(selected_idx + 1)
                .take_while(|(_, cand)| **cand < lock_end_exclusive)
                .find(|(_, cand)| {
                    let cand_pt = **cand;
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    let cur_h200_replcs_h300 =
                        selected_kind == JudgmentKind::Hit300 && cand_kind == JudgmentKind::Hit200;
                    cand_pt >= ho.time + w.hit300
                        && (cand_kind.score_value() >= selected_kind.score_value()
                            || cur_h200_replcs_h300)
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
                .map(|(fallback_idx, _)| {
                    col_notes
                        .get(note_pos + 1)
                        .zip(next_note_time)
                        .map(|((_, next_ho), next_head_time)| {
                            if next_ho.is_long_note() {
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
                                .enumerate()
                                .skip(fallback_idx + 1)
                                .take_while(|cand| *cand.1 < next_win_end)
                                .find(|(_, cand)| {
                                    let cand_pt = **cand;
                                    cand_pt >= next_window_start
                                        && next_next_tap_head
                                            .map(|next_next_head| cand_pt < next_next_head)
                                            .unwrap_or(true)
                                        && !reserved_ln_repr.contains(cand)
                                })
                                .map(|(next_tap_idx, cand)| {
                                    let cand_pt = *cand;
                                    let next_tap_steals_chain = col_notes
                                        .get(note_pos + 2)
                                        .map(|(_, next_next_ho)| {
                                            if !next_next_ho.is_long_note() {
                                                return false;
                                            }
                                            let next_next_head = next_next_ho.time;
                                            let next_next_end =
                                                next_next_ho.end_time.unwrap_or(next_next_head);
                                            let next_next_duration = next_next_end - next_next_head;
                                            let curr_gap = next_head_time - ho.time;
                                            let next_gap = next_next_head - next_head_time;
                                            let next_next_tail_start = next_next_end - w.hit50;
                                            let next2_tail_end = next_next_end + w.hit100;
                                            let next3_note_time =
                                                col_notes.get(note_pos + 3).map(|(_, ho)| ho.time);
                                            let next2_late_end = next3_note_time
                                                .map(|next_time| {
                                                    next_time <= next_next_head + w.hit50
                                                })
                                                .unwrap_or(false);
                                            let next2_lock_end = next_next_head
                                                + w.hit50
                                                + if next2_late_end { 1 } else { 0 };
                                            let next2_prewin_start =
                                                next_next_head - w.hit50 - early_penalty_window - 1;
                                            let cand_rel_before_nnext = events
                                                .iter()
                                                .find(|ev| ev.time > cand_pt && !ev.pressed)
                                                .map(|ev| {
                                                    ev.time > cand_pt && ev.time < next_next_head
                                                })
                                                .unwrap_or(false);
                                            next_gap >= curr_gap
                                                && next_next_duration <= w.hit50 + w.hit100
                                                && cand_pt >= next2_prewin_start
                                                && cand_pt < next_next_head
                                                && calc_hit_kind(
                                                    (cand_pt - next_next_head).abs(),
                                                    w,
                                                ) == JudgmentKind::Miss
                                                && cand_rel_before_nnext
                                                && presses
                                                    .iter()
                                                    .skip(next_tap_idx + 1)
                                                    .take_while(|cand| **cand < next2_lock_end)
                                                    .any(|cand| {
                                                        let follow_pt = *cand;
                                                        follow_pt >= next_next_head
                                                            && !reserved_ln_repr.contains(cand)
                                                            && events
                                                                .iter()
                                                                .find(|ev| {
                                                                    ev.time > follow_pt
                                                                        && !ev.pressed
                                                                })
                                                                .map(|ev| {
                                                                    ev.time >= next_next_tail_start
                                                                        && ev.time < next2_tail_end
                                                                        && next3_note_time
                                                                            .map(|next_time| {
                                                                                ev.time < next_time
                                                                            })
                                                                            .unwrap_or(true)
                                                                })
                                                                .unwrap_or(false)
                                                    })
                                        })
                                        .unwrap_or(false);
                                    !next_tap_steals_chain
                                })
                                .unwrap_or(false)
                        })
                        .unwrap_or(false)
                })
                .unwrap_or(false)
        };
    let prev_miss_keeps_long = true
        && ho.is_long_note()
        && prev_was_miss
        && !prev_had_prewin_pen
        && note_pos
            .checked_sub(1)
            .and_then(|p| col_notes.get(p))
            .map(|(_, prev_ho)| !prev_ho.is_long_note())
            .unwrap_or(false)
        && selected_pt < ho.time
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale
                    && selected_pt == prev_t + w.hit100
                    && ho.time - prev_t <= w.hit50 * 2
            })
            .unwrap_or(false)
        && !presses
            .iter()
            .skip(selected_idx + 1)
            .take_while(|cand| **cand < lock_end_exclusive)
            .any(|cand| !reserved_ln_repr.contains(cand))
        && events
            .iter()
            .find(|ev| ev.time > selected_pt && !ev.pressed)
            .map(|ev| ev.time >= ho.end_time.unwrap_or(ho.time))
            .unwrap_or(false)
        && next_note_time
            .map(|next_t| {
                presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next_t + w.hit100)
                    .any(|cand| *cand >= next_t - w.hit50 && !reserved_ln_repr.contains(cand))
            })
            .unwrap_or(false);
    let prev_miss_brkn_ghost = true
        && ho.is_long_note()
        && prev_was_miss
        && !prev_had_prewin_pen
        && note_pos
            .checked_sub(1)
            .and_then(|p| col_notes.get(p))
            .map(|(_, prev_ho)| !prev_ho.is_long_note())
            .unwrap_or(false)
        && selected_pt < ho.time
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale
                    && selected_pt == prev_t + w.hit100
                    && ho.time - prev_t <= w.hit50 * 2
            })
            .unwrap_or(false)
        && ho
            .end_time
            .map(|end_time| {
                let sel_rel_in_body = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time > ho.time && ev.time < end_time)
                    .unwrap_or(false);
                let has_ln_fol_pre_end = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < end_time)
                    .any(|cand| !reserved_ln_repr.contains(cand));
                sel_rel_in_body && !has_ln_fol_pre_end
            })
            .unwrap_or(false);
    let prev_miss_hless300 = true
        && ho.is_long_note()
        && prev_was_miss
        && !prev_had_prewin_pen
        && note_pos
            .checked_sub(1)
            .and_then(|p| col_notes.get(p))
            .map(|(_, prev_ho)| !prev_ho.is_long_note())
            .unwrap_or(false)
        && selected_pt < ho.time
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale
                    && selected_pt == prev_t + w.hit100
                    && calc_hit_kind((selected_pt - ho.time).abs(), w) == JudgmentKind::Hit300
            })
            .unwrap_or(false)
        && ho
            .end_time
            .map(|end_time| {
                let sel_rel_in_body = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time > ho.time && ev.time < end_time)
                    .unwrap_or(false);
                let has_ln_fol_pre_end = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < end_time)
                    .any(|cand| !reserved_ln_repr.contains(cand));
                let has_later_cur_cand = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| {
                        next_note_time
                            .map(|next_time| **cand < next_time)
                            .unwrap_or(true)
                    })
                    .any(|cand| {
                        let cand_pt = *cand;
                        cand_pt >= ho.time
                            && !reserved_ln_repr.contains(cand)
                            && calc_hit_kind((cand_pt - ho.time).abs(), w) != JudgmentKind::Miss
                    });
                sel_rel_in_body && !has_ln_fol_pre_end && !has_later_cur_cand
            })
            .unwrap_or(false);
    let prev_miss_keeps_hless = true
        && ho.is_long_note()
        && prev_was_miss
        && !prev_had_prewin_pen
        && note_pos
            .checked_sub(1)
            .and_then(|p| col_notes.get(p))
            .map(|(_, prev_ho)| !prev_ho.is_long_note())
            .unwrap_or(false)
        && selected_pt < ho.time
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale
                    && selected_pt == prev_t + w.hit100
                    && ho.time - prev_t <= w.hit50 * 2
            })
            .unwrap_or(false)
        && ho
            .end_time
            .map(|end_time| {
                let ln_duration = end_time - ho.time;
                let tail_end_exclusive = end_time + w.hit100;
                let sel_rel_in_body = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time > ho.time && ev.time < end_time)
                    .unwrap_or(false);
                sel_rel_in_body
                    && ln_duration > w.hit100
                    && ln_duration <= w.hit50 + w.hit100
                    && col_notes
                        .get(note_pos + 1)
                        .map(|(_, next_ho)| {
                            if !next_ho.is_long_note() {
                                return false;
                            }
                            let next_head = next_ho.time;
                            let next_window_start = next_head - w.hit50;
                            let next_win_end = next_head + w.hit100;
                            let next_end = next_ho.end_time.unwrap_or(next_head);
                            let next_tail_start = next_end - w.hit50;
                            let next_tail_end = next_end + w.hit100;
                            presses
                                .iter()
                                .enumerate()
                                .skip(selected_idx + 1)
                                .take_while(|(_, cand)| **cand < next_head)
                                .find(|(_, cand)| {
                                    let cand_pt = **cand;
                                    cand_pt > ho.time + w.hit100
                                        && cand_pt < end_time
                                        && !reserved_ln_repr.contains(cand)
                                        && events
                                            .iter()
                                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                                            .map(|ev| {
                                                ev.time > end_time
                                                    && ev.time < next_head
                                                    && ev.time < tail_end_exclusive
                                            })
                                            .unwrap_or(false)
                                })
                                .map(|(tail_idx, _)| {
                                    presses
                                        .iter()
                                        .skip(tail_idx + 1)
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
                                        })
                                })
                                .unwrap_or(false)
                        })
                        .unwrap_or(false)
            })
            .unwrap_or(false);
    let prev_miss_keep_hless = true
        && ho.is_long_note()
        && prev_was_miss
        && !prev_had_prewin_pen
        && note_pos
            .checked_sub(1)
            .and_then(|p| col_notes.get(p))
            .map(|(_, prev_ho)| !prev_ho.is_long_note())
            .unwrap_or(false)
        && selected_pt < ho.time
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale
                    && selected_pt == prev_t + w.hit100
                    && ho.time - prev_t <= w.hit50 * 2
            })
            .unwrap_or(false)
        && ho
            .end_time
            .map(|end_time| {
                let ln_duration = end_time - ho.time;
                let sel_rel_in_body = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time > ho.time && ev.time < end_time)
                    .unwrap_or(false);
                sel_rel_in_body
                    && ln_duration > w.hit100
                    && ln_duration <= w.hit50 + w.hit100
                    && col_notes
                        .get(note_pos + 1)
                        .and_then(|(_, next_ho)| (!next_ho.is_long_note()).then_some(next_ho.time))
                        .zip(col_notes.get(note_pos + 2))
                        .map(|(next_tap_time, (_, next_next_ho))| {
                            if !next_next_ho.is_long_note() {
                                return false;
                            }
                            let next_tap_window_start = next_tap_time - w.hit50;
                            let next_tap_end = next_tap_time + w.hit100;
                            let next_ln_head = next_next_ho.time;
                            let next_ln_window_start = next_ln_head - w.hit50;
                            let next_ln_win_end = next_ln_head + w.hit100;
                            let next_ln_end_time = next_next_ho.end_time.unwrap_or(next_ln_head);
                            let next_ln_tail_start = next_ln_end_time - w.hit50;
                            let next_ln_tail_end = next_ln_end_time + w.hit100;
                            let next_tap_has_own_pt = presses
                                .iter()
                                .skip(selected_idx + 1)
                                .take_while(|cand| **cand < next_tap_end)
                                .any(|cand| {
                                    let cand_pt = *cand;
                                    cand_pt >= next_tap_window_start
                                        && cand_pt < next_ln_head
                                        && !reserved_ln_repr.contains(cand)
                                });
                            let next_ln_has_own_pair = presses
                                .iter()
                                .skip(selected_idx + 1)
                                .take_while(|cand| **cand < next_ln_win_end)
                                .any(|cand| {
                                    let cand_pt = *cand;
                                    cand_pt >= next_ln_window_start
                                        && cand_pt < next_ln_win_end
                                        && !reserved_ln_repr.contains(cand)
                                        && events
                                            .iter()
                                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                                            .map(|ev| {
                                                ev.time >= next_ln_tail_start
                                                    && ev.time < next_ln_tail_end
                                            })
                                            .unwrap_or(false)
                                });
                            next_tap_has_own_pt && next_ln_has_own_pair
                        })
                        .unwrap_or(false)
            })
            .unwrap_or(false);
    let prev_short_repr_ghost = true
        && !ho.is_long_note()
        && selected_pt < ho.time
        && prev_same_col_kind
            .and_then(|prev_jj| {
                (prev_jj.is_ln && prev_jj.kind == JudgmentKind::Miss).then_some(prev_jj)
            })
            .and_then(|prev_jj| prev_jj.press_time.map(|prev_pt| (prev_jj, prev_pt)))
            .map(|(prev_jj, prev_pt)| {
                let prev_t = prev_jj.note_time;
                let prev_end = prev_jj.end_time.unwrap_or(prev_t);
                let prev_tail_start = prev_end - w.hit50;
                let prev_release = events
                    .iter()
                    .find(|ev| ev.time > prev_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let fir_rep_pos_prev_rel = prev_release.and_then(|release_t| {
                    events
                        .iter()
                        .find(|ev| ev.pressed && ev.time > release_t)
                        .map(|ev| ev.time)
                });
                prev_end - prev_t <= w.hit100
                    && prev_release.map(|rt| rt < prev_tail_start).unwrap_or(false)
                    && fir_rep_pos_prev_rel == Some(selected_pt)
                    && selected_pt > prev_end
                    && selected_pt <= prev_end + w.hit50
                    && ho.time - prev_t <= w.hit50 * 2
            })
            .unwrap_or(false)
        && events
            .iter()
            .find(|ev| ev.time > selected_pt && !ev.pressed)
            .map(|ev| ev.time >= ho.time)
            .unwrap_or(false)
        && !presses
            .iter()
            .skip(selected_idx + 1)
            .take_while(|cand| **cand < lock_end_exclusive)
            .any(|cand| !reserved_ln_repr.contains(cand));
    let mut ghost_prehead = prev_short_repr_ghost
        || (true
            && !ho.is_long_note()
            && prev_was_miss
            && !prev_had_prewin_pen
            && note_pos
                .checked_sub(1)
                .and_then(|p| col_notes.get(p))
                .map(|(_, prev_ho)| !prev_ho.is_long_note())
                .unwrap_or(false)
            && selected_pt < ho.time
            && prev_note_miss_time
                .map(|prev_t| {
                    let prev_auto_miss_bound = prev_t + w.hit100;
                    selected_pt <= prev_auto_miss_bound && ho.time - prev_t <= w.hit50 * 2
                })
                .unwrap_or(false)
            && (events
                .iter()
                .find(|ev| ev.time > selected_pt && !ev.pressed)
                .map(|ev| ev.time >= ho.time)
                .unwrap_or(false)
                || far_gap_prev_miss
                || prev_miss_micro_ghost
                || prev_miss_pre_ghost
                || prev_miss_late_ghost))
        || prev_miss_hless300
        || prev_miss_brkn_ghost
        || prev_miss_keeps_long
        || prev_miss_keeps_hless
        || prev_miss_keep_hless;
    let next_tap_follow_chain = ghost_prehead
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .map(|((_, next_ho), next_head_time)| {
                if next_ho.is_long_note() {
                    return false;
                }
                let next_window_start = next_head_time - w.hit50;
                let next_win_end = next_head_time + w.hit100;
                let next_next_tap = col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                    (!next_next_ho.is_long_note()).then_some(*next_next_ho)
                });
                let next_next_head = next_next_tap.map(|ho| ho.time);
                let next3_tap_head =
                    col_notes
                        .get(note_pos + 3)
                        .and_then(|(_, next_next_next_ho)| {
                            (!next_next_next_ho.is_long_note()).then_some(next_next_next_ho.time)
                        });
                let next_tap_pre_h300 = presses
                    .iter()
                    .enumerate()
                    .skip(selected_idx + 1)
                    .take_while(|(_, cand)| **cand < next_win_end)
                    .find(|(_, cand)| {
                        let cand_pt = **cand;
                        cand_pt >= next_window_start
                            && cand_pt < next_head_time
                            && next_next_head
                                .map(|next_next_head| cand_pt < next_next_head)
                                .unwrap_or(true)
                            && !reserved_ln_repr.contains(cand)
                            && calc_hit_kind((cand_pt - next_head_time).abs(), w)
                                == JudgmentKind::Hit300
                    })
                    .is_some();
                let fol_tap_starts_miss = next_next_tap
                    .map(|next_next_ho| {
                        let next_next_head = next_next_ho.time;
                        let next2_prewin_start =
                            next_next_head - w.hit50 - early_penalty_window - 1;
                        let next2_win_end = next_next_head + w.hit100;
                        presses
                            .iter()
                            .enumerate()
                            .skip(selected_idx + 1)
                            .take_while(|(_, cand)| **cand < next2_win_end)
                            .find(|(_, cand)| {
                                let cand_pt = **cand;
                                cand_pt >= next2_prewin_start && !reserved_ln_repr.contains(cand)
                            })
                            .map(|(next_next_idx, cand)| {
                                let cand_pt = *cand;
                                let mis_bou_rel_pre_head = events
                                    .iter()
                                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                                    .map(|ev| ev.time < next_next_head)
                                    .unwrap_or(false);
                                cand_pt < next_next_head
                                    && calc_hit_kind((cand_pt - next_next_head).abs(), w)
                                        == JudgmentKind::Miss
                                    && mis_bou_rel_pre_head
                                    && presses
                                        .iter()
                                        .skip(next_next_idx + 1)
                                        .take_while(|cand| **cand < next2_win_end)
                                        .any(|cand| {
                                            let follow_pt = *cand;
                                            follow_pt >= next_next_head
                                                && next3_tap_head
                                                    .map(|head| follow_pt < head)
                                                    .unwrap_or(true)
                                                && !reserved_ln_repr.contains(cand)
                                                && matches!(
                                                    calc_hit_kind(
                                                        (follow_pt - next_next_head).abs(),
                                                        w,
                                                    ),
                                                    JudgmentKind::Max | JudgmentKind::Hit300
                                                )
                                                && events
                                                    .iter()
                                                    .find(|ev| ev.time > follow_pt && !ev.pressed)
                                                    .map(|ev| {
                                                        next3_tap_head
                                                            .map(|head| ev.time < head)
                                                            .unwrap_or(true)
                                                    })
                                                    .unwrap_or(false)
                                        })
                            })
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                next_tap_pre_h300 && fol_tap_starts_miss
            })
            .unwrap_or(false);
    let pre_hold_weak_follow = ghost_prehead
        && !next_tap_follow_chain
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale && selected_pt == prev_t + w.hit100
            })
            .unwrap_or(false)
        && !presses
            .iter()
            .skip(selected_idx + 1)
            .take_while(|cand| **cand < lock_end_exclusive)
            .any(|cand| !reserved_ln_repr.contains(cand))
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .and_then(|((_, next_ho), next_head_time)| {
                if next_ho.is_long_note() {
                    return None;
                }
                col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                    (!next_next_ho.is_long_note()).then_some((next_head_time, next_next_ho.time))
                })
            })
            .map(|(next_head_time, next_next_head_time)| {
                let curr_gap = next_head_time - ho.time;
                let next_gap = next_next_head_time - next_head_time;
                let next_window_start = next_head_time - w.hit50;
                let next_win_end = next_head_time + w.hit100;
                let next2_win_start = next_next_head_time - w.hit50;
                let next2_win_end = next_next_head_time + w.hit100;
                let next3_tap_head =
                    col_notes
                        .get(note_pos + 3)
                        .and_then(|(_, next_next_next_ho)| {
                            (!next_next_next_ho.is_long_note()).then_some(next_next_next_ho.time)
                        });
                let next_has_strong_cand = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next_win_end)
                    .any(|cand| {
                        let cand_pt = *cand;
                        cand_pt >= next_window_start
                            && cand_pt < next_head_time
                            && !reserved_ln_repr.contains(cand)
                            && matches!(
                                calc_hit_kind((cand_pt - next_head_time).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300 | JudgmentKind::Hit200
                            )
                    });
                next_gap >= curr_gap
                    && !next_has_strong_cand
                    && presses
                        .iter()
                        .enumerate()
                        .skip(selected_idx + 1)
                        .take_while(|(_, cand)| **cand < next_win_end)
                        .find(|(_, cand)| {
                            let cand_pt = **cand;
                            cand_pt >= next_window_start
                                && cand_pt < next_next_head_time
                                && !reserved_ln_repr.contains(cand)
                                && calc_hit_kind((cand_pt - next_head_time).abs(), w)
                                    == JudgmentKind::Hit100
                        })
                        .map(|(next_idx, _)| {
                            presses
                                    .iter()
                                    .skip(next_idx + 1)
                                    .take_while(|cand| **cand < next2_win_end)
                                    .any(|cand| {
                                        let cand_pt = *cand;
                                        cand_pt >= next2_win_start
                                            && next3_tap_head
                                                .map(|next_next_next_head| {
                                                    cand_pt < next_next_next_head
                                                })
                                                .unwrap_or(true)
                                            && !reserved_ln_repr.contains(cand)
                                            && matches!(
                                                calc_hit_kind(
                                                    (cand_pt - next_next_head_time).abs(),
                                                    w,
                                                ),
                                                JudgmentKind::Max
                                                    | JudgmentKind::Hit300
                                                    | JudgmentKind::Hit200
                                            )
                                    })
                        })
                        .unwrap_or(false)
            })
            .unwrap_or(false);
    let prev_prev_note_is_ln = note_pos
        .checked_sub(2)
        .and_then(|p| col_notes.get(p))
        .map(|(_, prev_prev_ho)| prev_prev_ho.is_long_note())
        .unwrap_or(false);
    let prev2_tap_late_h200 = !prev_prev_was_miss
        && out
            .iter()
            .rev()
            .nth(1)
            .map(|prev_prev_judgment| {
                !prev_prev_judgment.is_ln
                    && prev_prev_judgment
                        .press_time
                        .map(|prev_prev_pt| {
                            prev_prev_pt > prev_prev_judgment.note_time + w.hit300
                                && prev_prev_pt <= prev_prev_judgment.note_time + w.hit200
                        })
                        .unwrap_or(false)
            })
            .unwrap_or(false);
    let pprev_tap_was_clean = !prev_prev_was_miss
        && out
            .iter()
            .rev()
            .nth(1)
            .map(|prev_prev_judgment| {
                !prev_prev_judgment.is_ln
                    && prev_prev_judgment
                        .press_time
                        .map(|prev_prev_pt| {
                            prev_prev_pt >= prev_prev_judgment.note_time
                                && matches!(
                                    calc_hit_kind(
                                        (prev_prev_pt - prev_prev_judgment.note_time).abs(),
                                        w,
                                    ),
                                    JudgmentKind::Max | JudgmentKind::Hit300
                                )
                        })
                        .unwrap_or(false)
            })
            .unwrap_or(false);
    let pre_hold_wide_gap = ghost_prehead
        && !next_tap_follow_chain
        && (prev_prev_note_is_ln || prev2_tap_late_h200 || pprev_tap_was_clean)
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale && selected_pt == prev_t + w.hit100
            })
            .unwrap_or(false)
        && !presses
            .iter()
            .skip(selected_idx + 1)
            .take_while(|cand| **cand < lock_end_exclusive)
            .any(|cand| !reserved_ln_repr.contains(cand))
        && matches!(
            calc_hit_kind((selected_pt - ho.time).abs(), w),
            JudgmentKind::Max | JudgmentKind::Hit300
        )
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .map(|((_, next_ho), next_head_time)| {
                if next_ho.is_long_note() {
                    return false;
                }
                let current_selected_kind = calc_hit_kind((selected_pt - ho.time).abs(), w);
                let first_rel_after_pick = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let sel_rel_post_head = first_rel_after_pick
                    .map(|rt| rt > ho.time && rt < next_head_time)
                    .unwrap_or(false);
                let sel_rel_post_h200 = first_rel_after_pick
                    .map(|rt| rt > ho.time + w.hit200 && rt < next_head_time)
                    .unwrap_or(false);
                let sel_rel_leaves_gap = first_rel_after_pick
                    .map(|rt| next_head_time - rt > w.hit50 + w.max)
                    .unwrap_or(false);
                let next_window_start = next_head_time - w.hit50;
                let next_win_end = next_head_time + w.hit100;
                let next_has_strong_cand = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next_win_end)
                    .any(|cand| {
                        let cand_pt = *cand;
                        cand_pt >= next_window_start
                            && !reserved_ln_repr.contains(cand)
                            && matches!(
                                calc_hit_kind((cand_pt - next_head_time).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300
                            )
                    });
                let next_note_has_max = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next_win_end)
                    .any(|cand| {
                        let cand_pt = *cand;
                        cand_pt >= next_window_start
                            && !reserved_ln_repr.contains(cand)
                            && calc_hit_kind((cand_pt - next_head_time).abs(), w)
                                == JudgmentKind::Max
                    });
                let tap_clear_needs_max = ho.is_long_note()
                    || current_selected_kind == JudgmentKind::Max
                    || next_note_has_max;
                let next_next_head_time =
                    col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                        (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                    });
                let next2_gap_flat = next_next_head_time
                    .map(|next_next_head_time| {
                        next_next_head_time - next_head_time >= next_head_time - ho.time
                    })
                    .unwrap_or(false);
                let next3_tap_head =
                    col_notes
                        .get(note_pos + 3)
                        .and_then(|(_, next_next_next_ho)| {
                            (!next_next_next_ho.is_long_note()).then_some(next_next_next_ho.time)
                        });
                let next2_note_strong = next_next_head_time
                    .map(|next_next_head_time| {
                        let next2_win_start = next_next_head_time - w.hit50;
                        let next2_win_end = next_next_head_time + w.hit100;
                        presses
                            .iter()
                            .skip(selected_idx + 1)
                            .take_while(|cand| **cand < next2_win_end)
                            .any(|cand| {
                                let cand_pt = *cand;
                                cand_pt >= next2_win_start
                                    && next3_tap_head.map(|head| cand_pt < head).unwrap_or(true)
                                    && !reserved_ln_repr.contains(cand)
                                    && matches!(
                                        calc_hit_kind((cand_pt - next_next_head_time).abs(), w,),
                                        JudgmentKind::Max | JudgmentKind::Hit300
                                    )
                            })
                    })
                    .unwrap_or(false);
                (prev_prev_note_is_ln
                    && next_head_time - ho.time > w.hit50 + w.hit300
                    && sel_rel_post_head
                    && next_has_strong_cand
                    && tap_clear_needs_max)
                    || (prev2_tap_late_h200
                        && ((sel_rel_post_h200 && next_note_has_max && next2_gap_flat)
                            || (current_selected_kind == JudgmentKind::Max
                                && next_head_time - ho.time > w.hit50 + w.hit300
                                && sel_rel_post_head
                                && next_note_has_max
                                && next2_gap_flat)))
                    || (pprev_tap_was_clean
                        && next_head_time - ho.time > w.hit50 + w.hit300
                        && (current_selected_kind == JudgmentKind::Max
                            || next_head_time - ho.time > w.hit50 * 2)
                        && sel_rel_post_head
                        && sel_rel_leaves_gap
                        && next_has_strong_cand
                        && tap_clear_needs_max
                        && next2_gap_flat
                        && next2_note_strong)
            })
            .unwrap_or(false);
    let pre_bound_h200_chain = ghost_prehead
        && !next_tap_follow_chain
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale && selected_pt == prev_t + w.hit100
            })
            .unwrap_or(false)
        && !presses
            .iter()
            .skip(selected_idx + 1)
            .take_while(|cand| **cand < lock_end_exclusive)
            .any(|cand| !reserved_ln_repr.contains(cand))
        && calc_hit_kind((selected_pt - ho.time).abs(), w) == JudgmentKind::Hit200
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .map(|((_, next_ho), next_head_time)| {
                if next_ho.is_long_note() {
                    return false;
                }
                let sel_rel_post_head = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time > ho.time && ev.time < next_head_time)
                    .unwrap_or(false);
                let next_window_start = next_head_time - w.hit50;
                let next_win_end = next_head_time + w.hit100;
                let next_next_head_time =
                    col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                        (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                    });
                let next3_tap_head =
                    col_notes
                        .get(note_pos + 3)
                        .and_then(|(_, next_next_next_ho)| {
                            (!next_next_next_ho.is_long_note()).then_some(next_next_next_ho.time)
                        });
                let next_has_prehead_max = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next_win_end)
                    .any(|cand| {
                        let cand_pt = *cand;
                        cand_pt >= next_window_start
                            && cand_pt < next_head_time
                            && !reserved_ln_repr.contains(cand)
                            && calc_hit_kind((cand_pt - next_head_time).abs(), w)
                                == JudgmentKind::Max
                    });
                let next_strt_post_chain = next_next_head_time
                    .map(|next_next_head_time| {
                        let next2_win_end = next_next_head_time + w.hit100;
                        presses
                            .iter()
                            .enumerate()
                            .skip(selected_idx + 1)
                            .take_while(|(_, cand)| **cand < next2_win_end)
                            .find(|(_, cand)| {
                                let cand_pt = **cand;
                                cand_pt > next_head_time
                                    && cand_pt < next_next_head_time
                                    && !reserved_ln_repr.contains(cand)
                                    && calc_hit_kind((cand_pt - next_head_time).abs(), w)
                                        == JudgmentKind::Miss
                            })
                            .map(|(miss_idx, cand)| {
                                let miss_pt = *cand;
                                let miss_rel_pre_next2 = events
                                    .iter()
                                    .find(|ev| ev.time > miss_pt && !ev.pressed)
                                    .map(|ev| ev.time < next_next_head_time)
                                    .unwrap_or(false);
                                presses
                                    .iter()
                                    .skip(miss_idx + 1)
                                    .take_while(|cand| **cand < next2_win_end)
                                    .any(|cand| {
                                        let follow_pt = *cand;
                                        follow_pt >= next_next_head_time
                                            && next3_tap_head
                                                .map(|head| follow_pt < head)
                                                .unwrap_or(true)
                                            && !reserved_ln_repr.contains(cand)
                                            && matches!(
                                                calc_hit_kind(
                                                    (follow_pt - next_next_head_time).abs(),
                                                    w,
                                                ),
                                                JudgmentKind::Max | JudgmentKind::Hit300
                                            )
                                    })
                                    && miss_rel_pre_next2
                            })
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                next_head_time - ho.time > w.hit50
                    && next_head_time - ho.time <= w.hit50 + w.hit300
                    && sel_rel_post_head
                    && next_has_prehead_max
                    && next_strt_post_chain
            })
            .unwrap_or(false);
    let pre_bound_h200_ln = ghost_prehead
        && !next_tap_follow_chain
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale && selected_pt == prev_t + w.hit100
            })
            .unwrap_or(false)
        && !presses
            .iter()
            .skip(selected_idx + 1)
            .take_while(|cand| **cand < lock_end_exclusive)
            .any(|cand| !reserved_ln_repr.contains(cand))
        && calc_hit_kind((selected_pt - ho.time).abs(), w) == JudgmentKind::Hit200
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .and_then(|((_, next_ho), next_head_time)| {
                if next_ho.is_long_note() {
                    return None;
                }
                col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                    next_next_ho
                        .is_long_note()
                        .then_some((next_head_time, next_next_ho.time))
                })
            })
            .map(|(next_head_time, next_next_head_time)| {
                let selected_release = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let sel_rel_post_head = selected_release
                    .map(|rt| rt > ho.time && rt < next_head_time)
                    .unwrap_or(false);
                let next_win_end = next_head_time + w.hit100;
                let nex_tap_pos_hea_cand = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next_win_end)
                    .any(|cand| {
                        let cand_pt = *cand;
                        cand_pt > next_head_time
                            && cand_pt < next_next_head_time
                            && selected_release.map(|rt| cand_pt > rt).unwrap_or(true)
                            && !reserved_ln_repr.contains(cand)
                            && matches!(
                                calc_hit_kind((cand_pt - next_head_time).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300
                            )
                    });
                sel_rel_post_head && nex_tap_pos_hea_cand
            })
            .unwrap_or(false);
    let pre_bound_h200_inlock = ghost_prehead
        && !next_tap_follow_chain
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale && selected_pt == prev_t + w.hit100
            })
            .unwrap_or(false)
        && calc_hit_kind((selected_pt - ho.time).abs(), w) == JudgmentKind::Hit200
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .and_then(|((_, next_ho), next_head_time)| {
                if next_ho.is_long_note() {
                    return None;
                }
                col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                    next_next_ho
                        .is_long_note()
                        .then_some((next_head_time, next_next_ho.time))
                })
            })
            .map(|(next_head_time, next_next_head_time)| {
                let selected_release = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let sel_rel_post_head = selected_release
                    .map(|rt| rt > ho.time && rt < next_head_time)
                    .unwrap_or(false);
                let followup_idx_and_time = presses
                    .iter()
                    .enumerate()
                    .skip(selected_idx + 1)
                    .take_while(|(_, cand)| **cand < next_head_time)
                    .find(|(_, cand)| {
                        let cand_pt = **cand;
                        cand_pt > ho.time
                            && cand_pt < lock_end_exclusive
                            && selected_release.map(|rt| cand_pt > rt).unwrap_or(false)
                            && !reserved_ln_repr.contains(cand)
                            && matches!(
                                calc_hit_kind((cand_pt - next_head_time).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300
                            )
                    })
                    .map(|(idx, cand)| (idx, *cand));
                let no_follow_before_head = followup_idx_and_time
                    .map(|(followup_idx, _)| {
                        !presses
                            .iter()
                            .skip(followup_idx + 1)
                            .take_while(|cand| **cand < next_head_time)
                            .any(|cand| !reserved_ln_repr.contains(cand))
                    })
                    .unwrap_or(false);
                next_head_time - ho.time <= w.hit50
                    && next_next_head_time > next_head_time
                    && sel_rel_post_head
                    && followup_idx_and_time.is_some()
                    && no_follow_before_head
            })
            .unwrap_or(false);
    let pre_bound_strong_tap = ghost_prehead
        && !next_tap_follow_chain
        && (prev_prev_was_miss || prev2_tap_late_h200)
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale && selected_pt == prev_t + w.hit100
            })
            .unwrap_or(false)
        && !presses
            .iter()
            .skip(selected_idx + 1)
            .take_while(|cand| **cand < lock_end_exclusive)
            .any(|cand| !reserved_ln_repr.contains(cand))
        && matches!(
            calc_hit_kind((selected_pt - ho.time).abs(), w),
            JudgmentKind::Max | JudgmentKind::Hit300
        )
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .and_then(|((_, next_ho), next_head_time)| {
                if next_ho.is_long_note() {
                    return None;
                }
                col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                    (!next_next_ho.is_long_note()).then_some((next_head_time, next_next_ho.time))
                })
            })
            .map(|(next_head_time, next_next_head_time)| {
                let selected_release = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let sel_rel_post_head = selected_release
                    .map(|rt| rt > ho.time && rt < next_head_time)
                    .unwrap_or(false);
                let next_win_end = next_head_time + w.hit100;
                let nex_tap_pos_hea_cand = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next_win_end)
                    .any(|cand| {
                        let cand_pt = *cand;
                        cand_pt > next_head_time
                            && cand_pt < next_next_head_time
                            && selected_release.map(|rt| cand_pt > rt).unwrap_or(true)
                            && !reserved_ln_repr.contains(cand)
                            && matches!(
                                calc_hit_kind((cand_pt - next_head_time).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300
                            )
                    });
                let next2_win_start = next_next_head_time - w.hit50;
                let next2_win_end = next_next_head_time + w.hit100;
                let next3_tap_head =
                    col_notes
                        .get(note_pos + 3)
                        .and_then(|(_, next_next_next_ho)| {
                            (!next_next_next_ho.is_long_note()).then_some(next_next_next_ho.time)
                        });
                let follow_tap_strong = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next2_win_end)
                    .any(|cand| {
                        let cand_pt = *cand;
                        cand_pt >= next2_win_start
                            && next3_tap_head.map(|head| cand_pt < head).unwrap_or(true)
                            && !reserved_ln_repr.contains(cand)
                            && matches!(
                                calc_hit_kind((cand_pt - next_next_head_time).abs(), w,),
                                JudgmentKind::Max | JudgmentKind::Hit300
                            )
                    });
                let next_gap = next_head_time - ho.time;
                let next_next_gap = next_next_head_time - next_head_time;
                sel_rel_post_head
                    && next_gap > w.hit50
                    && nex_tap_pos_hea_cand
                    && follow_tap_strong
                    && next_next_gap + w.max >= next_gap
            })
            .unwrap_or(false);
    let pre_bound_quick_tap = ghost_prehead
        && !next_tap_follow_chain
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale && selected_pt == prev_t + w.hit100
            })
            .unwrap_or(false)
        && !presses
            .iter()
            .skip(selected_idx + 1)
            .take_while(|cand| **cand < lock_end_exclusive)
            .any(|cand| !reserved_ln_repr.contains(cand))
        && matches!(
            calc_hit_kind((selected_pt - ho.time).abs(), w),
            JudgmentKind::Max | JudgmentKind::Hit300 | JudgmentKind::Hit200
        )
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .and_then(|((_, next_ho), next_head_time)| {
                if next_ho.is_long_note() {
                    return None;
                }
                col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                    (!next_next_ho.is_long_note()).then_some((next_head_time, next_next_ho.time))
                })
            })
            .map(|(next_head_time, next_next_head_time)| {
                let current_selected_kind = calc_hit_kind((selected_pt - ho.time).abs(), w);
                let selected_release = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let next_window_start = next_head_time - w.hit50;
                let next_win_end = next_head_time + w.hit100;
                let next2_win_start = next_next_head_time - w.hit50;
                let next2_win_end = next_next_head_time + w.hit100;
                let next3_tap_head =
                    col_notes
                        .get(note_pos + 3)
                        .and_then(|(_, next_next_next_ho)| {
                            (!next_next_next_ho.is_long_note()).then_some(next_next_next_ho.time)
                        });
                let next_tap_prehead = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next_win_end)
                    .any(|cand| {
                        let cand_pt = *cand;
                        cand_pt >= next_window_start
                            && cand_pt < next_head_time
                            && selected_release.map(|rt| cand_pt > rt).unwrap_or(true)
                            && !reserved_ln_repr.contains(cand)
                            && matches!(
                                calc_hit_kind((cand_pt - next_head_time).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300
                            )
                    });
                let follow_tap_strong = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next2_win_end)
                    .any(|cand| {
                        let cand_pt = *cand;
                        cand_pt >= next2_win_start
                            && next3_tap_head.map(|head| cand_pt < head).unwrap_or(true)
                            && !reserved_ln_repr.contains(cand)
                            && matches!(
                                calc_hit_kind((cand_pt - next_next_head_time).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300
                            )
                    });
                let qui_rel_cle_nex_prwn = selected_release
                    .map(|rt| {
                        let rel_start_head_ban = match current_selected_kind {
                            JudgmentKind::Hit200 => rt <= ho.time + w.hit200,
                            JudgmentKind::Max | JudgmentKind::Hit300 => rt <= ho.time + w.hit300,
                            _ => false,
                        };
                        let fol_gap_supports_cur = current_selected_kind != JudgmentKind::Hit200
                            || next_next_head_time - next_head_time
                                <= next_head_time - ho.time + w.max;
                        let rel_shape_prfrs_cur = ho.time - selected_pt > rt - ho.time
                            || (current_selected_kind == JudgmentKind::Hit200
                                && next_head_time - ho.time > w.hit50 * 2
                                && next_next_head_time - next_head_time < next_head_time - ho.time);
                        rt > ho.time
                            && fol_gap_supports_cur
                            && rel_shape_prfrs_cur
                            && rel_start_head_ban
                            && rt < next_window_start
                    })
                    .unwrap_or(false);
                qui_rel_cle_nex_prwn && next_tap_prehead && follow_tap_strong
            })
            .unwrap_or(false);
    let pre_bound_h200_h300 = ghost_prehead
        && !next_tap_follow_chain
        && (prev_prev_was_miss || prev2_tap_late_h200)
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale && selected_pt == prev_t + w.hit100
            })
            .unwrap_or(false)
        && !presses
            .iter()
            .skip(selected_idx + 1)
            .take_while(|cand| **cand < lock_end_exclusive)
            .any(|cand| !reserved_ln_repr.contains(cand))
        && matches!(
            calc_hit_kind((selected_pt - ho.time).abs(), w),
            JudgmentKind::Hit200 | JudgmentKind::Hit300
        )
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .and_then(|((_, next_ho), next_head_time)| {
                if next_ho.is_long_note() {
                    return None;
                }
                col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                    (!next_next_ho.is_long_note()).then_some((next_head_time, next_next_ho.time))
                })
            })
            .map(|(next_head_time, next_next_head_time)| {
                let selected_release = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let next_window_start = next_head_time - w.hit50;
                let next_win_end = next_head_time + w.hit100;
                let next2_win_start = next_next_head_time - w.hit50;
                let next2_win_end = next_next_head_time + w.hit100;
                let next3_tap_head =
                    col_notes
                        .get(note_pos + 3)
                        .and_then(|(_, next_next_next_ho)| {
                            (!next_next_next_ho.is_long_note()).then_some(next_next_next_ho.time)
                        });
                let next_tap_strong = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next_win_end)
                    .find_map(|cand| {
                        let cand_pt = *cand;
                        let cand_kind = calc_hit_kind((cand_pt - next_head_time).abs(), w);
                        (cand_pt >= next_window_start
                            && cand_pt < next_next_head_time
                            && selected_release.map(|rt| cand_pt > rt).unwrap_or(true)
                            && !reserved_ln_repr.contains(cand)
                            && cand_kind == JudgmentKind::Max)
                            .then_some(cand_pt)
                    })
                    .is_some();
                let fol_tap_h200 = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < next2_win_end)
                    .find_map(|cand| {
                        let cand_pt = *cand;
                        let cand_kind = calc_hit_kind((cand_pt - next_next_head_time).abs(), w);
                        (cand_pt >= next2_win_start
                            && next3_tap_head.map(|head| cand_pt < head).unwrap_or(true)
                            && !reserved_ln_repr.contains(cand)
                            && cand_kind == JudgmentKind::Hit200)
                            .then_some(cand_pt)
                    })
                    .is_some();
                selected_release
                    .map(|rt| rt < next_head_time)
                    .unwrap_or(false)
                    && next_tap_strong
                    && fol_tap_h200
            })
            .unwrap_or(false);
    let pre_bound_h300_gap = ghost_prehead
        && !next_tap_follow_chain
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale
                    && selected_pt == prev_t + w.hit100
                    && ho.time == prev_t + w.hit50
            })
            .unwrap_or(false)
        && !presses
            .iter()
            .skip(selected_idx + 1)
            .take_while(|cand| **cand < lock_end_exclusive)
            .any(|cand| !reserved_ln_repr.contains(cand))
        && calc_hit_kind((selected_pt - ho.time).abs(), w) == JudgmentKind::Hit300
        && col_notes
            .get(note_pos + 1)
            .zip(next_note_time)
            .map(|((_, next_ho), next_head_time)| {
                if next_ho.is_long_note() {
                    return false;
                }
                let selected_release = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let next_window_start = next_head_time - w.hit50;
                let next_win_end = next_head_time + w.hit100;
                let next_next_tap_head =
                    col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                        (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                    });
                let fol_tap_max = next_next_tap_head
                    .map(|next_next_head_time| {
                        let next2_win_start = next_next_head_time - w.hit50;
                        let next2_win_end = next_next_head_time + w.hit100;
                        presses
                            .iter()
                            .skip(selected_idx + 1)
                            .take_while(|cand| **cand < next2_win_end)
                            .any(|cand| {
                                let cand_pt = *cand;
                                cand_pt >= next2_win_start
                                    && !reserved_ln_repr.contains(cand)
                                    && calc_hit_kind((cand_pt - next_next_head_time).abs(), w)
                                        == JudgmentKind::Max
                            })
                    })
                    .unwrap_or(false);
                next_head_time - ho.time > w.hit50 + w.hit300
                    && selected_release
                        .filter(|rt| *rt > ho.time && *rt < next_head_time)
                        .map(|rt| {
                            presses
                                .iter()
                                .skip(selected_idx + 1)
                                .take_while(|cand| **cand < next_win_end)
                                .find(|cand| {
                                    let cand_pt = **cand;
                                    cand_pt > rt + w.hit50
                                        && cand_pt >= next_window_start
                                        && next_next_tap_head
                                            .map(|head| cand_pt < head)
                                            .unwrap_or(true)
                                        && !reserved_ln_repr.contains(cand)
                                        && calc_hit_kind((cand_pt - next_head_time).abs(), w)
                                            != JudgmentKind::Miss
                                })
                                .map(|cand| {
                                    let cand_pt = *cand;
                                    let next_owner_is_max =
                                        calc_hit_kind((cand_pt - next_head_time).abs(), w)
                                            == JudgmentKind::Max;
                                    !(next_owner_is_max && fol_tap_max)
                                })
                                .unwrap_or(false)
                        })
                        .unwrap_or(false)
            })
            .unwrap_or(false);
    let pre_bound_ln_pair = ghost_prehead
        && ho.is_long_note()
        && !next_tap_follow_chain
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale
                    && selected_pt == prev_t + w.hit100
                    && ho.time - prev_t <= w.hit50 * 2
            })
            .unwrap_or(false)
        && matches!(
            calc_hit_kind((selected_pt - ho.time).abs(), w),
            JudgmentKind::Max | JudgmentKind::Hit300
        )
        && ho
            .end_time
            .zip(col_notes.get(note_pos + 1))
            .zip(next_note_time)
            .map(|((end_time, (_, next_ho)), next_head_time)| {
                if !next_ho.is_long_note() {
                    return false;
                }
                let tail_start = end_time - w.hit50;
                let tail_end_exclusive = end_time + w.hit100;
                let selected_release = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let has_cur_fol_pre_end = presses
                    .iter()
                    .skip(selected_idx + 1)
                    .take_while(|cand| **cand < end_time)
                    .any(|cand| !reserved_ln_repr.contains(cand));
                let next_window_start = next_head_time - w.hit50;
                let next_win_end = next_head_time + w.hit100;
                let next_end = next_ho.end_time.unwrap_or(next_head_time);
                let next_tail_start = next_end - w.hit50;
                let next_tail_end = next_end + w.hit100;
                !has_cur_fol_pre_end
                    && selected_release
                        .map(|rt| {
                            rt >= tail_start && rt < tail_end_exclusive && rt < next_head_time
                        })
                        .unwrap_or(false)
                    && presses
                        .iter()
                        .skip(selected_idx + 1)
                        .take_while(|cand| **cand < next_win_end)
                        .any(|cand| {
                            let cand_pt = *cand;
                            cand_pt >= next_window_start
                                && !reserved_ln_repr.contains(cand)
                                && selected_release.map(|rt| cand_pt > rt).unwrap_or(true)
                                && events
                                    .iter()
                                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                                    .map(|ev| ev.time >= next_tail_start && ev.time < next_tail_end)
                                    .unwrap_or(false)
                        })
            })
            .unwrap_or(false);
    if pre_hold_weak_follow {
        ghost_prehead = false;
        prev_miss_clear_rule = Some("prev_miss_hold_pref");
    }
    if pre_hold_wide_gap {
        ghost_prehead = false;
        prev_miss_clear_rule = Some("prev_miss_hold_gap");
    }
    if pre_bound_h200_chain {
        ghost_prehead = false;
        prev_miss_clear_rule = Some("prev_miss_h200_chain");
    }
    if pre_bound_h200_ln {
        ghost_prehead = false;
        prev_miss_clear_rule = Some("prev_miss_h200_ln");
    }
    if pre_bound_h200_inlock {
        ghost_prehead = false;
        prev_miss_clear_rule = Some("prev_miss_h200_inlock");
    }
    if pre_bound_strong_tap {
        ghost_prehead = false;
        prev_miss_clear_rule = Some("prev_miss_strong_tap");
    }
    if pre_bound_quick_tap {
        ghost_prehead = false;
        prev_miss_clear_rule = Some("prev_miss_quick_tap");
    }
    if pre_bound_h200_h300 {
        ghost_prehead = false;
        prev_miss_clear_rule = Some("prev_miss_h200_h300");
    }
    if pre_bound_h300_gap {
        ghost_prehead = false;
        prev_miss_clear_rule = Some("prev_miss_h300_gap");
    }
    if pre_bound_ln_pair {
        ghost_prehead = false;
        prev_miss_clear_rule = Some("prev_miss_ln_pair");
    }
    if !ho.is_long_note()
        && selected_idx == original_selected_idx
        && selected_pt < ho.time
        && prev_was_miss
        && !prev_had_prewin_pen
        && prev_note_miss_time
            .map(|prev_t| selected_pt == prev_t + w.hit100)
            .unwrap_or(false)
        && selected_idx + 1 < presses.len()
    {
        if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
            if !next_ho.is_long_note() {
                let current_kind = calc_hit_kind((selected_pt - ho.time).abs(), w);
                let next_head_time = next_ho.time;
                let next_window_start = next_head_time - w.hit50;
                let next_win_end = next_head_time + w.hit100;
                let next_next_tap_head =
                    col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                        (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                    });
                let followup_idx = selected_idx + 1;
                let followup_pt = presses[followup_idx];
                let followup_kind = calc_hit_kind((followup_pt - ho.time).abs(), w);
                let rel_pre_fol = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time < followup_pt)
                    .unwrap_or(false);
                let fol_rel_pre_next = events
                    .iter()
                    .find(|ev| ev.time > followup_pt && !ev.pressed)
                    .map(|ev| ev.time < next_head_time)
                    .unwrap_or(false);
                let next_tap_has_cand = presses
                    .iter()
                    .skip(followup_idx + 1)
                    .take_while(|cand| **cand < next_win_end)
                    .any(|cand| {
                        let cand_pt = *cand;
                        cand_pt >= next_window_start
                            && next_next_tap_head
                                .map(|next_next_head| cand_pt < next_next_head)
                                .unwrap_or(true)
                            && !reserved_ln_repr.contains(cand)
                            && calc_hit_kind((cand_pt - next_head_time).abs(), w)
                                != JudgmentKind::Miss
                    });
                if matches!(current_kind, JudgmentKind::Hit50 | JudgmentKind::Hit100)
                    && followup_pt > selected_pt
                    && followup_pt >= ho.time - w.hit50
                    && followup_pt < ho.time
                    && matches!(followup_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                    && rel_pre_fol
                    && fol_rel_pre_next
                    && next_tap_has_cand
                {
                    selected_pt = followup_pt;
                    selected_idx = followup_idx;
                    ghost_prehead = false;
                    prev_miss_clear_rule = Some("prev_miss_prom_tap");
                }
            }
        }
    }
    if !ho.is_long_note()
        && !tap_micro_keep_idx
        && !prewin_follow_next_ln
        && selected_idx == original_selected_idx
        && selected_pt < ho.time
    {
        if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
            if next_ho.is_long_note() && selected_idx + 1 < presses.len() {
                let current_kind = calc_hit_kind((selected_pt - ho.time).abs(), w);
                let next_window_start = next_ho.time - w.hit50;
                let next_next_note_time = col_notes.get(note_pos + 2).map(|(_, ho)| ho.time);
                let next_ln_late_end = next_next_note_time
                    .map(|next_time| next_time <= next_ho.time + w.hit50)
                    .unwrap_or(false);
                let next_lock_end = next_ho.time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                let followup_idx = selected_idx + 1;
                let followup_pt = presses[followup_idx];
                let followup_kind = calc_hit_kind((followup_pt - ho.time).abs(), w);
                let rel_pre_fol = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time < followup_pt)
                    .unwrap_or(false);
                let next_ln_head_cand = presses
                    .iter()
                    .skip(followup_idx + 1)
                    .take_while(|cand| **cand < next_lock_end)
                    .any(|cand| *cand >= next_window_start && !reserved_ln_repr.contains(cand));
                if selected_pt < next_window_start
                    && current_kind != JudgmentKind::Miss
                    && followup_pt > selected_pt
                    && followup_pt - selected_pt <= 2
                    && followup_pt < ho.time
                    && followup_pt < next_window_start
                    && !reserved_ln_repr.contains(&followup_pt)
                    && !rel_pre_fol
                    && followup_kind == current_kind
                    && next_ln_head_cand
                {
                    selected_pt = followup_pt;
                    selected_idx = followup_idx;
                    prewin_follow_next_ln = true;
                }
            }
        }
    }
    if !ho.is_long_note()
        && !tap_micro_keep_idx
        && !prewin_follow_next_ln
        && selected_idx == original_selected_idx
        && selected_pt < ho.time
        && prev_was_miss
        && !prev_had_prewin_pen
        && prev_note_miss_time
            .map(|prev_t| {
                let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                prev_press_is_stale && selected_pt <= prev_t + w.hit100 + 1
            })
            .unwrap_or(false)
        && events
            .iter()
            .find(|ev| ev.time > selected_pt && !ev.pressed)
            .map(|ev| ev.time < ho.time)
            .unwrap_or(false)
    {
        if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
            if !next_ho.is_long_note() && selected_idx + 1 < presses.len() {
                let current_kind = calc_hit_kind((selected_pt - ho.time).abs(), w);
                let next_head_time = next_ho.time;
                let next_window_start = next_head_time - w.hit50;
                let next_win_end = next_head_time + w.hit100;
                let next_next_tap_head =
                    col_notes.get(note_pos + 2).and_then(|(_, next_next_ho)| {
                        (!next_next_ho.is_long_note()).then_some(next_next_ho.time)
                    });
                let followup_idx = selected_idx + 1;
                let followup_pt = presses[followup_idx];
                let followup_kind = calc_hit_kind((followup_pt - ho.time).abs(), w);
                let rel_pre_fol = events
                    .iter()
                    .find(|ev| ev.time > selected_pt && !ev.pressed)
                    .map(|ev| ev.time < followup_pt)
                    .unwrap_or(false);
                let followup_rel_time = events
                    .iter()
                    .find(|ev| ev.time > followup_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let fol_rel_pre_next = followup_rel_time
                    .map(|rt| rt < next_head_time)
                    .unwrap_or(false);
                let next_tap_own_cand = presses
                    .iter()
                    .skip(followup_idx + 1)
                    .take_while(|cand| **cand < next_win_end)
                    .find(|cand| {
                        **cand >= next_window_start
                            && next_next_tap_head
                                .map(|next_next_head| **cand < next_next_head)
                                .unwrap_or(true)
                            && !reserved_ln_repr.contains(cand)
                            && calc_hit_kind(((*cand) - next_head_time).abs(), w)
                                != JudgmentKind::Miss
                    })
                    .copied();
                let nex_tap_own_pre_nnex = next_tap_own_cand
                    .map(|next_tap_pt| {
                        next_next_tap_head
                            .map(|next_next_head| next_tap_pt < next_next_head - w.hit100)
                            .unwrap_or(true)
                    })
                    .unwrap_or(false);
                let next_tap_strong = presses
                    .iter()
                    .skip(followup_idx + 1)
                    .take_while(|cand| **cand < next_win_end)
                    .any(|cand| {
                        *cand >= next_window_start
                            && next_next_tap_head
                                .map(|next_next_head| *cand < next_next_head)
                                .unwrap_or(true)
                            && !reserved_ln_repr.contains(cand)
                            && matches!(
                                calc_hit_kind(((*cand) - next_head_time).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300
                            )
                    });
                let next2_has_cand = next_next_tap_head
                    .map(|next_next_head| {
                        let next2_win_start = next_next_head - w.hit50;
                        let next2_win_end = next_next_head + w.hit100;
                        let next3_tap_head =
                            col_notes
                                .get(note_pos + 3)
                                .and_then(|(_, next_next_next_ho)| {
                                    (!next_next_next_ho.is_long_note())
                                        .then_some(next_next_next_ho.time)
                                });
                        presses
                            .iter()
                            .skip(followup_idx + 1)
                            .take_while(|cand| **cand < next2_win_end)
                            .any(|cand| {
                                let cand_pt = *cand;
                                cand_pt >= next2_win_start
                                    && next3_tap_head.map(|head| cand_pt < head).unwrap_or(true)
                                    && !reserved_ln_repr.contains(cand)
                                    && calc_hit_kind((cand_pt - next_next_head).abs(), w)
                                        != JudgmentKind::Miss
                            })
                    })
                    .unwrap_or(false);
                let wide_dense_follow = same_time_tap_count >= 3
                    && next_head_time - ho.time > w.hit50 * 2
                    && followup_pt < next_window_start
                    && next_window_start - followup_pt > w.hit300 + w.max
                    && next_tap_strong
                    && nex_tap_own_pre_nnex
                    && next_next_tap_head
                        .map(|next_next_head| {
                            next_next_head - next_head_time < next_head_time - ho.time
                        })
                        .unwrap_or(false);
                let fol_rel_pre_tap = followup_rel_time
                    .map(|rel_time| {
                        next_tap_own_cand
                            .map(|next_tap_pt| rel_time < next_tap_pt)
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                let follow_starts_tap = followup_pt >= next_window_start - early_penalty_window - 1
                    && followup_pt < next_window_start
                    && calc_hit_kind((followup_pt - next_head_time).abs(), w) == JudgmentKind::Miss
                    && next_tap_strong
                    && nex_tap_own_pre_nnex;
                let follow_remains_tap = followup_pt >= next_window_start
                    && followup_pt < next_head_time
                    && calc_hit_kind((followup_pt - next_head_time).abs(), w)
                        == JudgmentKind::Hit50
                    && next_tap_strong
                    && nex_tap_own_pre_nnex
                    && fol_rel_pre_next;
                let follow_stays_cur = if current_kind == JudgmentKind::Hit200 {
                    (followup_pt < next_window_start && next_tap_strong && !follow_starts_tap)
                        || (followup_pt >= next_window_start
                            && followup_pt < next_head_time
                            && (next_tap_strong || fol_rel_pre_tap))
                } else {
                    wide_dense_follow
                        || (followup_pt >= next_window_start
                            && followup_pt < next_head_time
                            && nex_tap_own_pre_nnex
                            && (next_tap_strong || fol_rel_pre_tap))
                };
                let follow_h200_dense = current_kind == JudgmentKind::Hit100
                    && followup_kind == JudgmentKind::Hit200
                    && followup_pt >= ho.time
                    && next_head_time - ho.time <= w.hit50
                    && next_tap_own_cand.is_none()
                    && followup_rel_time
                        .map(|rel_time| {
                            rel_time >= next_head_time && rel_time <= next_head_time + w.max
                        })
                        .unwrap_or(false)
                    && next2_has_cand;
                let follow_strong_h200 = current_kind == JudgmentKind::Hit100
                    && followup_pt >= ho.time
                    && followup_pt < next_head_time
                    && next_head_time - ho.time <= w.hit50
                    && matches!(
                        followup_kind,
                        JudgmentKind::Max | JudgmentKind::Hit300 | JudgmentKind::Hit200
                    )
                    && calc_hit_kind((followup_pt - next_head_time).abs(), w)
                        == JudgmentKind::Hit200
                    && next_tap_own_cand.is_none()
                    && next2_has_cand
                    && followup_rel_time
                        .map(|rel_time| {
                            rel_time > next_head_time + w.hit300
                                && next_next_tap_head
                                    .map(|next_next_head| rel_time < next_next_head)
                                    .unwrap_or(true)
                        })
                        .unwrap_or(false);
                let follow_max_h200 = current_kind == JudgmentKind::Hit100
                    && followup_pt >= ho.time
                    && followup_pt < next_head_time
                    && next_head_time - ho.time <= w.hit50
                    && matches!(followup_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                    && calc_hit_kind((followup_pt - next_head_time).abs(), w)
                        == JudgmentKind::Hit200
                    && next_tap_own_cand.is_none()
                    && next2_has_cand
                    && followup_rel_time
                        .map(|rel_time| {
                            rel_time > next_head_time + w.max
                                && next_next_tap_head
                                    .map(|next_next_head| rel_time < next_next_head)
                                    .unwrap_or(true)
                        })
                        .unwrap_or(false);
                if matches!(current_kind, JudgmentKind::Hit100 | JudgmentKind::Hit200)
                    && (next_head_time - ho.time <= w.hit50 * 2 || wide_dense_follow)
                    && followup_pt > ho.time
                    && followup_pt < next_head_time
                    && !reserved_ln_repr.contains(&followup_pt)
                    && (matches!(followup_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                        || follow_h200_dense
                        || follow_strong_h200
                        || follow_max_h200)
                    && followup_kind.score_value() > current_kind.score_value()
                    && !follow_remains_tap
                    && rel_pre_fol
                    && ((fol_rel_pre_next || fol_rel_pre_tap)
                        || follow_h200_dense
                        || follow_strong_h200
                        || follow_max_h200)
                    && (follow_stays_cur
                        || follow_h200_dense
                        || follow_strong_h200
                        || follow_max_h200)
                {
                    selected_pt = followup_pt;
                    selected_idx = followup_idx;
                    pre_mis_pos_hea_prom = true;
                }
            }
        }
    }
    if !ho.is_long_note()
        && !tap_micro_keep_idx
        && selected_idx == original_selected_idx
        && selected_pt > ho.time
    {
        if let Some((_, next_ho)) = col_notes.get(note_pos + 1) {
            if !next_ho.is_long_note() {
                let current_kind = calc_hit_kind((selected_pt - ho.time).abs(), w);
                let next_tap_sel_kind = calc_hit_kind((selected_pt - next_ho.time).abs(), w);
                let cand_out_next_pen = col_notes
                    .get(note_pos + 2)
                    .map(|(_, next_next_ho)| {
                        let next2_early_start =
                            next_next_ho.time - w.hit50 - early_penalty_window - 1;
                        selected_pt < next2_early_start
                    })
                    .unwrap_or(true);
                if matches!(current_kind, JudgmentKind::Hit100 | JudgmentKind::Hit50)
                    && selected_pt >= next_ho.time - w.hit50
                    && selected_pt < next_ho.time
                    && matches!(next_tap_sel_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                    && cand_out_next_pen
                    && selected_idx + 1 < presses.len()
                {
                    let followup_idx = selected_idx + 1;
                    let followup_pt = presses[followup_idx];
                    let followup_kind = calc_hit_kind((followup_pt - ho.time).abs(), w);
                    let rel_pre_fol = events
                        .iter()
                        .find(|ev| ev.time > selected_pt && !ev.pressed)
                        .map(|ev| ev.time < followup_pt)
                        .unwrap_or(false);
                    if followup_pt > selected_pt
                        && followup_pt - selected_pt <= 2
                        && followup_pt < lock_end_exclusive
                        && followup_pt < next_ho.time
                        && !reserved_ln_repr.contains(&followup_pt)
                        && !rel_pre_fol
                        && followup_kind == current_kind
                    {
                        selected_pt = followup_pt;
                        selected_idx = followup_idx;
                        tap_micro_keep_idx = true;
                    }
                }
            }
        }
    }
    state.head_candidate.selected_pt = selected_pt;
    state.head_candidate.selected_idx = selected_idx;
    state.head_candidate.tap_micro_keeps_idx = tap_micro_keep_idx;
    state.head_candidate.prewin_follow_next_ln = prewin_follow_next_ln;
    state.head_candidate.pre_mis_pos_hea_prom = pre_mis_pos_hea_prom;
    state.head_candidate.ghost_prehead = ghost_prehead;
    state.head_candidate.prev_miss_clear_rule = prev_miss_clear_rule;
    state.head_candidate.prev_miss_hless300 = prev_miss_hless300;
    state.head_candidate.late_tap_cross_tap = late_tap_cross_tap;
    state.head_candidate.late_tap_dense_chain = late_tap_dense_chain;
    state.head_candidate.late_tap_iso_head = late_tap_iso_head;
    state.head_candidate.late_tap_cross_ln = late_tap_cross_ln;
    state.head_candidate.lat_tap_yild_next_ln = lat_tap_yild_next_ln;
    state.head_candidate.prev_miss_keeps_hless = prev_miss_keeps_hless;
}
