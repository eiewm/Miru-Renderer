use super::note::{PressNoteCtx, PressState};
use crate::modes::mania::judgment::{InternalJudgment, PressTracker};
use crate::types::JudgmentKind;
fn should_reserve_current_ln_repress(
    starts_in_next_prewin: bool,
    allow_tail_next_prwn: bool,
    next_tap_edge_blocks_reservation: bool,
    holds_thru_next_head: bool,
    open_segment_first_next_ln: bool,
) -> bool {
    holds_thru_next_head
        || open_segment_first_next_ln
        || ((allow_tail_next_prwn || !starts_in_next_prewin) && !next_tap_edge_blocks_reservation)
}
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
    let mut prev_had_prewin_pen = state.prev.had_prewin_pen;
    let mut prev_was_miss = state.prev.was_miss;
    let early_pen_pt = state.rules.early_pen;
    let final_press_time = state.final_pick.press;
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
        early_press_idx: early_pen_pt,
        early_pen_win: early_pen_pt.map(|_| early_penalty_window),
        deep_ln_pen: false,
    });
    let tail_claim_pt = final_press_time;
    let prev_col_pt = tail_claim_pt;
    let prev_prev_was_miss = prev_was_miss;
    let prev2_had_prewin_pen = prev_had_prewin_pen;
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
                        let allow_tail_next_prwn =
                            starts_in_next_prewin && rel_in_tail_win && rel_before_next_head;
                        let holds_thru_next_head = starts_in_next_prewin
                            && col_notes
                                .get(note_pos + 1)
                                .map(|(_, next_ho)| !next_ho.is_long_note())
                                .unwrap_or(false)
                            && next_note_time
                                .map(|next_time| ev.time > next_time)
                                .unwrap_or(false);
                        let late_repr_forced_miss =
                            ln_duration <= late_repr_dur && repress_time > tail_start;
                        let tail_closure_next_tap = !starts_in_next_prewin
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
                        let should_record_reserved_ln_repr =
                            resr_repr_for_cur_ln && !late_repr_forced_miss;
                        if should_record_reserved_ln_repr {
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
                    let allow_tail_next_prwn = rel_in_tail_win && rel_before_next_head;
                    let open_seg_next_tap_pt = col_notes
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
                    let open_seg_next_ln_pt = col_notes
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
                    let ope_seg_nex_tap_edge = next_window_start
                        .zip(col_notes.get(note_pos + 1))
                        .map(|(next_start, (_, next_ho))| {
                            !next_ho.is_long_note()
                                && repress_time < next_start
                                && next_start - repress_time <= w.max
                        })
                        .unwrap_or(false)
                        && rel_in_tail_win
                        && rel_before_next_head
                        && (!open_seg_next_tap_pt || late_repr_head);
                    let ope_seg_firs_next_ln = !late_repr_head
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
                    let ope_seg_shor_next_ln = late_repr_head
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
                    let open_seg_next_ln_fol = late_repr_head
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
                    let ope_seg_nex_tap_tail = late_repr_head
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
                    let holds_thru_next_head = starts_in_next_prewin
                        && col_notes
                            .get(note_pos + 1)
                            .map(|(_, next_ho)| !next_ho.is_long_note())
                            .unwrap_or(false)
                        && release_after_repress
                            .zip(next_note_time)
                            .map(|(rt, next_time)| rt > next_time)
                            .unwrap_or(true);
                    let open_tail_to_next_tap = false;
                    let ope_tai_to_next_shor = false;
                    let reserve_repress_for_current_ln = should_reserve_current_ln_repress(
                        starts_in_next_prewin,
                        allow_tail_next_prwn,
                        ope_seg_nex_tap_edge,
                        holds_thru_next_head,
                        ope_seg_firs_next_ln,
                    );
                    let blocked_by_followup_claim = open_tail_to_next_tap
                        || ope_tai_to_next_shor
                        || ope_seg_shor_next_ln
                        || open_seg_next_ln_fol
                        || ope_seg_nex_tap_tail;
                    if reserve_repress_for_current_ln && !blocked_by_followup_claim {
                        reserved_ln_repr.insert(repress_time);
                    }
                }
            }
        }
    }
    let prev_break_pre = cur_ln_body_break;
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
