use super::super::claims::{find_repl_pt, reclaim_pt_conflict};
use super::note::{ReleaseNoteCtx, ReleaseState};
use super::support::{calc_rel_kind, next_ln_keeps};
use crate::modes::mania::judgment::{
    calc_hit_kind, seg_hits_win, steals_next_ln_head, steals_next_tap_head, InternalJudgment,
    ReleaseKind,
};
use crate::types::{Beatmap, JudgmentKind, Windows};
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve(
    ctx: &ReleaseNoteCtx<'_>,
    state: &mut ReleaseState,
    map: &Beatmap,
    judgments: &mut [InternalJudgment],
    w: &Windows,
) {
    let idx = ctx.idx;
    let ho = ctx.ho;
    let end_time = ctx.end_time;
    let ln_duration = ctx.ln_duration;
    let tail_window_scale = ctx.tail_window_scale;
    let tail_start = ctx.tail_start;
    let late_repr_guard = ctx.late_repr_guard;
    let _early_release_cutoff = ctx.early_release_cutoff;
    let tail_end_exclusive = ctx.tail_end_exclusive;
    let press_time = ctx.press_time;
    let tail_only_pt = ctx.tail_only_pt;
    let tail_eval_press_time = ctx.tail_eval_press_time;
    let head_was_hit = ctx.head_was_hit;
    let head_is_h100 = ctx.head_is_h100;
    let _head_judgment_is_hit50 = ctx.head_is_h50;
    let strong_head_hit = ctx.strong_head_hit;
    let post_end_hless = ctx.post_end_hless;
    let _prev_same_col_idx = ctx.prev_same_col_idx;
    let _prev_same_col_ho = ctx.prev_same_col_ho;
    let _prev_same_col_is_ln = ctx.prev_same_col_is_ln;
    let _prev_same_col_time = ctx.prev_same_col_time;
    let _prev_same_col_end_time = ctx.prev_same_end;
    let next_same_col_idx = ctx.next_same_col_idx;
    let next_same_col_time = ctx.next_same_col_time;
    let last_note_idx_overall = ctx.last_note_idx_overall;
    let extreme_ln_ends = ctx.extreme_ln_ends;
    let events = ctx.events;
    let segments = state.segs.list.as_slice();
    let has_early_rel = state.early.has_rel;
    let first_early_rel = state.early.first_rel;
    let repr_after_rel = state.early.repr_after;
    let first_repr_after_rel = state.early.first_repr;
    let last_repr_time = state.early.last_repr;
    let first_free_repr = state.early.first_free_repr;
    let rel_after_repr = state.early.rel_after_repr;
    let rescue_rel_near_end = state.rescue.near_end_rel;
    let last_repr_free = state.early.last_repr_free;
    let _imm_rel_at_press = state.rescue.imm_rel_at_press;
    let late_headless_rescue = state.rescue.late_headless;
    let tail_pref_body = state.prefs.body;
    let tail_pref_bridge = state.prefs.bridge;
    let tail_pref_early = state.prefs.early;
    let tail_pref_pre_frag = state.prefs.pre_frag;
    let tail_pref_exact = state.prefs.exact;
    let _init_first_repress_post_early_rel = state.rescue.init_first_repr;
    let _init_rel_after_repr = state.rescue.init_rel_after_repr;
    let _short_miss_bridge = state.rescue.short_miss_bridge;
    let first_rel_after_press = tail_eval_press_time.and_then(|pt| {
        events
            .iter()
            .find(|e| e.time > pt && !e.pressed)
            .map(|e| e.time)
    });
    let mut rel_kind = ReleaseKind::Miss;
    let mut rel_time: Option<i32> = None;
    let mut end_diff = 0;
    let mut force_kind = false;
    let mut repr_hit_tail = false;
    let late_repr_dur = (w.hit50 + w.hit100 + w.max).max(w.hit50 * 2 + 1);
    let repr_claim_next_ln = |rp: i32| {
        if true {
            return false;
        }
        judgments
            .iter()
            .filter(|jj| {
                jj.index > idx
                    && jj.column == ho.column
                    && jj.kind == JudgmentKind::Miss
                    && jj.press_time == Some(rp)
            })
            .any(|claimed_judgment| {
                let Some(claimed_ho) = map.hit_objects.get(claimed_judgment.index) else {
                    return false;
                };
                if !claimed_ho.is_long_note() {
                    return false;
                }
                let next_head_start = claimed_ho.time - w.hit50;
                let nex_hea_win_end_incl = claimed_ho.time + w.hit100;
                events.iter().any(|ev| {
                    ev.pressed
                        && ev.time > rp
                        && ev.time >= next_head_start
                        && ev.time <= nex_hea_win_end_incl
                        && !judgments.iter().any(|jj| {
                            jj.index != claimed_judgment.index
                                && jj.column == ho.column
                                && jj.press_time == Some(ev.time)
                        })
                })
            })
    };
    let repr_claim_next_weak = |rp: i32| {
        if true {
            return false;
        }
        next_same_col_idx
            .and_then(|next_idx| {
                let next_judgment = judgments.iter().find(|jj| {
                    jj.index == next_idx && jj.column == ho.column && jj.press_time == Some(rp)
                })?;
                let next_ho = map.hit_objects.get(next_idx)?;
                if next_ho.is_long_note()
                    || !matches!(next_judgment.kind, JudgmentKind::Hit50 | JudgmentKind::Miss)
                {
                    return None;
                }
                Some((next_idx, next_ho))
            })
            .map(|(next_idx, next_ho)| {
                let next_head_start = next_ho.time - w.hit50;
                let next_head_win_end = next_ho.time + w.hit100;
                let has_next_tap_follow = events.iter().any(|ev| {
                    ev.pressed
                        && ev.time > rp
                        && ev.time >= next_head_start
                        && ev.time < next_head_win_end
                        && !judgments.iter().any(|jj| {
                            jj.index != next_idx
                                && jj.column == ho.column
                                && jj.press_time == Some(ev.time)
                        })
                });
                rp >= next_head_start
                    && rp < next_head_win_end
                    && rp <= next_ho.time + w.max
                    && has_next_tap_follow
            })
            .unwrap_or(false)
    };
    let repr_claim_next_tap = |rp: i32| {
        if true {
            return false;
        }
        next_same_col_idx
            .and_then(|next_idx| {
                let next_judgment = judgments.iter().find(|jj| {
                    jj.index == next_idx && jj.column == ho.column && jj.press_time == Some(rp)
                })?;
                let next_ho = map.hit_objects.get(next_idx)?;
                if next_ho.is_long_note() || next_judgment.kind != JudgmentKind::Miss {
                    return None;
                }
                let next_head_start = next_ho.time - w.hit50;
                find_repl_pt(judgments, map, events, next_idx, rp, w).map(|replacement_press| {
                    rp >= tail_start && rp < next_head_start && replacement_press > rp
                })
            })
            .unwrap_or(false)
    };
    let tail_hold_hit = true
        && !head_was_hit
        && press_time.is_some()
        && !has_early_rel
        && segments.iter().any(|(seg_start, seg_end)| {
            seg_hits_win(*seg_start, *seg_end, tail_start, tail_end_exclusive)
        });
    let miss_press_rel_tail = false
        && !head_was_hit
        && press_time.is_some()
        && ((!has_early_rel
            && first_rel_after_press
                .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
                .unwrap_or(false))
            || (has_early_rel
                && !repr_after_rel
                && press_time.map(|pt| pt < ho.time).unwrap_or(false)
                && first_early_rel
                    .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
                    .unwrap_or(false)
                && first_rel_after_press
                    .zip(first_early_rel)
                    .map(|(rt, first_rt)| rt == first_rt)
                    .unwrap_or(false)))
        && segments.iter().any(|(seg_start, seg_end)| {
            seg_hits_win(*seg_start, *seg_end, tail_start, tail_end_exclusive)
        });
    let hit_pref_repr = false
        && head_was_hit
        && has_early_rel
        && ln_duration > late_repr_dur
        && first_early_rel
            .map(|t| t >= tail_start && t <= tail_start + w.hit200)
            .unwrap_or(false)
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| {
                let repress_unassigned = !judgments.iter().any(|jj| {
                    jj.index != idx && jj.column == ho.column && jj.press_time == Some(rp)
                });
                let repr_tail_ok = repress_unassigned
                    || repr_claim_next_ln(rp)
                    || repr_claim_next_weak(rp)
                    || repr_claim_next_tap(rp);
                repr_tail_ok
                    && rp >= ho.time
                    && rp <= end_time
                    && rr >= tail_start
                    && rr < tail_end_exclusive
            })
            .unwrap_or(false);
    let short_guard_repr = false
        && head_was_hit
        && has_early_rel
        && ln_duration <= late_repr_dur
        && first_early_rel
            .map(|t| t >= tail_start && t <= tail_start + w.hit200 && t < late_repr_guard)
            .unwrap_or(false)
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| {
                let repress_unassigned = !judgments.iter().any(|jj| {
                    jj.index != idx && jj.column == ho.column && jj.press_time == Some(rp)
                });
                let repr_tail_ok = repress_unassigned
                    || repr_claim_next_ln(rp)
                    || repr_claim_next_weak(rp)
                    || repr_claim_next_tap(rp);
                repr_tail_ok
                    && first_early_rel.map(|t| rp >= t).unwrap_or(false)
                    && rp <= late_repr_guard
                    && rr >= tail_start
                    && rr < tail_end_exclusive
            })
            .unwrap_or(false);
    let near_tail_pref_repr = false
        && head_was_hit
        && has_early_rel
        && first_early_rel
            .map(|t| t <= tail_start && t >= tail_start - w.max)
            .unwrap_or(false)
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| {
                let repress_unassigned = !judgments.iter().any(|jj| {
                    jj.index != idx && jj.column == ho.column && jj.press_time == Some(rp)
                });
                let repr_tail_ok = repress_unassigned
                    || repr_claim_next_ln(rp)
                    || repr_claim_next_weak(rp)
                    || repr_claim_next_tap(rp);
                repr_tail_ok
                    && first_early_rel.map(|t| rp >= t).unwrap_or(false)
                    && rp >= ho.time
                    && rp <= end_time
                    && rr >= tail_start
                    && rr < tail_end_exclusive
            })
            .unwrap_or(false);
    let near_tail_claims_repr = false
        && head_was_hit
        && has_early_rel
        && first_early_rel
            .map(|t| t >= tail_start - w.max && t <= tail_start + w.max)
            .unwrap_or(false)
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| {
                repr_claim_next_ln(rp)
                    && rp > end_time
                    && rp <= end_time + w.hit100
                    && rr > end_time
                    && rr < tail_end_exclusive
            })
            .unwrap_or(false);
    let long_hit_tail_break = false
        && head_was_hit
        && strong_head_hit
        && has_early_rel
        && ln_duration > late_repr_dur
        && first_early_rel
            .map(|first_rt| {
                first_rt >= tail_start
                    && first_rt < end_time
                    && first_repr_after_rel
                        .zip(rel_after_repr)
                        .map(|(rp, rr)| {
                            let repress_unassigned = !judgments.iter().any(|jj| {
                                jj.index != idx
                                    && jj.column == ho.column
                                    && jj.press_time == Some(rp)
                            });
                            let repr_tail_ok = repress_unassigned
                                || repr_claim_next_ln(rp)
                                || repr_claim_next_weak(rp)
                                || repr_claim_next_tap(rp);
                            repr_tail_ok
                                && rp > end_time
                                && rp <= end_time + w.max
                                && rr > end_time
                                && rr > first_rt
                                && rr < tail_end_exclusive
                                && next_same_col_time.map(|next_t| rr < next_t).unwrap_or(true)
                        })
                        .unwrap_or(false)
            })
            .unwrap_or(false);
    let long_hit_pre_break = false
        && head_was_hit
        && strong_head_hit
        && has_early_rel
        && ln_duration > late_repr_dur
        && first_early_rel
            .map(|first_rt| {
                first_rt >= tail_start - w.max
                    && first_rt < tail_start
                    && first_repr_after_rel
                        .zip(rel_after_repr)
                        .map(|(rp, rr)| {
                            let repress_unassigned = !judgments.iter().any(|jj| {
                                jj.index != idx
                                    && jj.column == ho.column
                                    && jj.press_time == Some(rp)
                            });
                            let repr_tail_ok = repress_unassigned
                                || repr_claim_next_ln(rp)
                                || repr_claim_next_weak(rp)
                                || repr_claim_next_tap(rp);
                            repr_tail_ok
                                && rp > end_time
                                && rp <= end_time + w.hit100
                                && rr > end_time
                                && rr > first_rt
                                && rr < tail_end_exclusive
                                && next_same_col_time.map(|next_t| rr < next_t).unwrap_or(true)
                        })
                        .unwrap_or(false)
            })
            .unwrap_or(false);
    let long_hit_bridge = tail_pref_body
        || (false
            && head_was_hit
            && strong_head_hit
            && has_early_rel
            && ln_duration > late_repr_dur
            && first_early_rel
                .map(|t| t <= tail_start - w.hit50)
                .unwrap_or(false)
            && first_repr_after_rel
                .zip(rel_after_repr)
                .map(|(rp, rr)| {
                    rp >= tail_start && rp <= end_time && rr > end_time && rr < tail_end_exclusive
                })
                .unwrap_or(false));
    let tail_pref_post_repr = hit_pref_repr
        || short_guard_repr
        || near_tail_pref_repr
        || near_tail_claims_repr
        || long_hit_tail_break
        || long_hit_pre_break
        || tail_pref_bridge
        || tail_pref_early
        || tail_pref_pre_frag;
    let tail_pref_post_repr = tail_pref_post_repr || long_hit_bridge;
    let short_miss_next_ln = false
        && !head_was_hit
        && has_early_rel
        && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
        && ln_duration <= w.hit100
        && first_repr_after_rel
            .zip(next_same_col_idx)
            .map(|(rp, next_idx)| {
                map.hit_objects
                    .get(next_idx)
                    .map(|next_ho| {
                        let next_ln_duration =
                            next_ho.end_time.unwrap_or(next_ho.time) - next_ho.time;
                        next_ho.column == ho.column
                            && next_ho.is_long_note()
                            && next_ln_duration > w.hit100
                            && rp > end_time
                            && rp < next_ho.time
                            && judgments.iter().any(|jj| {
                                jj.index == next_idx
                                    && jj.column == ho.column
                                    && jj.press_time == Some(rp)
                            })
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false);
    let short_miss_weak_ln = false
        && !head_was_hit
        && has_early_rel
        && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
        && first_early_rel
            .map(|t| t >= ho.time - w.hit100)
            .unwrap_or(false)
        && ln_duration <= w.hit100
        && first_repr_after_rel
            .zip(next_same_col_idx)
            .map(|(rp, next_idx)| {
                map.hit_objects
                    .get(next_idx)
                    .map(|next_ho| {
                        let next_ln_duration =
                            next_ho.end_time.unwrap_or(next_ho.time) - next_ho.time;
                        let next_head_kind = calc_hit_kind((rp - next_ho.time).abs(), w);
                        let gap_from_current_end = rp - end_time;
                        let gap_to_next_head = next_ho.time - rp;
                        next_ho.column == ho.column
                            && next_ho.is_long_note()
                            && next_ln_duration <= w.hit100
                            && !matches!(next_head_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                            && rp > end_time
                            && rp < next_ho.time
                            && gap_from_current_end < gap_to_next_head
                            && judgments.iter().any(|jj| {
                                jj.index == next_idx
                                    && jj.column == ho.column
                                    && jj.press_time == Some(rp)
                            })
                            && find_repl_pt(judgments, map, events, next_idx, rp, w).is_none()
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false);
    let late_body_claim = false
        && !head_was_hit
        && has_early_rel
        && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
        && first_early_rel
            .map(|t| t >= ho.time - w.hit100)
            .unwrap_or(false)
        && ln_duration <= w.hit100
        && first_repr_after_rel
            .zip(rel_after_repr)
            .zip(next_same_col_idx)
            .map(|((rp, rr), next_idx)| {
                map.hit_objects
                    .get(next_idx)
                    .map(|next_ho| {
                        let next_ln_duration =
                            next_ho.end_time.unwrap_or(next_ho.time) - next_ho.time;
                        let next_head_kind = calc_hit_kind((rp - next_ho.time).abs(), w);
                        let gap_from_current_end = end_time - rp;
                        let gap_to_next_head = next_ho.time - rp;
                        next_ho.column == ho.column
                            && next_ho.is_long_note()
                            && next_ln_duration <= w.hit50 + w.max
                            && !matches!(next_head_kind, JudgmentKind::Max | JudgmentKind::Hit300)
                            && rp > ho.time
                            && rp <= end_time
                            && gap_from_current_end >= 0
                            && gap_from_current_end < gap_to_next_head
                            && next_ln_keeps(
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
                    .unwrap_or(false)
            })
            .unwrap_or(false);
    let sho_ln_post_end_clai = short_miss_next_ln || short_miss_weak_ln || late_body_claim;
    let short_miss_extends = false
        && !head_was_hit
        && press_time.map(|pt| pt < ho.time).unwrap_or(false)
        && has_early_rel
        && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| rp > end_time && rr >= tail_start && rr < tail_end_exclusive && rr > rp)
            .unwrap_or(false);
    let miss_repr_limit = if ln_duration <= late_repr_dur {
        let mut upper_bound = end_time + w.max;
        if sho_ln_post_end_clai || short_miss_extends {
            upper_bound = upper_bound.max(end_time + w.hit50);
        }
        upper_bound
    } else {
        end_time
    };
    let hea_mis_rep_tail_rec = false
        && !head_was_hit
        && tail_eval_press_time.is_some()
        && has_early_rel
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| {
                rp > ho.time
                    && rp <= miss_repr_limit
                    && rr >= tail_start
                    && rr < tail_end_exclusive
                    && rr > rp
            })
            .unwrap_or(false);
    let short_miss_exact_repr = false
        && !head_was_hit
        && press_time.is_some()
        && has_early_rel
        && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
        && ln_duration <= w.hit100
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| rp == ho.time && rr >= tail_start && rr < tail_end_exclusive && rr > rp)
            .unwrap_or(false);
    let short_miss_near_repr = false
        && !head_was_hit
        && press_time.is_some()
        && has_early_rel
        && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
        && ln_duration <= w.hit100
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| {
                rp >= ho.time - w.max
                    && rp < ho.time
                    && rr >= tail_start
                    && rr < tail_end_exclusive
                    && rr > rp
            })
            .unwrap_or(false);
    let miss_repr_tail_any = hea_mis_rep_tail_rec || short_miss_exact_repr || short_miss_near_repr;
    let short_miss_limit = end_time + w.hit50 + w.hit100;
    let short_body_miss = false
        && !head_was_hit
        && press_time.is_some()
        && has_early_rel
        && first_early_rel.map(|t| t < ho.time).unwrap_or(false)
        && ln_duration <= w.hit100
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(rp, rr)| {
                rp > ho.time + w.max && rp <= end_time && rr > end_time && rr <= short_miss_limit
            })
            .unwrap_or(false);
    if head_was_hit
        || post_end_hless
        || tail_only_pt.is_some()
        || miss_press_rel_tail
        || tail_hold_hit
        || miss_repr_tail_any
    {
        for (seg_start, seg_end) in segments {
            if has_early_rel && *seg_start > end_time && !miss_repr_tail_any && !tail_pref_post_repr
            {
                continue;
            }
            let post_brea_ovrlp_head = false
                && head_was_hit
                && has_early_rel
                && first_repr_after_rel
                    .map(|rp| *seg_start > rp)
                    .unwrap_or(false)
                && rel_after_repr.map(|rt| rt < tail_start).unwrap_or(false)
                && next_same_col_time
                    .map(|next_t| *seg_start >= next_t - w.hit50)
                    .unwrap_or(false);
            if post_brea_ovrlp_head {
                continue;
            }
            if !seg_hits_win(*seg_start, *seg_end, tail_start, tail_end_exclusive) {
                continue;
            }
            if tail_pref_post_repr || miss_repr_tail_any {
                let seg_before_first_repr = first_repr_after_rel
                    .map(|rp| *seg_start < rp && seg_end.map(|rt| rt <= rp).unwrap_or(false))
                    .unwrap_or(false);
                if seg_before_first_repr {
                    continue;
                }
            }
            let late_repr_short_ln = ln_duration <= late_repr_dur
                && has_early_rel
                && first_early_rel.map(|t| *seg_start > t).unwrap_or(false)
                && *seg_start > late_repr_guard;
            if late_repr_short_ln && !miss_repr_tail_any && !tail_pref_exact {
                continue;
            }
            let segment_open = seg_end.is_none();
            let raw_release = seg_end.unwrap_or(tail_end_exclusive - 1);
            let raw_rel_over = first_rel_after_press.unwrap_or(raw_release);
            let actl_rel_post_segmnt = events
                .iter()
                .find(|e| e.time > *seg_start && !e.pressed)
                .map(|e| e.time);
            let open_seg_late_over = segment_open
                && first_rel_after_press
                    .map(|rt| rt >= tail_end_exclusive)
                    .unwrap_or(false);
            let post_break_open_miss = false
                && head_was_hit
                && has_early_rel
                && first_early_rel.map(|t| *seg_start > t).unwrap_or(false)
                && *seg_start <= end_time
                && segment_open
                && actl_rel_post_segmnt
                    .map(|rt| rt >= tail_end_exclusive)
                    .unwrap_or(false)
                && tail_only_pt.is_none();
            if post_break_open_miss {
                repr_hit_tail = true;
                continue;
            }
            let sho_ln_late_rel_miss = false
                && ln_duration <= w.hit50 + w.hit100
                && !has_early_rel
                && (raw_release >= tail_end_exclusive || open_seg_late_over)
                && tail_only_pt.is_none();
            let headless_to_next_ln = false
                && tail_only_pt == Some(*seg_start)
                && press_time.is_none()
                && !head_was_hit
                && !has_early_rel
                && ln_duration <= w.hit100
                && raw_release >= tail_end_exclusive
                && next_same_col_idx
                    .and_then(|next_idx| {
                        map.hit_objects
                            .get(next_idx)
                            .map(|next_ho| (next_idx, next_ho))
                    })
                    .map(|(_, next_ho)| {
                        if !next_ho.is_long_note() {
                            return false;
                        }
                        let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                        let next_head_start = next_ho.time - w.hit50;
                        let next_head_win_end = next_ho.time + w.hit100;
                        let next_tail_start =
                            next_end_time - ((w.hit50 as f32) * tail_window_scale).round() as i32;
                        let next_tail_end =
                            next_end_time + ((w.hit100 as f32) * tail_window_scale).round() as i32;
                        let ovrlps_next_head_win =
                            *seg_start >= next_head_start && *seg_start < next_head_win_end;
                        ovrlps_next_head_win
                            && events
                                .iter()
                                .filter(|ev| {
                                    ev.pressed
                                        && ev.time > *seg_start
                                        && ev.time <= next_end_time + w.hit100
                                })
                                .any(|next_press| {
                                    events
                                        .iter()
                                        .find(|ev| !ev.pressed && ev.time > next_press.time)
                                        .map(|ev| {
                                            ev.time >= next_tail_start && ev.time < next_tail_end
                                        })
                                        .unwrap_or(false)
                                })
                    })
                    .unwrap_or(false);
            let head_hit_late_miss = false
                && head_was_hit
                && !has_early_rel
                && raw_rel_over >= tail_end_exclusive
                && tail_only_pt.is_none();
            let late_open_prewin = false
                && !has_early_rel
                && segment_open
                && open_seg_late_over
                && first_rel_after_press
                    .zip(next_same_col_time)
                    .map(|(rt, next_t)| rt >= next_t - w.hit50 && rt < next_t + w.hit50)
                    .unwrap_or(false);
            let open_far_tail_miss = false
                && !has_early_rel
                && segment_open
                && open_seg_late_over
                && first_rel_after_press
                    .map(|rt| rt - tail_end_exclusive >= w.max)
                    .unwrap_or(false);
            if sho_ln_late_rel_miss
                || headless_to_next_ln
                || head_hit_late_miss
                || late_open_prewin
                || open_far_tail_miss
            {
                continue;
            }
            let late_rel_auto_h50 = true
                && !has_early_rel
                && head_is_h100
                && ln_duration > w.hit100
                && (raw_rel_over > end_time + w.hit50 || raw_rel_over >= tail_end_exclusive);
            let (effective_release, scoring_release) = if segment_open
                || raw_release >= tail_end_exclusive
            {
                force_kind = true;
                if late_rel_auto_h50 {
                    let clamped = end_time + w.hit50;
                    (clamped, clamped)
                } else if false && tail_only_pt.is_some() && !has_early_rel && open_seg_late_over {
                    (raw_rel_over, tail_end_exclusive - 1)
                } else {
                    let clamped = tail_end_exclusive - 1;
                    (clamped, clamped)
                }
            } else {
                (raw_release, raw_release)
            };
            let first_repress_segment = first_repr_after_rel
                .map(|rp| *seg_start == rp)
                .unwrap_or(false);
            let first_repr_to_next_ln = false
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
            if first_repr_to_next_ln && !sho_ln_post_end_clai {
                continue;
            }
            let fir_repr_to_next_tap = if false
                && !head_was_hit
                && press_time.is_none()
                && tail_only_pt.is_some()
                && first_early_rel.map(|t| *seg_start > t).unwrap_or(false)
                && first_repress_segment
            {
                next_same_col_idx
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
                            .find(|next_next_ho| next_next_ho.column == ho.column)
                            .map(|next_next_ho| (next_ho, next_next_ho))
                    })
                    .map(|(next_tap_ho, next_next_ho)| {
                        if !next_next_ho.is_long_note() {
                            return false;
                        }
                        let next_tap_window_start = next_tap_ho.time - w.hit50;
                        let next_tap_end = next_tap_ho.time + w.hit100;
                        let next_ln_window_start = next_next_ho.time - w.hit50;
                        let next_ln_win_end = next_next_ho.time + w.hit100;
                        let next_ln_end_time = next_next_ho.end_time.unwrap_or(next_next_ho.time);
                        let next_ln_tail_start = next_ln_end_time
                            - ((w.hit50 as f32) * tail_window_scale).round() as i32;
                        let next_ln_tail_end = next_ln_end_time
                            + ((w.hit100 as f32) * tail_window_scale).round() as i32;
                        let next_ln_head_tail = events
                            .iter()
                            .filter(|ev| {
                                ev.pressed
                                    && ev.time > *seg_start
                                    && ev.time >= next_ln_window_start
                                    && ev.time < next_ln_win_end
                            })
                            .any(|next_ln_press| {
                                events
                                    .iter()
                                    .find(|ev| !ev.pressed && ev.time > next_ln_press.time)
                                    .map(|ev| {
                                        ev.time >= next_ln_tail_start && ev.time < next_ln_tail_end
                                    })
                                    .unwrap_or(false)
                            });
                        *seg_start >= next_tap_window_start
                            && *seg_start < next_tap_end
                            && calc_hit_kind((*seg_start - next_tap_ho.time).abs(), w)
                                != JudgmentKind::Miss
                            && effective_release < next_tap_ho.time
                            && next_ln_head_tail
                    })
                    .unwrap_or(false)
            } else {
                false
            };
            if fir_repr_to_next_tap {
                continue;
            }
            if (miss_repr_tail_any || tail_pref_post_repr) && first_repress_segment {
                reclaim_pt_conflict(judgments, map, events, idx, ho.column, *seg_start, w);
            }
            let post_break_steals_ln = first_early_rel.map(|t| *seg_start > t).unwrap_or(false)
                && false
                && steals_next_ln_head(
                    judgments,
                    map,
                    ho.column,
                    next_same_col_idx,
                    *seg_start,
                    effective_release,
                    w,
                    tail_window_scale,
                );
            let pos_bre_stl_next_tap = first_early_rel.map(|t| *seg_start > t).unwrap_or(false)
                && false
                && steals_next_tap_head(
                    judgments,
                    map,
                    events,
                    ho.column,
                    next_same_col_idx,
                    *seg_start,
                    w,
                );
            if post_break_steals_ln || pos_bre_stl_next_tap {
                continue;
            }
            let same_gap_repr = true
                && first_early_rel
                    .zip(first_repr_after_rel)
                    .map(|(early_rel_time, repress_time)| {
                        *seg_start == early_rel_time && repress_time == early_rel_time
                    })
                    .unwrap_or(false);
            if first_early_rel.map(|t| *seg_start > t).unwrap_or(false) || same_gap_repr {
                repr_hit_tail = true;
            }
            rel_time = Some(effective_release);
            end_diff = (scoring_release - end_time).abs();
            rel_kind = calc_rel_kind(end_diff, w, tail_window_scale);
            break;
        }
    }
    let pre_frag_keep_rel = false
        && !head_was_hit
        && rel_time.is_none()
        && press_time.map(|pt| pt < ho.time).unwrap_or(false)
        && has_early_rel
        && first_early_rel
            .map(|rt| rt >= tail_start && rt < ho.time)
            .unwrap_or(false)
        && first_repr_after_rel
            .zip(rel_after_repr)
            .map(|(_, rr)| rr >= tail_end_exclusive)
            .unwrap_or(false);
    if pre_frag_keep_rel {
        rel_time = first_early_rel;
        end_diff = first_early_rel.map(|rt| (rt - end_time).abs()).unwrap_or(0);
        rel_kind = ReleaseKind::Hit50;
        force_kind = false;
    }
    let pre_tail_frag_h50 = false
        && !head_was_hit
        && press_time.is_none()
        && !has_early_rel
        && ln_duration >= w.hit50 + w.max
        && tail_only_pt
            .map(|pt| {
                pt > ho.time + w.hit50
                    && pt < tail_end_exclusive
                    && events
                        .iter()
                        .any(|ev| !ev.pressed && ev.time > ho.time + w.hit50 && ev.time < pt)
            })
            .unwrap_or(false)
        && rel_time
            .map(|rt| rt >= tail_start && rt < tail_end_exclusive)
            .unwrap_or(false)
        && !matches!(rel_kind, ReleaseKind::Miss | ReleaseKind::None);
    if pre_tail_frag_h50 {
        rel_kind = ReleaseKind::Hit50;
        force_kind = false;
    }
    let term_long_stays_miss = false
        && head_was_hit
        && !has_early_rel
        && extreme_ln_ends.contains(&end_time)
        && rel_time
            .map(|rt| rt > end_time && rt <= end_time + w.max)
            .unwrap_or(false)
        && last_note_idx_overall
            .and_then(|last_idx| {
                map.hit_objects
                    .get(last_idx)
                    .map(|last_ho| (last_idx, last_ho))
            })
            .map(|(last_idx, last_ho)| {
                last_idx > idx && !last_ho.is_long_note() && last_ho.time == end_time
            })
            .unwrap_or(false);
    if term_long_stays_miss {
        rel_time = None;
        rel_kind = ReleaseKind::Miss;
        end_diff = 0;
        force_kind = false;
    }
    state.early.first_rel = first_early_rel;
    state.early.first_repr = first_repr_after_rel;
    state.early.last_repr = last_repr_time;
    state.early.first_free_repr = first_free_repr;
    state.early.rel_after_repr = rel_after_repr;
    state.rescue.near_end_rel = rescue_rel_near_end;
    state.early.last_repr_free = last_repr_free;
    state.rescue.late_headless = late_headless_rescue;
    state.prefs.body = tail_pref_body;
    state.prefs.bridge = tail_pref_bridge;
    state.prefs.early = tail_pref_early;
    state.prefs.pre_frag = tail_pref_pre_frag;
    state.prefs.exact = tail_pref_exact;
    state.rescue.first_rel_after_press = first_rel_after_press;
    state.pick.kind = rel_kind;
    state.pick.time = rel_time;
    state.pick.diff = end_diff;
    state.pick.force = force_kind;
    state.early.hit_tail = repr_hit_tail;
    state.rescue.late_repr_dur = late_repr_dur;
    state.rescue.miss_press_tail = miss_press_rel_tail;
    state.rescue.tail_hold_hit = tail_hold_hit;
    state.rescue.miss_repr_tail = miss_repr_tail_any;
    state.rescue.short_body_miss = short_body_miss;
    state.rescue.pre_frag_keep_rel = pre_frag_keep_rel;
    state.rescue.late_body_claim = late_body_claim;
}
