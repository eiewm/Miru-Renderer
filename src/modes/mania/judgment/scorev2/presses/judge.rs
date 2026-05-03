use super::note::{PressNoteCtx, PressState};
use crate::modes::mania::judgment::{calc_hit_kind, InternalJudgment};
use crate::types::JudgmentKind;
pub(super) fn finalize(ctx: &PressNoteCtx<'_>, state: &mut PressState, out: &[InternalJudgment]) {
    let _idx = ctx.idx;
    let note_pos = ctx.note_pos;
    let ho = ctx.ho;
    let col_notes = ctx.col_notes;
    let presses = ctx.presses;
    let events = ctx.events;
    let w = ctx.windows;
    let next_note_time = ctx.next_note_time;
    let note_window = ctx.note_window;
    let _window_start = note_window.window_start;
    let _lock_end_exclusive = note_window.lock_end_exclusive;
    let _next_window_start = note_window.next_window_start;
    let _early_penalty_window = note_window.early_penalty_window;
    let _last_note_idx_overall = ctx.last_note_idx_overall;
    let _terminal_extreme_ln_end_times = ctx.extreme_ln_ends;
    let mut press_idx = state.press_idx;
    let prev_was_miss = state.prev.was_miss;
    let prev_had_prewin_pen = state.prev.had_prewin_pen;
    let _prev_body_break_pre_tail = state.prev.body_break_pre_tail;
    let prev_col_pt = state.prev.col_pt;
    let late_tap_cross_ln = state.head_candidate.late_tap_cross_ln;
    let reserved_ln_repr = &mut state.prev.reserved_ln_repr;
    let press_time = state.pick.press;
    let tail_only_pt = state.pick.tail;
    let prev_stale_time = note_pos
        .checked_sub(1)
        .and_then(|p| col_notes.get(p))
        .map(|(_, prev_ho)| prev_ho.time);
    let (kind, delta) = if let Some(pt) = press_time {
        (calc_hit_kind((pt - ho.time).abs(), w), pt - ho.time)
    } else {
        (JudgmentKind::Miss, 0)
    };
    let mut final_press_time = press_time;
    let mut final_tail_pt = tail_only_pt;
    let mut final_kind = kind;
    let mut final_delta = delta;
    let mut _tail_post_handoff = false;
    if true && ho.is_long_note() {
        let end_time = ho.end_time.unwrap_or(ho.time);
        let ln_duration = end_time - ho.time;
        let tail_start = end_time - w.hit50;
        let prev_short_miss_ln = note_pos
            .checked_sub(1)
            .and_then(|p| col_notes.get(p))
            .and_then(|(_, prev_ho)| {
                let prev_end = prev_ho.end_time.unwrap_or(prev_ho.time);
                let prev_duration = prev_end - prev_ho.time;
                (prev_ho.is_long_note() && prev_duration <= w.hit100)
                    .then_some((prev_ho.time, prev_end))
            });
        let prev_short_hold = prev_col_pt
            .zip(prev_short_miss_ln)
            .map(|(prev_pt, (prev_t, prev_end))| {
                let prev_duration = prev_end - prev_t;
                events
                    .iter()
                    .find(|ev| ev.time > prev_pt && !ev.pressed)
                    .map(|ev| ev.time - prev_pt >= prev_duration)
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        let prev_same_col_ln_info = note_pos
            .checked_sub(1)
            .and_then(|p| col_notes.get(p))
            .and_then(|(_, prev_ho)| {
                prev_ho
                    .is_long_note()
                    .then_some((prev_ho.time, prev_ho.end_time.unwrap_or(prev_ho.time)))
            });
        if let Some(pt) = press_time {
            let release_after_pt = events
                .iter()
                .find(|ev| ev.time > pt && !ev.pressed)
                .map(|ev| ev.time);
            let late_ln_h50_post_end = matches!(kind, JudgmentKind::Hit50) && pt > end_time;
            let late_ln_h50_post_h100 = matches!(kind, JudgmentKind::Hit50) && delta > w.hit100;
            let late_ln_h100_post_end =
                matches!(kind, JudgmentKind::Hit100) && pt > end_time && pt >= ho.time + w.hit100;
            let late_ln_h200_post_end = matches!(kind, JudgmentKind::Hit200) && pt > end_time;
            let late_ln_h100_tail = matches!(kind, JudgmentKind::Hit100)
                && pt >= ho.time + w.hit100
                && pt <= end_time
                && (pt >= tail_start
                    || (pt >= tail_start - w.max
                        && release_after_pt
                            .map(|rt| rt >= tail_start && rt < end_time + w.hit100)
                            .unwrap_or(false)));
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
                .map(|release_t| release_t > ho.time && release_t < tail_start)
                .unwrap_or(false);
            let late_ln_h100_hold = matches!(kind, JudgmentKind::Hit100)
                && pt == ho.time + w.hit100
                && pre_break_near_tail
                && release_after_pt
                    .map(|rt| rt >= tail_start && rt <= end_time + w.hit100)
                    .unwrap_or(false);
            let short_post_end_h100 = matches!(kind, JudgmentKind::Hit100) && pt > end_time;
            let late_short_to_next_ln = (late_ln_h50_post_end
                || late_ln_h50_post_h100
                || late_ln_h100_post_end
                || late_ln_h200_post_end
                || short_post_end_h100)
                && ln_duration <= w.hit100
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
                        let next_ln_follow = presses
                            .iter()
                            .skip(press_idx + 1)
                            .take_while(|cand| **cand < next_win_end)
                            .find(|cand| {
                                **cand >= next_window_start && !reserved_ln_repr.contains(cand)
                            })
                            .copied();
                        let follow_ln_to_ln = next_ln_follow
                            .zip(col_notes.get(note_pos + 2))
                            .map(|(followup_pt, (_, next_next_ho))| {
                                if !next_next_ho.is_long_note() {
                                    return false;
                                }
                                let next_next_head = next_next_ho.time;
                                let next_next_end =
                                    next_next_ho.end_time.unwrap_or(next_next_ho.time);
                                let next2_win_start = next_next_head - w.hit50;
                                let next2_win_end = next_next_head + w.hit100;
                                let next_next_tail_start = next_next_end - w.hit50;
                                let next2_tail_end = next_next_end + w.hit100;
                                let rel_post_fol = events
                                    .iter()
                                    .find(|ev| ev.time > followup_pt && !ev.pressed)
                                    .map(|ev| ev.time);
                                followup_pt >= next2_win_start
                                    && followup_pt < next2_win_end
                                    && rel_post_fol
                                        .map(|rt| rt >= next_next_tail_start && rt < next2_tail_end)
                                        .unwrap_or(false)
                            })
                            .unwrap_or(false);
                        let has_blc_fol_nex_ln_pt = next_ln_follow.is_some() && !follow_ln_to_ln;
                        let nex_cha_fol_nex_ln_pt = presses
                            .iter()
                            .skip(press_idx)
                            .take_while(|cand| **cand < next_win_end)
                            .find(|cand| {
                                **cand != pt
                                    && **cand >= next_window_start
                                    && !reserved_ln_repr.contains(cand)
                            })
                            .copied();
                        let cha_fol_to_imm_fol_ln = nex_cha_fol_nex_ln_pt
                            .zip(col_notes.get(note_pos + 2))
                            .map(|(followup_pt, (_, next_next_ho))| {
                                if !next_next_ho.is_long_note() {
                                    return false;
                                }
                                let next_next_head = next_next_ho.time;
                                let next_next_end =
                                    next_next_ho.end_time.unwrap_or(next_next_ho.time);
                                let next2_win_start = next_next_head - w.hit50;
                                let next2_win_end = next_next_head + w.hit100;
                                let next_next_tail_start = next_next_end - w.hit50;
                                let next2_tail_end = next_next_end + w.hit100;
                                let rel_post_fol = events
                                    .iter()
                                    .find(|ev| ev.time > followup_pt && !ev.pressed)
                                    .map(|ev| ev.time);
                                followup_pt >= next2_win_start
                                    && followup_pt < next2_win_end
                                    && rel_post_fol
                                        .map(|rt| rt >= next_next_tail_start && rt < next2_tail_end)
                                        .unwrap_or(false)
                            })
                            .unwrap_or(false);
                        let release_feeds_next_ln = release_after_pt
                            .map(|rt| rt >= next_tail_start && rt < next_tail_end)
                            .unwrap_or(false);
                        let short_post_end_h50 = matches!(kind, JudgmentKind::Hit50)
                            && pt > end_time
                            && pt >= next_head
                            && pt <= next_head + w.hit300
                            && release_feeds_next_ln
                            && cha_fol_to_imm_fol_ln;
                        let h100_feeds_next_ln =
                            short_post_end_h100 && release_feeds_next_ln && !has_blc_fol_nex_ln_pt;
                        let h50_feed_nln = late_ln_h50_post_end
                            && pt >= next_head
                            && pt <= next_head + w.hit200
                            && release_feeds_next_ln
                            && !has_blc_fol_nex_ln_pt;
                        let h200_feed_nln = late_ln_h200_post_end
                            && pt >= next_head
                            && matches!(
                                calc_hit_kind((pt - next_head).abs(), w),
                                JudgmentKind::Max | JudgmentKind::Hit300
                            )
                            && release_feeds_next_ln
                            && !has_blc_fol_nex_ln_pt;
                        h50_feed_nln
                            || h200_feed_nln
                            || (pt >= next_head
                                && pt <= next_head + w.hit300
                                && (short_post_end_h50
                                    || h100_feeds_next_ln
                                    || (!has_blc_fol_nex_ln_pt
                                        && release_after_pt
                                            .map(|rt| {
                                                rt > end_time + w.hit100
                                                    && rt <= next_end + w.hit100
                                            })
                                            .unwrap_or(false))))
                    })
                    .unwrap_or(false);
            if late_ln_h50_post_end
                || late_ln_h50_post_h100
                || late_ln_h100_post_end
                || late_ln_h100_tail
                || late_ln_h100_hold
                || late_short_to_next_ln
            {
                final_press_time = None;
                final_kind = JudgmentKind::Miss;
                final_delta = 0;
                if late_short_to_next_ln {
                    final_tail_pt = None;
                    if press_idx > 0 {
                        let prev_idx = press_idx - 1;
                        if presses.get(prev_idx).copied() == Some(pt) {
                            press_idx = prev_idx;
                        }
                    }
                } else {
                    final_tail_pt = Some(pt);
                    reserved_ln_repr.insert(pt);
                }
            }
            let cur_rel_before_head = events
                .iter()
                .find(|ev| ev.time > pt && !ev.pressed)
                .map(|ev| ev.time < ho.time)
                .unwrap_or(false);
            let late_post_head_tail = events
                .iter()
                .filter(|ev| ev.pressed && ev.time > pt && ev.time <= end_time)
                .any(|press_ev| {
                    events
                        .iter()
                        .find(|ev| ev.time > press_ev.time && !ev.pressed)
                        .map(|ev| ev.time >= tail_start && ev.time < end_time + w.hit100)
                        .unwrap_or(false)
                });
            let prewin_miss_pref = final_kind == JudgmentKind::Miss
                && pt < ho.time
                && cur_rel_before_head
                && late_post_head_tail;
            if prewin_miss_pref {
                let prior_prewin_frag = events
                    .iter()
                    .rev()
                    .filter(|ev| ev.pressed && ev.time < pt)
                    .find_map(|press_ev| {
                        let cand_pt = press_ev.time;
                        let cand_release = events
                            .iter()
                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                            .map(|ev| ev.time);
                        let already_assigned = out
                            .iter()
                            .any(|jj| jj.column == ho.column && jj.press_time == Some(cand_pt));
                        let cand_rel_pre_head = cand_release
                            .map(|rt| rt < ho.time && rt < pt)
                            .unwrap_or(false);
                        if prev_col_pt
                            .map(|prev_pt| cand_pt <= prev_pt)
                            .unwrap_or(false)
                            || already_assigned
                            || pt - cand_pt > w.hit300
                            || !cand_rel_pre_head
                        {
                            return None;
                        }
                        Some(cand_pt)
                    });
                if let Some(cand_pt) = prior_prewin_frag {
                    final_press_time = Some(cand_pt);
                    final_delta = cand_pt - ho.time;
                }
            }
            let short_miss_pref_later = ln_duration <= w.hit100
                && final_kind == JudgmentKind::Miss
                && calc_hit_kind((pt - ho.time).abs(), w) == JudgmentKind::Miss
                && cur_rel_before_head
                && late_post_head_tail;
            if short_miss_pref_later {
                let current_release = events
                    .iter()
                    .find(|ev| ev.time > pt && !ev.pressed)
                    .map(|ev| ev.time);
                let later_prewin_frag = current_release.and_then(|cur_release| {
                    events
                        .iter()
                        .filter(|ev| ev.pressed && ev.time > pt && ev.time < ho.time)
                        .find_map(|press_ev| {
                            let cand_pt = press_ev.time;
                            let cand_release = events
                                .iter()
                                .find(|ev| ev.time > cand_pt && !ev.pressed)
                                .map(|ev| ev.time)?;
                            let immediate_repress = events
                                .iter()
                                .find(|ev| ev.pressed && ev.time > cand_release)
                                .map(|ev| ev.time)?;
                            let imm_repr_rel = events
                                .iter()
                                .find(|ev| ev.time > immediate_repress && !ev.pressed)
                                .map(|ev| ev.time)?;
                            let already_assigned = out
                                .iter()
                                .any(|jj| jj.column == ho.column && jj.press_time == Some(cand_pt));
                            if already_assigned
                                || cand_pt - pt > w.hit300
                                || calc_hit_kind((cand_pt - ho.time).abs(), w) != JudgmentKind::Miss
                                || cur_release >= cand_pt
                                || cand_release >= ho.time
                                || immediate_repress <= ho.time
                                || imm_repr_rel < tail_start
                                || imm_repr_rel >= end_time + w.hit100
                            {
                                return None;
                            }
                            Some(cand_pt)
                        })
                });
                if let Some(cand_pt) = later_prewin_frag {
                    final_press_time = Some(cand_pt);
                    final_tail_pt = None;
                    final_delta = cand_pt - ho.time;
                }
            }
            let short_post_short_pref = ln_duration <= w.hit100
                && !matches!(final_kind, JudgmentKind::Miss)
                && prev_was_miss
                && prev_had_prewin_pen
                && prev_short_miss_ln.is_some()
                && prev_short_hold
                && pt >= ho.time - w.max
                && pt <= end_time
                && release_after_pt
                    .map(|rt| {
                        rt >= tail_start
                            && rt < end_time + w.hit100
                            && next_note_time.map(|nt| rt < nt).unwrap_or(true)
                    })
                    .unwrap_or(false);
            if short_post_short_pref {
                let prior_body_fragment = prev_col_pt.zip(prev_short_miss_ln).and_then(
                    |(prev_pt, (prev_t, prev_end))| {
                        events
                            .iter()
                            .rev()
                            .filter(|ev| ev.pressed && ev.time < pt)
                            .find_map(|press_ev| {
                                let cand_pt = press_ev.time;
                                let cand_release = events
                                    .iter()
                                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                                    .map(|ev| ev.time)?;
                                let immediate_repress = events
                                    .iter()
                                    .find(|ev| ev.pressed && ev.time > cand_release)
                                    .map(|ev| ev.time);
                                let already_assigned = out.iter().any(|jj| {
                                    jj.column == ho.column && jj.press_time == Some(cand_pt)
                                });
                                if cand_pt <= prev_pt
                                    || cand_pt >= ho.time
                                    || cand_pt > prev_end
                                    || already_assigned
                                    || calc_hit_kind((cand_pt - ho.time).abs(), w)
                                        != JudgmentKind::Miss
                                    || cand_release <= prev_t
                                    || cand_release > prev_end + 1
                                    || cand_release >= ho.time
                                    || immediate_repress != Some(pt)
                                {
                                    return None;
                                }
                                Some(cand_pt)
                            })
                    },
                );
                if let Some(cand_pt) = prior_body_fragment {
                    final_press_time = Some(cand_pt);
                    final_kind = JudgmentKind::Miss;
                    final_delta = cand_pt - ho.time;
                }
            }
            let prev_miss_ln_frag = prev_short_miss_ln.or_else(|| {
                note_pos
                    .checked_sub(1)
                    .and_then(|p| col_notes.get(p))
                    .and_then(|(_, prev_ho)| {
                        prev_ho
                            .is_long_note()
                            .then_some((prev_ho.time, prev_ho.end_time.unwrap_or(prev_ho.time)))
                    })
            });
            let short_noprs_gap_pref = true
                && ln_duration <= w.hit100
                && !matches!(final_kind, JudgmentKind::Miss)
                && prev_was_miss
                && !prev_had_prewin_pen
                && (prev_col_pt.is_none() || prev_short_hold)
                && prev_miss_ln_frag.is_some();
            if short_noprs_gap_pref {
                let cur_sel_pt = final_press_time.filter(|cur_pt| {
                    *cur_pt >= ho.time - w.max
                        && *cur_pt <= ho.time + w.hit300
                        && events
                            .iter()
                            .find(|ev| ev.time > *cur_pt && !ev.pressed)
                            .map(|rt_ev| {
                                rt_ev.time >= tail_start
                                    && rt_ev.time < end_time + w.hit100
                                    && next_note_time.map(|nt| rt_ev.time < nt).unwrap_or(true)
                            })
                            .unwrap_or(false)
                });
                let prior_gap_fragment = cur_sel_pt.and_then(|cur_pt| {
                    prev_miss_ln_frag.and_then(|(prev_t, prev_end)| {
                        let current_selected_kind = calc_hit_kind((cur_pt - ho.time).abs(), w);
                        let pre_own_pt_lowe_boun = prev_col_pt.unwrap_or(prev_t);
                        events
                            .iter()
                            .rev()
                            .filter(|ev| ev.pressed && ev.time < cur_pt)
                            .find_map(|press_ev| {
                                let cand_pt = press_ev.time;
                                let cand_release = events
                                    .iter()
                                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                                    .map(|ev| ev.time)?;
                                let immediate_repress = events
                                    .iter()
                                    .find(|ev| ev.pressed && ev.time > cand_release)
                                    .map(|ev| ev.time);
                                let already_assigned = out.iter().any(|jj| {
                                    jj.column == ho.column && jj.press_time == Some(cand_pt)
                                });
                                let strong_short_pref = matches!(
                                    current_selected_kind,
                                    JudgmentKind::Max | JudgmentKind::Hit300
                                ) && cur_pt >= ho.time
                                    && cur_pt <= end_time
                                    && cand_pt >= prev_end - (w.max + 4)
                                    && cand_pt <= prev_end
                                    && cand_release > prev_end
                                    && cand_release < ho.time
                                    && immediate_repress == Some(cur_pt);
                                if cand_pt <= pre_own_pt_lowe_boun
                                    || cand_pt >= ho.time
                                    || cand_pt < prev_end - w.hit300
                                    || cand_pt > prev_end
                                    || already_assigned
                                    || strong_short_pref
                                    || calc_hit_kind((cand_pt - ho.time).abs(), w)
                                        != JudgmentKind::Miss
                                    || cand_release <= prev_end
                                    || cand_release >= ho.time
                                    || cand_release > prev_end + w.hit50
                                    || immediate_repress != Some(cur_pt)
                                {
                                    return None;
                                }
                                Some(cand_pt)
                            })
                    })
                });
                if let Some(cand_pt) = prior_gap_fragment {
                    final_press_time = Some(cand_pt);
                    final_tail_pt = None;
                    final_kind = JudgmentKind::Miss;
                    final_delta = cand_pt - ho.time;
                }
            }
            let prev_hid_tail_pt =
                out.iter()
                    .rev()
                    .find(|jj| jj.column == ho.column)
                    .and_then(|jj| {
                        (true
                            && jj.is_ln
                            && jj.kind == JudgmentKind::Miss
                            && jj.press_time.is_none()
                            && jj.early_pen_win.is_none()
                            && jj.early_press_idx == prev_col_pt)
                            .then_some(jj.early_press_idx)
                            .flatten()
                    });
            let shor_ln_hid_tail_gap = true
                && ln_duration <= w.hit100
                && !matches!(final_kind, JudgmentKind::Miss)
                && prev_was_miss
                && !prev_had_prewin_pen
                && prev_hid_tail_pt.is_some()
                && prev_miss_ln_frag.is_some();
            let long_nopress_gap = true
                && ho.is_long_note()
                && ln_duration > w.hit100
                && !matches!(final_kind, JudgmentKind::Miss)
                && prev_was_miss
                && !prev_had_prewin_pen
                && prev_hid_tail_pt.is_some()
                && prev_miss_ln_frag.is_some();
            if shor_ln_hid_tail_gap || long_nopress_gap {
                let cur_sel_pt = final_press_time.filter(|cur_pt| {
                    *cur_pt >= ho.time - w.max
                        && *cur_pt <= ho.time + w.hit300
                        && events
                            .iter()
                            .find(|ev| ev.time > *cur_pt && !ev.pressed)
                            .map(|rt_ev| {
                                rt_ev.time >= tail_start
                                    && rt_ev.time < end_time + w.hit100
                                    && next_note_time.map(|nt| rt_ev.time < nt).unwrap_or(true)
                            })
                            .unwrap_or(false)
                });
                let hid_tail_bound_frag = cur_sel_pt.and_then(|cur_pt| {
                    prev_miss_ln_frag.and_then(|(prev_t, prev_end)| {
                        let current_selected_kind = calc_hit_kind((cur_pt - ho.time).abs(), w);
                        let cand_pt = prev_hid_tail_pt?;
                        let cand_release = events
                            .iter()
                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                            .map(|ev| ev.time)?;
                        let immediate_repress = events
                            .iter()
                            .find(|ev| ev.pressed && ev.time > cand_release)
                            .map(|ev| ev.time);
                        let already_assigned = out
                            .iter()
                            .any(|jj| jj.column == ho.column && jj.press_time == Some(cand_pt));
                        let strong_short_pref = matches!(
                            current_selected_kind,
                            JudgmentKind::Max | JudgmentKind::Hit300
                        ) && shor_ln_hid_tail_gap
                            && cur_pt >= ho.time
                            && cur_pt <= end_time
                            && cand_pt >= prev_end - (w.max + 4)
                            && cand_pt <= prev_end
                            && cand_release > prev_end
                            && cand_release < ho.time
                            && immediate_repress == Some(cur_pt);
                        let prev_hidden_break_cur = long_nopress_gap
                            && cand_release > prev_t
                            && cand_release <= prev_end + 1
                            && cand_release < ho.time
                            && immediate_repress == Some(cur_pt);
                        let prev_hidden_to_cur = cand_release > prev_end
                            && cand_release < ho.time
                            && cand_release <= prev_end + w.hit300
                            && immediate_repress == Some(cur_pt);
                        if cand_pt <= prev_t
                            || cand_pt < prev_end - w.max
                            || cand_pt > prev_end
                            || already_assigned
                            || strong_short_pref
                            || calc_hit_kind((cand_pt - ho.time).abs(), w) != JudgmentKind::Miss
                            || !(prev_hidden_to_cur || prev_hidden_break_cur)
                        {
                            return None;
                        }
                        Some(cand_pt)
                    })
                });
                if let Some(cand_pt) = hid_tail_bound_frag {
                    final_press_time = Some(cand_pt);
                    final_tail_pt = None;
                    final_kind = JudgmentKind::Miss;
                    final_delta = cand_pt - ho.time;
                }
            }
            let short_post_body_pref = true
                && ln_duration <= w.hit100
                && final_kind == JudgmentKind::Miss
                && !prev_was_miss
                && prev_same_col_ln_info.is_some()
                && pt < ho.time
                && release_after_pt
                    .map(|rt| {
                        rt >= tail_start
                            && rt < ho.time
                            && next_note_time.map(|nt| rt < nt).unwrap_or(true)
                    })
                    .unwrap_or(false);
            if short_post_body_pref {
                let prior_body_fragment = prev_col_pt.zip(prev_same_col_ln_info).and_then(
                    |(prev_pt, (prev_t, prev_end))| {
                        events
                            .iter()
                            .rev()
                            .filter(|ev| ev.pressed && ev.time < pt)
                            .find_map(|press_ev| {
                                let cand_pt = press_ev.time;
                                let cand_release = events
                                    .iter()
                                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                                    .map(|ev| ev.time)?;
                                let immediate_repress = events
                                    .iter()
                                    .find(|ev| ev.pressed && ev.time > cand_release)
                                    .map(|ev| ev.time);
                                let already_assigned = out.iter().any(|jj| {
                                    jj.column == ho.column && jj.press_time == Some(cand_pt)
                                });
                                if cand_pt <= prev_pt
                                    || cand_pt >= ho.time
                                    || cand_pt < prev_end - w.hit50
                                    || cand_pt > prev_end
                                    || already_assigned
                                    || cand_release <= prev_t
                                    || cand_release > prev_end + 1
                                    || cand_release >= ho.time
                                    || immediate_repress != Some(pt)
                                {
                                    return None;
                                }
                                Some(cand_pt)
                            })
                    },
                );
                if let Some(cand_pt) = prior_body_fragment {
                    final_press_time = Some(cand_pt);
                    final_delta = cand_pt - ho.time;
                }
            }
            let long_post_tail_pref = true
                && ln_duration > w.hit100
                && !matches!(final_kind, JudgmentKind::Miss)
                && !prev_was_miss
                && prev_same_col_ln_info
                    .map(|(prev_t, prev_end)| prev_end - prev_t <= w.hit100)
                    .unwrap_or(false)
                && pt >= ho.time
                && pt <= end_time
                && release_after_pt
                    .map(|rt| {
                        rt >= tail_start
                            && rt < end_time + w.hit100
                            && next_note_time.map(|nt| rt < nt).unwrap_or(true)
                    })
                    .unwrap_or(false);
            let short_post_tail_keeps = true
                && ln_duration <= w.hit100
                && !matches!(final_kind, JudgmentKind::Miss)
                && !prev_was_miss
                && prev_same_col_ln_info
                    .map(|(prev_t, prev_end)| prev_end - prev_t <= w.hit100)
                    .unwrap_or(false)
                && pt < ho.time
                && release_after_pt
                    .map(|rt| {
                        rt >= tail_start
                            && rt < end_time + w.hit100
                            && next_note_time.map(|nt| rt < nt).unwrap_or(true)
                    })
                    .unwrap_or(false);
            if long_post_tail_pref || short_post_tail_keeps {
                let prior_body_fragment = prev_same_col_ln_info.and_then(|(prev_t, prev_end)| {
                    let current_release = release_after_pt?;
                    events
                        .iter()
                        .rev()
                        .filter(|ev| ev.pressed && ev.time < pt)
                        .find_map(|press_ev| {
                            let cand_pt = press_ev.time;
                            let cand_release = events
                                .iter()
                                .find(|ev| ev.time > cand_pt && !ev.pressed)
                                .map(|ev| ev.time)?;
                            let immediate_repress = events
                                .iter()
                                .find(|ev| ev.pressed && ev.time > cand_release)
                                .map(|ev| ev.time);
                            let already_assigned = out
                                .iter()
                                .any(|jj| jj.column == ho.column && jj.press_time == Some(cand_pt));
                            let reused_prev_short = short_post_tail_keeps
                                && out.iter().any(|jj| {
                                    jj.column == ho.column
                                        && jj.is_ln
                                        && jj.note_time == prev_t
                                        && jj.end_time == Some(prev_end)
                                        && jj.press_time == Some(cand_pt)
                                });
                            let carrs_same_tail_rel =
                                cand_release == current_release && cand_release > pt;
                            let breaks_post_to_cur = cand_release > ho.time
                                && cand_release < pt
                                && immediate_repress == Some(pt);
                            if cand_pt >= ho.time
                                || cand_pt < prev_end - w.hit50
                                || cand_pt > prev_end
                                || (already_assigned && !reused_prev_short)
                                || cand_release <= prev_end
                                || !(carrs_same_tail_rel || breaks_post_to_cur)
                            {
                                return None;
                            }
                            Some(cand_pt)
                        })
                });
                if let Some(cand_pt) = prior_body_fragment {
                    final_press_time = Some(cand_pt);
                    if short_post_tail_keeps {
                        final_tail_pt = Some(pt);
                    }
                    final_kind = JudgmentKind::Miss;
                    final_delta = cand_pt - ho.time;
                }
            }
            let short_post_long_keeps = true
                && state.penalty_flags.short_ln_post_long
                && ln_duration <= w.hit100
                && matches!(
                    final_kind,
                    JudgmentKind::Max | JudgmentKind::Hit300 | JudgmentKind::Hit200
                )
                && prev_same_col_ln_info
                    .map(|(prev_t, prev_end)| prev_end - prev_t >= w.hit50 * 2)
                    .unwrap_or(false)
                && pt >= ho.time
                && pt <= end_time
                && release_after_pt
                    .map(|rt| {
                        rt >= tail_start
                            && rt < end_time + w.hit100
                            && next_note_time.map(|nt| rt < nt).unwrap_or(true)
                    })
                    .unwrap_or(false);
            if short_post_long_keeps {
                let prior_body_fragment = prev_same_col_ln_info.and_then(|(prev_t, prev_end)| {
                    events
                        .iter()
                        .rev()
                        .filter(|ev| ev.pressed && ev.time < pt)
                        .find_map(|press_ev| {
                            let cand_pt = press_ev.time;
                            let cand_release = events
                                .iter()
                                .find(|ev| ev.time > cand_pt && !ev.pressed)
                                .map(|ev| ev.time)?;
                            let immediate_repress = events
                                .iter()
                                .find(|ev| ev.pressed && ev.time > cand_release)
                                .map(|ev| ev.time);
                            let already_assigned = out
                                .iter()
                                .any(|jj| jj.column == ho.column && jj.press_time == Some(cand_pt));
                            if cand_pt >= ho.time
                                || cand_pt < prev_t
                                || cand_pt < prev_end - w.hit100
                                || cand_pt > prev_end
                                || already_assigned
                                || calc_hit_kind((cand_pt - ho.time).abs(), w) != JudgmentKind::Miss
                                || cand_release <= cand_pt
                                || cand_release <= prev_t
                                || cand_release > prev_end + 1
                                || cand_release >= ho.time
                                || immediate_repress != Some(pt)
                            {
                                return None;
                            }
                            Some(cand_pt)
                        })
                });
                if let Some(cand_pt) = prior_body_fragment {
                    final_press_time = Some(cand_pt);
                    final_tail_pt = Some(pt);
                    final_kind = JudgmentKind::Miss;
                    final_delta = cand_pt - ho.time;
                }
            }
            let long_post_short_pre = ln_duration > w.hit100
                && !matches!(final_kind, JudgmentKind::Miss)
                && prev_was_miss
                && prev_had_prewin_pen
                && prev_short_miss_ln
                    .map(|(_, prev_end)| ho.time - prev_end <= w.hit50)
                    .unwrap_or(false)
                && prev_short_hold
                && pt >= ho.time - w.max
                && pt <= end_time
                && release_after_pt
                    .map(|rt| {
                        rt >= tail_start
                            && rt < end_time + w.hit100
                            && next_note_time.map(|nt| rt < nt).unwrap_or(true)
                    })
                    .unwrap_or(false);
            if long_post_short_pre {
                let prior_prehead_frag = prev_col_pt.zip(prev_short_miss_ln).and_then(
                    |(prev_pt, (prev_t, prev_end))| {
                        events
                            .iter()
                            .rev()
                            .filter(|ev| ev.pressed && ev.time < pt)
                            .find_map(|press_ev| {
                                let cand_pt = press_ev.time;
                                let cand_release = events
                                    .iter()
                                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                                    .map(|ev| ev.time)?;
                                let immediate_repress = events
                                    .iter()
                                    .find(|ev| ev.pressed && ev.time > cand_release)
                                    .map(|ev| ev.time);
                                let already_assigned = out.iter().any(|jj| {
                                    jj.column == ho.column && jj.press_time == Some(cand_pt)
                                });
                                let frag_rels_near_head = cand_release > prev_end
                                    && cand_release < ho.time
                                    && ho.time - cand_release <= w.hit300 + w.max;
                                let frag_break_short_ln = cand_release > prev_t
                                    && cand_release <= prev_end + 1
                                    && cand_release < ho.time;
                                let late_cand_cur_body = pt >= ho.time - w.hit300;
                                if cand_pt <= prev_pt
                                    || cand_pt >= ho.time
                                    || cand_pt > prev_end
                                    || already_assigned
                                    || calc_hit_kind((cand_pt - ho.time).abs(), w)
                                        != JudgmentKind::Miss
                                    || !((frag_rels_near_head && late_cand_cur_body)
                                        || frag_break_short_ln)
                                    || cand_release <= prev_t
                                    || immediate_repress != Some(pt)
                                {
                                    return None;
                                }
                                Some(cand_pt)
                            })
                    },
                );
                if let Some(cand_pt) = prior_prehead_frag {
                    final_press_time = Some(cand_pt);
                    final_kind = JudgmentKind::Miss;
                    final_delta = cand_pt - ho.time;
                }
            }
            let prev_miss_stale_head = prev_was_miss
                && !prev_had_prewin_pen
                && note_pos
                    .checked_sub(1)
                    .and_then(|p| col_notes.get(p))
                    .map(|(_, prev_ho)| !prev_ho.is_long_note())
                    .unwrap_or(false)
                && prev_stale_time
                    .map(|prev_t| {
                        let prev_press_is_stale = prev_col_pt.map(|pt| pt < prev_t).unwrap_or(true);
                        prev_press_is_stale && final_press_time == Some(prev_t + w.hit100)
                    })
                    .unwrap_or(false);
            let prev_miss_post_break = if prev_miss_stale_head
                && ln_duration <= w.hit100
                && !matches!(final_kind, JudgmentKind::Miss)
            {
                final_press_time.and_then(|cur_pt| {
                    if cur_pt >= ho.time {
                        return None;
                    }
                    let current_release = events
                        .iter()
                        .find(|ev| ev.time > cur_pt && !ev.pressed)
                        .map(|ev| ev.time)?;
                    let (_, next_ho) = col_notes.get(note_pos + 1)?;
                    if current_release <= ho.time || current_release >= end_time {
                        return None;
                    }
                    if next_ho.is_long_note() {
                        return None;
                    }
                    let next_head = next_ho.time;
                    if current_release >= next_head {
                        return None;
                    }
                    let (_, next_next_ho) = col_notes.get(note_pos + 2)?;
                    let next_next_head = next_next_ho.time;
                    let next_next_next_head = col_notes
                        .get(note_pos + 3)
                        .map(|(_, next_next_next_ho)| next_next_next_ho.time);
                    let next_window_start = next_head - w.hit50;
                    let next_win_end = next_head + w.hit100;
                    let tail_end_exclusive = end_time + ((w.hit100 as f32) * 1.5).round() as i32;
                    let cand_pt = presses.iter().copied().find(|cand| {
                        *cand > current_release
                            && *cand > end_time
                            && *cand >= next_window_start
                            && *cand < next_head
                            && !reserved_ln_repr.contains(cand)
                    })?;
                    let cand_kind = calc_hit_kind((cand_pt - ho.time).abs(), w);
                    if cand_kind != JudgmentKind::Hit100 {
                        return None;
                    }
                    let cand_release = events
                        .iter()
                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                        .map(|ev| ev.time)?;
                    if cand_release <= next_head
                        || cand_release >= next_next_head
                        || cand_release < tail_start
                        || cand_release >= tail_end_exclusive
                    {
                        return None;
                    }
                    let next_tap_has_followup = presses
                        .iter()
                        .copied()
                        .find(|cand| {
                            *cand > cand_pt
                                && *cand >= next_window_start
                                && *cand < next_win_end
                                && !reserved_ln_repr.contains(cand)
                        })
                        .is_some();
                    if next_tap_has_followup {
                        return None;
                    }
                    let next2_win_start = next_next_head - w.hit50;
                    let next2_win_end = next_next_head + w.hit100;
                    let next_next_followup_pt = presses.iter().copied().find(|cand| {
                        *cand > cand_release
                            && *cand >= next2_win_start
                            && *cand < next2_win_end
                            && !reserved_ln_repr.contains(cand)
                    })?;
                    let next2_follow_rel = events
                        .iter()
                        .find(|ev| ev.time > next_next_followup_pt && !ev.pressed)
                        .map(|ev| ev.time)?;
                    let next_next_survives = if next_next_ho.is_long_note() {
                        let next_next_end = next_next_ho.end_time.unwrap_or(next_next_head);
                        let next_next_tail_start = next_next_end - w.hit50;
                        let next2_tail_end = next_next_end + w.hit100;
                        next2_follow_rel >= next_next_tail_start
                            && next2_follow_rel < next2_tail_end
                            && next_next_next_head
                                .map(|head| next2_follow_rel < head)
                                .unwrap_or(true)
                    } else {
                        next_next_next_head
                            .map(|head| next2_follow_rel < head)
                            .unwrap_or(true)
                    };
                    next_next_survives.then_some((cand_pt, cand_kind))
                })
            } else {
                None
            };
            if let Some((cand_pt, cand_kind)) = prev_miss_post_break {
                final_press_time = Some(cand_pt);
                final_tail_pt = None;
                final_kind = cand_kind;
                final_delta = cand_pt - ho.time;
                reserved_ln_repr.insert(cand_pt);
            }
        }
        let long_post_reject = final_press_time.is_none()
            && final_tail_pt.is_none()
            && ln_duration > w.hit100
            && !prev_was_miss
            && prev_same_col_ln_info
                .map(|(prev_t, prev_end)| prev_end - prev_t <= w.hit100)
                .unwrap_or(false)
            && state.head_candidate.has_candidate
            && calc_hit_kind((state.head_candidate.selected_pt - ho.time).abs(), w)
                == JudgmentKind::Miss
            && state.head_candidate.selected_pt > end_time
            && col_notes
                .get(note_pos + 1)
                .zip(next_note_time)
                .map(|((_, next_ho), next_head)| {
                    if !next_ho.is_long_note() || state.head_candidate.selected_pt >= next_head {
                        return false;
                    }
                    let next_window_start = next_head - w.hit50;
                    let next_end = next_ho.end_time.unwrap_or(next_ho.time);
                    let next_tail_start = next_end - w.hit50;
                    let next_tail_end = next_end + w.hit100;
                    state.head_candidate.selected_pt >= next_window_start
                        && events
                            .iter()
                            .find(|ev| ev.time > state.head_candidate.selected_pt && !ev.pressed)
                            .map(|ev| ev.time >= next_tail_start && ev.time < next_tail_end)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        if long_post_reject {
            let rejected_pt = state.head_candidate.selected_pt;
            let prior_body_fragment = prev_same_col_ln_info.and_then(|(_, prev_end)| {
                events
                    .iter()
                    .rev()
                    .filter(|ev| ev.pressed && ev.time < rejected_pt)
                    .find_map(|press_ev| {
                        let cand_pt = press_ev.time;
                        let cand_release = events
                            .iter()
                            .find(|ev| ev.time > cand_pt && !ev.pressed)
                            .map(|ev| ev.time)?;
                        let immediate_repress = events
                            .iter()
                            .find(|ev| ev.pressed && ev.time > cand_release)
                            .map(|ev| ev.time);
                        let already_assigned = out
                            .iter()
                            .any(|jj| jj.column == ho.column && jj.press_time == Some(cand_pt));
                        let breaks_post_rejected = cand_release > ho.time
                            && cand_release >= tail_start
                            && cand_release < rejected_pt
                            && next_note_time
                                .map(|next_head| cand_release < next_head)
                                .unwrap_or(true)
                            && immediate_repress == Some(rejected_pt);
                        if cand_pt >= ho.time
                            || cand_pt < prev_end - w.hit50
                            || cand_pt > prev_end
                            || already_assigned
                            || calc_hit_kind((cand_pt - ho.time).abs(), w) != JudgmentKind::Miss
                            || cand_release <= prev_end
                            || !breaks_post_rejected
                        {
                            return None;
                        }
                        Some(cand_pt)
                    })
            });
            if let Some(cand_pt) = prior_body_fragment {
                final_press_time = Some(cand_pt);
                final_kind = JudgmentKind::Miss;
                final_delta = cand_pt - ho.time;
            }
        }
        if final_press_time.is_none() {
            if let Some(pt) = final_tail_pt {
                let release_after_pt = events
                    .iter()
                    .find(|ev| ev.time > pt && !ev.pressed)
                    .map(|ev| ev.time);
                let cur_rel_before_head = events
                    .iter()
                    .find(|ev| ev.time > pt && !ev.pressed)
                    .map(|ev| ev.time < ho.time)
                    .unwrap_or(false);
                let late_post_head_tail = events
                    .iter()
                    .filter(|ev| ev.pressed && ev.time > pt && ev.time <= end_time)
                    .any(|press_ev| {
                        events
                            .iter()
                            .find(|ev| ev.time > press_ev.time && !ev.pressed)
                            .map(|ev| ev.time >= tail_start && ev.time < end_time + w.hit100)
                            .unwrap_or(false)
                    });
                let short_tail_pref_later = ln_duration <= w.hit100
                    && calc_hit_kind((pt - ho.time).abs(), w) == JudgmentKind::Miss
                    && cur_rel_before_head
                    && late_post_head_tail;
                if short_tail_pref_later {
                    let later_prewin_frag = release_after_pt.and_then(|cur_release| {
                        events
                            .iter()
                            .filter(|ev| ev.pressed && ev.time > pt && ev.time < ho.time)
                            .find_map(|press_ev| {
                                let cand_pt = press_ev.time;
                                let cand_release = events
                                    .iter()
                                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                                    .map(|ev| ev.time)?;
                                let immediate_repress = events
                                    .iter()
                                    .find(|ev| ev.pressed && ev.time > cand_release)
                                    .map(|ev| ev.time)?;
                                let imm_repr_rel = events
                                    .iter()
                                    .find(|ev| ev.time > immediate_repress && !ev.pressed)
                                    .map(|ev| ev.time)?;
                                let already_assigned = out.iter().any(|jj| {
                                    jj.column == ho.column && jj.press_time == Some(cand_pt)
                                });
                                if already_assigned
                                    || cand_pt - pt > w.hit300
                                    || calc_hit_kind((cand_pt - ho.time).abs(), w)
                                        != JudgmentKind::Miss
                                    || cur_release >= cand_pt
                                    || cand_release >= ho.time
                                    || immediate_repress <= ho.time
                                    || imm_repr_rel < tail_start
                                    || imm_repr_rel >= end_time + w.hit100
                                {
                                    return None;
                                }
                                Some(cand_pt)
                            })
                    });
                    if let Some(cand_pt) = later_prewin_frag {
                        final_press_time = Some(cand_pt);
                        final_tail_pt = None;
                        final_kind = JudgmentKind::Miss;
                        final_delta = cand_pt - ho.time;
                    }
                }
                let shr_ln_tai_post_hand = true
                    && ln_duration > w.hit100
                    && ln_duration <= w.hit50 + w.hit100
                    && pt > end_time
                    && pt <= end_time + w.max
                    && col_notes
                        .get(note_pos + 1)
                        .map(|(_, next_ho)| {
                            if !next_ho.is_long_note() {
                                return false;
                            }
                            let next_head = next_ho.time;
                            let next_end = next_ho.end_time.unwrap_or(next_ho.time);
                            let next_duration = next_end - next_head;
                            let next_window_start = next_head - w.hit50;
                            let next_next_note_time =
                                col_notes.get(note_pos + 2).map(|(_, ho)| ho.time);
                            let next_late_end = next_next_note_time
                                .map(|next_time| next_time <= next_head + w.hit50)
                                .unwrap_or(false);
                            let next_lock_end =
                                next_head + w.hit50 + if next_late_end { 1 } else { 0 };
                            let has_next_ln_follow = presses
                                .iter()
                                .skip(press_idx)
                                .take_while(|cand| **cand < next_lock_end)
                                .any(|cand| {
                                    *cand >= next_window_start && !reserved_ln_repr.contains(cand)
                                });
                            next_duration > w.hit300 + w.max
                                && next_duration <= w.hit50 + w.hit100
                                && pt >= next_head - w.hit100
                                && pt < next_head
                                && calc_hit_kind((pt - next_head).abs(), w) == JudgmentKind::Hit100
                                && !has_next_ln_follow
                                && release_after_pt
                                    .map(|rt| {
                                        rt > next_head
                                            && rt < next_end
                                            && next_head - pt > rt - next_head
                                            && next_next_note_time
                                                .map(|next_time| rt < next_time)
                                                .unwrap_or(true)
                                    })
                                    .unwrap_or(false)
                        })
                        .unwrap_or(false);
                if shr_ln_tai_post_hand {
                    final_tail_pt = None;
                    _tail_post_handoff = true;
                    if press_idx > 0 {
                        let prev_idx = press_idx - 1;
                        if presses.get(prev_idx).copied() == Some(pt) {
                            press_idx = prev_idx;
                        }
                    }
                }
            }
        }
    }
    let iso_tap_stays_nopress = true
        && !ho.is_long_note()
        && final_kind == JudgmentKind::Miss
        && final_press_time.is_some()
        && final_tail_pt.is_none()
        && state.penalty_flags.deep_tap
        && !prev_was_miss
        && !prev_had_prewin_pen
        && note_pos
            .checked_sub(1)
            .and_then(|p| col_notes.get(p))
            .and_then(|(_, prev_ho)| {
                prev_ho
                    .is_long_note()
                    .then_some((prev_ho.time, prev_ho.end_time.unwrap_or(prev_ho.time)))
            })
            .zip(prev_col_pt)
            .zip(final_press_time)
            .map(|(((_prev_t, prev_end), prev_pt), pt)| {
                let prev_release = events
                    .iter()
                    .find(|ev| ev.time > prev_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let current_release = events
                    .iter()
                    .find(|ev| ev.time > pt && !ev.pressed)
                    .map(|ev| ev.time);
                let lat_fra_pos_prev_end =
                    pt > prev_end && prev_release.map(|rt| rt < pt).unwrap_or(false);
                let close_tail_pre_end = prev_release
                    .zip(current_release)
                    .map(|(prev_rt, current_rt)| {
                        pt > prev_rt && pt < prev_end && current_rt < ho.time
                    })
                    .unwrap_or(false);
                lat_fra_pos_prev_end || close_tail_pre_end
            })
            .unwrap_or(false)
        && next_note_time
            .map(|next_t| next_t - ho.time > w.hit50 + w.hit300)
            .unwrap_or(true);
    if iso_tap_stays_nopress {
        final_press_time = None;
        final_delta = 0;
    }
    let exact_deep_nopress = true
        && !ho.is_long_note()
        && final_kind == JudgmentKind::Miss
        && final_press_time.is_some()
        && final_tail_pt.is_none()
        && state.penalty_flags.deep_tap
        && !prev_was_miss
        && !prev_had_prewin_pen
        && note_pos
            .checked_sub(1)
            .and_then(|p| col_notes.get(p))
            .map(|(_, prev_ho)| !prev_ho.is_long_note())
            .unwrap_or(false)
        && prev_col_pt
            .zip(prev_stale_time)
            .zip(final_press_time)
            .map(|((prev_pt, prev_t), pt)| {
                let prev_kind = calc_hit_kind((prev_pt - prev_t).abs(), w);
                let prev_release = events
                    .iter()
                    .find(|ev| ev.time > prev_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let current_release = events
                    .iter()
                    .find(|ev| ev.time > pt && !ev.pressed)
                    .map(|ev| ev.time);
                matches!(
                    prev_kind,
                    JudgmentKind::Hit200 | JudgmentKind::Hit100 | JudgmentKind::Hit50
                ) && matches!((ho.time - pt).abs(), 165 | 166)
                    && prev_release.map(|rt| rt < pt).unwrap_or(false)
                    && current_release.map(|rt| rt < ho.time).unwrap_or(false)
            })
            .unwrap_or(false);
    if exact_deep_nopress {
        final_press_time = None;
        final_delta = 0;
    }
    let miss_pre_tap = if true
        && !ho.is_long_note()
        && final_kind == JudgmentKind::Miss
        && final_press_time.is_none()
        && final_tail_pt.is_none()
        && prev_was_miss
    {
        prev_col_pt.zip(prev_stale_time).and_then(|(pp, ptm)| {
            if !(pp < ptm && ho.time - ptm <= w.hit50 + w.max) {
                return None;
            }
            presses
                .iter()
                .find(|cand| {
                    let cp = **cand;
                    cp > pp
                        && cp < ptm
                        && ptm - cp <= w.max
                        && ho.time - cp <= w.hit50 + w.hit300 + w.max
                        && !reserved_ln_repr.contains(cand)
                        && !out
                            .iter()
                            .any(|j| j.column == ho.column && j.press_time == Some(cp))
                        && events
                            .iter()
                            .find(|ev| ev.time > cp && !ev.pressed)
                            .map(|ev| ev.time < ho.time)
                            .unwrap_or(false)
                })
                .copied()
        })
    } else {
        None
    };
    if let Some(pt) = miss_pre_tap {
        final_press_time = Some(pt);
        final_delta = pt - ho.time;
    }
    let prev_nonperf_prehead = if true
        && !ho.is_long_note()
        && final_kind == JudgmentKind::Miss
        && final_press_time.is_none()
        && final_tail_pt.is_none()
        && late_tap_cross_ln
    {
        prev_col_pt
            .zip(prev_stale_time)
            .zip(col_notes.get(note_pos + 1).zip(next_note_time))
            .and_then(|((prev_pt, prev_t), ((_, next_ho), next_head_time))| {
                if next_ho.is_long_note() {
                    Some((prev_pt, prev_t, next_head_time))
                } else {
                    None
                }
            })
            .and_then(|(prev_pt, prev_t, next_head_time)| {
                let prev_kind = calc_hit_kind((prev_pt - prev_t).abs(), w);
                let prev_release = events
                    .iter()
                    .find(|ev| ev.time > prev_pt && !ev.pressed)
                    .map(|ev| ev.time)?;
                let ghost_pt = events
                    .iter()
                    .find(|ev| ev.pressed && ev.time > prev_release && ev.time < prev_t)
                    .map(|ev| ev.time)?;
                let ghost_release = events
                    .iter()
                    .find(|ev| ev.time > ghost_pt && !ev.pressed)
                    .map(|ev| ev.time)?;
                let already_assigned = out
                    .iter()
                    .any(|jj| jj.column == ho.column && jj.press_time == Some(ghost_pt));
                (prev_pt < prev_t
                    && matches!(
                        prev_kind,
                        JudgmentKind::Hit200 | JudgmentKind::Hit100 | JudgmentKind::Hit50
                    )
                    && calc_hit_kind((ghost_pt - ho.time).abs(), w) == JudgmentKind::Miss
                    && ghost_release > ho.time
                    && ghost_release < next_head_time
                    && !already_assigned)
                    .then_some(ghost_pt)
            })
    } else {
        None
    };
    if let Some(cand_pt) = prev_nonperf_prehead {
        final_press_time = Some(cand_pt);
        final_delta = cand_pt - ho.time;
    }
    let prev_nonpr_dense_tap = if true
        && !ho.is_long_note()
        && final_kind == JudgmentKind::Miss
        && final_press_time.is_none()
        && final_tail_pt.is_none()
        && state.penalty_flags.post_prev_frag_next
    {
        prev_col_pt
            .zip(prev_stale_time)
            .zip(col_notes.get(note_pos + 1).zip(next_note_time))
            .and_then(|((prev_pt, prev_t), ((_, next_ho), next_head_time))| {
                (!next_ho.is_long_note() && next_head_time - ho.time <= w.hit50 + w.max)
                    .then_some((prev_pt, prev_t, next_head_time))
            })
            .and_then(|(prev_pt, prev_t, next_head_time)| {
                let prev_kind = calc_hit_kind((prev_pt - prev_t).abs(), w);
                let prev_release = events
                    .iter()
                    .find(|ev| ev.time > prev_pt && !ev.pressed)
                    .map(|ev| ev.time)?;
                let ghost_pt = events
                    .iter()
                    .find(|ev| {
                        ev.pressed
                            && ev.time > prev_t
                            && ev.time > prev_release
                            && ev.time < ho.time
                    })
                    .map(|ev| ev.time)?;
                let ghost_release = events
                    .iter()
                    .find(|ev| ev.time > ghost_pt && !ev.pressed)
                    .map(|ev| ev.time)?;
                let already_assigned = out
                    .iter()
                    .any(|jj| jj.column == ho.column && jj.press_time == Some(ghost_pt));
                (prev_pt < prev_t
                    && matches!(
                        prev_kind,
                        JudgmentKind::Hit200 | JudgmentKind::Hit100 | JudgmentKind::Hit50
                    )
                    && calc_hit_kind((ghost_pt - ho.time).abs(), w) == JudgmentKind::Miss
                    && ghost_release < ho.time
                    && ghost_release < next_head_time
                    && !already_assigned)
                    .then_some(ghost_pt)
            })
    } else {
        None
    };
    if let Some(cand_pt) = prev_nonpr_dense_tap {
        final_press_time = Some(cand_pt);
        final_delta = cand_pt - ho.time;
    }
    let prev_nonperf_wide_tap = if true
        && !ho.is_long_note()
        && final_kind == JudgmentKind::Miss
        && final_press_time.is_none()
        && final_tail_pt.is_none()
        && state.penalty_flags.post_prev_frag_next
    {
        prev_col_pt
            .zip(prev_stale_time)
            .zip(col_notes.get(note_pos + 1).zip(next_note_time))
            .and_then(|((prev_pt, prev_t), ((_, next_ho), next_head_time))| {
                if next_ho.is_long_note() || !state.head_candidate.has_candidate {
                    return None;
                }
                let next_pt = state.head_candidate.selected_pt;
                let next_window_start = next_head_time - w.hit50;
                let next_win_end = next_head_time + w.hit100;
                let next_kind = calc_hit_kind((next_pt - next_head_time).abs(), w);
                (next_head_time - ho.time > w.hit50 + w.max
                    && next_pt >= next_window_start
                    && next_pt < next_head_time
                    && next_pt < next_win_end
                    && next_kind == JudgmentKind::Max
                    && !reserved_ln_repr.contains(&next_pt))
                .then_some((prev_pt, prev_t, next_head_time))
            })
            .and_then(|(prev_pt, prev_t, next_head_time)| {
                let prev_kind = calc_hit_kind((prev_pt - prev_t).abs(), w);
                let prev_release = events
                    .iter()
                    .find(|ev| ev.time > prev_pt && !ev.pressed)
                    .map(|ev| ev.time)?;
                let ghost_pt = events
                    .iter()
                    .find(|ev| {
                        ev.pressed
                            && ev.time > prev_t
                            && ev.time > prev_release
                            && ev.time < ho.time
                    })
                    .map(|ev| ev.time)?;
                let ghost_release = events
                    .iter()
                    .find(|ev| ev.time > ghost_pt && !ev.pressed)
                    .map(|ev| ev.time)?;
                let already_assigned = out
                    .iter()
                    .any(|jj| jj.column == ho.column && jj.press_time == Some(ghost_pt));
                (prev_pt < prev_t
                    && matches!(
                        prev_kind,
                        JudgmentKind::Hit200 | JudgmentKind::Hit100 | JudgmentKind::Hit50
                    )
                    && calc_hit_kind((ghost_pt - ho.time).abs(), w) == JudgmentKind::Miss
                    && ghost_release < ho.time
                    && ghost_release < next_head_time
                    && !already_assigned)
                    .then_some(ghost_pt)
            })
    } else {
        None
    };
    if let Some(cand_pt) = prev_nonperf_wide_tap {
        final_press_time = Some(cand_pt);
        final_delta = cand_pt - ho.time;
    }
    state.press_idx = press_idx;
    state.final_pick.press = final_press_time;
    state.final_pick.tail = final_tail_pt;
    state.final_pick.kind = Some(final_kind);
    state.final_pick.delta = final_delta;
}
