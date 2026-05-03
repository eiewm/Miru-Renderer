use super::note::{PressNoteCtx, PressState};
use crate::modes::mania::judgment::{calc_hit_kind, NoteWindowView};
use crate::types::JudgmentKind;
pub(super) fn scan(ctx: &PressNoteCtx<'_>, state: &mut PressState) {
    let note_pos = ctx.note_pos;
    let ho = ctx.ho;
    let col_notes = ctx.col_notes;
    let presses = ctx.presses;
    let events = ctx.events;
    let w = ctx.windows;
    let _start_press_idx = state.press_idx;
    let mut press_idx = state.press_idx;
    let reserved_ln_repr = &state.prev.reserved_ln_repr;
    let prev_had_prewin_pen = state.prev.had_prewin_pen;
    let prev_break_pre = state.prev.body_break_pre_tail;
    let prev_was_miss = state.prev.was_miss;
    let prev_col_pt = state.prev.col_pt;
    let next_note_time = col_notes.get(note_pos + 1).map(|(_, next_ho)| next_ho.time);
    let _legacy_early_penalty_window = w.max + 4;
    let note_window = NoteWindowView::from_note(ho, next_note_time, w);
    let window_start = note_window.window_start;
    let lock_end_exclusive = note_window.lock_end_exclusive;
    let _next_window_start = note_window.next_window_start;
    let _early_penalty_window = note_window.early_penalty_window;
    let early_penalty_start = note_window.early_penalty_start;
    let _next_early_penalty_start = note_window.next_early_pen;
    let mut early_pen_pt: Option<i32> = None;
    while press_idx < presses.len() && presses[press_idx] < window_start {
        let pt = presses[press_idx];
        if reserved_ln_repr.contains(&pt) {
            press_idx += 1;
            continue;
        }
        if pt >= early_penalty_start {
            early_pen_pt = Some(pt);
        }
        press_idx += 1;
    }
    while press_idx < presses.len() && reserved_ln_repr.contains(&presses[press_idx]) {
        press_idx += 1;
    }
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
    let mut skipped_stale_prev = false;
    while press_idx < presses.len() {
        let cand_pt = presses[press_idx];
        let same_as_prev_note_pt = prev_col_pt
            .map(|prev_pt| prev_pt == cand_pt)
            .unwrap_or(false);
        let current_ln_duration = ho.end_time.unwrap_or(ho.time) - ho.time;
        let has_follow_near_head = ho.is_long_note()
            && current_ln_duration <= w.hit100
            && presses
                .iter()
                .skip(press_idx + 1)
                .take_while(|next_pt| **next_pt < lock_end_exclusive)
                .any(|next_pt| {
                    *next_pt >= ho.time
                        && *next_pt <= ho.time + w.hit100
                        && !reserved_ln_repr.contains(next_pt)
                });
        let has_h50_follow_cur = !false
            && ho.is_long_note()
            && current_ln_duration <= w.hit100
            && presses
                .iter()
                .skip(press_idx + 1)
                .take_while(|next_pt| **next_pt < lock_end_exclusive)
                .any(|next_pt| {
                    *next_pt >= ho.time
                        && *next_pt <= ho.time + w.hit50
                        && !reserved_ln_repr.contains(next_pt)
                });
        let holds_through_head = events
            .iter()
            .find(|ev| ev.time > cand_pt && !ev.pressed)
            .map(|ev| ev.time >= ho.time)
            .unwrap_or(true);
        let stale_prev_ln_cand =
            prev_is_ln_stale && same_as_prev_note_pt && cand_pt < ho.time && holds_through_head;
        let release_after_cand = events
            .iter()
            .find(|ev| ev.time > cand_pt && !ev.pressed)
            .map(|ev| ev.time);
        let stale_prev_tail_repr = ho.is_long_note()
            && prev_is_ln_stale
            && prev_break_pre
            && cand_pt < ho.time
            && holds_through_head
            && prev_dur_stale.map(|d| d >= w.hit50 * 3).unwrap_or(false)
            && prev_end_stale
                .map(|prev_end| cand_pt >= prev_end - w.max && cand_pt <= prev_end + w.max)
                .unwrap_or(false)
            && has_follow_near_head;
        let far_prehead_repr = !false
            && ho.is_long_note()
            && prev_is_ln_stale
            && prev_break_pre
            && cand_pt <= ho.time - w.hit300
            && holds_through_head
            && prev_dur_stale.map(|d| d >= w.hit50 * 2).unwrap_or(false)
            && prev_end_stale
                .map(|prev_end| cand_pt >= prev_end - w.max && cand_pt <= prev_end)
                .unwrap_or(false)
            && has_h50_follow_cur;
        let short_ln_prehold_h50 = !false
            && ho.is_long_note()
            && current_ln_duration <= w.hit100
            && prev_is_ln_stale
            && prev_break_pre
            && !prev_was_miss
            && prev_dur_stale.map(|d| d >= w.hit50 * 2).unwrap_or(false)
            && cand_pt < ho.time
            && holds_through_head
            && prev_end_stale
                .map(|prev_end| cand_pt > prev_end && cand_pt - prev_end <= w.hit100)
                .unwrap_or(false)
            && {
                let boundary_pt = ho.time + w.hit50;
                let release_after_cand = events
                    .iter()
                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                    .map(|ev| ev.time);
                let bound_cand_ok = presses
                    .iter()
                    .skip(press_idx + 1)
                    .take_while(|next_pt| **next_pt <= boundary_pt)
                    .any(|next_pt| *next_pt == boundary_pt && !reserved_ln_repr.contains(next_pt));
                let rel_pre_bound = release_after_cand
                    .map(|rt| rt >= ho.time && rt < boundary_pt)
                    .unwrap_or(false);
                let next_ln_bound_ovrlp = col_notes
                    .get(note_pos + 1)
                    .map(|(_, next_ho)| {
                        next_ho.is_long_note()
                            && boundary_pt >= next_ho.time - w.hit50
                            && boundary_pt < next_ho.time
                    })
                    .unwrap_or(false);
                bound_cand_ok && rel_pre_bound && next_ln_bound_ovrlp
            };
        let nex_sam_col_hea_cand = col_notes
            .get(note_pos + 1)
            .map(|(_, next_ho)| {
                if !next_ho.is_long_note() {
                    return false;
                }
                let next_head = next_ho.time;
                let next_window_start = next_head - w.hit50;
                let next_next_note_time = col_notes.get(note_pos + 2).map(|(_, n)| n.time);
                let next_ln_late_end = next_next_note_time
                    .map(|next_time| next_time <= next_head + w.hit50)
                    .unwrap_or(false);
                let next_lock_end = next_head + w.hit50 + if next_ln_late_end { 1 } else { 0 };
                presses
                    .iter()
                    .skip(press_idx + 1)
                    .take_while(|cand| **cand < next_lock_end)
                    .any(|cand| *cand >= next_window_start && !reserved_ln_repr.contains(cand))
            })
            .unwrap_or(false);
        let pre_shor_h50_to_prev = false
            && ho.is_long_note()
            && prev_is_ln_stale
            && prev_was_miss
            && !prev_had_prewin_pen
            && prev_dur_stale.map(|d| d <= w.hit100).unwrap_or(false)
            && current_ln_duration > w.hit100
            && current_ln_duration <= w.hit50 + w.hit100
            && prev_stale_time
                .map(|prev_t| {
                    cand_pt == prev_t + w.hit50 && ho.time > cand_pt && ho.time - cand_pt <= w.max
                })
                .unwrap_or(false)
            && prev_end_stale
                .map(|prev_end| cand_pt > prev_end)
                .unwrap_or(false)
            && holds_through_head
            && release_after_cand
                .map(|rt| {
                    rt <= ho.end_time.unwrap_or(ho.time)
                        && next_note_time.map(|next_t| rt < next_t).unwrap_or(true)
                })
                .unwrap_or(false)
            && nex_sam_col_hea_cand;
        let pre_sho_h50_tail_win = false
            && ho.is_long_note()
            && prev_is_ln_stale
            && prev_was_miss
            && !prev_had_prewin_pen
            && prev_dur_stale.map(|d| d <= w.hit100).unwrap_or(false)
            && current_ln_duration <= w.hit100
            && prev_stale_time
                .map(|prev_t| {
                    cand_pt == prev_t + w.hit50 && ho.time > cand_pt && ho.time - cand_pt <= w.max
                })
                .unwrap_or(false)
            && prev_end_stale
                .map(|prev_end| cand_pt > prev_end)
                .unwrap_or(false)
            && holds_through_head
            && release_after_cand
                .map(|rt| {
                    let end_time = ho.end_time.unwrap_or(ho.time);
                    rt > end_time
                        && rt <= end_time + w.hit100
                        && next_note_time.map(|next_t| rt < next_t).unwrap_or(true)
                })
                .unwrap_or(false)
            && nex_sam_col_hea_cand;
        let prev_miss_shor_stays = false
            && ho.is_long_note()
            && prev_is_ln_stale
            && prev_was_miss
            && prev_break_pre
            && prev_dur_stale.map(|d| d <= w.hit100).unwrap_or(false)
            && matches!(
                calc_hit_kind((ho.time - cand_pt).abs(), w),
                JudgmentKind::Hit50 | JudgmentKind::Miss
            )
            && cand_pt < ho.time
            && holds_through_head
            && prev_end_stale
                .map(|prev_end| cand_pt >= prev_end - w.hit50 && cand_pt < prev_end)
                .unwrap_or(false)
            && release_after_cand
                .map(|rt| rt > ho.time && rt <= ho.end_time.unwrap_or(ho.time))
                .unwrap_or(false)
            && presses
                .iter()
                .skip(press_idx + 1)
                .take_while(|next_pt| {
                    next_note_time
                        .map(|next_t| **next_pt < next_t)
                        .unwrap_or(true)
                })
                .any(|next_pt| {
                    let next_press = *next_pt;
                    next_press > ho.end_time.unwrap_or(ho.time)
                        && next_press <= ho.end_time.unwrap_or(ho.time) + w.hit100
                        && !reserved_ln_repr.contains(next_pt)
                        && events
                            .iter()
                            .find(|ev| ev.time > next_press && !ev.pressed)
                            .map(|ev| {
                                ev.time >= ho.end_time.unwrap_or(ho.time)
                                    && next_note_time
                                        .map(|next_t| ev.time < next_t)
                                        .unwrap_or(true)
                            })
                            .unwrap_or(false)
                })
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| {
                    if !next_ho.is_long_note() {
                        return false;
                    }
                    let next_window_start = next_ho.time - w.hit50;
                    let next_win_end = next_ho.time + w.hit100;
                    presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_win_end)
                        .any(|cand| *cand >= next_window_start && !reserved_ln_repr.contains(cand))
                })
                .unwrap_or(false);
        let prev_miss_short_stays = false
            && ho.is_long_note()
            && prev_is_ln_stale
            && prev_was_miss
            && prev_break_pre
            && prev_dur_stale.map(|d| d <= w.hit100).unwrap_or(false)
            && current_ln_duration <= w.hit100
            && matches!(
                calc_hit_kind((ho.time - cand_pt).abs(), w),
                JudgmentKind::Hit50 | JudgmentKind::Miss
            )
            && cand_pt < ho.time
            && holds_through_head
            && prev_end_stale
                .map(|prev_end| cand_pt >= prev_end - w.hit50 && cand_pt < prev_end)
                .unwrap_or(false)
            && release_after_cand
                .zip(prev_end_stale)
                .map(|(rt, prev_end)| rt > prev_end && rt <= ho.end_time.unwrap_or(ho.time))
                .unwrap_or(false)
            && !presses
                .iter()
                .skip(press_idx + 1)
                .take_while(|next_pt| {
                    next_note_time
                        .map(|next_t| **next_pt < next_t)
                        .unwrap_or(true)
                })
                .any(|next_pt| *next_pt >= ho.time && !reserved_ln_repr.contains(next_pt))
            && nex_sam_col_hea_cand;
        let prev_miss_long_stays = false
            && ho.is_long_note()
            && prev_is_ln_stale
            && prev_was_miss
            && prev_break_pre
            && prev_dur_stale.map(|d| d <= w.hit100).unwrap_or(false)
            && current_ln_duration > w.hit50 + w.hit100
            && cand_pt < ho.time
            && holds_through_head
            && prev_end_stale
                .map(|prev_end| cand_pt >= prev_end - w.hit50 && cand_pt < prev_end)
                .unwrap_or(false)
            && release_after_cand
                .map(|rt| {
                    rt > ho.end_time.unwrap_or(ho.time)
                        && next_note_time.map(|next_t| rt < next_t).unwrap_or(true)
                })
                .unwrap_or(false)
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| {
                    let next_window_start = next_ho.time - w.hit50;
                    let next_win_end = next_ho.time + w.hit100;
                    presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_win_end)
                        .any(|cand| *cand >= next_window_start && !reserved_ln_repr.contains(cand))
                })
                .unwrap_or(false);
        let stale_rule = if stale_prev_ln_cand {
            Some("pre_ln_hel_head_cand")
        } else if stale_prev_tail_repr {
            Some("pre_ln_tai_repr_cand")
        } else if far_prehead_repr {
            Some("far_prehead_repr")
        } else if short_ln_prehold_h50 {
            Some("short_ln_prehold_h50")
        } else if pre_shor_h50_to_prev {
            Some("pre_shor_h50_to_prev")
        } else if pre_sho_h50_tail_win {
            Some("pre_sho_h50_tail_win")
        } else if prev_miss_shor_stays {
            Some("prev_miss_shor_stays")
        } else if prev_miss_short_stays {
            Some("prev_miss_short_stays")
        } else if prev_miss_long_stays {
            Some("prev_miss_long_stays")
        } else {
            None
        };
        if let Some(rule_id) = stale_rule {
            skipped_stale_prev = true;
            state.rules.stale = Some(rule_id);
            press_idx += 1;
            continue;
        }
        break;
    }
    state.press_idx = press_idx;
    state.rules.early_pen = early_pen_pt;
    state.prev.skipped_stale = skipped_stale_prev;
}
