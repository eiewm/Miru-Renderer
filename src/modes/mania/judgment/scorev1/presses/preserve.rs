use super::note::{PressNoteCtx, PressState};
use crate::modes::mania::judgment::calc_hit_kind;
use crate::types::JudgmentKind;
#[derive(Clone, Copy)]
enum KeepTapReason {
    Pen,
    Early,
    Chain,
    IsoFollow,
    PrewinEarly,
    PrewinPen,
    NearPrewin,
    PrewinNoise,
    PrewinBody,
    PrewinHeadBreak,
}
#[derive(Clone, Copy)]
enum KeepLnReason {
    Early,
    PenPair,
    PrewinEarly,
    PrewinTapPen,
    PrewinNoise,
    PrewinHeadBreak,
    HeadCand,
}
pub(super) fn preserve(ctx: &PressNoteCtx<'_>, state: &mut PressState) {
    let note_pos = ctx.note_pos;
    let ho = ctx.ho;
    let col_notes = ctx.col_notes;
    let presses = ctx.presses;
    let events = ctx.events;
    let w = ctx.windows;
    let next_note_time = ctx.next_note_time;
    let note_window = ctx.note_window;
    let early_penalty_window = note_window.early_penalty_window;
    let _start_press_idx = state.press_idx;
    let mut press_idx = state.press_idx;
    let press_time = state.pick.press;
    let reserved_ln_repr = &state.prev.reserved_ln_repr;
    let mut preserved_candidate: Option<(usize, i32)> = None;
    let earl_head_nois_start = ho.time - early_penalty_window;
    let cur_tap_pen_miss_pt = false
        && !ho.is_long_note()
        && press_time
            .map(|pt| pt < ho.time && calc_hit_kind((pt - ho.time).abs(), w) == JudgmentKind::Miss)
            .unwrap_or(false);
    let cur_tap_nonmiss_early = false
        && !ho.is_long_note()
        && press_time
            .map(|pt| pt < ho.time && calc_hit_kind((pt - ho.time).abs(), w) != JudgmentKind::Miss)
            .unwrap_or(false);
    let cur_tap_nonperf_early = false
        && !ho.is_long_note()
        && press_time
            .map(|pt| {
                pt < ho.time
                    && ho.time - pt > w.max
                    && calc_hit_kind((pt - ho.time).abs(), w) != JudgmentKind::Miss
            })
            .unwrap_or(false);
    let cur_short_pre_frag = false
        && ho.is_long_note()
        && ho
            .end_time
            .map(|end_time| end_time - ho.time <= w.hit100)
            .unwrap_or(false)
        && press_time
            .map(|pt| pt < ho.time && calc_hit_kind((pt - ho.time).abs(), w) != JudgmentKind::Miss)
            .unwrap_or(false)
        && press_time
            .and_then(|pt| {
                events
                    .iter()
                    .find(|ev| ev.time > pt && !ev.pressed)
                    .map(|ev| ev.time)
            })
            .map(|rt| rt < ho.time)
            .unwrap_or(false);
    while press_idx < presses.len() {
        let next_pt = presses[press_idx];
        if reserved_ln_repr.contains(&next_pt) {
            press_idx += 1;
            continue;
        }
        let tap_pen = cur_tap_pen_miss_pt
            && next_note_time
                .map(|next_time| {
                    let next_window_start = next_time - w.hit50;
                    next_time - ho.time <= w.hit50
                        && next_pt >= next_window_start
                        && next_pt < ho.time
                })
                .unwrap_or(false);
        let tap_early = cur_tap_nonmiss_early
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| !next_ho.is_long_note())
                .unwrap_or(false)
            && next_note_time
                .map(|next_time| {
                    let next_window_start = next_time - w.hit50;
                    next_time - ho.time <= w.hit50 + w.hit300
                        && next_pt >= next_window_start
                        && next_pt < ho.time
                        && events
                            .iter()
                            .find(|ev| ev.time > next_pt && !ev.pressed)
                            .map(|ev| ev.time < next_time)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let tap_chain = cur_tap_nonperf_early
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| {
                    let Some((_, next_next_ho)) = col_notes.get(note_pos + 2) else {
                        return false;
                    };
                    let Some((_, next_next_next_ho)) = col_notes.get(note_pos + 3) else {
                        return false;
                    };
                    let Some((_, next_fourth_ho)) = col_notes.get(note_pos + 4) else {
                        return false;
                    };
                    let Some((_, next_fifth_ho)) = col_notes.get(note_pos + 5) else {
                        return false;
                    };
                    if next_ho.is_long_note()
                        || next_next_ho.is_long_note()
                        || next_next_next_ho.is_long_note()
                        || next_fourth_ho.is_long_note()
                        || next_fifth_ho.is_long_note()
                    {
                        return false;
                    }
                    let next_head = next_ho.time;
                    let next_next_head = next_next_ho.time;
                    let next_next_next_head = next_next_next_ho.time;
                    let next_fourth_head = next_fourth_ho.time;
                    let next_fifth_head = next_fifth_ho.time;
                    let next_window_start = next_head - w.hit50;
                    let release_after_next_pt = events
                        .iter()
                        .find(|ev| ev.time > next_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    if !(next_head - ho.time <= w.hit50 + w.hit300
                        && next_pt >= ho.time - w.max
                        && next_pt >= next_window_start
                        && next_pt < ho.time
                        && release_after_next_pt
                            .map(|rt| rt > next_head && rt < next_next_head)
                            .unwrap_or(false))
                    {
                        return false;
                    }
                    let Some((followup1_idx, _)) = presses
                        .iter()
                        .enumerate()
                        .skip(press_idx + 1)
                        .take_while(|(_, cand)| **cand < next_next_head)
                        .find(|(_, cand)| {
                            let cand_pt = **cand;
                            cand_pt >= next_head
                                && !reserved_ln_repr.contains(cand)
                                && calc_hit_kind((cand_pt - next_next_head).abs(), w)
                                    != JudgmentKind::Miss
                                && events
                                    .iter()
                                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                                    .map(|ev| ev.time < next_next_next_head)
                                    .unwrap_or(false)
                        })
                    else {
                        return false;
                    };
                    let Some((followup2_idx, _)) = presses
                        .iter()
                        .enumerate()
                        .skip(followup1_idx + 1)
                        .take_while(|(_, cand)| **cand < next_next_next_head)
                        .find(|(_, cand)| {
                            let cand_pt = **cand;
                            cand_pt >= next_next_head
                                && !reserved_ln_repr.contains(cand)
                                && calc_hit_kind((cand_pt - next_next_next_head).abs(), w)
                                    != JudgmentKind::Miss
                                && events
                                    .iter()
                                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                                    .map(|ev| ev.time < next_fourth_head)
                                    .unwrap_or(false)
                        })
                    else {
                        return false;
                    };
                    presses
                        .iter()
                        .skip(followup2_idx + 1)
                        .take_while(|cand| **cand < next_fourth_head)
                        .any(|cand| {
                            let cand_pt = *cand;
                            cand_pt >= next_next_next_head
                                && !reserved_ln_repr.contains(cand)
                                && calc_hit_kind((cand_pt - next_fourth_head).abs(), w)
                                    != JudgmentKind::Miss
                                && events
                                    .iter()
                                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                                    .map(|ev| ev.time < next_fifth_head)
                                    .unwrap_or(false)
                        })
                })
                .unwrap_or(false);
        let tap_iso_follow = cur_tap_nonperf_early
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| !next_ho.is_long_note())
                .unwrap_or(false)
            && next_note_time
                .map(|next_time| {
                    let Some((_, next_next_ho)) = col_notes.get(note_pos + 2) else {
                        return false;
                    };
                    if next_next_ho.is_long_note() {
                        return false;
                    }
                    let next_window_start = next_time - w.hit50;
                    let next_next_head = next_next_ho.time;
                    let release_after_next_pt = events
                        .iter()
                        .find(|ev| ev.time > next_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let next_tap_post_follow = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_next_head)
                        .any(|cand| {
                            let cand_pt = *cand;
                            cand_pt >= next_time
                                && !reserved_ln_repr.contains(cand)
                                && calc_hit_kind((cand_pt - next_time).abs(), w)
                                    != JudgmentKind::Miss
                                && events
                                    .iter()
                                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                                    .map(|ev| ev.time < next_next_head)
                                    .unwrap_or(false)
                        });
                    next_time - ho.time <= w.hit50
                        && next_next_head - next_time > w.hit50 + w.hit300
                        && next_pt >= next_window_start
                        && next_pt < ho.time
                        && next_pt >= ho.time - (w.hit300 + w.max)
                        && release_after_next_pt
                            .map(|rt| rt > next_time && rt < next_next_head)
                            .unwrap_or(false)
                        && next_tap_post_follow
                })
                .unwrap_or(false);
        let ln_early = (cur_tap_nonmiss_early || cur_short_pre_frag)
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| next_ho.is_long_note())
                .unwrap_or(false)
            && next_note_time
                .map(|next_time| {
                    let next_window_start = next_time - w.hit50;
                    let release_after_next_pt = events
                        .iter()
                        .find(|ev| ev.time > next_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let short_next_no_self = cur_tap_nonperf_early
                        && next_pt < ho.time
                        && col_notes
                            .get(note_pos + 1)
                            .zip(col_notes.get(note_pos + 2))
                            .map(|((_, next_ho), (_, next_next_ho))| {
                                let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                                let next_duration = next_end_time - next_ho.time;
                                let next2_win_start = next_next_ho.time - w.hit50;
                                let next2_win_end = next_next_ho.time + w.hit100;
                                let next3_note_time = col_notes
                                    .get(note_pos + 3)
                                    .map(|(_, next_next_next_ho)| next_next_next_ho.time);
                                let next_ln_sel_head = presses
                                    .iter()
                                    .skip(press_idx + 1)
                                    .take_while(|cand| **cand < next2_win_start)
                                    .any(|cand| {
                                        *cand >= next_time
                                            && !reserved_ln_repr.contains(cand)
                                            && calc_hit_kind((*cand - next_time).abs(), w)
                                                != JudgmentKind::Miss
                                            && events
                                                .iter()
                                                .find(|ev| ev.time > *cand && !ev.pressed)
                                                .map(|ev| {
                                                    ev.time > *cand
                                                        && ev.time <= next_end_time
                                                        && ev.time < next2_win_start
                                                })
                                                .unwrap_or(false)
                                    });
                                let next_tap_follow_cand = !next_next_ho.is_long_note()
                                    && release_after_next_pt
                                        .map(|rt| rt < next_next_ho.time)
                                        .unwrap_or(false)
                                    && presses
                                        .iter()
                                        .skip(press_idx + 1)
                                        .take_while(|cand| **cand < next2_win_end)
                                        .any(|cand| {
                                            let cand_pt = *cand;
                                            cand_pt >= next2_win_start
                                                && !reserved_ln_repr.contains(cand)
                                                && calc_hit_kind(
                                                    (cand_pt - next_next_ho.time).abs(),
                                                    w,
                                                ) != JudgmentKind::Miss
                                                && events
                                                    .iter()
                                                    .find(|ev| ev.time > cand_pt && !ev.pressed)
                                                    .map(|ev| {
                                                        next3_note_time
                                                            .map(|next_time| ev.time < next_time)
                                                            .unwrap_or(true)
                                                    })
                                                    .unwrap_or(false)
                                        });
                                let next_tap_miss_ln = !next_next_ho.is_long_note()
                                    && !next_tap_follow_cand
                                    && release_after_next_pt
                                        .map(|rt| rt < next_next_ho.time)
                                        .unwrap_or(false)
                                    && col_notes
                                        .get(note_pos + 3)
                                        .map(|(_, next_next_next_ho)| {
                                            if !next_next_next_ho.is_long_note() {
                                                return false;
                                            }
                                            let follow_head = next_next_next_ho.time;
                                            let follow_end =
                                                next_next_next_ho.end_time.unwrap_or(follow_head);
                                            let follow_window_start = follow_head - w.hit50;
                                            let follow_tail_start = follow_end - w.hit50;
                                            let fol_tail_end_end = follow_end + w.hit100;
                                            let follow_next_note_time =
                                                col_notes.get(note_pos + 4).map(|(_, ho)| ho.time);
                                            let fol_late_bound_incls = follow_next_note_time
                                                .map(|next_time| next_time <= follow_head + w.hit50)
                                                .unwrap_or(false);
                                            let fol_win_end_end = follow_head
                                                + w.hit50
                                                + if fol_late_bound_incls { 1 } else { 0 };
                                            presses
                                                .iter()
                                                .skip(press_idx + 1)
                                                .take_while(|cand| **cand < fol_win_end_end)
                                                .any(|cand| {
                                                    let cand_pt = *cand;
                                                    cand_pt >= next_next_ho.time
                                                        && cand_pt >= follow_window_start
                                                        && !reserved_ln_repr.contains(cand)
                                                        && events
                                                            .iter()
                                                            .find(|ev| {
                                                                ev.time > cand_pt && !ev.pressed
                                                            })
                                                            .map(|ev| {
                                                                ev.time >= follow_tail_start
                                                                    && ev.time < fol_tail_end_end
                                                                    && follow_next_note_time
                                                                        .map(|next_time| {
                                                                            ev.time < next_time
                                                                        })
                                                                        .unwrap_or(true)
                                                            })
                                                            .unwrap_or(false)
                                                })
                                        })
                                        .unwrap_or(false);
                                let frag_starts_near_head = next_pt >= ho.time - w.max
                                    || (next_tap_follow_cand && next_pt >= ho.time - w.hit300);
                                next_ho.is_long_note()
                                    && (next_next_ho.is_long_note()
                                        || next_tap_follow_cand
                                        || next_tap_miss_ln)
                                    && frag_starts_near_head
                                    && next_duration <= w.hit100
                                    && release_after_next_pt
                                        .map(|rt| rt > next_time && rt <= next_end_time)
                                        .unwrap_or(false)
                                    && !next_ln_sel_head
                            })
                            .unwrap_or(false);
                    next_time - ho.time <= w.hit50 + w.hit300
                        && next_pt >= next_window_start
                        && next_pt < ho.time
                        && (release_after_next_pt
                            .map(|rt| rt < next_time)
                            .unwrap_or(false)
                            || short_next_no_self)
                })
                .unwrap_or(false);
        let ln_pen_pair = cur_tap_pen_miss_pt
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| next_ho.is_long_note())
                .unwrap_or(false)
            && next_note_time
                .zip(col_notes.get(note_pos + 1))
                .map(|(next_time, (_, next_ho))| {
                    let next_window_start = next_time - w.hit50;
                    let release_after_next_pt = events
                        .iter()
                        .find(|ev| ev.time > next_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                    let next_duration = next_end_time - next_ho.time;
                    let next_tail_start = next_end_time - w.hit50;
                    let next_tail_end = next_end_time + w.hit100;
                    let next_next_note_time = col_notes.get(note_pos + 2).map(|(_, ho)| ho.time);
                    next_time - ho.time <= w.hit50 + w.hit300
                        && next_pt >= next_window_start
                        && next_pt < ho.time
                        && next_duration <= w.hit100
                        && release_after_next_pt
                            .map(|rt| {
                                rt >= next_tail_start
                                    && rt < next_tail_end
                                    && rt > next_pt
                                    && next_next_note_time
                                        .map(|next_time| rt < next_time)
                                        .unwrap_or(true)
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let tap_prewin_early = cur_tap_nonperf_early
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| !next_ho.is_long_note())
                .unwrap_or(false)
            && next_note_time
                .map(|next_time| {
                    let next_window_start = next_time - w.hit50;
                    let next_prewin_start = next_window_start - early_penalty_window - 1;
                    next_time - ho.time <= w.hit50 + w.hit300
                        && next_pt >= next_prewin_start
                        && next_pt < next_window_start
                        && next_pt < ho.time
                        && events
                            .iter()
                            .find(|ev| ev.time > next_pt && !ev.pressed)
                            .map(|ev| ev.time < next_time)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let ln_prewin_early = cur_tap_nonperf_early
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| next_ho.is_long_note())
                .unwrap_or(false)
            && next_note_time
                .map(|next_time| {
                    let next_window_start = next_time - w.hit50;
                    let next_prewin_start = next_window_start - early_penalty_window - 1;
                    next_time - ho.time <= w.hit50 + w.hit300
                        && next_pt > ho.time - w.hit300
                        && next_pt >= next_prewin_start
                        && next_pt < next_window_start
                        && next_pt < ho.time
                        && events
                            .iter()
                            .find(|ev| ev.time > next_pt && !ev.pressed)
                            .map(|ev| ev.time < next_time)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let tap_near_prewin = cur_tap_pen_miss_pt
            && press_time
                .map(|cur_pt| next_pt > cur_pt && ho.time - cur_pt <= w.hit50 + w.max)
                .unwrap_or(false)
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| !next_ho.is_long_note())
                .unwrap_or(false)
            && next_note_time
                .map(|next_time| {
                    let next_window_start = next_time - w.hit50;
                    let near_next_prwn_start = next_window_start - (w.max + 1);
                    next_time - ho.time <= w.hit50
                        && next_pt >= ho.time - (w.hit300 + w.max)
                        && next_pt >= near_next_prwn_start
                        && next_pt < next_window_start
                        && next_pt < ho.time
                        && events
                            .iter()
                            .find(|ev| ev.time > next_pt && !ev.pressed)
                            .map(|ev| ev.time < next_time)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let tap_prewin_pen = cur_tap_pen_miss_pt
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| !next_ho.is_long_note())
                .unwrap_or(false)
            && next_note_time
                .map(|next_time| {
                    let next_window_start = next_time - w.hit50;
                    let next_prewin_start = next_window_start - early_penalty_window - 1;
                    next_time - ho.time <= w.hit50 + w.hit300
                        && next_pt >= ho.time - (w.hit300 + w.max)
                        && next_pt >= next_prewin_start
                        && next_pt < next_window_start
                        && next_pt < ho.time
                        && events
                            .iter()
                            .find(|ev| ev.time > next_pt && !ev.pressed)
                            .map(|ev| ev.time < next_time + w.hit300)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let ln_prewin_tap_pen = cur_tap_pen_miss_pt
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| next_ho.is_long_note())
                .unwrap_or(false)
            && next_note_time
                .map(|next_time| {
                    let next_window_start = next_time - w.hit50;
                    let next_prewin_start = next_window_start - early_penalty_window - 1;
                    next_time - ho.time <= w.hit50 + w.hit300
                        && next_pt >= next_prewin_start
                        && next_pt < next_window_start
                        && next_pt < ho.time
                        && events
                            .iter()
                            .find(|ev| ev.time > next_pt && !ev.pressed)
                            .map(|ev| ev.time < next_time)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let ln_prewin_noise = false
            && ho.is_long_note()
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| next_ho.is_long_note())
                .unwrap_or(false)
            && press_time
                .map(|cur_pt| {
                    next_pt > cur_pt
                        && calc_hit_kind((cur_pt - ho.time).abs(), w) != JudgmentKind::Miss
                })
                .unwrap_or(false)
            && next_note_time
                .map(|next_time| {
                    let next_window_start = next_time - w.hit50;
                    let next_prewin_start = next_window_start - early_penalty_window - 1;
                    next_time - ho.time <= w.hit50 * 2 + w.max
                        && next_pt >= next_prewin_start
                        && next_pt < next_window_start
                        && next_pt < ho.time
                        && events
                            .iter()
                            .find(|ev| ev.time > next_pt && !ev.pressed)
                            .map(|ev| ev.time < next_time)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let tap_prewin_noise = false
            && ho.is_long_note()
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| !next_ho.is_long_note())
                .unwrap_or(false)
            && press_time
                .map(|cur_pt| {
                    next_pt > cur_pt
                        && calc_hit_kind((cur_pt - ho.time).abs(), w) != JudgmentKind::Miss
                })
                .unwrap_or(false)
            && next_note_time
                .map(|next_time| {
                    let current_end_time = ho.end_time.unwrap_or(ho.time);
                    let current_duration = current_end_time - ho.time;
                    let next_window_start = next_time - w.hit50;
                    let next_prewin_start = next_window_start - early_penalty_window - 1;
                    next_time - ho.time <= w.hit50 + w.hit300
                        && next_pt >= next_prewin_start
                        && next_pt < next_window_start
                        && next_pt < ho.time
                        && events
                            .iter()
                            .find(|ev| ev.time > next_pt && !ev.pressed)
                            .map(|ev| {
                                ev.time < next_time
                                    && ((ev.time >= current_end_time
                                        && ev.time <= current_end_time + w.max)
                                        || (current_duration <= w.hit100
                                            && ev.time > current_end_time + w.max
                                            && ev.time <= current_end_time + w.hit300)
                                        || (next_pt >= next_window_start - w.max
                                            && ev.time >= current_end_time - w.max
                                            && ev.time < current_end_time))
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let tap_prewin_body = false
            && ho.is_long_note()
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| !next_ho.is_long_note())
                .unwrap_or(false)
            && press_time
                .map(|cur_pt| {
                    next_pt > cur_pt
                        && calc_hit_kind((cur_pt - ho.time).abs(), w) != JudgmentKind::Miss
                })
                .unwrap_or(false)
            && next_note_time
                .map(|next_time| {
                    let current_end_time = ho.end_time.unwrap_or(ho.time);
                    let current_ln_duration = current_end_time - ho.time;
                    let next_window_start = next_time - w.hit50;
                    let next_prewin_start = next_window_start - early_penalty_window - 1;
                    let rel_after_press = press_time.and_then(|cur_pt| {
                        events
                            .iter()
                            .find(|ev| ev.time > cur_pt && !ev.pressed)
                            .map(|ev| ev.time)
                    });
                    let release_after_next_pt = events
                        .iter()
                        .find(|ev| ev.time > next_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    let fol_pt_post_end = presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_time + w.hit100)
                        .find(|cand| !reserved_ln_repr.contains(cand))
                        .map(|cand| *cand > current_end_time)
                        .unwrap_or(false);
                    let next2_ln_h50_pair = col_notes
                        .get(note_pos + 2)
                        .map(|(_, next_next_ho)| {
                            if !next_next_ho.is_long_note() {
                                return false;
                            }
                            let next_next_duration =
                                next_next_ho.end_time.unwrap_or(next_next_ho.time)
                                    - next_next_ho.time;
                            let next_next_head = next_next_ho.time;
                            let next_next_end = next_next_ho.end_time.unwrap_or(next_next_ho.time);
                            let next2_win_start = next_next_head - w.hit50;
                            let next_next_tail_start = next_next_end - w.hit50;
                            let next2_tail_end = next_next_end + w.hit100;
                            let next3_note_time =
                                col_notes.get(note_pos + 3).map(|(_, ho)| ho.time);
                            let next2_late_end = next3_note_time
                                .map(|next_time| next_time <= next_next_head + w.hit50)
                                .unwrap_or(false);
                            let next2_lock_end =
                                next_next_head + w.hit50 + if next2_late_end { 1 } else { 0 };
                            let prehead_h50_cand = presses
                                .iter()
                                .copied()
                                .skip(press_idx + 1)
                                .take_while(|cand| *cand < next_next_head)
                                .find(|cand| {
                                    *cand > current_end_time
                                        && *cand >= next2_win_start
                                        && !reserved_ln_repr.contains(cand)
                                });
                            prehead_h50_cand
                                .and_then(|cand_pt| {
                                    let cand_release = events
                                        .iter()
                                        .find(|ev| ev.time > cand_pt && !ev.pressed)
                                        .map(|ev| ev.time)?;
                                    let followup_pt = presses
                                        .iter()
                                        .copied()
                                        .skip(press_idx + 1)
                                        .take_while(|cand| *cand < next2_lock_end)
                                        .find(|cand| {
                                            *cand > cand_pt
                                                && *cand >= next2_win_start
                                                && !reserved_ln_repr.contains(cand)
                                        })?;
                                    let followup_release = events
                                        .iter()
                                        .find(|ev| ev.time > followup_pt && !ev.pressed)
                                        .map(|ev| ev.time)?;
                                    Some(
                                        next_next_duration > w.hit100
                                            && calc_hit_kind((cand_pt - next_next_head).abs(), w)
                                                == JudgmentKind::Hit50
                                            && cand_release > cand_pt
                                            && cand_release < next_next_head
                                            && followup_release >= next_next_tail_start
                                            && followup_release < next2_tail_end,
                                    )
                                })
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    current_ln_duration <= w.hit100
                        && next_time - ho.time <= w.hit50 + w.hit300
                        && next_pt >= next_prewin_start
                        && (next_pt >= next_window_start - ((w.hit300 + 1) / 2)
                            || (next_time - ho.time <= w.hit50 + w.max
                                && next_pt >= next_window_start - w.hit300))
                        && next_pt < next_window_start
                        && next_pt < ho.time
                        && rel_after_press.map(|rt| rt < next_pt).unwrap_or(false)
                        && release_after_next_pt
                            .map(|rt| {
                                (rt > next_pt && rt < current_end_time)
                                    || (next_time - ho.time <= w.hit50 + w.max
                                        && rt > current_end_time
                                        && rt < next_time
                                        && next2_ln_h50_pair)
                            })
                            .unwrap_or(false)
                        && fol_pt_post_end
                })
                .unwrap_or(false);
        let ln_prewin_head_break = {
            let current_duration = ho.end_time.unwrap_or(ho.time) - ho.time;
            let next_duration = col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| next_ho.end_time.unwrap_or(next_ho.time) - next_ho.time);
            let next_is_ln = col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| next_ho.is_long_note())
                .unwrap_or(false);
            let current_is_miss = press_time
                .map(|cur_pt| calc_hit_kind((cur_pt - ho.time).abs(), w) == JudgmentKind::Miss)
                .unwrap_or(false);
            let current_short_enough = ho
                .end_time
                .map(|end_time| end_time - ho.time <= w.hit300 + w.max)
                .unwrap_or(false);
            let next_short_enough = next_duration
                .map(|dur| dur <= w.hit300 + w.max)
                .unwrap_or(false);
            let cur_rel_breaks_next = press_time
                .map(|cur_pt| {
                    events
                        .iter()
                        .find(|ev| ev.time > cur_pt && !ev.pressed)
                        .map(|ev| ev.time < next_pt)
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            let nex_ln_prw_frag_vali = next_note_time
                .map(|next_time| {
                    let next_window_start = next_time - w.hit50;
                    let next_prewin_start = next_window_start - early_penalty_window - 1;
                    next_time - ho.time <= w.hit50 * 2 + w.hit300
                        && next_pt >= next_prewin_start
                        && next_pt < next_window_start
                        && next_pt < ho.time
                        && events
                            .iter()
                            .find(|ev| ev.time > next_pt && !ev.pressed)
                            .map(|ev| ev.time < next_time)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
            let nex_ln_post_fol_cand = col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| {
                    let next_head = next_ho.time;
                    let next2_win_start = col_notes
                        .get(note_pos + 2)
                        .map(|(_, next_next_ho)| next_next_ho.time - w.hit50);
                    let next_late_end = col_notes
                        .get(note_pos + 2)
                        .map(|(_, next_next_ho)| next_next_ho.time <= next_head + w.hit50)
                        .unwrap_or(false);
                    let next_win_end = next_head + w.hit50 + if next_late_end { 1 } else { 0 };
                    presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_win_end)
                        .any(|cand| {
                            *cand >= next_head
                                && next2_win_start.map(|start| *cand < start).unwrap_or(true)
                                && calc_hit_kind((*cand - next_head).abs(), w) != JudgmentKind::Miss
                                && !reserved_ln_repr.contains(cand)
                        })
                })
                .unwrap_or(false);
            let short_pair_head = next_duration
                .map(|next_dur| {
                    current_duration > w.hit300 + w.max
                        && current_duration <= w.hit100
                        && next_dur > w.hit300 + w.max
                        && next_dur <= w.hit100
                        && next_pt >= ho.time - w.max
                        && next_pt < ho.time
                        && !nex_ln_post_fol_cand
                        && events
                            .iter()
                            .find(|ev| ev.time > next_pt && !ev.pressed)
                            .map(|ev| {
                                ev.time > next_pt && ev.time <= ho.end_time.unwrap_or(ho.time)
                            })
                            .unwrap_or(false)
                })
                .unwrap_or(false);
            false
                && ho.is_long_note()
                && next_is_ln
                && current_is_miss
                && (((current_short_enough && next_short_enough) && !nex_ln_post_fol_cand)
                    || short_pair_head)
                && cur_rel_breaks_next
                && nex_ln_prw_frag_vali
        };
        let tap_prewin_head_break = {
            let current_end_time = ho.end_time.unwrap_or(ho.time);
            let current_duration = current_end_time - ho.time;
            let next_is_tap = col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| !next_ho.is_long_note())
                .unwrap_or(false);
            let current_is_miss = press_time
                .map(|cur_pt| calc_hit_kind((cur_pt - ho.time).abs(), w) == JudgmentKind::Miss)
                .unwrap_or(false);
            let cur_rel_breaks_next = press_time
                .map(|cur_pt| {
                    events
                        .iter()
                        .find(|ev| ev.time > cur_pt && !ev.pressed)
                        .map(|ev| ev.time < next_pt)
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            let nex_tap_prw_fra_vali = next_note_time
                .map(|next_time| {
                    let next_window_start = next_time - w.hit50;
                    let next_prewin_start = next_window_start - early_penalty_window - 1;
                    next_time - ho.time <= w.hit50 * 2 + w.hit300
                        && next_pt >= next_prewin_start
                        && next_pt < next_window_start
                        && next_pt < ho.time
                        && events
                            .iter()
                            .find(|ev| ev.time > next_pt && !ev.pressed)
                            .map(|ev| ev.time >= current_end_time && ev.time < next_time)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
            let next_tap_follow_cand = next_note_time
                .zip(col_notes.get(note_pos + 2))
                .map(|(next_time, (_, next_next_ho))| {
                    let next_window_start = next_time - w.hit50;
                    let next_win_end = next_time + w.hit100;
                    presses
                        .iter()
                        .skip(press_idx + 1)
                        .take_while(|cand| **cand < next_next_ho.time)
                        .any(|cand| {
                            *cand >= next_window_start
                                && *cand < next_win_end
                                && !reserved_ln_repr.contains(cand)
                        })
                })
                .unwrap_or(false);
            false
                && ho.is_long_note()
                && next_is_tap
                && current_duration <= w.hit100
                && current_is_miss
                && cur_rel_breaks_next
                && nex_tap_prw_fra_vali
                && next_tap_follow_cand
        };
        let ln_head_cand = false
            && ho.is_long_note()
            && ho
                .end_time
                .map(|end_time| end_time - ho.time <= w.hit100)
                .unwrap_or(false)
            && col_notes
                .get(note_pos + 1)
                .map(|(_, next_ho)| next_ho.is_long_note())
                .unwrap_or(false)
            && press_time
                .zip(ho.end_time)
                .zip(next_note_time)
                .map(|((cur_pt, end_time), next_time)| {
                    let current_tail_start = end_time - w.hit50;
                    let next_window_start = next_time - w.hit50;
                    let rel_after_press = events
                        .iter()
                        .find(|ev| ev.time > cur_pt && !ev.pressed)
                        .map(|ev| ev.time);
                    next_time - ho.time <= w.hit50 + w.hit300
                        && cur_pt < ho.time
                        && next_pt >= next_window_start
                        && next_pt < ho.time
                        && rel_after_press
                            .map(|rt| rt < next_pt && rt >= current_tail_start && rt <= end_time)
                            .unwrap_or(false)
                        && events
                            .iter()
                            .find(|ev| ev.time > next_pt && !ev.pressed)
                            .map(|ev| ev.time < next_time)
                            .unwrap_or(false)
                })
                .unwrap_or(false);
        let tap_reason = if tap_pen {
            Some(KeepTapReason::Pen)
        } else if tap_early {
            Some(KeepTapReason::Early)
        } else if tap_chain {
            Some(KeepTapReason::Chain)
        } else if tap_iso_follow {
            Some(KeepTapReason::IsoFollow)
        } else if tap_prewin_early {
            Some(KeepTapReason::PrewinEarly)
        } else if tap_prewin_pen {
            Some(KeepTapReason::PrewinPen)
        } else if tap_near_prewin {
            Some(KeepTapReason::NearPrewin)
        } else if tap_prewin_noise {
            Some(KeepTapReason::PrewinNoise)
        } else if tap_prewin_body {
            Some(KeepTapReason::PrewinBody)
        } else if tap_prewin_head_break {
            Some(KeepTapReason::PrewinHeadBreak)
        } else {
            None
        };
        let ln_reason = if ln_early {
            Some(KeepLnReason::Early)
        } else if ln_pen_pair {
            Some(KeepLnReason::PenPair)
        } else if ln_prewin_early {
            Some(KeepLnReason::PrewinEarly)
        } else if ln_prewin_tap_pen {
            Some(KeepLnReason::PrewinTapPen)
        } else if ln_prewin_noise {
            Some(KeepLnReason::PrewinNoise)
        } else if ln_prewin_head_break {
            Some(KeepLnReason::PrewinHeadBreak)
        } else if ln_head_cand {
            Some(KeepLnReason::HeadCand)
        } else {
            None
        };
        let keep_next = tap_reason.is_some() || ln_reason.is_some();
        if next_pt < ho.time {
            if keep_next {
                preserved_candidate = Some((press_idx, next_pt));
                break;
            }
            if next_pt >= earl_head_nois_start {
                press_idx += 1;
                continue;
            }
        }
        break;
    }
    state.press_idx = press_idx;
    let next_note = ctx.col_notes.get(ctx.note_pos + 1).copied();
    if let (Some((_next_idx, _next_ho)), Some((_preserved_idx, _preserved_pt))) =
        (next_note, preserved_candidate)
    {}
}
