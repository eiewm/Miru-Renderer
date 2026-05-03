use super::super::claims::find_repl_pt;
use super::note::{ReleaseNoteCtx, ReleaseState};
use crate::modes::mania::judgment::{
    calc_hit_kind, steals_next_ln_head, InternalJudgment, LnDebugInfo,
};
use crate::types::{Beatmap, JudgmentKind, Windows};
use std::collections::HashMap;
fn next_rel_after_press(
    events: &[crate::modes::mania::judgment::KeyEvent],
    pt: i32,
) -> Option<i32> {
    let mut seen_press = false;
    for ev in events {
        if !seen_press {
            if ev.pressed && ev.time == pt {
                seen_press = true;
            }
            continue;
        }
        if !ev.pressed {
            return Some(ev.time);
        }
    }
    events
        .iter()
        .find(|ev| !ev.pressed && ev.time > pt)
        .map(|ev| ev.time)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn scan(
    ctx: &ReleaseNoteCtx<'_>,
    state: &mut ReleaseState,
    map: &Beatmap,
    judgments: &mut [InternalJudgment],
    w: &Windows,
    j_by_idx: &[Option<usize>],
    ln_debug_info: &HashMap<usize, LnDebugInfo>,
) {
    let idx = ctx.idx;
    let ho = ctx.ho;
    let end_time = ctx.end_time;
    let ln_duration = ctx.ln_duration;
    let tail_window_scale = ctx.tail_window_scale;
    let tail_start = ctx.tail_start;
    let late_repr_guard = ctx.late_repr_guard;
    let early_release_cutoff = ctx.early_release_cutoff;
    let tail_end_exclusive = ctx.tail_end_exclusive;
    let press_time = ctx.press_time;
    let _tail_only_pt = ctx.tail_only_pt;
    let tail_eval_press_time = ctx.tail_eval_press_time;
    let head_was_hit = ctx.head_was_hit;
    let head_is_h100 = ctx.head_is_h100;
    let head_is_h50 = ctx.head_is_h50;
    let strong_head_hit = ctx.strong_head_hit;
    let _post_end_headless_press = ctx.post_end_hless;
    let prev_same_col_idx = ctx.prev_same_col_idx;
    let _prev_same_col_ho = ctx.prev_same_col_ho;
    let prev_same_col_is_ln = ctx.prev_same_col_is_ln;
    let prev_same_col_time = ctx.prev_same_col_time;
    let prev_same_end = ctx.prev_same_end;
    let next_same_col_idx = ctx.next_same_col_idx;
    let next_same_col_time = ctx.next_same_col_time;
    let events = ctx.events;
    let mut segments = std::mem::take(&mut state.segs.list);
    let mut has_early_rel = state.early.has_rel;
    let mut first_early_rel = state.early.first_rel;
    let mut repr_after_rel = state.early.repr_after;
    let mut first_repr_after_rel = state.early.first_repr;
    let mut last_repr_time = state.early.last_repr;
    let first_free_repr = state.early.first_free_repr;
    let mut rel_after_repr = state.early.rel_after_repr;
    let rescue_rel_near_end = state.rescue.near_end_rel;
    let last_repr_free = state.early.last_repr_free;
    let mut imm_rel_at_press = state.rescue.imm_rel_at_press;
    let late_headless_rescue = state.rescue.late_headless;
    let mut tail_pref_body = state.prefs.body;
    let mut tail_pref_bridge = state.prefs.bridge;
    let mut tail_pref_early = state.prefs.early;
    let mut tail_pref_pre_frag = state.prefs.pre_frag;
    let mut tail_pref_exact = state.prefs.exact;
    let mut init_first_repr = state.rescue.init_first_repr;
    let init_rel_after_repr: Option<i32>;
    let mut short_miss_bridge = state.rescue.short_miss_bridge;
    if let Some(pt) = tail_eval_press_time {
        let exact_rel_at_press = events.iter().any(|e| !e.pressed && e.time == pt);
        let prhd_exact_tail_meta = false
            && head_was_hit
            && pt < ho.time
            && ln_duration < w.hit50 * 2
            && exact_rel_at_press;
        if (true || prhd_exact_tail_meta) && exact_rel_at_press {
            imm_rel_at_press = Some(pt);
            has_early_rel = true;
            if first_early_rel.is_none() {
                first_early_rel = Some(pt);
            }
            if prhd_exact_tail_meta {
                if let Some(repress_time) = events
                    .iter()
                    .find(|ev| ev.pressed && ev.time > pt && ev.time <= end_time)
                    .map(|ev| ev.time)
                {
                    repr_after_rel = true;
                    if first_repr_after_rel.is_none() {
                        first_repr_after_rel = Some(repress_time);
                    }
                    last_repr_time = Some(repress_time);
                }
            }
        }
        let pre_head_rel_back = w.hit50 + w.max + 1;
        let pre_ln_nea_head_repr = true
            && prev_same_col_is_ln
            && pt >= ho.time
            && pt - ho.time > w.max
            && pt - ho.time <= w.hit300;
        if head_was_hit
            && (!prev_same_col_is_ln || pre_ln_nea_head_repr)
            && pt >= ho.time
            && (pt - ho.time <= w.max || pre_ln_nea_head_repr)
            && first_repr_after_rel.is_none()
        {
            let pre_head_release = events
                .iter()
                .rev()
                .find(|ev| {
                    !ev.pressed
                        && ev.time < ho.time
                        && ev.time < pt
                        && ho.time - ev.time <= pre_head_rel_back
                })
                .map(|ev| ev.time);
            if let Some(pre_rel_time) = pre_head_release {
                let pre_head_press = events
                    .iter()
                    .rev()
                    .find(|ev| ev.pressed && ev.time < pre_rel_time)
                    .map(|ev| ev.time);
                let rel_post_assigned_pt = events
                    .iter()
                    .find(|ev| ev.time > pt && !ev.pressed)
                    .map(|ev| ev.time);
                let valid_pre_head_pair = pre_head_press
                    .map(|pre_press_time| {
                        let short_pair = pre_rel_time > pre_press_time
                            && pre_rel_time - pre_press_time <= w.max * 2 + 4;
                        let pre_ln_clm_pre_hea_pt = prev_same_col_idx
                            .and_then(|prev_idx| ln_debug_info.get(&prev_idx))
                            .map(|prev_debug| {
                                prev_debug.first_repr_after_rel == Some(pre_press_time)
                                    || prev_debug.alt_head_press_time == Some(pre_press_time)
                            })
                            .unwrap_or(false);
                        let ghost_pre_head_press = !judgments.iter().any(|jj| {
                            jj.index != idx
                                && jj.column == ho.column
                                && jj.press_time == Some(pre_press_time)
                        }) && !pre_ln_clm_pre_hea_pt;
                        let near_assigned_repress = pt - pre_rel_time <= w.hit50 + w.max + 8;
                        let pre_head_pt_off = prev_same_col_time
                            .map(|prev_t| (pre_press_time - prev_t).abs() > w.max)
                            .unwrap_or(true);
                        let prev_ln_pre_rel_end = if prev_same_col_is_ln && true {
                            prev_same_end
                                .map(|prev_end| {
                                    pre_rel_time >= prev_end - w.max
                                        && pre_rel_time <= prev_end + 1
                                        && pre_press_time >= prev_end - w.hit300
                                })
                                .unwrap_or(false)
                        } else {
                            true
                        };
                        short_pair
                            && ghost_pre_head_press
                            && near_assigned_repress
                            && pre_head_pt_off
                            && prev_ln_pre_rel_end
                    })
                    .unwrap_or(false);
                let post_head_rechs_tail = rel_post_assigned_pt
                    .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
                    .unwrap_or(false);
                if valid_pre_head_pair && post_head_rechs_tail {
                    has_early_rel = true;
                    first_early_rel = Some(pre_rel_time);
                    repr_after_rel = true;
                    first_repr_after_rel = Some(pt);
                    last_repr_time = Some(pt);
                }
            }
        }
        if head_was_hit
            && head_is_h50
            && prev_same_col_is_ln
            && pt < ho.time
            && !has_early_rel
            && first_repr_after_rel.is_none()
        {
            let pre_head_release = events
                .iter()
                .rev()
                .find(|ev| {
                    !ev.pressed
                        && ev.time < pt
                        && pt - ev.time <= 2
                        && ho.time - ev.time <= pre_head_rel_back
                })
                .map(|ev| ev.time);
            if let Some(pre_rel_time) = pre_head_release {
                let assigned_kind = calc_hit_kind((pt - ho.time).abs(), w);
                let assigned_rel_post_pt = events
                    .iter()
                    .find(|ev| !ev.pressed && ev.time > pt)
                    .map(|ev| ev.time);
                let later_prehead_press = events
                    .iter()
                    .find(|ev| {
                        ev.pressed
                            && ev.time > pt
                            && ev.time >= ho.time - w.hit50
                            && ev.time <= ho.time + w.max
                            && assigned_rel_post_pt.map(|rt| rt > ev.time).unwrap_or(true)
                            && calc_hit_kind((ev.time - ho.time).abs(), w).score_value()
                                > assigned_kind.score_value()
                    })
                    .map(|ev| ev.time);
                if let Some(later_pt) = later_prehead_press {
                    let later_rel_post_pt = events
                        .iter()
                        .find(|ev| !ev.pressed && ev.time > later_pt)
                        .map(|ev| ev.time);
                    let later_kind = calc_hit_kind((later_pt - ho.time).abs(), w);
                    let prior_rel_pre_end = prev_same_end
                        .map(|prev_end| {
                            pre_rel_time >= prev_end - w.hit50 && pre_rel_time <= prev_end + 1
                        })
                        .unwrap_or(false);
                    let later_press_unclaimed = !judgments.iter().any(|jj| {
                        jj.index != idx && jj.column == ho.column && jj.press_time == Some(later_pt)
                    });
                    let no_rel_pre_later_pt =
                        assigned_rel_post_pt.map(|rt| rt > later_pt).unwrap_or(true);
                    let lat_rel_rec_tail_win = later_rel_post_pt
                        .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
                        .unwrap_or(false);
                    let late_pair_pre_next = next_same_col_time
                        .map(|next_t| {
                            later_pt < next_t
                                && later_rel_post_pt.map(|rt| rt < next_t).unwrap_or(false)
                        })
                        .unwrap_or(true);
                    if pt > pre_rel_time
                        && later_pt > pt
                        && later_kind.score_value() > assigned_kind.score_value()
                        && prior_rel_pre_end
                        && later_press_unclaimed
                        && no_rel_pre_later_pt
                        && lat_rel_rec_tail_win
                        && late_pair_pre_next
                    {
                        has_early_rel = true;
                        first_early_rel = Some(pre_rel_time);
                        repr_after_rel = true;
                        first_repr_after_rel = Some(later_pt);
                        last_repr_time = Some(later_pt);
                        rel_after_repr = later_rel_post_pt;
                    }
                }
            }
        }
        let exa_rel_cls_ini_hold = imm_rel_at_press == Some(pt);
        if exa_rel_cls_ini_hold {
            segments.push((pt, Some(pt)));
        }
        let mut down = !exa_rel_cls_ini_hold;
        let mut seg_start = pt;
        let mut same_time_rel: Option<i32> = None;
        for ev in events
            .iter()
            .filter(|e| e.time > pt && e.time <= tail_end_exclusive)
        {
            if let Some(synth_time) = same_time_rel {
                if ev.time > synth_time {
                    same_time_rel = None;
                } else if !ev.pressed && ev.time == synth_time {
                    continue;
                }
            }
            let same_rel_hold = true
                && down
                && ev.pressed
                && ev.time > seg_start
                && ev.time < early_release_cutoff
                && events.iter().any(|other| {
                    !other.pressed
                        && other.time == ev.time
                        && other.time > pt
                        && other.time <= tail_end_exclusive
                });
            let same_rel_short_hold = false
                && head_was_hit
                && ln_duration < w.hit50 * 2
                && down
                && ev.pressed
                && ev.time > seg_start
                && ev.time < early_release_cutoff
                && events.iter().any(|other| {
                    !other.pressed
                        && other.time == ev.time
                        && other.time > pt
                        && other.time <= tail_end_exclusive
                });
            if same_rel_hold || same_rel_short_hold {
                has_early_rel = true;
                if first_early_rel.is_none() {
                    first_early_rel = Some(ev.time);
                }
                segments.push((seg_start, Some(ev.time)));
                repr_after_rel = true;
                if first_repr_after_rel.is_none() {
                    first_repr_after_rel = Some(ev.time);
                }
                last_repr_time = Some(ev.time);
                seg_start = ev.time;
                same_time_rel = Some(ev.time);
                continue;
            }
            if down && !ev.pressed {
                if ev.time < early_release_cutoff {
                    has_early_rel = true;
                    if first_early_rel.is_none() {
                        first_early_rel = Some(ev.time);
                    }
                }
                segments.push((seg_start, Some(ev.time)));
                down = false;
            } else if !down && ev.pressed {
                if has_early_rel {
                    repr_after_rel = true;
                    if first_repr_after_rel.is_none() {
                        first_repr_after_rel = Some(ev.time);
                    }
                    last_repr_time = Some(ev.time);
                }
                seg_start = ev.time;
                down = true;
            }
        }
        if down {
            segments.push((seg_start, None));
        }
        if let Some(rp) = first_repr_after_rel {
            rel_after_repr = next_rel_after_press(events, rp);
        }
        init_first_repr = first_repr_after_rel;
        init_rel_after_repr = rel_after_repr;
        let firs_repr_needs_resc = first_repr_after_rel
            .map(|_| rel_after_repr.map(|rt| rt < tail_start).unwrap_or(true))
            .unwrap_or(false);
        let has_post_head_repr = segments.iter().any(|(seg_start, seg_end)| {
            *seg_start >= ho.time
                && first_early_rel.map(|t| *seg_start > t).unwrap_or(false)
                && seg_end
                    .map(|t| t >= tail_start && t < tail_end_exclusive)
                    .unwrap_or(false)
        });
        let near_head_repr_win = if true { w.hit200 } else { w.hit300 };
        let has_tail_repr_near = segments.iter().any(|(seg_start, seg_end)| {
            *seg_start > ho.time
                && *seg_start <= ho.time + near_head_repr_win
                && first_early_rel.map(|t| *seg_start > t).unwrap_or(false)
                && seg_end
                    .map(|t| t >= tail_start && t < tail_end_exclusive)
                    .unwrap_or(false)
        });
        let keep_near_repr = !head_was_hit
            && first_repr_after_rel
                .map(|rp| rp == ho.time)
                .unwrap_or(false)
            && rel_after_repr.map(|rt| rt < tail_start).unwrap_or(false)
            && !has_post_head_repr;
        let keep_prehead_repr = !head_was_hit
            && tail_eval_press_time
                .map(|pt| pt < ho.time - w.hit50)
                .unwrap_or(false)
            && first_repr_after_rel
                .map(|rp| rp < ho.time && ho.time - rp <= w.hit300)
                .unwrap_or(false)
            && rel_after_repr.map(|rt| rt < ho.time).unwrap_or(false)
            && !has_tail_repr_near;
        let keep_hit_repr = head_was_hit
            && first_repr_after_rel.map(|rp| rp < ho.time).unwrap_or(false)
            && rel_after_repr.map(|rt| rt < ho.time).unwrap_or(false)
            && !has_post_head_repr;
        if has_early_rel
            && firs_repr_needs_resc
            && !keep_near_repr
            && !keep_prehead_repr
            && !keep_hit_repr
        {
            let pref_repr_late_limit = (w.hit50 + w.hit100 + w.max).max(w.hit50 * 2 + 1);
            let mut pref_repr_post_rel: Option<i32> = None;
            for (seg_start, seg_end) in &segments {
                if first_early_rel.map(|t| *seg_start <= t).unwrap_or(true) {
                    continue;
                }
                let short_head_prwn_brdg = false
                    && !head_was_hit
                    && press_time.is_some()
                    && ln_duration <= w.hit100
                    && press_time
                        .and_then(|pt| {
                            events
                                .iter()
                                .find(|e| e.time > pt && !e.pressed)
                                .map(|e| e.time)
                        })
                        .map(|rt| rt < tail_start)
                        .unwrap_or(true)
                    && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
                    && first_repr_after_rel
                        .zip(rel_after_repr)
                        .map(|(rp, rr)| rp < ho.time && rr < ho.time)
                        .unwrap_or(false)
                    && *seg_start >= tail_start
                    && *seg_start <= end_time
                    && seg_end
                        .map(|rr| rr >= tail_start && rr < tail_end_exclusive)
                        .unwrap_or(false)
                    && judgments.iter().any(|jj| {
                        jj.index > idx
                            && jj.column == ho.column
                            && jj.press_time == Some(*seg_start)
                            && find_repl_pt(judgments, map, events, jj.index, *seg_start, w)
                                .is_some()
                    });
                let pref_repr_forced_miss =
                    ln_duration <= pref_repr_late_limit && *seg_start > late_repr_guard;
                if pref_repr_forced_miss && !short_head_prwn_brdg {
                    continue;
                }
                let repr_overlaps_next = false
                    && next_same_col_time
                        .map(|next_t| {
                            *seg_start >= next_t - w.hit50 && *seg_start < next_t + w.hit100
                        })
                        .unwrap_or(false);
                if repr_overlaps_next && !short_head_prwn_brdg {
                    continue;
                }
                let rel_in_tail_win = seg_end
                    .map(|t| t >= tail_start && t < tail_end_exclusive)
                    .unwrap_or(false);
                let lat_ope_hol_nea_tail =
                    seg_end.is_none() && *seg_start > end_time && *seg_start - end_time <= w.hit100;
                let open_hold_tail_win =
                    seg_end.is_none() && *seg_start >= ho.time && *seg_start <= end_time;
                let open_hold_keeps_repr = false
                    && head_was_hit
                    && seg_end.is_none()
                    && *seg_start <= end_time
                    && rel_after_repr.map(|rt| rt < tail_start).unwrap_or(false)
                    && first_repr_after_rel
                        .map(|rp| *seg_start > rp)
                        .unwrap_or(false)
                    && events
                        .iter()
                        .find(|e| e.time > *seg_start && !e.pressed)
                        .map(|rt| rt.time >= tail_end_exclusive)
                        .unwrap_or(false);
                if open_hold_keeps_repr {
                    continue;
                }
                if rel_in_tail_win || lat_ope_hol_nea_tail || open_hold_tail_win {
                    pref_repr_post_rel = Some(*seg_start);
                    break;
                }
            }
            let long_post_pref_tail = false
                && head_was_hit
                && strong_head_hit
                && has_early_rel
                && ln_duration > pref_repr_late_limit
                && first_early_rel.map(|t| t > ho.time).unwrap_or(false)
                && first_repr_after_rel
                    .zip(rel_after_repr)
                    .map(|(rp, rr)| rp >= ho.time && rp <= end_time && rr < tail_start)
                    .unwrap_or(false);
            if pref_repr_post_rel.is_none() && firs_repr_needs_resc && (true || long_post_pref_tail)
            {
                if let Some(first_rp) = first_repr_after_rel {
                    let latest_pre_tail_frag =
                        segments.iter().rev().find_map(|(seg_start, seg_end)| {
                            let seg_end_time = (*seg_end)?;
                            if *seg_start <= first_rp
                                || *seg_start > end_time
                                || *seg_start >= tail_start
                                || seg_end_time >= tail_start
                            {
                                return None;
                            }
                            Some((*seg_start, seg_end_time))
                        });
                    if let Some((latest_rp, latest_rr)) = latest_pre_tail_frag {
                        let last_pre_tail_stays = next_same_col_time
                            .map(|next_t| latest_rp < next_t && latest_rr < next_t)
                            .unwrap_or(true);
                        let last_pre_tail_claim = judgments.iter().any(|jj| {
                            jj.index != idx
                                && jj.column == ho.column
                                && jj.press_time == Some(latest_rp)
                        });
                        if true || (!last_pre_tail_claim && last_pre_tail_stays) {
                            pref_repr_post_rel = Some(latest_rp);
                        }
                    }
                }
            }
            if pref_repr_post_rel.is_some() && pref_repr_post_rel != first_repr_after_rel {
                first_repr_after_rel = pref_repr_post_rel;
                if let Some(rp) = first_repr_after_rel {
                    rel_after_repr = next_rel_after_press(events, rp);
                }
            }
        }
        if false
            && !head_was_hit
            && press_time.is_some()
            && ln_duration <= w.hit100
            && press_time
                .and_then(|pt| {
                    events
                        .iter()
                        .find(|e| e.time > pt && !e.pressed)
                        .map(|e| e.time)
                })
                .map(|rt| rt < tail_start)
                .unwrap_or(true)
            && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
            && init_first_repr
                .zip(init_rel_after_repr)
                .map(|(rp, rr)| rp < ho.time && rr < ho.time)
                .unwrap_or(false)
            && first_repr_after_rel == init_first_repr
        {
            let late_tail_with_repl = segments.iter().find_map(|(seg_start, seg_end)| {
                let seg_end_time = (*seg_end)?;
                if *seg_start < tail_start
                    || *seg_start > end_time
                    || seg_end_time < tail_start
                    || seg_end_time >= tail_end_exclusive
                {
                    return None;
                }
                let claimed_by_later_idx = judgments
                    .iter()
                    .find(|jj| {
                        jj.index > idx
                            && jj.column == ho.column
                            && jj.press_time == Some(*seg_start)
                    })
                    .map(|jj| jj.index)?;
                let claim_is_next_short = next_same_col_idx
                    .filter(|next_idx| *next_idx == claimed_by_later_idx)
                    .and_then(|next_idx| {
                        let next_ho = map.hit_objects.get(next_idx)?;
                        if next_ho.column != ho.column || !next_ho.is_long_note() {
                            return None;
                        }
                        let next_end = next_ho.end_time.unwrap_or(next_ho.time);
                        let next_duration = next_end - next_ho.time;
                        if next_duration > w.hit100 {
                            return None;
                        }
                        let next_head_start = next_ho.time - w.hit50;
                        let next_head_win_end = next_ho.time + w.hit100;
                        let next_tail_start = next_end - w.hit50;
                        let next_tail_end = next_end + w.hit100;
                        Some(
                            *seg_start >= next_head_start
                                && *seg_start < next_head_win_end
                                && seg_end_time >= next_tail_start
                                && seg_end_time < next_tail_end,
                        )
                    })
                    .unwrap_or(false);
                if claim_is_next_short {
                    return None;
                }
                let claimed_repl_pt =
                    find_repl_pt(judgments, map, events, claimed_by_later_idx, *seg_start, w)?;
                let next_short_retks_tap = next_same_col_idx
                    .filter(|next_idx| *next_idx == claimed_by_later_idx)
                    .and_then(|next_idx| {
                        let next_ho = map.hit_objects.get(next_idx)?;
                        if next_ho.column != ho.column || next_ho.is_long_note() {
                            return None;
                        }
                        map.hit_objects
                            .iter()
                            .enumerate()
                            .skip(next_idx + 1)
                            .find(|(_, next_next_ho)| next_next_ho.column == ho.column)
                            .map(|(next_next_idx, next_next_ho)| {
                                (next_idx, next_ho, next_next_idx, next_next_ho)
                            })
                    })
                    .map(|(next_idx, next_tap_ho, next_next_idx, next_next_ho)| {
                        if !next_next_ho.is_long_note() {
                            return false;
                        }
                        let next_tap_judgment = judgments
                            .iter()
                            .find(|jj| jj.index == next_idx && jj.column == ho.column);
                        let next_ln_judgment = judgments
                            .iter()
                            .find(|jj| jj.index == next_next_idx && jj.column == ho.column);
                        let next_ln_end_time = next_next_ho.end_time.unwrap_or(next_next_ho.time);
                        let next_ln_duration = next_ln_end_time - next_next_ho.time;
                        let next_ln_window_start = next_next_ho.time - w.hit50;
                        let next_ln_win_end = next_next_ho.time + w.hit100;
                        let next_ln_tail_start = next_ln_end_time - w.hit50;
                        let next_ln_tail_end = next_ln_end_time + w.hit100;
                        let next_ln_press_time = next_ln_judgment.and_then(|jj| jj.press_time);
                        let next_ln_rel_time = next_ln_press_time.and_then(|pt| {
                            events
                                .iter()
                                .find(|ev| !ev.pressed && ev.time > pt)
                                .map(|ev| ev.time)
                        });
                        let next_ln_repl_press = next_ln_press_time.and_then(|pt| {
                            find_repl_pt(judgments, map, events, next_next_idx, pt, w)
                        });
                        let nex_ln_repl_rel_time = next_ln_repl_press.and_then(|pt| {
                            events
                                .iter()
                                .find(|ev| !ev.pressed && ev.time > pt)
                                .map(|ev| ev.time)
                        });
                        next_tap_judgment
                            .map(|jj| {
                                jj.kind == JudgmentKind::Miss
                                    && jj.press_time == Some(*seg_start)
                                    && calc_hit_kind((*seg_start - next_tap_ho.time).abs(), w)
                                        == JudgmentKind::Miss
                            })
                            .unwrap_or(false)
                            && next_ln_duration <= w.hit100
                            && next_ln_judgment
                                .map(|jj| jj.kind != JudgmentKind::Miss)
                                .unwrap_or(false)
                            && claimed_repl_pt == next_ln_press_time.unwrap_or(i32::MIN)
                            && next_ln_press_time
                                .map(|pt| pt >= next_ln_window_start && pt < next_ln_win_end)
                                .unwrap_or(false)
                            && next_ln_rel_time
                                .map(|rt| rt >= next_ln_tail_start && rt < next_ln_tail_end)
                                .unwrap_or(false)
                            && next_ln_repl_press
                                .zip(nex_ln_repl_rel_time)
                                .map(|(pt, rt)| {
                                    pt >= next_ln_window_start
                                        && pt < next_ln_win_end
                                        && rt >= next_ln_tail_start
                                        && rt < next_ln_tail_end
                                })
                                .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if next_short_retks_tap {
                    return None;
                }
                Some((*seg_start, seg_end_time))
            });
            if let Some((late_rp, late_rr)) = late_tail_with_repl {
                first_repr_after_rel = Some(late_rp);
                last_repr_time = Some(late_rp);
                rel_after_repr = Some(late_rr);
            }
        }
        short_miss_bridge = false
            && !head_was_hit
            && ln_duration <= w.hit100
            && press_time
                .and_then(|pt| {
                    events
                        .iter()
                        .find(|e| e.time > pt && !e.pressed)
                        .map(|e| e.time)
                })
                .map(|rt| rt < tail_start)
                .unwrap_or(true)
            && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
            && init_first_repr
                .zip(init_rel_after_repr)
                .map(|(rp, rr)| rp < ho.time && rr < ho.time)
                .unwrap_or(false)
            && first_repr_after_rel
                .zip(rel_after_repr)
                .map(|(rp, rr)| {
                    rp >= tail_start
                        && rp <= end_time
                        && rr >= tail_start
                        && rr < tail_end_exclusive
                })
                .unwrap_or(false)
            && first_repr_after_rel != init_first_repr;
        if short_miss_bridge {
            if let Some(head_pt) = init_first_repr {
                if let Some(pos) = j_by_idx[idx] {
                    if let Some(jj) = judgments.get_mut(pos) {
                        jj.press_time = Some(head_pt);
                        jj.kind = JudgmentKind::Miss;
                    }
                }
            }
        }
        let late_body_lim = (w.hit50 + w.hit100 + w.max).max(w.hit50 * 2 + 1);
        let str_head_tail_cutoff = late_repr_guard - w.hit50 + w.max;
        let next_ln_stronger_pair = |candidate_press_time: i32, candidate_rel_time: i32| {
            if true {
                return false;
            }
            next_same_col_idx
                .and_then(|next_idx| {
                    let next_ho = map.hit_objects.get(next_idx)?;
                    if next_ho.column != ho.column || !next_ho.is_long_note() {
                        return None;
                    }
                    let next_head_start = next_ho.time - w.hit50;
                    let next_head_win_end = next_ho.time + w.hit100;
                    if candidate_press_time < next_head_start
                        || candidate_press_time >= next_ho.time
                        || candidate_rel_time >= next_ho.time
                    {
                        return None;
                    }
                    let next_press_time = judgments
                        .iter()
                        .find(|jj| jj.index == next_idx && jj.column == ho.column)
                        .and_then(|jj| jj.press_time)?;
                    if next_press_time <= candidate_rel_time
                        || next_press_time < next_head_start
                        || next_press_time >= next_head_win_end
                    {
                        return None;
                    }
                    let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                    let next_tail_start =
                        next_end_time - ((w.hit50 as f32) * tail_window_scale).round() as i32;
                    let next_tail_end =
                        next_end_time + ((w.hit100 as f32) * tail_window_scale).round() as i32;
                    let next_rel_time = events
                        .iter()
                        .find(|ev| !ev.pressed && ev.time > next_press_time)
                        .map(|ev| ev.time)?;
                    (next_rel_time >= next_tail_start && next_rel_time < next_tail_end)
                        .then_some(())
                })
                .is_some()
        };
        let long_hit_maybe_bridge = false
            && head_was_hit
            && strong_head_hit
            && has_early_rel
            && ln_duration > late_body_lim
            && first_early_rel
                .map(|t| t <= tail_start - w.hit50)
                .unwrap_or(false)
            && first_repr_after_rel
                .zip(rel_after_repr)
                .map(|(rp, rr)| {
                    rp >= ho.time && rp <= end_time && rr >= tail_start && rr < end_time
                })
                .unwrap_or(false);
        let tail_pref_long_scaled = false
            && head_was_hit
            && strong_head_hit
            && has_early_rel
            && ln_duration > late_body_lim
            && first_early_rel
                .map(|t| t <= str_head_tail_cutoff)
                .unwrap_or(false)
            && first_repr_after_rel
                .zip(rel_after_repr)
                .map(|(rp, rr)| {
                    rp >= ho.time && rp <= end_time && rr >= tail_start && rr < late_repr_guard
                })
                .unwrap_or(false);
        let tail_pref_long_expnd = false
            && head_was_hit
            && strong_head_hit
            && has_early_rel
            && ln_duration > late_body_lim
            && first_early_rel
                .map(|t| t > str_head_tail_cutoff && t < tail_start)
                .unwrap_or(false)
            && first_repr_after_rel
                .zip(rel_after_repr)
                .map(|(rp, rr)| {
                    rp >= tail_start && rp <= end_time && rr >= tail_start && rr < late_repr_guard
                })
                .unwrap_or(false);
        let tail_pref_early_maybe = false
            && head_was_hit
            && head_is_h100
            && press_time.map(|pt| pt < ho.time).unwrap_or(false)
            && has_early_rel
            && ln_duration > late_body_lim
            && first_early_rel
                .map(|t| t <= tail_start - w.hit50)
                .unwrap_or(false)
            && first_repr_after_rel
                .zip(rel_after_repr)
                .map(|(rp, rr)| {
                    rp >= ho.time && rp <= end_time && rr >= tail_start && rr < end_time
                })
                .unwrap_or(false)
            && last_repr_time
                .zip(first_repr_after_rel)
                .map(|(last_rp, first_rp)| last_rp > first_rp && last_rp <= end_time)
                .unwrap_or(false);
        let tail_pref_frag = false
            && head_was_hit
            && head_is_h50
            && press_time.map(|pt| pt < ho.time).unwrap_or(false)
            && has_early_rel
            && ln_duration <= late_body_lim
            && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
            && first_repr_after_rel
                .zip(rel_after_repr)
                .map(|(rp, rr)| {
                    rp >= ho.time && rp <= ho.time + w.hit50 && rr >= tail_start && rr < end_time
                })
                .unwrap_or(false)
            && segments.iter().any(|(seg_start, seg_end)| {
                seg_end
                    .map(|seg_end_time| {
                        let seg_stays_pre_next = next_same_col_time
                            .map(|next_t| *seg_start < next_t - w.hit50 && seg_end_time < next_t)
                            .unwrap_or(true);
                        let h50_end_seg_prom = seg_end_time == end_time
                            && (seg_stays_pre_next
                                || next_ln_stronger_pair(*seg_start, seg_end_time));
                        first_repr_after_rel
                            .map(|rp| *seg_start > rp)
                            .unwrap_or(false)
                            && *seg_start <= end_time
                            && seg_end_time > rel_after_repr.unwrap_or(i32::MIN)
                            && ((seg_end_time < end_time && seg_stays_pre_next) || h50_end_seg_prom)
                    })
                    .unwrap_or(false)
            });
        let tail_pref_exact_maybe = false
            && head_was_hit
            && head_is_h50
            && press_time.map(|pt| pt < ho.time).unwrap_or(false)
            && has_early_rel
            && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
            && first_repr_after_rel
                .zip(rel_after_repr)
                .map(|(rp, rr)| {
                    rp >= ho.time && rp <= ho.time + w.hit50 && rr >= tail_start && rr < end_time
                })
                .unwrap_or(false)
            && segments.iter().any(|(seg_start, seg_end)| {
                seg_end
                    .map(|seg_end_time| {
                        let seg_stays_pre_next = next_same_col_time
                            .map(|next_t| *seg_start < next_t - w.hit50 && seg_end_time < next_t)
                            .unwrap_or(true);
                        first_repr_after_rel
                            .map(|rp| *seg_start > rp)
                            .unwrap_or(false)
                            && *seg_start <= end_time
                            && seg_end_time > rel_after_repr.unwrap_or(i32::MIN)
                            && seg_end_time == end_time
                            && (seg_stays_pre_next
                                || next_ln_stronger_pair(*seg_start, seg_end_time))
                    })
                    .unwrap_or(false)
            });
        let prehead_hit_bridge = false
            && head_was_hit
            && !strong_head_hit
            && press_time.map(|pt| pt < ho.time).unwrap_or(false)
            && has_early_rel
            && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
            && first_repr_after_rel
                .zip(rel_after_repr)
                .map(|(rp, rr)| rp < ho.time && rr < late_repr_guard && rr >= tail_start)
                .unwrap_or(false)
            && segments.iter().any(|(seg_start, seg_end)| {
                seg_end
                    .map(|seg_end_time| {
                        *seg_start > ho.time
                            && *seg_start >= late_repr_guard - w.max
                            && *seg_start <= late_repr_guard
                            && seg_end_time > end_time - w.hit50
                            && seg_end_time <= end_time
                    })
                    .unwrap_or(false)
            });
        if prehead_hit_bridge {
            let mut pref_bound_tail: Option<(i32, i32)> = None;
            for (seg_start, seg_end) in &segments {
                let Some(seg_end_time) = *seg_end else {
                    continue;
                };
                if *seg_start <= ho.time
                    || *seg_start < late_repr_guard - w.max
                    || *seg_start > late_repr_guard
                    || seg_end_time <= end_time - w.hit50
                    || seg_end_time > end_time
                {
                    continue;
                }
                let claimed_by_other_note = judgments.iter().any(|jj| {
                    jj.index != idx && jj.column == ho.column && jj.press_time == Some(*seg_start)
                });
                let steals_imm_ln = steals_next_ln_head(
                    judgments,
                    map,
                    ho.column,
                    next_same_col_idx,
                    *seg_start,
                    seg_end_time,
                    w,
                    tail_window_scale,
                );
                if claimed_by_other_note || steals_imm_ln {
                    continue;
                }
                pref_bound_tail = Some((*seg_start, seg_end_time));
            }
            if let Some((preferred_rp, preferred_rr)) = pref_bound_tail {
                first_repr_after_rel = Some(preferred_rp);
                rel_after_repr = Some(preferred_rr);
                tail_pref_bridge = true;
            }
        }
        if long_hit_maybe_bridge
            || tail_pref_long_scaled
            || tail_pref_long_expnd
            || tail_pref_early_maybe
            || tail_pref_frag
            || tail_pref_exact_maybe
        {
            let mut pref_late_body_pair: Option<(i32, i32)> = None;
            let mut pref_late_body_exact = false;
            let fir_tai_rel_pos_repr = rel_after_repr.unwrap_or(i32::MIN);
            for (seg_start, seg_end) in &segments {
                let Some(seg_end_time) = *seg_end else {
                    continue;
                };
                if first_repr_after_rel
                    .map(|rp| *seg_start <= rp)
                    .unwrap_or(true)
                {
                    continue;
                }
                if *seg_start > end_time
                    || seg_end_time <= fir_tai_rel_pos_repr
                    || seg_end_time > tail_end_exclusive
                    || (seg_end_time == tail_end_exclusive && !tail_pref_early_maybe)
                {
                    continue;
                }
                if (tail_pref_long_scaled || tail_pref_long_expnd) && seg_end_time < late_repr_guard
                {
                    continue;
                }
                if tail_pref_early_maybe || tail_pref_frag || tail_pref_exact_maybe {
                    let seg_stays_pre_next = next_same_col_time
                        .map(|next_t| *seg_start < next_t - w.hit50 && seg_end_time < next_t)
                        .unwrap_or(true);
                    let h50_end_seg_prom = (tail_pref_frag || tail_pref_exact_maybe)
                        && seg_end_time == end_time
                        && (seg_stays_pre_next || next_ln_stronger_pair(*seg_start, seg_end_time));
                    if !seg_stays_pre_next && !h50_end_seg_prom {
                        continue;
                    }
                }
                let claimed_by_other_note = judgments.iter().any(|jj| {
                    jj.index != idx && jj.column == ho.column && jj.press_time == Some(*seg_start)
                });
                let claimed_by_next_ln = next_same_col_idx
                    .and_then(|next_idx| {
                        map.hit_objects
                            .get(next_idx)
                            .map(|next_ho| (next_idx, next_ho))
                    })
                    .filter(|(_, next_ho)| next_ho.is_long_note())
                    .map(|(next_idx, _)| {
                        judgments.iter().any(|jj| {
                            jj.index == next_idx
                                && jj.column == ho.column
                                && jj.press_time == Some(*seg_start)
                        })
                    })
                    .unwrap_or(false);
                let clmd_by_non_imm_note = claimed_by_other_note && !claimed_by_next_ln;
                let steals_imm_ln = steals_next_ln_head(
                    judgments,
                    map,
                    ho.column,
                    next_same_col_idx,
                    *seg_start,
                    seg_end_time,
                    w,
                    tail_window_scale,
                );
                let rec_imm_next_ln_clai = false
                    && claimed_by_next_ln
                    && next_same_col_idx
                        .and_then(|next_idx| {
                            let next_judgment = judgments.iter().find(|jj| {
                                jj.index == next_idx
                                    && jj.column == ho.column
                                    && jj.press_time == Some(*seg_start)
                            })?;
                            let next_ho = map.hit_objects.get(next_idx)?;
                            if !next_ho.is_long_note() || next_judgment.kind != JudgmentKind::Miss {
                                return None;
                            }
                            find_repl_pt(judgments, map, events, next_idx, *seg_start, w)
                        })
                        .is_some();
                if clmd_by_non_imm_note
                    || ((claimed_by_other_note || steals_imm_ln) && !rec_imm_next_ln_clai)
                {
                    continue;
                }
                pref_late_body_pair = Some((*seg_start, seg_end_time));
                pref_late_body_exact = (tail_pref_frag || tail_pref_exact_maybe)
                    && seg_end_time == end_time
                    && (next_same_col_time
                        .map(|next_t| *seg_start < next_t - w.hit50 && seg_end_time < next_t)
                        .unwrap_or(true)
                        || next_ln_stronger_pair(*seg_start, seg_end_time));
            }
            if let Some((preferred_rp, preferred_rr)) = pref_late_body_pair {
                first_repr_after_rel = Some(preferred_rp);
                rel_after_repr = Some(preferred_rr);
                if tail_pref_early_maybe {
                    tail_pref_early = true;
                } else if tail_pref_frag || tail_pref_exact_maybe {
                    tail_pref_pre_frag = true;
                    tail_pref_exact = pref_late_body_exact;
                } else {
                    tail_pref_body = true;
                }
            }
        }
        let prehead_miss_bridge = false
            && !head_was_hit
            && has_early_rel
            && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
            && first_repr_after_rel
                .zip(rel_after_repr)
                .map(|(rp, rr)| {
                    rp >= ho.time - w.hit300
                        && rp <= end_time
                        && rr >= tail_start
                        && rr <= tail_start + w.hit300
                        && rr < end_time
                })
                .unwrap_or(false)
            && last_repr_time
                .zip(first_repr_after_rel)
                .map(|(last_rp, first_rp)| last_rp > first_rp && last_rp <= end_time)
                .unwrap_or(false);
        if prehead_miss_bridge {
            let mut pref_late_body_pair: Option<(i32, i32)> = None;
            for (seg_start, seg_end) in &segments {
                let Some(seg_end_time) = *seg_end else {
                    continue;
                };
                if first_repr_after_rel
                    .map(|rp| *seg_start <= rp)
                    .unwrap_or(true)
                {
                    continue;
                }
                if *seg_start > end_time
                    || seg_end_time <= end_time
                    || seg_end_time >= tail_end_exclusive
                {
                    continue;
                }
                let claimed_by_other_note = judgments.iter().any(|jj| {
                    jj.index != idx && jj.column == ho.column && jj.press_time == Some(*seg_start)
                });
                let steals_imm_ln = steals_next_ln_head(
                    judgments,
                    map,
                    ho.column,
                    next_same_col_idx,
                    *seg_start,
                    seg_end_time,
                    w,
                    tail_window_scale,
                );
                if claimed_by_other_note || steals_imm_ln {
                    continue;
                }
                pref_late_body_pair = Some((*seg_start, seg_end_time));
            }
            if let Some((preferred_rp, preferred_rr)) = pref_late_body_pair {
                first_repr_after_rel = Some(preferred_rp);
                rel_after_repr = Some(preferred_rr);
            }
        }
    } else {
        init_rel_after_repr = None;
    }
    state.segs.list = segments;
    state.early.has_rel = has_early_rel;
    state.early.first_rel = first_early_rel;
    state.early.repr_after = repr_after_rel;
    state.early.first_repr = first_repr_after_rel;
    state.early.last_repr = last_repr_time;
    state.early.first_free_repr = first_free_repr;
    state.early.rel_after_repr = rel_after_repr;
    state.rescue.near_end_rel = rescue_rel_near_end;
    state.early.last_repr_free = last_repr_free;
    state.rescue.imm_rel_at_press = imm_rel_at_press;
    state.rescue.late_headless = late_headless_rescue;
    state.prefs.body = tail_pref_body;
    state.prefs.bridge = tail_pref_bridge;
    state.prefs.early = tail_pref_early;
    state.prefs.pre_frag = tail_pref_pre_frag;
    state.prefs.exact = tail_pref_exact;
    state.rescue.init_first_repr = init_first_repr;
    state.rescue.init_rel_after_repr = init_rel_after_repr;
    state.rescue.short_miss_bridge = short_miss_bridge;
}
