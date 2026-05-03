use super::note::{PressNoteCtx, PressState};
use crate::modes::mania::judgment::{calc_hit_kind, InternalJudgment, PressTracker};
use crate::types::JudgmentKind;
pub(super) fn apply(
    ctx: &PressNoteCtx<'_>,
    state: &mut PressState,
    out: &mut Vec<InternalJudgment>,
    tracker: &mut PressTracker,
) {
    let idx = ctx.idx;
    let note_pos = ctx.note_pos;
    let ho = ctx.ho;
    let col_notes = ctx.col_notes;
    let presses = ctx.presses;
    let events = ctx.events;
    let w = ctx.windows;
    let next_note_time = ctx.next_note_time;
    let note_window = ctx.note_window;
    let next_window_start = note_window.next_window_start;
    let early_penalty_window = note_window.early_penalty_window;
    let press_idx = state.press_idx;
    let mut prev_had_prewin_pen = state.prev.had_prewin_pen;

    let mut prev_was_miss = state.prev.was_miss;

    let early_pen_pt = state.rules.early_pen;
    let final_press_time = state.final_pick.press;
    let final_tail_pt = state.final_pick.tail;
    let final_kind = state.final_pick.kind.unwrap_or(JudgmentKind::Miss);
    let final_delta = state.final_pick.delta;
    let reserved_ln_repr = &mut state.prev.reserved_ln_repr;
    out.push(InternalJudgment {
        index: idx,
        column: ho.column,
        note_time: ho.time,
        kind: final_kind,
        delta: final_delta,
        press_time: final_press_time,
        is_ln: ho.is_long_note(),
        end_time: ho.end_time,
        early_press_idx: early_pen_pt.or(if true { final_tail_pt } else { None }),
        early_pen_win: early_pen_pt.map(|_| early_penalty_window),
        deep_ln_pen: state.penalty_flags.deep_ln,
    });
    let tail_owner_pt = if true && ho.is_long_note() {
        final_press_time.or(final_tail_pt)
    } else {
        final_press_time
    };
    let prev_col_pt: Option<i32> = tail_owner_pt;
    let prev_prev_was_miss: bool = prev_was_miss;
    let prev2_had_prewin_pen: bool = prev_had_prewin_pen;
    prev_was_miss = final_kind == JudgmentKind::Miss;
    prev_had_prewin_pen = early_pen_pt.is_some();
    let mut cur_ln_body_break = false;
    if ho.is_long_note() && final_press_time.is_some() {
        let end_time = ho.end_time.unwrap_or(ho.time);
        let tail_start = end_time - w.hit50;
        let first_rel_post_head = final_press_time.and_then(|head_pt| {
            events
                .iter()
                .find(|ev| ev.time > head_pt && !ev.pressed)
                .map(|ev| ev.time)
        });
        let body_break_pre_tail = first_rel_post_head
            .map(|release_t| {
                if release_t >= ho.time {
                    release_t < tail_start
                } else {
                    final_kind == JudgmentKind::Miss
                }
            })
            .unwrap_or(false);
        cur_ln_body_break = body_break_pre_tail;
        if body_break_pre_tail {
            let ln_duration = end_time - ho.time;
            let late_repr_dur = (w.hit50 + w.hit100 + w.max).max(w.hit50 * 2 + 1);
            let tail_end_exclusive = end_time + w.hit100;
            let first_release = first_rel_post_head.unwrap_or(i32::MAX);
            let mut repress_candidate: Option<i32> = None;
            let mut first_repr_post_head: Option<i32> = None;
            let mut first_rel_post_repr: Option<i32> = None;
            let mut key_down = false;
            for ev in events
                .iter()
                .filter(|ev| ev.time > first_release && ev.time <= end_time)
            {
                if !key_down && ev.pressed {
                    repress_candidate = Some(ev.time);
                    if first_repr_post_head.is_none() {
                        first_repr_post_head = Some(ev.time);
                    }
                    key_down = true;
                } else if key_down && !ev.pressed {
                    if let Some(repress_time) = repress_candidate {
                        let late_repr_head = first_repr_post_head
                            .map(|first_rp| repress_time != first_rp)
                            .unwrap_or(false);
                        let rel_near_next_win = next_window_start
                            .map(|next_start| ev.time <= next_start + w.max + 4)
                            .unwrap_or(false);
                        let starts_in_next_prewin = next_window_start
                            .map(|next_start| repress_time >= next_start)
                            .unwrap_or(false);
                        let near_next_tap_left = next_window_start
                            .zip(col_notes.get(note_pos + 1))
                            .map(|(next_start, (_, next_ho))| {
                                !next_ho.is_long_note()
                                    && repress_time < next_start
                                    && next_start - repress_time <= w.max
                            })
                            .unwrap_or(false);
                        let next_tap_left_edge = near_next_tap_left && late_repr_head;
                        let rel_in_tail_win = ev.time >= tail_start && ev.time < tail_end_exclusive;
                        let rel_before_next_head = next_note_time
                            .map(|next_time| ev.time <= next_time)
                            .unwrap_or(false);
                        let allow_tail_next_prwn = !true
                            && starts_in_next_prewin
                            && rel_in_tail_win
                            && rel_before_next_head;
                        let holds_thru_next_head = !true
                            && starts_in_next_prewin
                            && col_notes
                                .get(note_pos + 1)
                                .map(|(_, next_ho)| !next_ho.is_long_note())
                                .unwrap_or(false)
                            && next_note_time
                                .map(|next_time| ev.time > next_time)
                                .unwrap_or(false);
                        let late_repr_forced_miss =
                            ln_duration <= late_repr_dur && repress_time > tail_start;
                        let tail_closure_next_tap = !true
                            && !starts_in_next_prewin
                            && !next_tap_left_edge
                            && rel_in_tail_win
                            && rel_before_next_head
                            && ln_duration >= w.hit50 * 2
                            && col_notes
                                .get(note_pos + 1)
                                .map(|(_, next_ho)| !next_ho.is_long_note())
                                .unwrap_or(false);
                        let resr_repr_for_cur_ln = (final_kind != JudgmentKind::Miss
                            && (ev.time < tail_start
                                || (rel_near_next_win && !next_tap_left_edge)
                                || allow_tail_next_prwn
                                || holds_thru_next_head))
                            || tail_closure_next_tap;
                        let sho_ln_rep_tail_frag = true
                            && final_kind == JudgmentKind::Miss
                            && ln_duration <= w.hit100
                            && repress_time >= ho.time - w.max
                            && repress_time < ho.time
                            && rel_in_tail_win
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
                                        .map(|time| time <= next_head + w.hit50)
                                        .unwrap_or(false);
                                    let next_win_end =
                                        next_head + w.hit50 + if next_late_end { 1 } else { 0 };
                                    let next_tail_start = next_end - w.hit50;
                                    let next_tail_end = next_end + w.hit100;
                                    let next_followup_press = presses
                                        .iter()
                                        .find(|pt| {
                                            **pt > repress_time
                                                && **pt >= next_head
                                                && **pt < next_win_end
                                                && !reserved_ln_repr.contains(pt)
                                        })
                                        .copied();
                                    next_duration <= w.hit100 + w.max
                                        && repress_time < next_window_start
                                        && next_window_start - repress_time <= w.max
                                        && ev.time < next_head
                                        && next_followup_press
                                            .map(|followup_pt| {
                                                events
                                                    .iter()
                                                    .find(|ev| ev.time > followup_pt && !ev.pressed)
                                                    .map(|ev| {
                                                        ev.time >= next_tail_start
                                                            && ev.time < next_tail_end
                                                            && next_next_note_time
                                                                .map(|next_next_time| {
                                                                    ev.time < next_next_time
                                                                })
                                                                .unwrap_or(true)
                                                    })
                                                    .unwrap_or(false)
                                            })
                                            .unwrap_or(false)
                                })
                                .unwrap_or(false);
                        if (resr_repr_for_cur_ln || sho_ln_rep_tail_frag)
                            && (!late_repr_forced_miss || sho_ln_rep_tail_frag)
                        {
                            reserved_ln_repr.insert(repress_time);
                        }
                        if first_rel_post_repr.is_none() {
                            first_rel_post_repr = Some(ev.time);
                        }
                    }
                    repress_candidate = None;
                    key_down = false;
                }
            }
            if key_down && final_kind != JudgmentKind::Miss && ln_duration >= w.hit50 * 2 {
                if let Some(repress_time) = repress_candidate {
                    let late_repr_head = first_repr_post_head
                        .map(|first_rp| repress_time != first_rp)
                        .unwrap_or(false);
                    let starts_in_next_prewin = next_window_start
                        .map(|next_start| repress_time >= next_start)
                        .unwrap_or(false);
                    let release_after_repress = events
                        .iter()
                        .find(|ev| ev.time > repress_time && !ev.pressed)
                        .map(|ev| ev.time);
                    let rel_in_tail_win = release_after_repress
                        .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
                        .unwrap_or(false);
                    let rel_before_next_head = release_after_repress
                        .zip(next_note_time)
                        .map(|(rt, next_time)| rt <= next_time)
                        .unwrap_or(false);
                    let allow_tail_next_prwn = !true && rel_in_tail_win && rel_before_next_head;
                    let open_seg_next_tap_pt = !true
                        && col_notes
                            .get(note_pos + 1)
                            .map(|(_, next_ho)| {
                                if next_ho.is_long_note() {
                                    false
                                } else {
                                    let next_tap_window_start = next_ho.time - w.hit50;
                                    let next_tap_end = next_ho.time + w.hit100;
                                    presses.iter().any(|pt| {
                                        *pt > repress_time
                                            && *pt >= next_tap_window_start
                                            && *pt < next_tap_end
                                            && !reserved_ln_repr.contains(pt)
                                    })
                                }
                            })
                            .unwrap_or(false);
                    let open_seg_next_ln_pt = !true
                        && col_notes
                            .get(note_pos + 1)
                            .map(|(_, next_ho)| {
                                if !next_ho.is_long_note() {
                                    return false;
                                }
                                let next_window_start_ln = next_ho.time - w.hit50;
                                let next_next_note_time =
                                    col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                                let next_ln_late_end = next_next_note_time
                                    .map(|next_time| next_time <= next_ho.time + w.hit50)
                                    .unwrap_or(false);
                                let next_lock_end =
                                    next_ho.time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                                presses.iter().any(|pt| {
                                    *pt > repress_time
                                        && *pt >= next_window_start_ln
                                        && *pt < next_lock_end
                                        && !reserved_ln_repr.contains(pt)
                                })
                            })
                            .unwrap_or(false);
                    let ope_seg_nex_tap_edge = !true
                        && next_window_start
                            .zip(col_notes.get(note_pos + 1))
                            .map(|(next_start, (_, next_ho))| {
                                !next_ho.is_long_note()
                                    && repress_time < next_start
                                    && next_start - repress_time <= w.max
                            })
                            .unwrap_or(false)
                        && rel_in_tail_win
                        && rel_before_next_head
                        && !(open_seg_next_tap_pt && !late_repr_head);
                    let ope_seg_firs_next_ln = !true
                        && !late_repr_head
                        && starts_in_next_prewin
                        && col_notes
                            .get(note_pos + 1)
                            .map(|(_, next_ho)| next_ho.is_long_note())
                            .unwrap_or(false)
                        && release_after_repress
                            .zip(next_note_time)
                            .map(|(rt, next_time)| rt > next_time)
                            .unwrap_or(true)
                        && !open_seg_next_ln_pt;
                    let ope_seg_shor_next_ln = !true
                        && late_repr_head
                        && next_window_start
                            .zip(col_notes.get(note_pos + 1))
                            .map(|(next_start, (_, next_ho))| {
                                next_ho.is_long_note()
                                    && (next_ho.end_time.unwrap_or(next_ho.time) - next_ho.time)
                                        <= w.hit100
                                    && repress_time < next_start
                            })
                            .unwrap_or(false)
                        && release_after_repress
                            .zip(next_window_start)
                            .map(|(rt, next_start)| rt >= next_start)
                            .unwrap_or(false)
                        && rel_before_next_head
                        && col_notes
                            .get(note_pos + 1)
                            .map(|(_, next_ho)| {
                                if !next_ho.is_long_note() {
                                    return false;
                                }
                                let next_window_start_ln = next_ho.time - w.hit50;
                                let next_next_note_time =
                                    col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                                let next_ln_late_end = next_next_note_time
                                    .map(|next_time| next_time <= next_ho.time + w.hit50)
                                    .unwrap_or(false);
                                let next_lock_end =
                                    next_ho.time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                                presses.iter().any(|pt| {
                                    *pt > repress_time
                                        && *pt >= next_window_start_ln
                                        && *pt < next_lock_end
                                        && !reserved_ln_repr.contains(pt)
                                })
                            })
                            .unwrap_or(false);
                    let open_seg_next_ln_fol = !true
                        && late_repr_head
                        && starts_in_next_prewin
                        && rel_before_next_head
                        && col_notes
                            .get(note_pos + 1)
                            .map(|(_, next_ho)| {
                                if !next_ho.is_long_note() {
                                    return false;
                                }
                                let next_window_start_ln = next_ho.time - w.hit50;
                                let next_next_note_time =
                                    col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                                let next_ln_late_end = next_next_note_time
                                    .map(|next_time| next_time <= next_ho.time + w.hit50)
                                    .unwrap_or(false);
                                let next_lock_end =
                                    next_ho.time + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                                presses.iter().any(|pt| {
                                    *pt > repress_time
                                        && *pt >= next_window_start_ln
                                        && *pt < next_lock_end
                                        && !reserved_ln_repr.contains(pt)
                                })
                            })
                            .unwrap_or(false);
                    let ope_seg_nex_tap_tail = !true
                        && late_repr_head
                        && starts_in_next_prewin
                        && col_notes
                            .get(note_pos + 1)
                            .map(|(_, next_ho)| !next_ho.is_long_note())
                            .unwrap_or(false)
                        && release_after_repress
                            .zip(next_note_time)
                            .map(|(rt, next_time)| rt > next_time)
                            .unwrap_or(false)
                        && first_repr_post_head
                            .and_then(|first_rp| {
                                events
                                    .iter()
                                    .find(|ev| ev.time > first_rp && !ev.pressed)
                                    .map(|ev| ev.time)
                            })
                            .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
                            .unwrap_or(false);
                    let holds_thru_next_head = !true
                        && starts_in_next_prewin
                        && col_notes
                            .get(note_pos + 1)
                            .map(|(_, next_ho)| !next_ho.is_long_note())
                            .unwrap_or(false)
                        && release_after_repress
                            .zip(next_note_time)
                            .map(|(rt, next_time)| rt > next_time)
                            .unwrap_or(true);
                    let open_tail_to_next_tap = true
                        && col_notes
                            .get(note_pos + 1)
                            .map(|(_, next_ho)| !next_ho.is_long_note())
                            .unwrap_or(false)
                        && next_window_start
                            .zip(next_note_time)
                            .zip(col_notes.get(note_pos + 2))
                            .map(|((next_start, next_time), (_, next_next_ho))| {
                                if !next_next_ho.is_long_note() {
                                    return false;
                                }
                                let next_next_head = next_next_ho.time;
                                let next2_win_start = next_next_head - w.hit50;
                                let next2_prewin_start = next2_win_start - early_penalty_window - 1;
                                let next_followup_press = presses
                                    .iter()
                                    .find(|pt| {
                                        **pt > repress_time
                                            && **pt < next_time
                                            && !reserved_ln_repr.contains(pt)
                                    })
                                    .copied();
                                let next_early_pen = next_start - early_penalty_window - 1;
                                repress_time >= tail_start
                                    && repress_time >= next_early_pen
                                    && repress_time < next_start
                                    && next_next_head - next_time <= w.hit50 + w.hit300
                                    && release_after_repress
                                        .map(|rt| rt > end_time && rt < next_time)
                                        .unwrap_or(false)
                                    && calc_hit_kind((repress_time - next_time).abs(), w)
                                        == JudgmentKind::Miss
                                    && next_followup_press
                                        .map(|followup_pt| {
                                            followup_pt >= next2_prewin_start
                                                && followup_pt < next2_win_start
                                                && events
                                                    .iter()
                                                    .find(|ev| ev.time > followup_pt && !ev.pressed)
                                                    .map(|ev| {
                                                        ev.time > next_time
                                                            && ev.time < next_next_head
                                                    })
                                                    .unwrap_or(false)
                                        })
                                        .unwrap_or(false)
                            })
                            .unwrap_or(false);
                    let ope_tai_to_next_shor = true
                        && late_repr_head
                        && first_rel_post_repr
                            .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
                            .unwrap_or(false)
                        && col_notes
                            .get(note_pos + 1)
                            .map(|(_, next_ho)| next_ho.is_long_note())
                            .unwrap_or(false)
                        && next_window_start
                            .zip(next_note_time)
                            .zip(col_notes.get(note_pos + 1))
                            .map(|((next_start, next_time), (_, next_ho))| {
                                let next_end = next_ho.end_time.unwrap_or(next_ho.time);
                                let next_duration = next_end - next_ho.time;
                                let next_tail_start = next_end - w.hit50;
                                let next_tail_end = next_end + w.hit100;
                                let next_next_note_time =
                                    col_notes.get(note_pos + 2).map(|(_, ho)| ho.time);
                                let next_late_end = next_next_note_time
                                    .map(|time| time <= next_ho.time + w.hit50)
                                    .unwrap_or(false);
                                let next_win_end =
                                    next_ho.time + w.hit50 + if next_late_end { 1 } else { 0 };
                                let next_followup_press = presses
                                    .iter()
                                    .skip(press_idx + 1)
                                    .take_while(|pt| **pt < next_win_end)
                                    .find(|pt| {
                                        **pt > next_ho.time
                                            && **pt <= next_end
                                            && !reserved_ln_repr.contains(pt)
                                    })
                                    .copied();
                                let next_early_pen = next_start - early_penalty_window - 1;
                                repress_time < end_time
                                    && next_time - end_time <= w.hit100
                                    && next_duration <= w.hit100
                                    && repress_time >= next_early_pen
                                    && repress_time < next_start
                                    && release_after_repress
                                        .map(|rt| rt > end_time && rt < next_time)
                                        .unwrap_or(false)
                                    && calc_hit_kind((repress_time - next_time).abs(), w)
                                        == JudgmentKind::Miss
                                    && next_followup_press
                                        .map(|followup_pt| {
                                            events
                                                .iter()
                                                .find(|ev| ev.time > followup_pt && !ev.pressed)
                                                .map(|ev| {
                                                    ev.time >= next_tail_start
                                                        && ev.time < next_tail_end
                                                        && next_next_note_time
                                                            .map(|next_next_time| {
                                                                ev.time < next_next_time
                                                            })
                                                            .unwrap_or(true)
                                                })
                                                .unwrap_or(false)
                                        })
                                        .unwrap_or(false)
                            })
                            .unwrap_or(false);
                    let hld_next_tap = true
                        && starts_in_next_prewin
                        && col_notes
                            .get(note_pos + 1)
                            .map(|(_, next_ho)| {
                                !next_ho.is_long_note()
                                    && repress_time >= next_ho.time - w.hit50
                                    && repress_time < next_ho.time + w.hit100
                                    && matches!(
                                        calc_hit_kind((repress_time - next_ho.time).abs(), w),
                                        JudgmentKind::Max | JudgmentKind::Hit300
                                    )
                            })
                            .unwrap_or(false)
                        && release_after_repress
                            .zip(next_note_time)
                            .map(|(rt, next_time)| rt > next_time)
                            .unwrap_or(false);
                    if ((!starts_in_next_prewin && !ope_seg_nex_tap_edge)
                        || (allow_tail_next_prwn && !ope_seg_nex_tap_edge)
                        || holds_thru_next_head
                        || ope_seg_firs_next_ln)
                        && !open_tail_to_next_tap
                        && !ope_tai_to_next_shor
                        && !ope_seg_shor_next_ln
                        && !open_seg_next_ln_fol
                        && !ope_seg_nex_tap_tail
                        && !hld_next_tap
                    {
                        reserved_ln_repr.insert(repress_time);
                    }
                }
            }
        }
    }
    let prev_break_pre: bool = cur_ln_body_break;
    tracker.press_idx = state.press_idx;
    tracker.prev_had_prewin_pen = prev_had_prewin_pen;
    tracker.prev_break_pre = prev_break_pre;
    tracker.prev_was_miss = prev_was_miss;
    tracker.prev2_had_prewin_pen = prev2_had_prewin_pen;
    tracker.prev_prev_was_miss = prev_prev_was_miss;
    tracker.prev_col_pt = prev_col_pt;
    tracker.reserved_ln_repr = std::mem::take(&mut state.prev.reserved_ln_repr);
    state.prev.had_prewin_pen = prev_had_prewin_pen;
    state.prev.body_break_pre_tail = prev_break_pre;
    state.prev.was_miss = prev_was_miss;
    state.prev.prev2_had_prewin_pen = prev2_had_prewin_pen;
    state.prev.prev2_was_miss = prev_prev_was_miss;
    state.prev.col_pt = prev_col_pt;
}
