use super::super::claims::{find_repl_pt, reclaim_pt_conflict, reconcile_tail_rescue};
use super::note::{ReleaseNoteCtx, ReleaseState};
use super::support::{calc_rel_kind, next_ln_keeps};
use crate::modes::mania::judgment::{
    calc_hit_kind, seg_hits_win, steals_next_ln_head, steals_next_tap_head, InternalJudgment,
    LnDebugInfo, LnReleaseInfo, ReleaseKind,
};
use crate::types::{Beatmap, JudgmentKind, Windows};
use std::collections::HashMap;
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve(
    ctx: &ReleaseNoteCtx<'_>,
    state: &mut ReleaseState,
    map: &Beatmap,
    judgments: &mut [InternalJudgment],
    w: &Windows,
    j_by_idx: &[Option<usize>],
    _ln_debug_info: &HashMap<usize, LnDebugInfo>,
    ln_release_info: &HashMap<usize, LnReleaseInfo>,
    metadata_clears: &mut Vec<(usize, i32)>,
) -> bool {
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
    let tail_only_pt = ctx.tail_only_pt;
    let tail_eval_press_time = ctx.tail_eval_press_time;
    let deep_ln_pen = ctx.deep_ln_pen;
    let head_was_hit = ctx.head_was_hit;
    let head_is_h100 = ctx.head_is_h100;
    let head_is_h50 = ctx.head_is_h50;
    let strong_head_hit = ctx.strong_head_hit;
    let post_end_hless = ctx.post_end_hless;
    let prev_same_col_idx = ctx.prev_same_col_idx;
    let _prev_same_col_ho = ctx.prev_same_col_ho;
    let _prev_same_col_is_ln = ctx.prev_same_col_is_ln;
    let _prev_same_col_time = ctx.prev_same_col_time;
    let _prev_same_col_end_time = ctx.prev_same_end;
    let next_same_col_idx = ctx.next_same_col_idx;
    let next_same_col_time = ctx.next_same_col_time;
    let events = ctx.events;
    let segments = state.segs.list.as_slice();
    let mut has_early_rel = state.early.has_rel;
    let mut first_early_rel = state.early.first_rel;
    let mut repr_after_rel = state.early.repr_after;
    let mut first_repr_after_rel = state.early.first_repr;
    let mut last_repr_time = state.early.last_repr;
    let mut first_free_repr = state.early.first_free_repr;
    let mut rel_after_repr = state.early.rel_after_repr;
    let mut rescue_rel_near_end = state.rescue.near_end_rel;
    let mut last_repr_free = state.early.last_repr_free;
    let imm_rel_at_press = state.rescue.imm_rel_at_press;
    let _pre_late_headless = state.rescue.late_headless;
    let mut late_headless_rescue = state.rescue.late_headless;
    let _tail_pref_body = state.prefs.body;
    let _tail_pref_bridge = state.prefs.bridge;
    let _tail_pref_early = state.prefs.early;
    let _tail_pref_pre_frag = state.prefs.pre_frag;
    let tail_pref_exact = state.prefs.exact;
    let init_first_repr = state.rescue.init_first_repr;
    let _init_rel_after_repr = state.rescue.init_rel_after_repr;
    let short_miss_bridge = state.rescue.short_miss_bridge;
    let first_rel_after_press = state.rescue.first_rel_after_press;
    let _before_rel_kind = state.pick.kind;
    let mut rel_kind = state.pick.kind;
    let _before_rel_time = state.pick.time;
    let mut rel_time = state.pick.time;
    let mut _head_h50_caps_h50 = false;
    let mut _pre_tail_break_keep_rel = false;
    let mut _zero_head_to_ln = false;
    let mut _same_ms_keep_rel = false;
    let mut _same_ms_pref_rel = false;
    let mut _tail_exact_next_ln = false;
    let mut _prehead_caps_h50 = false;
    let mut _rescue_steals_tap = false;
    let mut end_diff = state.pick.diff;
    let mut force_kind = state.pick.force;
    let _before_repr_hit_tail = state.early.hit_tail;
    let mut repr_hit_tail = state.early.hit_tail;
    let late_repr_dur = state.rescue.late_repr_dur;
    let tail_hold_hit = state.rescue.tail_hold_hit;
    let miss_press_rel_tail = state.rescue.miss_press_tail;
    let miss_repr_tail_any = state.rescue.miss_repr_tail;
    let short_body_miss = state.rescue.short_body_miss;
    let pre_frag_keep_rel = state.rescue.pre_frag_keep_rel;
    let _late_body_claim = state.rescue.late_body_claim;
    let _before_alt_head_press_time = state.rescue.alt_head_pt;
    let mut alt_head_press_time: Option<i32> = None;
    let mut alt_head_prehold = false;
    let mut alt_head_cross_hold = false;
    if !head_was_hit && press_time.is_some() && !pre_frag_keep_rel {
        let sho_ln_repr_cros_tap = if true
            && ln_duration <= w.hit100
            && has_early_rel
            && first_repr_after_rel
                .zip(rel_after_repr)
                .map(|(rp, rr)| {
                    rp > ho.time + w.max
                        && rp <= end_time
                        && rr > end_time
                        && rr <= end_time + w.hit50 + w.hit100
                })
                .unwrap_or(false)
        {
            map.hit_objects[(idx + 1)..]
                .iter()
                .enumerate()
                .find(|(_, next_ho)| next_ho.column == ho.column)
                .map(|(offset, next_ho)| {
                    if next_ho.is_long_note() {
                        return false;
                    }
                    let next_idx = idx + 1 + offset;
                    let next_press_time = judgments
                        .iter()
                        .find(|jj| jj.index == next_idx)
                        .and_then(|jj| jj.press_time);
                    let next_tap_window_start = next_ho.time - w.hit50;
                    let early_penalty_window = (w.max + 4).max(w.hit300.min(39));
                    first_repr_after_rel
                        .map(|rp| {
                            rp >= next_tap_window_start - early_penalty_window - 1
                                && rp < next_tap_window_start
                        })
                        .unwrap_or(false)
                        && next_press_time.is_none()
                })
                .unwrap_or(false)
        } else {
            false
        };
        let head_window_start = ho.time - w.hit50;
        let head_win_end = ho.time + w.hit50;
        let mut alternate_candidate = if short_miss_bridge {
            init_first_repr
        } else if sho_ln_repr_cros_tap {
            None
        } else {
            first_repr_after_rel
                .filter(|t| *t >= head_window_start && *t < head_win_end && *t <= end_time)
        };
        if alternate_candidate.is_none() && !sho_ln_repr_cros_tap {
            let late_body_tail_rec = first_repr_after_rel
                .map(|rp| {
                    let near_tail_repr_long = false
                        && ln_duration > w.hit50 + w.hit100 + w.max
                        && rp > tail_start
                        && rp <= tail_start + w.max;
                    let rep_hol_thr_tail_win = false
                        && rp <= tail_start
                        && rel_after_repr
                            .map(|rt| rt >= tail_end_exclusive)
                            .unwrap_or(true);
                    rp > head_win_end
                        && (rp <= tail_start || near_tail_repr_long)
                        && ln_duration >= w.hit50 * 2
                        && (rel_after_repr
                            .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
                            .unwrap_or(false)
                            || rep_hol_thr_tail_win)
                })
                .unwrap_or(false);
            if late_body_tail_rec {
                alternate_candidate = first_repr_after_rel;
            }
        }
        if alternate_candidate.is_none() {
            let hit100_window_start = ho.time - w.hit100;
            if let Some(pt) = press_time {
                let pre_head_release = events
                    .iter()
                    .find(|e| e.time > pt && !e.pressed)
                    .map(|e| e.time)
                    .filter(|t| *t >= hit100_window_start && *t < ho.time);
                if pt < head_window_start && pre_head_release.is_some() {
                    alternate_candidate = Some(pt);
                    alt_head_prehold = true;
                }
            }
        }
        if alternate_candidate.is_none() {
            if let Some(pt) = press_time {
                let cros_head_over_limit = if ln_duration <= w.hit100 { w.max } else { 4 };
                let release_crossing_head = events
                    .iter()
                    .find(|e| e.time > pt && !e.pressed)
                    .map(|e| e.time)
                    .filter(|t| *t >= ho.time && *t <= tail_end_exclusive);
                let near_prewin_over =
                    pt < head_window_start && head_window_start - pt <= cros_head_over_limit;
                let short_over_rel = pt < head_window_start
                    && ln_duration <= w.hit100 + w.max
                    && head_window_start - pt <= w.hit200
                    && release_crossing_head
                        .map(|t| t <= ho.time + w.hit300)
                        .unwrap_or(false);
                if (near_prewin_over && release_crossing_head.is_some()) || short_over_rel {
                    alternate_candidate = Some(pt);
                    alt_head_cross_hold = true;
                }
            }
        }
        if alternate_candidate.is_none() {
            if let Some(pt) = press_time {
                let first_rel_after_press = events
                    .iter()
                    .find(|e| e.time > pt && !e.pressed)
                    .map(|e| e.time);
                let lat_nea_tai_sta_limi = if false { w.hit100 } else { w.max };
                let late_near_tail_start =
                    pt >= tail_start && pt <= tail_start + lat_nea_tai_sta_limi;
                let late_hless_tail = pt >= head_win_end
                    && (pt < tail_start || (late_near_tail_start && ln_duration >= w.hit50 * 2))
                    && ln_duration >= w.hit50 * 2
                    && first_rel_after_press
                        .map(|t| t >= tail_start && t < tail_end_exclusive)
                        .unwrap_or(false);
                if late_hless_tail {
                    alternate_candidate = Some(pt);
                }
            }
        }
        if let Some(candidate) = alternate_candidate {
            let already_assigned = judgments.iter().any(|jj| {
                jj.index != idx && jj.column == ho.column && jj.press_time == Some(candidate)
            });
            if !already_assigned {
                alt_head_press_time = Some(candidate);
                let short_pre_tail_no_rec = false
                    && ln_duration < w.hit50 * 2
                    && has_early_rel
                    && first_repr_after_rel
                        .map(|rp| rp == candidate && rp <= tail_start)
                        .unwrap_or(false)
                    && rel_after_repr.map(|rt| rt < tail_start).unwrap_or(false);
                if !short_pre_tail_no_rec {
                    for (seg_start, seg_end) in segments {
                        if *seg_start > end_time && !alt_head_prehold {
                            continue;
                        }
                        if !seg_hits_win(*seg_start, *seg_end, tail_start, tail_end_exclusive) {
                            continue;
                        }
                        if *seg_start < candidate {
                            let seg_end_time = seg_end.unwrap_or(i32::MAX);
                            let crosses_candidate = seg_end_time >= candidate;
                            if !(alt_head_cross_hold && crosses_candidate) {
                                continue;
                            }
                        }
                        if miss_repr_tail_any {
                            let seg_before_first_repr = first_repr_after_rel
                                .map(|rp| {
                                    *seg_start < rp && seg_end.map(|rt| rt <= rp).unwrap_or(false)
                                })
                                .unwrap_or(false);
                            let pre_tap = true
                                && seg_before_first_repr
                                && !head_was_hit
                                && press_time.map(|pt| pt < ho.time).unwrap_or(false)
                                && ln_duration <= w.hit100
                                && seg_end
                                    .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
                                    .unwrap_or(false)
                                && next_same_col_idx
                                    .and_then(|ni| map.hit_objects.get(ni))
                                    .map(|n| {
                                        if n.is_long_note() {
                                            return false;
                                        }
                                        let ns = n.time - w.hit50;
                                        let ne = n.time + w.hit100;
                                        first_repr_after_rel
                                            .map(|rp| {
                                                rp >= ns
                                                    && rp < ne
                                                    && rp <= n.time + w.max
                                                    && matches!(
                                                        calc_hit_kind((rp - n.time).abs(), w),
                                                        JudgmentKind::Max | JudgmentKind::Hit300
                                                    )
                                            })
                                            .unwrap_or(false)
                                    })
                                    .unwrap_or(false);
                            if seg_before_first_repr && !pre_tap {
                                continue;
                            }
                        }
                        let segment_open = seg_end.is_none();
                        let raw_release = seg_end.unwrap_or(tail_end_exclusive - 1);
                        let raw_rel_over = first_rel_after_press.unwrap_or(raw_release);
                        let open_seg_late_over = segment_open
                            && first_rel_after_press
                                .map(|rt| rt >= tail_end_exclusive)
                                .unwrap_or(false);
                        let sho_ln_late_rel_miss = true
                            && ln_duration <= w.hit100
                            && !has_early_rel
                            && (raw_release >= tail_end_exclusive || open_seg_late_over);
                        let open_far_tail_miss = true
                            && !has_early_rel
                            && segment_open
                            && open_seg_late_over
                            && first_rel_after_press
                                .map(|rt| rt - tail_end_exclusive >= w.max)
                                .unwrap_or(false);
                        if sho_ln_late_rel_miss {
                            continue;
                        }
                        if open_far_tail_miss {
                            continue;
                        }
                        let late_rel_auto_h50 = false
                            && !has_early_rel
                            && head_is_h100
                            && ln_duration > w.hit100
                            && (raw_rel_over > end_time + w.hit50
                                || raw_rel_over >= tail_end_exclusive);
                        let effective_release = if segment_open || raw_release >= tail_end_exclusive
                        {
                            force_kind = true;
                            if late_rel_auto_h50 {
                                end_time + w.hit50
                            } else {
                                tail_end_exclusive - 1
                            }
                        } else {
                            raw_release
                        };
                        let first_repress_segment = first_repr_after_rel
                            .map(|rp| *seg_start == rp)
                            .unwrap_or(false);
                        let er_mis = first_early_rel
                            .map(|rt| {
                                calc_rel_kind((rt - end_time).abs(), w, tail_window_scale)
                                    == ReleaseKind::Miss
                            })
                            .unwrap_or(false);
                        let repr_tap = if true
                            && er_mis
                            && !head_was_hit
                            && press_time.map(|pt| pt < ho.time).unwrap_or(false)
                            && first_early_rel.map(|t| *seg_start > t).unwrap_or(false)
                            && first_repress_segment
                        {
                            next_same_col_idx
                                .and_then(|ni| map.hit_objects.get(ni))
                                .map(|n| {
                                    if n.is_long_note() {
                                        return false;
                                    }
                                    let ns = n.time - w.hit50;
                                    let ne = n.time + w.hit100;
                                    let k = calc_hit_kind((*seg_start - n.time).abs(), w);
                                    *seg_start >= ns
                                        && *seg_start < ne
                                        && !matches!(k, JudgmentKind::Miss)
                                        && rel_after_repr.map(|rr| rr > n.time).unwrap_or(false)
                                })
                                .unwrap_or(false)
                        } else {
                            false
                        };
                        if repr_tap {
                            rel_time = first_early_rel;
                            end_diff = first_early_rel.map(|rt| (rt - end_time).abs()).unwrap_or(0);
                            rel_kind = ReleaseKind::Miss;
                            force_kind = false;
                            repr_hit_tail = false;
                            rescue_rel_near_end = None;
                            break;
                        }
                        let alt_hea_yild_next_ln = true
                            && first_early_rel.map(|t| *seg_start > t).unwrap_or(false)
                            && first_repress_segment
                            && next_ln_keeps(
                                judgments,
                                map,
                                events,
                                ho.column,
                                next_same_col_idx,
                                *seg_start,
                                effective_release,
                                w,
                                tail_window_scale,
                            );
                        let alt_head_keeps_tail = alt_hea_yild_next_ln
                            && next_same_col_idx
                                .and_then(|next_idx| {
                                    map.hit_objects
                                        .get(next_idx)
                                        .map(|next_ho| (next_idx, next_ho))
                                })
                                .map(|(_, next_ho)| {
                                    let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                                    let next_duration = next_end_time - next_ho.time;
                                    let next_head_kind =
                                        calc_hit_kind((*seg_start - next_ho.time).abs(), w);
                                    next_ho.is_long_note()
                                        && next_duration <= w.hit50 + w.max
                                        && *seg_start >= next_ho.time - w.hit100
                                        && *seg_start < next_ho.time
                                        && matches!(
                                            next_head_kind,
                                            JudgmentKind::Hit200
                                                | JudgmentKind::Hit100
                                                | JudgmentKind::Hit50
                                        )
                                })
                                .unwrap_or(false);
                        if alt_hea_yild_next_ln {
                            if alt_head_keeps_tail {
                                rel_time = first_early_rel;
                                end_diff =
                                    first_early_rel.map(|rt| (rt - end_time).abs()).unwrap_or(0);
                                let yildd_first_rel_kind =
                                    calc_rel_kind(end_diff, w, tail_window_scale);
                                rel_kind = if matches!(
                                    yildd_first_rel_kind,
                                    ReleaseKind::Miss | ReleaseKind::None
                                ) {
                                    yildd_first_rel_kind
                                } else {
                                    ReleaseKind::Hit50
                                };
                                force_kind = false;
                                break;
                            }
                            continue;
                        }
                        rel_time = Some(effective_release);
                        end_diff = (effective_release - end_time).abs();
                        rel_kind = calc_rel_kind(end_diff, w, tail_window_scale);
                        break;
                    }
                }
            }
        }
    }
    let prev_same_left = prev_same_col_idx
        .and_then(|prev_idx| ln_release_info.get(&prev_idx))
        .and_then(|info| info.time)
        .map(|rt| rt <= ho.time)
        .unwrap_or(true);
    let cle_lat_hle_can_serc = true
        && !head_was_hit
        && press_time.is_none()
        && tail_only_pt.is_none()
        && alt_head_press_time.is_none()
        && !has_early_rel
        && matches!(rel_kind, ReleaseKind::Miss | ReleaseKind::None)
        && prev_same_left;
    if (false || cle_lat_hle_can_serc)
        && !head_was_hit
        && press_time.is_none()
        && alt_head_press_time.is_none()
        && matches!(rel_kind, ReleaseKind::Miss | ReleaseKind::None)
    {
        let next_same_col_time = map.hit_objects[(idx + 1)..]
            .iter()
            .find(|next_ho| next_ho.column == ho.column)
            .map(|next_ho| next_ho.time);
        let late_hless_cand = events
            .iter()
            .find(|ev| {
                ev.pressed
                    && ev.time > ho.time + w.hit50
                    && ev.time >= tail_start
                    && ev.time <= end_time + w.hit100
                    && (next_same_col_time
                        .map(|next_t| ev.time < next_t - (w.hit50 + w.hit300))
                        .unwrap_or(true)
                        || judgments.iter().any(|jj| {
                            if jj.index <= idx
                                || jj.column != ho.column
                                || jj.press_time != Some(ev.time)
                            {
                                return false;
                            }
                            let Some(next_ho) = map.hit_objects.get(jj.index) else {
                                return false;
                            };
                            let next_end = next_ho.end_time.unwrap_or(next_ho.time);
                            let next_duration = next_end - next_ho.time;
                            let is_next_same_col = next_same_col_time
                                .map(|t| next_ho.time == t)
                                .unwrap_or(false);
                            next_ho.is_long_note()
                                && is_next_same_col
                                && next_duration <= w.hit100
                                && ev.time > end_time
                                && ev.time >= next_ho.time - w.hit50
                                && ev.time < next_ho.time - w.hit100
                        }))
            })
            .map(|ev| ev.time);
        if let Some(candidate) = late_hless_cand {
            let taken_by_earlier_note = judgments.iter().any(|jj| {
                jj.index < idx && jj.column == ho.column && jj.press_time == Some(candidate)
            });
            if taken_by_earlier_note {
                return true;
            }
            let first_rel_after_cand = events
                .iter()
                .find(|ev| ev.time > candidate && !ev.pressed)
                .map(|ev| ev.time);
            let _same_time_tail_win_rel_at_cand = false
                && events.iter().any(|ev| !ev.pressed && ev.time == candidate)
                && candidate >= tail_start
                && candidate < tail_end_exclusive;
            let same_time_tail_rel = true
                && events.iter().any(|ev| !ev.pressed && ev.time == candidate)
                && candidate >= tail_start
                && candidate < tail_end_exclusive;
            let first_rel_at_cand = if same_time_tail_rel {
                Some(candidate)
            } else {
                first_rel_after_cand
            };
            let cand_next_short_pair = true
                && first_rel_at_cand
                    .and_then(|next_rt| {
                        next_same_col_idx
                            .and_then(|next_idx| map.hit_objects.get(next_idx))
                            .map(|next_ho| {
                                let next_end = next_ho.end_time.unwrap_or(next_ho.time);
                                let next_duration = next_end - next_ho.time;
                                let next_window_start = next_ho.time - w.hit50;
                                let next_tail_start = next_end - w.hit50;
                                let next_tail_end = next_end + w.hit100;
                                next_ho.is_long_note()
                                    && next_duration <= w.hit100
                                    && candidate >= next_window_start
                                    && candidate < next_ho.time
                                    && next_rt >= next_tail_start
                                    && next_rt < next_tail_end
                            })
                    })
                    .unwrap_or(false);
            let taken_by_later_note = judgments.iter().any(|jj| {
                jj.index > idx && jj.column == ho.column && jj.press_time == Some(candidate)
            });
            if taken_by_later_note {
                if cand_next_short_pair {
                    return true;
                }
                let confl_short_pre_h100 = judgments.iter().any(|jj| {
                    if jj.index <= idx || jj.column != ho.column || jj.press_time != Some(candidate)
                    {
                        return false;
                    }
                    let Some(next_ho) = map.hit_objects.get(jj.index) else {
                        return false;
                    };
                    let next_end = next_ho.end_time.unwrap_or(next_ho.time);
                    let next_duration = next_end - next_ho.time;
                    let is_next_same_col = next_same_col_time
                        .map(|t| next_ho.time == t)
                        .unwrap_or(false);
                    next_ho.is_long_note()
                        && is_next_same_col
                        && next_duration <= w.hit100
                        && candidate > end_time
                        && candidate >= next_ho.time - w.hit50
                        && candidate < next_ho.time - w.hit100
                });
                if !confl_short_pre_h100 {
                    return true;
                }
                let conflict = reconcile_tail_rescue(
                    judgments,
                    map,
                    events,
                    idx,
                    ho.column,
                    candidate,
                    ln_duration,
                    tail_start,
                    end_time,
                    w,
                );
                if !conflict.allowed {
                    return true;
                }
            }
            let late_hless_prev_rel = events
                .iter()
                .rev()
                .find(|ev| ev.pressed && ev.time < candidate && ev.time > ho.time + w.hit50)
                .and_then(|prior_press| {
                    events
                        .iter()
                        .find(|ev| !ev.pressed && ev.time > prior_press.time && ev.time < candidate)
                        .map(|ev| ev.time)
                });
            let has_tail_rel_pre = events
                .iter()
                .any(|ev| !ev.pressed && ev.time >= tail_start && ev.time < candidate);
            let candidate_release = first_rel_at_cand.filter(|rt| {
                let short_clean_post = true
                    && ln_duration <= w.hit100
                    && candidate > end_time
                    && *rt > end_time
                    && *rt < tail_end_exclusive;
                (*rt >= end_time - w.max && *rt < tail_end_exclusive && *rt <= end_time + w.hit300)
                    || same_time_tail_rel
                    || same_time_tail_rel
                    || short_clean_post
            });
            if cand_next_short_pair {
                return true;
            }
            if let Some(rt) = candidate_release {
                let short_post_headless = true
                    && ln_duration <= w.hit100
                    && candidate > end_time
                    && next_same_col_time
                        .map(|next_t| rt < next_t - w.hit50)
                        .unwrap_or(true);
                if false && ln_duration < w.hit50 * 2 + w.max {
                    return true;
                }
                if true && !(ln_duration >= w.hit50 * 2 + w.max || short_post_headless) {
                    return true;
                }
                alt_head_press_time = Some(candidate);
                rel_time = Some(rt);
                end_diff = (rt - end_time).abs();
                rel_kind = calc_rel_kind(end_diff, w, tail_window_scale);
                if let Some(prior_release) = late_hless_prev_rel {
                    late_headless_rescue = true;
                    has_early_rel = true;
                    repr_after_rel = true;
                    first_early_rel = Some(prior_release);
                    first_repr_after_rel = Some(candidate);
                    last_repr_time = Some(candidate);
                    rel_after_repr = Some(rt);
                }
            } else {
                let open_late_no_tail = candidate > end_time
                    && ln_duration >= w.hit50 * 2
                    && !has_tail_rel_pre
                    && first_rel_after_cand
                        .map(|rt| rt > end_time + w.hit50)
                        .unwrap_or(true);
                if open_late_no_tail {
                    alt_head_press_time = Some(candidate);
                    rel_time = Some(end_time + w.hit50);
                    end_diff = w.hit50;
                    rel_kind = ReleaseKind::Hit50;
                    force_kind = true;
                    if let Some(prior_release) = late_hless_prev_rel {
                        late_headless_rescue = true;
                        has_early_rel = true;
                        repr_after_rel = true;
                        first_early_rel = Some(prior_release);
                        first_repr_after_rel = Some(candidate);
                        last_repr_time = Some(candidate);
                        rel_after_repr = rel_time;
                    }
                }
            }
        }
    }
    let hls_tap_tail = if true
        && !head_was_hit
        && press_time.is_none()
        && tail_only_pt.is_none()
        && alt_head_press_time.is_none()
        && !has_early_rel
        && matches!(rel_kind, ReleaseKind::Miss | ReleaseKind::None)
        && ln_duration <= w.hit100
    {
        next_same_col_idx.and_then(|ni| {
            let n = map.hit_objects.get(ni)?;
            if n.is_long_note() {
                return None;
            }
            let cp = ho.time + w.hit50;
            let ns = n.time - w.hit50;
            let ne = n.time + w.hit100;
            let used = judgments
                .iter()
                .any(|jj| jj.index == ni && jj.column == ho.column && jj.press_time == Some(cp));
            let rr = events
                .iter()
                .find(|ev| !ev.pressed && ev.time > cp)
                .map(|ev| ev.time)?;
            let repl = find_repl_pt(judgments, map, events, ni, cp, w);
            (used
                && cp > end_time
                && cp >= ns
                && cp < ne
                && rr > end_time
                && rr < tail_end_exclusive
                && rr < n.time
                && n.time - rr <= w.max
                && repl.is_some())
            .then_some((cp, rr))
        })
    } else {
        None
    };
    if let Some((cp, rr)) = hls_tap_tail {
        reclaim_pt_conflict(judgments, map, events, idx, ho.column, cp, w);
        rel_time = Some(rr);
        end_diff = (rr - end_time).abs();
        rel_kind = calc_rel_kind(end_diff, w, tail_window_scale);
        force_kind = false;
        repr_hit_tail = false;
        rescue_rel_near_end = None;
    }
    let hls_tap_tail_hit = hls_tap_tail.is_some();
    let sta_tap_tail = if true
        && head_was_hit
        && strong_head_hit
        && tail_only_pt.is_none()
        && ln_duration <= w.hit100
        && press_time.map(|pt| pt < ho.time).unwrap_or(false)
        && prev_same_col_idx
            .and_then(|pi| {
                let ph = map.hit_objects.get(pi)?;
                let pj = j_by_idx
                    .get(pi)
                    .and_then(|pos| *pos)
                    .and_then(|pos| judgments.get(pos))?;
                press_time.map(|pt| {
                    !ph.is_long_note()
                        && pj.kind == JudgmentKind::Miss
                        && pj.press_time.is_none()
                        && pt - ph.time == w.hit100
                })
            })
            .unwrap_or(false)
    {
        next_same_col_idx.and_then(|ni| {
            let nh = map.hit_objects.get(ni)?;
            if nh.is_long_note() {
                return None;
            }
            let nj = judgments
                .iter()
                .find(|jj| jj.index == ni && jj.column == ho.column)?;
            let cp = nj.press_time?;
            let rr = events
                .iter()
                .find(|ev| !ev.pressed && ev.time > cp)
                .map(|ev| ev.time)?;
            let nk = calc_hit_kind((cp - nh.time).abs(), w);
            let rk = calc_rel_kind((rr - end_time).abs(), w, tail_window_scale);
            let repl = find_repl_pt(judgments, map, events, ni, cp, w);
            (cp > end_time
                && cp >= ho.time + w.hit100
                && cp >= nh.time - w.hit50
                && cp < nh.time + w.hit100
                && !matches!(nk, JudgmentKind::Miss)
                && rr > cp
                && rr > end_time
                && rr < tail_end_exclusive
                && !matches!(rk, ReleaseKind::Miss | ReleaseKind::None)
                && repl.is_none())
            .then_some((cp, rr, rk))
        })
    } else {
        None
    };
    if let Some((cp, rr, rk)) = sta_tap_tail {
        reclaim_pt_conflict(judgments, map, events, idx, ho.column, cp, w);
        rel_time = Some(rr);
        end_diff = (rr - end_time).abs();
        rel_kind = rk;
        force_kind = false;
        repr_hit_tail = false;
        rescue_rel_near_end = None;
    }
    let hls_self = if true
        && !head_was_hit
        && press_time.is_none()
        && tail_only_pt.is_none()
        && alt_head_press_time.is_none()
        && ln_duration <= w.hit100
        && matches!(rel_kind, ReleaseKind::Miss | ReleaseKind::None)
    {
        next_same_col_idx.and_then(|ni| {
            let nh = map.hit_objects.get(ni)?;
            if !nh.is_long_note() {
                return None;
            }
            let nj_mis = j_by_idx
                .get(ni)
                .and_then(|pos| *pos)
                .and_then(|pos| judgments.get(pos))
                .map(|jj| jj.kind == JudgmentKind::Miss && jj.press_time.is_none())
                .unwrap_or(false);
            if !nj_mis {
                return None;
            }
            events.iter().filter(|ev| ev.pressed).find_map(|ev| {
                let cp = ev.time;
                if cp < ho.time || cp > end_time || cp >= nh.time {
                    return None;
                }
                if judgments
                    .iter()
                    .any(|jj| jj.column == ho.column && jj.press_time == Some(cp))
                {
                    return None;
                }
                let hk = calc_hit_kind((cp - ho.time).abs(), w);
                if !matches!(hk, JudgmentKind::Max | JudgmentKind::Hit300) {
                    return None;
                }
                let rr = events
                    .iter()
                    .find(|rel| !rel.pressed && rel.time > cp)
                    .map(|rel| rel.time)?;
                let rk = calc_rel_kind((rr - end_time).abs(), w, tail_window_scale);
                (rr > nh.time
                    && rr > end_time
                    && rr < tail_end_exclusive
                    && !matches!(rk, ReleaseKind::Miss | ReleaseKind::None))
                .then_some((cp, rr, rk))
            })
        })
    } else {
        None
    };
    if let Some((cp, rr, rk)) = hls_self {
        alt_head_press_time = Some(cp);
        rel_time = Some(rr);
        end_diff = (rr - end_time).abs();
        rel_kind = rk;
        force_kind = false;
        repr_hit_tail = false;
        rescue_rel_near_end = None;
    }
    let hls_tail = if true
        && !head_was_hit
        && press_time.is_none()
        && tail_only_pt.is_none()
        && alt_head_press_time.is_none()
        && ln_duration <= w.hit50 + w.hit100
        && prev_same_col_idx
            .and_then(|pi| map.hit_objects.get(pi))
            .map(|ph| !ph.is_long_note() && ho.time - ph.time <= w.hit50)
            .unwrap_or(false)
        && matches!(rel_kind, ReleaseKind::Miss | ReleaseKind::None)
    {
        events.iter().filter(|ev| ev.pressed).find_map(|ev| {
            let cp = ev.time;
            if cp < tail_start || cp < end_time - w.max || cp > end_time || cp < ho.time + w.hit100
            {
                return None;
            }
            if judgments
                .iter()
                .any(|jj| jj.column == ho.column && jj.press_time == Some(cp))
            {
                return None;
            }
            if !events
                .iter()
                .any(|rel| !rel.pressed && rel.time > ho.time && rel.time < cp)
            {
                return None;
            }
            let rr = events
                .iter()
                .find(|rel| !rel.pressed && rel.time > cp)
                .map(|rel| rel.time)?;
            let rk = calc_rel_kind((rr - end_time).abs(), w, tail_window_scale);
            let before_next = next_same_col_time.map(|nt| rr < nt).unwrap_or(true);
            (rr > end_time
                && rr < tail_end_exclusive
                && before_next
                && !matches!(rk, ReleaseKind::Miss | ReleaseKind::None))
            .then_some((rr, rk))
        })
    } else {
        None
    };
    if let Some((rr, rk)) = hls_tail {
        rel_time = Some(rr);
        end_diff = (rr - end_time).abs();
        rel_kind = rk;
        force_kind = false;
        repr_hit_tail = false;
        rescue_rel_near_end = None;
    }
    let hls_tail_hit = hls_tail.is_some();
    let pos_hea_aut_mis_meta = if true
        && !head_was_hit
        && press_time.is_none()
        && tail_only_pt.is_none()
        && alt_head_press_time.is_none()
        && !has_early_rel
        && ln_duration >= w.hit50 * 2
    {
        events
            .iter()
            .find(|ev| ev.pressed && ev.time > ho.time && ev.time < tail_start)
            .map(|ev| ev.time)
            .and_then(|first_press| {
                events
                    .iter()
                    .find(|ev| !ev.pressed && ev.time > first_press)
                    .map(|ev| (first_press, ev.time))
            })
            .and_then(|(_, first_release)| {
                if !(first_release > ho.time && first_release < tail_start) {
                    return None;
                }
                let later_press = events
                    .iter()
                    .find(|ev| ev.pressed && ev.time > first_release && ev.time <= end_time)
                    .map(|ev| ev.time)?;
                let later_release = events
                    .iter()
                    .find(|ev| !ev.pressed && ev.time > later_press)
                    .map(|ev| ev.time)?;
                let lat_rel_pre_nex_head = next_same_col_time
                    .map(|next_t| later_release < next_t)
                    .unwrap_or(true);
                if later_release >= tail_end_exclusive && lat_rel_pre_nex_head {
                    Some((first_release, later_press, later_release))
                } else {
                    None
                }
            })
    } else {
        None
    };
    if let Some((first_release, later_press, later_release)) = pos_hea_aut_mis_meta {
        has_early_rel = true;
        first_early_rel = Some(first_release);
        repr_after_rel = true;
        first_repr_after_rel = Some(later_press);
        last_repr_time = Some(later_press);
        rel_after_repr = Some(later_release);
    }
    let post_end_rescue = false
        && !head_was_hit
        && press_time.is_some()
        && matches!(rel_kind, ReleaseKind::Miss | ReleaseKind::None)
        && first_early_rel.map(|t| t < tail_start).unwrap_or(false)
        && first_repr_after_rel
            .map(|rp| rp > end_time && rp - end_time <= w.hit100)
            .unwrap_or(false);
    let late_body_rescue_cand = false
        && !head_was_hit
        && press_time
            .map(|pt| pt < ho.time - w.hit100)
            .unwrap_or(false)
        && matches!(rel_kind, ReleaseKind::Miss | ReleaseKind::None)
        && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| {
                rp > tail_start && rp <= end_time && rr > end_time && rr < tail_end_exclusive
            })
            .unwrap_or(false);
    if (head_was_hit || alt_head_press_time.is_some() || post_end_rescue || late_body_rescue_cand)
        && has_early_rel
        && !(true
            && matches!(
                rel_kind,
                ReleaseKind::Max
                    | ReleaseKind::Hit300
                    | ReleaseKind::Hit200
                    | ReleaseKind::Hit100
                    | ReleaseKind::Hit50
            )
            && rel_time.is_some())
        && (first_repr_after_rel
            .map(|rp| rp > end_time && rp - end_time <= w.hit100)
            .unwrap_or(false)
            || late_body_rescue_cand)
    {
        if let Some(rescue_press_time) = first_repr_after_rel {
            let rescue_rel_time = rel_after_repr.or_else(|| {
                if rescue_press_time > end_time && rescue_press_time - end_time <= w.hit100 {
                    Some(end_time + w.hit50)
                } else {
                    None
                }
            });
            if let Some(rescue_rel_time) = rescue_rel_time {
                let rescue_steals_short = if true {
                    steals_next_ln_head(
                        judgments,
                        map,
                        ho.column,
                        next_same_col_idx,
                        rescue_press_time,
                        rescue_rel_time,
                        w,
                        tail_window_scale,
                    )
                } else {
                    false
                };
                let rescue_steals_tap = if true {
                    steals_next_tap_head(
                        judgments,
                        map,
                        events,
                        ho.column,
                        next_same_col_idx,
                        rescue_press_time,
                        w,
                    )
                } else {
                    false
                };
                let rescue_steals_tap_now = if true {
                    first_early_rel.is_some()
                        && next_same_col_idx
                            .and_then(|next_idx| {
                                map.hit_objects
                                    .get(next_idx)
                                    .map(|next_ho| (next_idx, next_ho))
                            })
                            .map(|(next_idx, next_ho)| {
                                if next_ho.is_long_note() {
                                    return false;
                                }
                                let next_head_start = next_ho.time - w.hit50;
                                let next_head_win_end = next_ho.time + w.hit100;
                                let next_tap_other_pt = judgments
                                    .iter()
                                    .find(|jj| jj.index == next_idx && jj.column == ho.column)
                                    .and_then(|jj| jj.press_time)
                                    .map(|pt| pt != rescue_press_time)
                                    .unwrap_or(false);
                                let has_next_tap_follow = events.iter().any(|ev| {
                                    ev.pressed
                                        && ev.time > rescue_press_time
                                        && ev.time >= next_head_start
                                        && ev.time < next_head_win_end
                                });
                                rescue_press_time > next_ho.time + w.max
                                    && rescue_press_time >= next_head_start
                                    && rescue_press_time < next_head_win_end
                                    && calc_hit_kind((rescue_press_time - next_ho.time).abs(), w)
                                        == JudgmentKind::Hit300
                                    && !next_tap_other_pt
                                    && !has_next_tap_follow
                            })
                            .unwrap_or(false)
                } else {
                    false
                };
                let rescue_steals_tap_cur = if true {
                    first_early_rel.is_some()
                        && next_same_col_idx
                            .and_then(|next_idx| {
                                let next_ho = map.hit_objects.get(next_idx)?;
                                if next_ho.is_long_note() {
                                    return None;
                                }
                                let next_j = judgments.iter().find(|jj| {
                                    jj.index == next_idx
                                        && jj.column == ho.column
                                        && jj.press_time == Some(rescue_press_time)
                                })?;
                                Some((next_ho, next_j))
                            })
                            .map(|(next_ho, _)| {
                                let next_head_start = next_ho.time - w.hit50;
                                let next_head_win_end = next_ho.time + w.hit100;
                                rescue_press_time >= next_head_start
                                    && rescue_press_time < next_head_win_end
                                    && rescue_press_time <= next_ho.time + w.max
                                    && matches!(
                                        calc_hit_kind((rescue_press_time - next_ho.time).abs(), w),
                                        JudgmentKind::Max | JudgmentKind::Hit300
                                    )
                            })
                            .unwrap_or(false)
                } else {
                    false
                };
                if rescue_steals_short
                    || rescue_steals_tap
                    || rescue_steals_tap_now
                    || rescue_steals_tap_cur
                {
                    if rescue_steals_tap_now || rescue_steals_tap_cur {
                        _rescue_steals_tap = true;
                    }
                    if rel_time.is_none() {
                        rel_time = first_early_rel;
                        if let Some(rt) = rel_time {
                            end_diff = (rt - end_time).abs();
                        }
                    }
                } else {
                    let rescue_conflict = reconcile_tail_rescue(
                        judgments,
                        map,
                        events,
                        idx,
                        ho.column,
                        rescue_press_time,
                        ln_duration,
                        tail_start,
                        end_time,
                        w,
                    );
                    let long_no_conf_tail = false
                        && !rescue_conflict.consumed_by_other
                        && rescue_press_time > end_time
                        && rescue_press_time - end_time <= w.hit300 + 5
                        && ln_duration >= w.hit50 * 2
                        && {
                            let late_repr_dur = (w.hit50 + w.hit100 + w.max).max(w.hit50 * 2 + 1);
                            !(ln_duration <= late_repr_dur && rescue_press_time > tail_start)
                        };
                    if rescue_conflict.allowed || long_no_conf_tail {
                        if rescue_conflict.consumed_by_other {
                            first_free_repr = Some(rescue_press_time);
                            last_repr_free = true;
                        }
                        rel_time = Some(rescue_rel_time);
                        end_diff = (rescue_rel_time - end_time).abs();
                        rel_kind = ReleaseKind::Hit50;
                        rescue_rel_near_end = Some(rescue_rel_time);
                        repr_hit_tail = true;
                    }
                }
            }
        }
    }
    let deep_prewin_headless = true
        && !head_was_hit
        && alt_head_press_time.is_some()
        && deep_ln_pen
        && has_early_rel
        && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
        && repr_after_rel
        && repr_hit_tail
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| {
                rp > ho.time
                    && rp <= end_time
                    && rr >= tail_start
                    && rr < tail_end_exclusive
                    && rr > rp
            })
            .unwrap_or(false)
        && rel_time
            .zip(rel_after_repr)
            .map(|(rt, rr)| rt == rr)
            .unwrap_or(false)
        && !matches!(rel_kind, ReleaseKind::Miss | ReleaseKind::None);
    if deep_prewin_headless {
        alt_head_press_time = None;
        if let Some(pt) = press_time {
            metadata_clears.push((idx, pt));
        }
    }
    let hea_mis_fir_repr_h50 = true
        && !head_was_hit
        && alt_head_press_time.is_some()
        && matches!(rel_kind, ReleaseKind::Miss)
        && press_time
            .map(|pt| ho.time - pt <= w.hit100 + w.hit300)
            .unwrap_or(false)
        && rel_time
            .zip(first_early_rel)
            .map(|(rt, early_rt)| rt == early_rt)
            .unwrap_or(false)
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| {
                next_ln_keeps(
                    judgments,
                    map,
                    events,
                    ho.column,
                    next_same_col_idx,
                    rp,
                    rr,
                    w,
                    tail_window_scale,
                )
            })
            .unwrap_or(false);
    if hea_mis_fir_repr_h50 {
        rel_kind = ReleaseKind::Hit50;
        force_kind = false;
    }
    if false {
        if let Some(immediate_rel_time) = imm_rel_at_press {
            rel_kind = ReleaseKind::Miss;
            rel_time = Some(immediate_rel_time);
            end_diff = (immediate_rel_time - end_time).abs();
            force_kind = false;
            repr_after_rel = false;
            first_repr_after_rel = None;
            last_repr_time = None;
            first_free_repr = None;
            rel_after_repr = None;
            rescue_rel_near_end = None;
            last_repr_free = false;
            repr_hit_tail = false;
        }
    }
    let exact_tail_pre_break = false
        && press_time.map(|pt| pt < ho.time - w.max).unwrap_or(false)
        && first_early_rel
            .map(|t| t >= ho.time && t <= ho.time + w.max)
            .unwrap_or(false)
        && first_repr_after_rel
            .map(|rp| rp == tail_start)
            .unwrap_or(false);
    let exact_tail_post_break = false
        && press_time
            .map(|pt| pt >= ho.time && pt <= ho.time + w.max)
            .unwrap_or(false)
        && first_early_rel
            .map(|t| t > ho.time + w.max && t < tail_start)
            .unwrap_or(false)
        && first_repr_after_rel
            .map(|rp| rp == tail_start)
            .unwrap_or(false);
    let exac_tail_micr_break = exact_tail_pre_break || exact_tail_post_break;
    let short_post_repr_tail = rel_after_repr
        .map(|rt| {
            if true {
                rt < tail_start || rt >= tail_end_exclusive
            } else {
                rt < tail_start || rt > end_time
            }
        })
        .unwrap_or(true);
    let short_tail_no_rec = head_was_hit
        && has_early_rel
        && repr_after_rel
        && ln_duration < w.hit50 * 2
        && short_post_repr_tail
        && first_early_rel
            .map(|t| t >= tail_start - (w.max + 4))
            .unwrap_or(false)
        && first_repr_after_rel
            .map(|rp| (rp > tail_start || exac_tail_micr_break) && rp <= tail_start + w.max + 2)
            .unwrap_or(false);
    if short_tail_no_rec {
        rel_kind = ReleaseKind::Miss;
        if exac_tail_micr_break {
            rel_time = first_early_rel;
            end_diff = first_early_rel.map(|t| (t - end_time).abs()).unwrap_or(0);
        } else {
            rel_time = None;
            end_diff = 0;
        }
        force_kind = false;
        repr_hit_tail = false;
        rescue_rel_near_end = None;
    }
    let prehead_break_h50 = true
        && head_was_hit
        && press_time.map(|pt| pt < ho.time).unwrap_or(false)
        && has_early_rel
        && repr_after_rel
        && rel_time
            .zip(rel_after_repr)
            .map(|(rt, rr)| rt == rr)
            .unwrap_or(false)
        && first_early_rel
            .zip(first_repr_after_rel)
            .zip(rel_after_repr)
            .map(|((first_rt, rp), rr)| {
                first_rt < ho.time
                    && rp >= first_rt
                    && rp <= first_rt + 3
                    && rp < ho.time
                    && rr >= tail_start
                    && rr < tail_end_exclusive
            })
            .unwrap_or(false);
    if prehead_break_h50 {
        rel_time = first_early_rel;
        end_diff = first_early_rel.map(|rt| (rt - end_time).abs()).unwrap_or(0);
        rel_kind = ReleaseKind::Hit50;
        force_kind = false;
        repr_hit_tail = false;
        rescue_rel_near_end = None;
    }
    let pre_fol_ghost_pt = press_time
        .zip(first_early_rel)
        .and_then(|(pt, first_rel_time)| {
            events
                .iter()
                .find(|ev| {
                    ev.pressed && ev.time > pt && ev.time < first_rel_time && ev.time < ho.time
                })
                .map(|ev| ev.time)
        });
    let prehead_ghost_keeps = true
        && head_was_hit
        && press_time.map(|pt| pt < ho.time).unwrap_or(false)
        && has_early_rel
        && repr_after_rel
        && repr_hit_tail
        && first_early_rel.map(|rt| rt < ho.time).unwrap_or(false)
        && first_repr_after_rel.map(|rp| rp < ho.time).unwrap_or(false)
        && rel_time
            .zip(rel_after_repr)
            .map(|(rt, rr)| rt == rr && rt >= tail_start && rt < tail_end_exclusive)
            .unwrap_or(false)
        && pre_fol_ghost_pt
            .zip(first_early_rel)
            .map(|(ghost_press_time, first_rel_time)| {
                first_rel_time > ghost_press_time && first_rel_time <= ghost_press_time + 1
            })
            .unwrap_or(false)
        && pre_fol_ghost_pt
            .map(|ghost_press_time| {
                ghost_press_time >= ho.time - w.hit50
                    && ghost_press_time < ho.time + w.hit100
                    && !judgments.iter().any(|jj| {
                        jj.index != idx
                            && jj.column == ho.column
                            && jj.press_time == Some(ghost_press_time)
                    })
            })
            .unwrap_or(false);
    if prehead_ghost_keeps {
        if let Some(ghost_press_time) = pre_fol_ghost_pt {
            alt_head_press_time = Some(ghost_press_time);
            rel_kind = ReleaseKind::Hit50;
            force_kind = false;
            has_early_rel = false;
            first_early_rel = None;
            repr_after_rel = false;
            first_repr_after_rel = None;
            last_repr_time = None;
            first_free_repr = None;
            rel_after_repr = None;
            rescue_rel_near_end = None;
            last_repr_free = false;
            repr_hit_tail = false;
        }
    }
    let head_miss_pre_meta = true
        && !head_was_hit
        && ln_duration > w.hit100
        && alt_head_press_time.is_some()
        && press_time.map(|pt| pt < ho.time).unwrap_or(false)
        && has_early_rel
        && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
        && repr_after_rel
        && !repr_hit_tail
        && rel_time.is_none()
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| rp > ho.time && rp <= tail_start && rr > rp && rr < tail_start)
            .unwrap_or(false);
    if true
        && (head_was_hit || head_miss_pre_meta)
        && has_early_rel
        && matches!(rel_kind, ReleaseKind::Miss)
        && rel_time.is_none()
    {
        if let Some(pre_tail_rel_time) = rel_after_repr.filter(|rt| *rt < tail_start) {
            rel_time = Some(pre_tail_rel_time);
            end_diff = (pre_tail_rel_time - end_time).abs();
        } else if let Some(early_rel_time) = first_early_rel {
            rel_time = Some(early_rel_time);
            end_diff = (early_rel_time - end_time).abs();
        }
    }
    let head_hit_early_tail50 = true
        && head_was_hit
        && has_early_rel
        && repr_after_rel
        && ln_duration < w.hit50 * 2
        && !repr_hit_tail
        && matches!(rel_kind, ReleaseKind::Miss | ReleaseKind::None)
        && first_early_rel
            .map(|rt| {
                rt >= tail_start
                    && rt < tail_end_exclusive
                    && !matches!(
                        calc_rel_kind((rt - end_time).abs(), w, tail_window_scale),
                        ReleaseKind::Miss | ReleaseKind::None
                    )
            })
            .unwrap_or(false)
        && first_repr_after_rel
            .map(|rp| rp > tail_start && rp <= end_time)
            .unwrap_or(false)
        && rel_after_repr
            .zip(first_repr_after_rel)
            .map(|(rt, rp)| {
                let next_same_long_head = next_same_col_idx
                    .and_then(|next_idx| {
                        let next_ho = map.hit_objects.get(next_idx)?;
                        if !next_ho.is_long_note() {
                            return None;
                        }
                        let next_head_start = next_ho.time - w.hit50;
                        let next_head_win_end = next_ho.time + w.hit100;
                        let next_press_time = judgments
                            .iter()
                            .find(|jj| jj.index == next_idx && jj.column == ho.column)
                            .and_then(|jj| jj.press_time);
                        Some(
                            rt < next_ho.time
                                && next_press_time
                                    .map(|press_time| {
                                        press_time > rt
                                            && press_time >= next_head_start
                                            && press_time < next_head_win_end
                                    })
                                    .unwrap_or(false),
                        )
                    })
                    .unwrap_or(false);
                (rt < end_time
                    && !(strong_head_hit
                        && next_same_col_idx
                            .and_then(|next_idx| {
                                let next_ho = map.hit_objects.get(next_idx)?;
                                if !next_ho.is_long_note() {
                                    return None;
                                }
                                let next_duration =
                                    next_ho.end_time.unwrap_or(next_ho.time) - next_ho.time;
                                let next_press_time = judgments
                                    .iter()
                                    .find(|jj| jj.index == next_idx && jj.column == ho.column)
                                    .and_then(|jj| jj.press_time);
                                Some(
                                    next_duration > w.hit50
                                        && next_press_time
                                            .map(|press_time| {
                                                press_time > end_time && press_time > rt
                                            })
                                            .unwrap_or(false),
                                )
                            })
                            .unwrap_or(false)))
                    || (head_is_h50
                        && rt > end_time
                        && rt < tail_end_exclusive
                        && !next_same_long_head
                        && (rp > late_repr_guard
                            || steals_next_ln_head(
                                judgments,
                                map,
                                ho.column,
                                next_same_col_idx,
                                rp,
                                rt,
                                w,
                                tail_window_scale,
                            )))
                    || (strong_head_hit
                        && rt > end_time
                        && rt < tail_end_exclusive
                        && next_same_col_idx
                            .and_then(|next_idx| {
                                let next_ho = map.hit_objects.get(next_idx)?;
                                if next_ho.is_long_note() {
                                    return None;
                                }
                                Some((next_idx, next_ho))
                            })
                            .map(|(next_idx, next_ho)| {
                                let next_head_start = next_ho.time - w.hit50;
                                let next_head_win_end = next_ho.time + w.hit100;
                                let next_press_time = judgments
                                    .iter()
                                    .find(|jj| jj.index == next_idx && jj.column == ho.column)
                                    .and_then(|jj| jj.press_time);
                                let raw_followup_press = events.iter().any(|ev| {
                                    ev.pressed
                                        && ev.time > rt
                                        && ev.time >= next_head_start
                                        && ev.time < next_head_win_end
                                });
                                rp < next_head_start
                                    && (next_press_time
                                        .map(|press_time| {
                                            press_time > rt
                                                && press_time >= next_head_start
                                                && press_time < next_head_win_end
                                        })
                                        .unwrap_or(false)
                                        || raw_followup_press)
                            })
                            .unwrap_or(false))
            })
            .unwrap_or(false);
    if head_hit_early_tail50 {
        rel_time = first_early_rel;
        end_diff = first_early_rel.map(|t| (t - end_time).abs()).unwrap_or(0);
        rel_kind = ReleaseKind::Hit50;
        force_kind = false;
    }
    let head_h50_caps_now = true
        && head_was_hit
        && head_is_h50
        && press_time.map(|pt| pt < ho.time).unwrap_or(false)
        && !has_early_rel
        && !repr_after_rel
        && matches!(rel_kind, ReleaseKind::Hit100)
        && rel_time.map(|rt| rt < end_time).unwrap_or(false)
        && ln_duration <= w.hit50 + w.hit100
        && press_time
            .zip(rel_time)
            .map(|(pt, rt)| {
                let head_window_start = ho.time - w.hit50;
                let head_win_end = ho.time + w.hit100;
                events.iter().any(|ev| {
                    ev.pressed
                        && ev.time > pt
                        && ev.time >= head_window_start
                        && ev.time < head_win_end
                        && ev.time < rt
                        && next_same_col_time
                            .map(|next_t| ev.time < next_t)
                            .unwrap_or(true)
                })
            })
            .unwrap_or(false);
    if head_h50_caps_now {
        rel_kind = ReleaseKind::Hit50;
        force_kind = false;
        _head_h50_caps_h50 = true;
    }
    let hit50_keeps_head = true
        && tail_pref_exact
        && head_was_hit
        && head_is_h50
        && press_time.map(|pt| pt < ho.time).unwrap_or(false)
        && has_early_rel
        && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
        && rel_time
            .zip(rel_after_repr)
            .map(|(rt, rr)| rt == rr && rt == end_time)
            .unwrap_or(false);
    if hit50_keeps_head {
        repr_hit_tail = false;
        rescue_rel_near_end = None;
    }
    let prehead_follow_repr = first_rel_after_press
        .zip(first_early_rel)
        .zip(first_repr_after_rel)
        .and_then(|((followup_release, first_release), first_repress)| {
            (followup_release > first_release
                && followup_release <= first_release + 1
                && followup_release < first_repress
                && followup_release < ho.time
                && followup_release >= tail_start)
                .then_some(followup_release)
        });
    let pre_tail_keeps_rel = true
        && head_was_hit
        && (head_is_h100
            || (strong_head_hit
                && imm_rel_at_press
                    .zip(first_early_rel)
                    .map(|(immediate_rel_time, first_rel_time)| {
                        immediate_rel_time == first_rel_time
                    })
                    .unwrap_or(false)))
        && press_time.map(|pt| pt < ho.time).unwrap_or(false)
        && has_early_rel
        && repr_after_rel
        && ln_duration < w.hit50 * 2
        && first_early_rel
            .map(|rt| rt >= tail_start && rt < tail_end_exclusive && rt <= ho.time + w.hit300)
            .unwrap_or(false)
        && first_early_rel
            .map(|rt| rt >= ho.time || ln_duration <= w.hit50 + w.max)
            .unwrap_or(false)
        && first_repr_after_rel
            .zip(first_early_rel)
            .map(|(rp, first_rt)| rp > first_rt && rp <= end_time)
            .unwrap_or(false)
        && rel_after_repr
            .map(|rt| rt > end_time && rt < tail_end_exclusive)
            .unwrap_or(false)
        && next_same_col_idx
            .and_then(|next_idx| {
                let next_ho = map.hit_objects.get(next_idx)?;
                if next_ho.is_long_note() {
                    return None;
                }
                let next_head_start = next_ho.time - w.hit50;
                let next_head_win_end = next_ho.time + w.hit100;
                let next_press_time = judgments
                    .iter()
                    .find(|jj| jj.index == next_idx && jj.column == ho.column)
                    .and_then(|jj| jj.press_time);
                Some((next_head_start, next_head_win_end, next_press_time))
            })
            .zip(rel_after_repr)
            .map(
                |((next_head_start, next_head_win_end, next_press_time), rr)| {
                    next_press_time
                        .map(|press_time| {
                            press_time > rr
                                && press_time >= next_head_start
                                && press_time < next_head_win_end
                        })
                        .unwrap_or(false)
                        || events.iter().any(|ev| {
                            ev.pressed
                                && ev.time > rr
                                && ev.time >= next_head_start
                                && ev.time < next_head_win_end
                        })
                },
            )
            .unwrap_or(false);
    if pre_tail_keeps_rel {
        let preferred_first_rel = prehead_follow_repr.or(first_early_rel);
        rel_time = preferred_first_rel;
        end_diff = preferred_first_rel
            .map(|t| (t - end_time).abs())
            .unwrap_or(0);
        rel_kind = ReleaseKind::Hit50;
        force_kind = false;
        repr_hit_tail = false;
        rescue_rel_near_end = None;
        _pre_tail_break_keep_rel = true;
    }
    let strong_head_keeps = true
        && head_was_hit
        && strong_head_hit
        && has_early_rel
        && repr_after_rel
        && repr_hit_tail
        && rel_time
            .zip(rel_after_repr)
            .map(|(rt, rr)| rt == rr)
            .unwrap_or(false)
        && ln_duration < w.hit50 * 2
        && press_time
            .map(|pt| pt >= ho.time && pt <= ho.time + w.max)
            .unwrap_or(false)
        && first_early_rel
            .map(|rt| rt >= tail_start && rt < tail_end_exclusive && rt > ho.time + w.max)
            .unwrap_or(false)
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| {
                rp > first_early_rel.unwrap_or(i32::MIN)
                    && rp >= late_repr_guard
                    && rp <= end_time
                    && rr > end_time
                    && rr < tail_end_exclusive
                    && next_same_col_idx
                        .and_then(|next_idx| {
                            let next_ho = map.hit_objects.get(next_idx)?;
                            if !next_ho.is_long_note() {
                                return None;
                            }
                            let next_head_start = next_ho.time - w.hit50;
                            let next_head_win_end = next_ho.time + w.hit100;
                            let next_press_time = judgments
                                .iter()
                                .find(|jj| jj.index == next_idx && jj.column == ho.column)
                                .and_then(|jj| jj.press_time);
                            Some(
                                rr < next_ho.time
                                    && next_press_time
                                        .map(|press_time| {
                                            press_time > rr
                                                && press_time >= next_head_start
                                                && press_time < next_head_win_end
                                        })
                                        .unwrap_or(false),
                            )
                        })
                        .unwrap_or(false)
            })
            .unwrap_or(false);
    if strong_head_keeps {
        rel_time = first_early_rel;
        end_diff = first_early_rel.map(|t| (t - end_time).abs()).unwrap_or(0);
        rel_kind = ReleaseKind::Hit50;
        force_kind = false;
        repr_hit_tail = false;
        rescue_rel_near_end = None;
    }
    let short_near_keeps = true
        && !head_was_hit
        && ln_duration <= w.hit100
        && press_time.map(|pt| pt < ho.time).unwrap_or(false)
        && has_early_rel
        && repr_after_rel
        && repr_hit_tail
        && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
        && rel_time
            .zip(rel_after_repr)
            .map(|(rt, rr)| rt == rr)
            .unwrap_or(false)
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| {
                rp >= ho.time - w.max && rp < ho.time && rr >= tail_start && rr < tail_end_exclusive
            })
            .unwrap_or(false);
    if short_near_keeps {
        repr_hit_tail = false;
        rescue_rel_near_end = None;
    }
    if short_body_miss && matches!(rel_kind, ReleaseKind::Miss | ReleaseKind::None) {
        rel_kind = ReleaseKind::Hit50;
        rel_time = first_early_rel;
        end_diff = first_early_rel.map(|t| (t - end_time).abs()).unwrap_or(0);
        force_kind = false;
    }
    if has_early_rel
        && !matches!(rel_kind, ReleaseKind::Miss | ReleaseKind::None)
        && !deep_prewin_headless
    {
        rel_kind = ReleaseKind::Hit50;
    }
    let post_end_keeps_tail = false
        && !head_was_hit
        && repr_hit_tail
        && first_repr_after_rel
            .map(|rp| rp > end_time && rp - end_time <= w.hit100)
            .unwrap_or(false);
    let late_miss_keeps_tail = false
        && !head_was_hit
        && repr_hit_tail
        && press_time
            .map(|pt| pt < ho.time - w.hit100)
            .unwrap_or(false)
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| {
                rp > tail_start && rp <= end_time && rr > end_time && rr < tail_end_exclusive
            })
            .unwrap_or(false);
    let effective_head_hit = head_was_hit
        || alt_head_press_time.is_some()
        || post_end_hless
        || tail_only_pt.is_some()
        || miss_press_rel_tail
        || tail_hold_hit
        || miss_repr_tail_any
        || short_body_miss
        || post_end_keeps_tail
        || late_miss_keeps_tail
        || hls_tap_tail_hit
        || hls_tail_hit;
    let sho_hea_pre_keep_rel = true
        && !head_was_hit
        && ln_duration <= w.hit100
        && press_time.map(|pt| pt < ho.time).unwrap_or(false)
        && has_early_rel
        && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
        && repr_after_rel
        && !repr_hit_tail
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| rp > end_time && rr > rp)
            .unwrap_or(false);
    let shrtsh_head_keep_rel = true
        && !head_was_hit
        && ln_duration > w.hit100
        && ln_duration <= late_repr_dur
        && alt_head_press_time.is_some()
        && press_time.map(|pt| pt < ho.time).unwrap_or(false)
        && has_early_rel
        && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
        && repr_after_rel
        && !repr_hit_tail
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| rp > end_time && rr > rp)
            .unwrap_or(false);
    let hea_mis_next_ln_clai = true
        && !head_was_hit
        && alt_head_press_time.is_none()
        && press_time.map(|pt| pt < ho.time).unwrap_or(false)
        && has_early_rel
        && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
        && repr_after_rel
        && !repr_hit_tail
        && matches!(rel_kind, ReleaseKind::Miss)
        && rel_time.is_none()
        && first_repr_after_rel
            .zip(rel_after_repr)
            .zip(next_same_col_idx)
            .map(|((rp, rr), next_idx)| {
                map.hit_objects
                    .get(next_idx)
                    .map(|next_ho| {
                        let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                        let next_tail_start =
                            next_end_time - ((w.hit50 as f32) * tail_window_scale).round() as i32;
                        let next_tail_end =
                            next_end_time + ((w.hit100 as f32) * tail_window_scale).round() as i32;
                        let next_jj = judgments
                            .iter()
                            .find(|jj| jj.index == next_idx && jj.column == ho.column);
                        next_ho.column == ho.column
                            && next_ho.is_long_note()
                            && rp > ho.time
                            && rp <= end_time
                            && rr > end_time
                            && rr > rp
                            && first_early_rel.map(|fr| fr < rp).unwrap_or(false)
                            && rr >= next_tail_start
                            && rr < next_tail_end
                            && next_jj
                                .map(|jj| {
                                    jj.press_time == Some(rp) && jj.kind == JudgmentKind::Miss
                                })
                                .unwrap_or(false)
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false);
    if !effective_head_hit
        || (((press_time.is_none() && tail_only_pt.is_none()) && alt_head_press_time.is_none())
            && !hls_tap_tail_hit
            && !hls_tail_hit)
    {
        rel_kind = ReleaseKind::Miss;
        if sho_hea_pre_keep_rel
            || shrtsh_head_keep_rel
            || hea_mis_next_ln_clai
            || pos_hea_aut_mis_meta.is_some()
        {
            rel_time = first_early_rel;
            end_diff = first_early_rel.map(|t| (t - end_time).abs()).unwrap_or(0);
        } else {
            rel_time = None;
            end_diff = 0;
        }
        force_kind = false;
    }
    if matches!(rel_kind, ReleaseKind::Miss)
        && rel_time.is_none()
        && (sho_hea_pre_keep_rel || shrtsh_head_keep_rel || hea_mis_next_ln_clai)
    {
        rel_time = first_early_rel;
        end_diff = first_early_rel.map(|t| (t - end_time).abs()).unwrap_or(0);
        force_kind = false;
    }
    if pre_frag_keep_rel {
        rel_time = first_early_rel;
        end_diff = first_early_rel.map(|rt| (rt - end_time).abs()).unwrap_or(0);
        rel_kind = ReleaseKind::Hit50;
        force_kind = false;
    }
    let tail_closed_keeps_rel = if true
        && !head_was_hit
        && press_time.is_none()
        && tail_only_pt.is_some()
        && has_early_rel
        && repr_after_rel
        && (rel_time
            .zip(rel_after_repr)
            .map(|(rt, rr)| rt == rr)
            .unwrap_or(false)
            || (rel_time.is_none() && !repr_hit_tail))
    {
        first_early_rel
            .zip(tail_only_pt)
            .zip(first_repr_after_rel.zip(rel_after_repr))
            .map(|((first_release, tail_only_pt), (rp, rr))| {
                first_release > tail_only_pt
                    && first_release < rp
                    && ((steals_next_ln_head(
                        judgments,
                        map,
                        ho.column,
                        next_same_col_idx,
                        rp,
                        rr,
                        w,
                        tail_window_scale,
                    ) || steals_next_tap_head(
                        judgments,
                        map,
                        events,
                        ho.column,
                        next_same_col_idx,
                        rp,
                        w,
                    )) || next_same_col_idx
                        .and_then(|next_idx| {
                            map.hit_objects
                                .get(next_idx)
                                .map(|next_ho| (next_idx, next_ho))
                        })
                        .and_then(|(next_idx, next_ho)| {
                            if next_ho.column != ho.column || next_ho.is_long_note() {
                                return None;
                            }
                            map.hit_objects[(next_idx + 1)..]
                                .iter()
                                .enumerate()
                                .find(|(_, next_next_ho)| next_next_ho.column == ho.column)
                                .map(|(offset, next_next_ho)| {
                                    (next_ho, next_next_ho, next_idx + 1 + offset)
                                })
                        })
                        .map(|(next_tap_ho, next_next_ho, _)| {
                            if !next_next_ho.is_long_note() {
                                return false;
                            }
                            let next_tap_window_start = next_tap_ho.time - w.hit50;
                            let next_tap_end = next_tap_ho.time + w.hit100;
                            let next_ln_window_start = next_next_ho.time - w.hit50;
                            let next_ln_win_end = next_next_ho.time + w.hit100;
                            let next_ln_end_time =
                                next_next_ho.end_time.unwrap_or(next_next_ho.time);
                            let next_ln_tail_start = next_ln_end_time
                                - ((w.hit50 as f32) * tail_window_scale).round() as i32;
                            let next_ln_tail_end = next_ln_end_time
                                + ((w.hit100 as f32) * tail_window_scale).round() as i32;
                            let next_ln_head_tail = events
                                .iter()
                                .filter(|ev| {
                                    ev.pressed
                                        && ev.time > rp
                                        && ev.time >= next_ln_window_start
                                        && ev.time < next_ln_win_end
                                })
                                .any(|next_ln_press| {
                                    events
                                        .iter()
                                        .find(|ev| !ev.pressed && ev.time > next_ln_press.time)
                                        .map(|ev| {
                                            ev.time >= next_ln_tail_start
                                                && ev.time < next_ln_tail_end
                                        })
                                        .unwrap_or(false)
                                });
                            rp >= next_tap_window_start
                                && rp < next_tap_end
                                && calc_hit_kind((rp - next_tap_ho.time).abs(), w)
                                    != JudgmentKind::Miss
                                && rr < next_tap_ho.time
                                && next_ln_head_tail
                        })
                        .unwrap_or(false))
            })
            .unwrap_or(false)
    } else {
        false
    };
    if tail_closed_keeps_rel {
        rel_time = first_early_rel;
        end_diff = first_early_rel.map(|rt| (rt - end_time).abs()).unwrap_or(0);
        rel_kind = ReleaseKind::Hit50;
        repr_hit_tail = false;
        rescue_rel_near_end = None;
        force_kind = false;
    }
    let late_tail_hit_hless = true
        && !head_was_hit
        && !has_early_rel
        && tail_only_pt.is_none()
        && press_time
            .map(|pt| pt > ho.time + w.hit50 && pt <= end_time)
            .unwrap_or(false)
        && rel_time
            .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
            .unwrap_or(false)
        && !matches!(rel_kind, ReleaseKind::Miss | ReleaseKind::None);
    if late_tail_hit_hless {
        if let Some(pt) = press_time {
            metadata_clears.push((idx, pt));
        }
    }
    let short_headless_miss = true
        && !head_was_hit
        && press_time.is_none()
        && !has_early_rel
        && ln_duration <= w.hit100
        && tail_only_pt
            .zip(rel_time)
            .map(|(pt, rt)| pt > end_time && rt > end_time && rt - pt > w.hit300)
            .unwrap_or(false)
        && next_same_col_time
            .zip(rel_time)
            .map(|(next_t, rt)| rt < next_t - w.hit100)
            .unwrap_or(false)
        && rel_kind == ReleaseKind::Hit50
        && prev_same_col_idx
            .and_then(|prev_idx| {
                let prev_ho = map.hit_objects.get(prev_idx)?;
                let prev_end = prev_ho.end_time.unwrap_or(prev_ho.time);
                let prev_release = ln_release_info.get(&prev_idx).and_then(|info| info.time)?;
                let prev_press = j_by_idx
                    .get(prev_idx)
                    .and_then(|pos| *pos)
                    .and_then(|pos| judgments.get(pos))
                    .and_then(|jj| jj.press_time)?;
                Some((prev_ho, prev_end, prev_press, prev_release))
            })
            .map(|(prev_ho, prev_end, prev_press, prev_release)| {
                prev_ho.is_long_note()
                    && prev_end - prev_ho.time <= w.hit100
                    && prev_end <= ho.time
                    && prev_press < ho.time
                    && prev_release > ho.time
                    && prev_release >= end_time - w.max
                    && tail_only_pt
                        .map(|pt| pt > prev_release && pt - prev_release <= w.hit100)
                        .unwrap_or(false)
            })
            .unwrap_or(false);
    if short_headless_miss {
        rel_time = None;
        rel_kind = ReleaseKind::Miss;
        end_diff = 0;
        force_kind = false;
    }
    let hless_body_auto_miss = if true && !head_was_hit && press_time.is_none() && !has_early_rel {
        tail_only_pt
            .zip(rel_time)
            .and_then(|(tail_only_pt, tail_rt)| {
                if !(tail_only_pt >= tail_start
                    && tail_only_pt <= end_time
                    && tail_rt > tail_only_pt
                    && tail_rt >= tail_start
                    && tail_rt < tail_end_exclusive
                    && !matches!(rel_kind, ReleaseKind::Miss | ReleaseKind::None))
                {
                    return None;
                }
                prev_same_col_idx.and_then(|prev_idx| {
                    let prev_ho = map.hit_objects.get(prev_idx)?;
                    if !prev_ho.is_long_note() {
                        return None;
                    }
                    let prev_end = prev_ho.end_time.unwrap_or(prev_ho.time);
                    if prev_end > ho.time {
                        return None;
                    }
                    let prev_press = j_by_idx
                        .get(prev_idx)
                        .and_then(|pos| *pos)
                        .and_then(|pos| judgments.get(pos))
                        .and_then(|jj| jj.press_time)?;
                    if prev_press >= ho.time {
                        return None;
                    }
                    let hidden_release = events
                        .iter()
                        .rev()
                        .find(|ev| !ev.pressed && ev.time < tail_only_pt && ev.time > ho.time)
                        .map(|ev| ev.time)?;
                    let hidden_press = events
                        .iter()
                        .rev()
                        .find(|ev| ev.pressed && ev.time < hidden_release)
                        .map(|ev| ev.time)?;
                    Some(
                        hidden_press >= prev_press
                            && hidden_press <= prev_end
                            && hidden_release > ho.time + w.hit50
                            && hidden_release >= early_release_cutoff
                            && hidden_release < tail_only_pt
                            && tail_only_pt - hidden_release <= w.hit50,
                    )
                })
            })
            .unwrap_or(false)
    } else {
        false
    };
    if hless_body_auto_miss {
        rel_time = None;
        rel_kind = ReleaseKind::Miss;
        end_diff = 0;
        force_kind = false;
    }
    let alt_rescue_tail = if true
        && !head_was_hit
        && alt_head_press_time.is_some()
        && has_early_rel
        && repr_after_rel
        && !repr_hit_tail
        && matches!(rel_kind, ReleaseKind::Hit50 | ReleaseKind::Miss)
        && rel_time
            .zip(rel_after_repr)
            .map(|(rt, rr)| rt == rr)
            .unwrap_or(false)
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| {
                rp >= ho.time
                    || rr < ho.time
                    || (ln_duration > w.hit100
                        && rp >= ho.time - w.hit50
                        && rp < ho.time - w.max
                        && rr >= ho.time
                        && rr < end_time - w.hit50)
            })
            .unwrap_or(false)
    {
        rel_time.and_then(|current_rel_time| {
            let next_same_col_pt = next_same_col_idx.and_then(|next_idx| {
                let next_ho = map.hit_objects.get(next_idx)?;
                let next_press_time = judgments
                    .iter()
                    .find(|jj| jj.index == next_idx && jj.column == ho.column)
                    .and_then(|jj| jj.press_time)?;
                let next_head_start = next_ho.time - w.hit50;
                let next_head_win_end = next_ho.time + w.hit100;
                (next_press_time >= next_head_start && next_press_time < next_head_win_end)
                    .then_some(next_press_time)
            });
            let next_head_start = next_same_col_time
                .map(|t| t - w.hit50)
                .unwrap_or(tail_end_exclusive);
            let metadata_boundary = next_same_col_pt
                .unwrap_or(next_head_start)
                .min(tail_end_exclusive);
            if metadata_boundary <= current_rel_time {
                return None;
            }
            let mut active_press_time: Option<i32> = None;
            let mut last_rel_pre_bound: Option<(i32, i32)> = None;
            for ev in events.iter() {
                if ev.time <= current_rel_time {
                    continue;
                }
                if ev.time >= metadata_boundary {
                    break;
                }
                if ev.pressed {
                    active_press_time = Some(ev.time);
                } else if let Some(active_press) = active_press_time {
                    last_rel_pre_bound = Some((active_press, ev.time));
                    active_press_time = None;
                }
            }
            last_rel_pre_bound.filter(|(late_press_time, _)| {
                *late_press_time >= ho.time && *late_press_time <= end_time
            })
        })
    } else {
        None
    }
    .filter(|(late_press_time, _)| {
        !(true
            && !head_was_hit
            && ln_duration <= w.hit100
            && next_same_col_idx
                .and_then(|next_idx| {
                    let next_ho = map.hit_objects.get(next_idx)?;
                    if next_ho.column != ho.column || next_ho.is_long_note() {
                        return None;
                    }
                    let next_judgment = judgments
                        .iter()
                        .find(|jj| jj.index == next_idx && jj.column == ho.column)?;
                    let next_press_time = next_judgment.press_time?;
                    if next_judgment.kind != JudgmentKind::Miss || next_press_time >= next_ho.time {
                        return None;
                    }
                    find_repl_pt(judgments, map, events, next_idx, next_press_time, w)
                })
                .map(|replacement_pt| replacement_pt == *late_press_time)
                .unwrap_or(false))
    });
    if let Some((late_press_time, late_rel_time)) = alt_rescue_tail {
        rel_time = Some(late_rel_time);
        end_diff = (late_rel_time - end_time).abs();
        let miss_meta_prom_pair = matches!(rel_kind, ReleaseKind::Miss)
            && rel_after_repr.map(|rt| rt < tail_start).unwrap_or(false)
            && late_rel_time < tail_start;
        let near_prehead_promotes = true
            && !head_was_hit
            && alt_head_press_time.is_some()
            && ln_duration > w.hit100
            && first_repr_after_rel
                .zip(rel_after_repr)
                .map(|(rp, rr)| {
                    rp >= ho.time - w.hit50
                        && rp < ho.time - w.max
                        && rr >= ho.time
                        && rr < end_time - w.hit50
                })
                .unwrap_or(false)
            && late_rel_time >= tail_start
            && late_rel_time < tail_end_exclusive;
        if miss_meta_prom_pair || near_prehead_promotes {
            first_repr_after_rel = Some(late_press_time);
            rel_after_repr = Some(late_rel_time);
        }
    }
    let zero_head_follows_ln = if true
        && head_was_hit
        && press_time.map(|pt| pt < ho.time).unwrap_or(false)
        && ln_duration <= w.hit50 + w.max
        && imm_rel_at_press
            .zip(first_early_rel)
            .map(|(immediate_rel_time, first_rel_time)| immediate_rel_time == first_rel_time)
            .unwrap_or(false)
        && has_early_rel
        && repr_after_rel
        && !repr_hit_tail
    {
        imm_rel_at_press
            .zip(rel_time)
            .zip(next_same_col_idx)
            .map(|((immediate_rel_time, current_rel_time), next_idx)| {
                let Some(next_ho) = map.hit_objects.get(next_idx) else {
                    return false;
                };
                if next_ho.column != ho.column || !next_ho.is_long_note() {
                    return false;
                }
                if current_rel_time < next_ho.time {
                    return false;
                }
                let next_window_start = next_ho.time - w.hit50;
                let next_win_end = next_ho.time + w.hit100;
                let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                let next_tail_start =
                    next_end_time - ((w.hit50 as f32) * tail_window_scale).round() as i32;
                let next_tail_end =
                    next_end_time + ((w.hit100 as f32) * tail_window_scale).round() as i32;
                let next_press_time = judgments
                    .iter()
                    .find(|jj| jj.index == next_idx && jj.column == ho.column)
                    .and_then(|jj| {
                        jj.press_time
                            .filter(|pt| *pt >= next_window_start && *pt < next_win_end)
                            .or_else(|| {
                                jj.early_press_idx.filter(|tail_only_pt| {
                                    jj.kind == JudgmentKind::Miss
                                        && jj.early_pen_win.is_none()
                                        && jj
                                            .press_time
                                            .map(|head_pt| {
                                                head_pt < next_ho.time && head_pt < *tail_only_pt
                                            })
                                            .unwrap_or(false)
                                        && *tail_only_pt >= next_window_start
                                        && *tail_only_pt < next_win_end
                                })
                            })
                    });
                let next_rel_time = next_press_time.and_then(|pt| {
                    events
                        .iter()
                        .find(|ev| !ev.pressed && ev.time > pt)
                        .map(|ev| ev.time)
                });
                next_press_time
                    .zip(next_rel_time)
                    .map(|(pt, rt)| {
                        pt > immediate_rel_time
                            && pt >= next_window_start
                            && pt < next_win_end
                            && rt >= next_tail_start
                            && rt < next_tail_end
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    } else {
        false
    };
    if zero_head_follows_ln {
        if let Some(immediate_rel_time) = imm_rel_at_press {
            rel_time = Some(immediate_rel_time);
            end_diff = (immediate_rel_time - end_time).abs();
            rel_kind = calc_rel_kind(end_diff, w, tail_window_scale);
            force_kind = false;
            has_early_rel = false;
            first_early_rel = None;
            repr_after_rel = false;
            first_repr_after_rel = None;
            last_repr_time = None;
            first_free_repr = None;
            rel_after_repr = None;
            rescue_rel_near_end = None;
            last_repr_free = false;
            repr_hit_tail = false;
            _zero_head_to_ln = true;
        }
    }
    let same_ms_keep_rel_now = if true
        && head_was_hit
        && press_time.is_some()
        && tail_only_pt.is_none()
        && !matches!(rel_kind, ReleaseKind::Miss | ReleaseKind::None)
    {
        let classic_same_ms_rel = if !has_early_rel && !repr_after_rel {
            press_time
                .and_then(|pt| {
                    events
                        .iter()
                        .find(|ev| !ev.pressed && ev.time == pt)
                        .map(|ev| ev.time)
                })
                .zip(rel_time)
                .zip(next_same_col_idx)
                .and_then(|((immediate_rel_time, current_rel_time), next_idx)| {
                    let next_ho = map.hit_objects.get(next_idx)?;
                    let next_press_time = judgments
                        .iter()
                        .find(|jj| jj.index == next_idx && jj.column == ho.column)
                        .and_then(|jj| {
                            jj.press_time.or_else(|| {
                                jj.early_press_idx.filter(|tail_only_pt| {
                                    jj.kind == JudgmentKind::Miss
                                        && jj.early_pen_win.is_none()
                                        && jj
                                            .press_time
                                            .map(|head_pt| {
                                                head_pt < next_ho.time && head_pt < *tail_only_pt
                                            })
                                            .unwrap_or(false)
                                })
                            })
                        })?;
                    (next_press_time > immediate_rel_time && next_press_time < current_rel_time)
                        .then_some((immediate_rel_time, None, false))
                })
        } else {
            None
        };
        let prh_same_ms_frag_rel = if classic_same_ms_rel.is_none()
            && press_time.map(|pt| pt < ho.time).unwrap_or(false)
            && ln_duration <= w.hit50 + w.max
            && imm_rel_at_press
                .zip(first_early_rel)
                .map(|(immediate_rel_time, first_rel_time)| immediate_rel_time == first_rel_time)
                .unwrap_or(false)
            && has_early_rel
            && repr_after_rel
            && !repr_hit_tail
        {
            imm_rel_at_press
                .zip(rel_time)
                .zip(first_repr_after_rel)
                .zip(next_same_col_idx)
                .and_then(
                    |(((immediate_rel_time, current_rel_time), followup_press_time), next_idx)| {
                        let head_window_start = ho.time - w.hit50;
                        let head_win_end = ho.time + w.hit100;
                        let next_ho = map.hit_objects.get(next_idx)?;
                        let next_press_time = judgments
                            .iter()
                            .find(|jj| jj.index == next_idx && jj.column == ho.column)
                            .and_then(|jj| {
                                jj.press_time.or_else(|| {
                                    jj.early_press_idx.filter(|tail_only_pt| {
                                        jj.kind == JudgmentKind::Miss
                                            && jj.early_pen_win.is_none()
                                            && jj
                                                .press_time
                                                .map(|head_pt| {
                                                    head_pt < next_ho.time
                                                        && head_pt < *tail_only_pt
                                                })
                                                .unwrap_or(false)
                                    })
                                })
                            })?;
                        let fol_pt_unclaimed = !judgments.iter().any(|jj| {
                            jj.index != idx
                                && jj.column == ho.column
                                && jj.press_time == Some(followup_press_time)
                        });
                        (current_rel_time > immediate_rel_time
                            && current_rel_time <= immediate_rel_time + 1
                            && followup_press_time > current_rel_time
                            && followup_press_time >= head_window_start
                            && followup_press_time < head_win_end
                            && followup_press_time <= end_time
                            && followup_press_time < next_press_time
                            && fol_pt_unclaimed)
                            .then_some((immediate_rel_time, Some(followup_press_time), true))
                    },
                )
        } else {
            None
        };
        classic_same_ms_rel.or(prh_same_ms_frag_rel)
    } else {
        None
    };
    if let Some((immediate_rel_time, followup_press_time, clear_fragment_state)) =
        same_ms_keep_rel_now
    {
        rel_time = Some(immediate_rel_time);
        end_diff = (immediate_rel_time - end_time).abs();
        rel_kind = calc_rel_kind(end_diff, w, tail_window_scale);
        force_kind = false;
        if let Some(followup_press_time) = followup_press_time {
            alt_head_press_time = Some(followup_press_time);
        }
        if clear_fragment_state {
            has_early_rel = false;
            first_early_rel = None;
            repr_after_rel = false;
            first_repr_after_rel = None;
            last_repr_time = None;
            first_free_repr = None;
            rel_after_repr = None;
            rescue_rel_near_end = None;
            last_repr_free = false;
            repr_hit_tail = false;
        }
        _same_ms_keep_rel = true;
    }
    let same_ms_next_rel = if true
        && head_was_hit
        && press_time.is_some()
        && tail_only_pt.is_none()
        && !has_early_rel
        && !repr_after_rel
        && !matches!(rel_kind, ReleaseKind::Miss | ReleaseKind::None)
        && ln_duration <= w.hit50 + w.max
    {
        rel_time
            .zip(next_same_col_idx)
            .and_then(|(current_rel_time, next_idx)| {
                let next_ho = map.hit_objects.get(next_idx)?;
                let next_press_time = judgments
                    .iter()
                    .find(|jj| jj.index == next_idx && jj.column == ho.column)
                    .and_then(|jj| jj.press_time)?;
                let next_head_start = next_ho.time - w.hit50;
                let next_head_win_end = next_ho.time + w.hit100;
                if next_press_time < next_head_start
                    || next_press_time >= next_head_win_end
                    || next_press_time != current_rel_time
                {
                    return None;
                }
                events
                    .iter()
                    .find(|ev| {
                        !ev.pressed
                            && ev.time > current_rel_time
                            && ev.time <= current_rel_time + 1
                            && ev.time < next_ho.time
                    })
                    .map(|ev| ev.time)
            })
            .filter(|later_rel_time| {
                calc_rel_kind((later_rel_time - end_time).abs(), w, tail_window_scale) == rel_kind
            })
    } else {
        None
    };
    if let Some(later_rel_time) = same_ms_next_rel {
        rel_time = Some(later_rel_time);
        end_diff = (later_rel_time - end_time).abs();
        _same_ms_pref_rel = true;
    }
    let tai_exac_next_ln_now = if true
        && !head_was_hit
        && press_time.is_none()
        && ln_duration <= w.hit100
        && !has_early_rel
        && !repr_after_rel
    {
        tail_only_pt
            .zip(rel_time)
            .zip(next_same_col_idx)
            .and_then(|((tail_only_pt, current_rel_time), next_idx)| {
                let next_ho = map.hit_objects.get(next_idx)?;
                if !next_ho.is_long_note() {
                    return None;
                }
                let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                let next_ln_duration = next_end_time - next_ho.time;
                let next2_same_col_time = map.hit_objects[(next_idx + 1)..]
                    .iter()
                    .find(|next_next_ho| next_next_ho.column == ho.column)
                    .map(|next_next_ho| next_next_ho.time);
                let tai_onl_kin_for_next = calc_hit_kind((tail_only_pt - next_ho.time).abs(), w);
                let rel_kind_for_next = calc_rel_kind(
                    (current_rel_time - next_end_time).abs(),
                    w,
                    tail_window_scale,
                );
                Some(
                    next_ln_duration <= w.hit100
                        && tail_only_pt >= next_ho.time
                        && tail_only_pt <= next_ho.time + w.max
                        && matches!(
                            tai_onl_kin_for_next,
                            JudgmentKind::Max | JudgmentKind::Hit300
                        )
                        && !matches!(rel_kind_for_next, ReleaseKind::Miss | ReleaseKind::None)
                        && current_rel_time > tail_only_pt
                        && current_rel_time >= next_end_time
                        && next2_same_col_time
                            .map(|next_next_time| current_rel_time < next_next_time)
                            .unwrap_or(true)
                        && (current_rel_time - next_end_time).abs()
                            < (current_rel_time - end_time).abs(),
                )
            })
            .unwrap_or(false)
    } else {
        false
    };
    if tai_exac_next_ln_now {
        rel_kind = ReleaseKind::Miss;
        rel_time = None;
        end_diff = 0;
        force_kind = false;
        _tail_exact_next_ln = true;
    }
    let start_diff = press_time.map(|pt| (pt - ho.time).abs()).unwrap_or(0);
    let _miss_with_repress_before_tail = rel_kind == ReleaseKind::Miss
        && has_early_rel
        && repr_after_rel
        && first_repr_after_rel
            .map(|rp| rp <= tail_start)
            .unwrap_or(false)
        && rel_after_repr.map(|rt| rt < tail_start).unwrap_or(false)
        && !head_miss_pre_meta;
    let _alt_head_near_tail_start_rescue = false
        && alt_head_press_time.is_some()
        && has_early_rel
        && repr_after_rel
        && ln_duration > w.hit50 + w.hit100 + w.max
        && first_repr_after_rel
            .map(|rp| rp > tail_start && rp <= tail_start + w.max)
            .unwrap_or(false);
    if !head_was_hit
        && alt_head_prehold
        && ln_duration <= w.hit100
        && !has_early_rel
        && press_time
            .zip(tail_eval_press_time)
            .map(|(head_pt, tail_pt)| head_pt < ho.time && tail_pt > ho.time && tail_pt <= end_time)
            .unwrap_or(false)
        && rel_time
            .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
            .unwrap_or(false)
        && matches!(
            rel_kind,
            ReleaseKind::Max | ReleaseKind::Hit300 | ReleaseKind::Hit200 | ReleaseKind::Hit100
        )
    {
        rel_kind = ReleaseKind::Hit50;
        force_kind = false;
        _prehead_caps_h50 = true;
    }
    let _total_diff = start_diff + end_diff;
    let _held_until_end = press_time.is_some() && !has_early_rel;
    state.early.has_rel = has_early_rel;
    state.early.first_rel = first_early_rel;
    state.early.repr_after = repr_after_rel;
    state.early.first_repr = first_repr_after_rel;
    state.early.last_repr = last_repr_time;
    state.early.first_free_repr = first_free_repr;
    state.early.rel_after_repr = rel_after_repr;
    state.rescue.near_end_rel = rescue_rel_near_end;
    state.early.last_repr_free = last_repr_free;
    state.rescue.late_headless = late_headless_rescue;
    state.pick.kind = rel_kind;
    state.pick.time = rel_time;
    state.pick.diff = end_diff;
    state.pick.force = force_kind;
    state.early.hit_tail = repr_hit_tail;
    state.rescue.miss_pre_meta = head_miss_pre_meta;
    state.rescue.miss_next_ln_claim = hea_mis_next_ln_clai;
    state.rescue.alt_head_pt = alt_head_press_time;
    state.rescue.alt_prehold = alt_head_prehold;
    state.rescue.alt_cross_hold = alt_head_cross_hold;
    false
}
