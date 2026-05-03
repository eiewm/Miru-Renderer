use super::note::{ReleaseNoteCtx, ReleaseState};
use crate::modes::mania::judgment::{
    Authority, InternalJudgment, LnDebugInfo, LnReleaseInfo, ReleaseKind, RuleMeta,
};
use crate::types::Beatmap;
use crate::types::JudgmentKind;
use std::collections::HashMap;
pub(super) fn finalize(
    ctx: &ReleaseNoteCtx<'_>,
    state: &mut ReleaseState,
    map: &Beatmap,
    judgments: &mut [InternalJudgment],
    j_by_idx: &[Option<usize>],
    ln_release_info: &mut HashMap<usize, LnReleaseInfo>,
    ln_debug_info: &mut HashMap<usize, LnDebugInfo>,
) {
    let idx = ctx.idx;
    let ho = ctx.ho;
    let end_time = ctx.end_time;
    let ln_duration = ctx.ln_duration;
    let tail_start = ctx.tail_start;
    let press_time = ctx.press_time;
    let head_was_hit = ctx.head_was_hit;
    let prev_same_end = ctx.prev_same_end;
    let next_same_col_idx = ctx.next_same_col_idx;
    let tail_window_scale = ctx.tail_window_scale;
    let events = ctx.events;
    let w = ctx.windows;
    let mut has_early_rel = state.early.has_rel;
    let mut repr_after_rel = state.early.repr_after;
    let mut first_early_rel = state.early.first_rel;
    let mut first_repr_after_rel = state.early.first_repr;
    let mut last_repr_time = state.early.last_repr;
    let mut first_free_repr = state.early.first_free_repr;
    let mut rel_after_repr = state.early.rel_after_repr;
    let mut rescue_rel_near_end = state.rescue.near_end_rel;
    let mut last_repr_free = state.early.last_repr_free;
    let imm_rel_at_press = state.rescue.imm_rel_at_press;
    let late_headless_rescue = state.rescue.late_headless;
    let rel_kind = state.pick.kind;
    let rel_time = state.pick.time;
    let end_diff = state.pick.diff;
    let force_kind = state.pick.force;
    let mut repr_hit_tail = state.early.hit_tail;
    let alt_head_press_time = state.rescue.alt_head_pt;
    let conflict_first_repr = state.conflict.first_repr;
    let conflc_rel_post_repr = state.conflict.rel_after_repr;
    let conf_first_repr_head = state.conflict.first_repr_head;
    let first_repr_owner_idx = state.conflict.owner_idx;
    let first_repr_owner_time = state.conflict.owner_time;
    let fir_rep_yiel_next_ln = state.conflict.yield_next_ln;
    let head_miss_pre_meta = state.rescue.miss_pre_meta;
    let late_body_claim = state.rescue.late_body_claim;
    let zero_head_to_note = head_was_hit
        && imm_rel_at_press
            .zip(first_early_rel)
            .map(|(immediate_rel_time, first_rel_time)| immediate_rel_time == first_rel_time)
            .unwrap_or(false)
        && has_early_rel
        && repr_after_rel
        && !repr_hit_tail
        && rel_time
            .zip(imm_rel_at_press)
            .map(|(rt, immediate_rel_time)| rt == immediate_rel_time)
            .unwrap_or(false)
        && first_repr_after_rel
            .zip(rel_after_repr)
            .zip(next_same_col_idx)
            .map(|((rp, rr), next_idx)| {
                if rp <= ho.time || rr <= rp {
                    return false;
                }
                let Some(next_ho) = map.hit_objects.get(next_idx) else {
                    return false;
                };
                if !next_ho.is_long_note() {
                    return false;
                }
                let next_head_start = next_ho.time - w.hit50;
                let next_head_win_end = next_ho.time + w.hit100;
                let next_end_time = next_ho.end_time.unwrap_or(next_ho.time);
                let next_tail_start =
                    next_end_time - ((w.hit50 as f32) * tail_window_scale).round() as i32;
                let next_tail_end =
                    next_end_time + ((w.hit100 as f32) * tail_window_scale).round() as i32;
                let fol_is_nex_ln_hea_fra = rp >= next_head_start && rp < next_head_win_end;
                let same_follow_closes = rr >= next_tail_start && rr < next_tail_end;
                let late_head_pair_ln = fol_is_nex_ln_hea_fra
                    && rr < next_ho.time
                    && events
                        .iter()
                        .filter(|ev| {
                            ev.pressed
                                && ev.time > rr
                                && ev.time >= next_head_start
                                && ev.time < next_head_win_end
                        })
                        .any(|next_press| {
                            events
                                .iter()
                                .find(|ev| !ev.pressed && ev.time > next_press.time)
                                .map(|ev| ev.time >= next_tail_start && ev.time < next_tail_end)
                                .unwrap_or(false)
                        });
                fol_is_nex_ln_hea_fra && (same_follow_closes || late_head_pair_ln)
            })
            .unwrap_or(false);
    if zero_head_to_note {
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
    let start_diff = press_time.map(|pt| (pt - ho.time).abs()).unwrap_or(0);
    let total_diff = start_diff + end_diff;
    let held_until_end = press_time.is_some() && !has_early_rel;
    let pre_alt_miss = !head_was_hit
        && alt_head_press_time
            .map(|ap| (ap - ho.time).abs() <= w.hit300)
            .unwrap_or(false)
        && rel_kind == ReleaseKind::Miss
        && rel_time.is_none()
        && press_time
            .zip(prev_same_end)
            .map(|(pt, pe)| pt <= pe)
            .unwrap_or(false)
        && rel_after_repr.map(|rr| rr <= ho.time).unwrap_or(false);
    let mis_wit_rep_pre_tail = rel_kind == ReleaseKind::Miss
        && has_early_rel
        && repr_after_rel
        && !pre_alt_miss
        && first_repr_after_rel
            .map(|rp| rp <= tail_start)
            .unwrap_or(false)
        && rel_after_repr.map(|rt| rt < tail_start).unwrap_or(false)
        && !head_miss_pre_meta;
    let short_hit_repr_meta = rel_kind == ReleaseKind::Miss
        && head_was_hit
        && has_early_rel
        && repr_after_rel
        && ln_duration <= w.hit100
        && first_early_rel.map(|t| t <= ho.time).unwrap_or(false)
        && first_repr_after_rel
            .zip(first_early_rel)
            .map(|(rp, early_rt)| rp > early_rt && rp <= tail_start)
            .unwrap_or(false)
        && rel_after_repr
            .map(|rt| rt > ho.time && rt < end_time)
            .unwrap_or(false);
    let alt_head_tail_res = alt_head_press_time.is_some()
        && has_early_rel
        && repr_after_rel
        && ln_duration > w.hit50 + w.hit100 + w.max
        && first_repr_after_rel
            .map(|rp| rp > tail_start && rp <= tail_start + w.max)
            .unwrap_or(false);
    let alt_resc = alt_head_press_time.is_some()
        && has_early_rel
        && repr_after_rel
        && !pre_alt_miss
        && first_repr_after_rel
            .map(|rp| rp <= tail_start)
            .unwrap_or(false);
    let late_hless_resc = late_headless_rescue && !pre_alt_miss;
    let raw_rel_from_press = press_time.and_then(|pt| {
        events
            .iter()
            .find(|ev| ev.time > pt && !ev.pressed)
            .map(|ev| ev.time)
    });
    let owner_suffix = if let Some(owner_idx) = first_repr_owner_idx {
        match first_repr_owner_time {
            Some(owner_time) => format!(" [first_repress_owner=#{}@{}ms]", owner_idx, owner_time),
            None => format!(" [first_repress_owner=#{}]", owner_idx),
        }
    } else {
        String::new()
    };
    let conflc_entry_suffx = if conflict_first_repr.is_some() || conflc_rel_post_repr.is_some() {
        format!(
            " [conflicts_entry_rp={:?}ms, rr={:?}ms, in_head_window={}]",
            conflict_first_repr, conflc_rel_post_repr, conf_first_repr_head
        )
    } else {
        String::new()
    };
    let branch = format!("stable_basic{}{}", owner_suffix, conflc_entry_suffx);
    ln_release_info.insert(
        idx,
        LnReleaseInfo {
            kind: rel_kind,
            time: rel_time,
            double_tap: false,
            rescued: repr_hit_tail
                || mis_wit_rep_pre_tail
                || short_hit_repr_meta
                || alt_resc
                || alt_head_tail_res
                || late_hless_resc,
            force_kind,
            alt_head_press_time,
        },
    );
    ln_debug_info.insert(
        idx,
        LnDebugInfo {
            head_was_hit,
            held_until_end,
            has_early_rel,
            repr_after_rel,
            repr_hit_tail,
            first_early_rel,
            first_repr_after_rel,
            last_repr_time,
            first_free_repr,
            rel_after_repr,
            rescue_rel_near_end,
            last_repr_free,
            branch,
            rule: RuleMeta::Unknown,
            authority: Authority::Derived,
            start_diff,
            end_diff,
            total_diff,
            effective_rel_time: rel_time,
            raw_rel_from_press,
            alt_head_used: alt_head_press_time.is_some() && !head_was_hit,
            alt_head_press_time,
            first_repr_owner_idx,
            first_repr_owner_time,
            fir_rep_yiel_next_ln,
            ..Default::default()
        },
    );
    if late_body_claim {
        if let Some((next_idx, rp, _rr)) = first_repr_after_rel
            .zip(rel_after_repr)
            .zip(next_same_col_idx)
            .map(|((rp, rr), next_idx)| (next_idx, rp, rr))
            .filter(|(_, _, rr)| rel_time == Some(*rr))
        {
            if let Some(pos) = j_by_idx.get(next_idx).and_then(|pos| *pos) {
                if let Some(next_jj) = judgments.get_mut(pos) {
                    if next_jj.column == ho.column && next_jj.press_time == Some(rp) {
                        next_jj.press_time = None;
                        next_jj.kind = JudgmentKind::Miss;
                        next_jj.early_press_idx = None;
                        next_jj.early_pen_win = None;
                    }
                }
            }
        }
    }
}
