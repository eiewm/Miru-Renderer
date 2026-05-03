use super::note::{ReleaseNoteCtx, ReleaseState};
use crate::modes::mania::judgment::{
    Authority, InternalJudgment, LnDebugInfo, LnReleaseInfo, ReleaseKind, RuleMeta,
};
use crate::types::JudgmentKind;
use std::collections::HashMap;
pub(super) fn finalize(
    ctx: &ReleaseNoteCtx<'_>,
    state: &mut ReleaseState,
    judgments: &mut [InternalJudgment],
    j_by_idx: &[Option<usize>],
    ln_release_info: &mut HashMap<usize, LnReleaseInfo>,
    ln_debug_info: &mut HashMap<usize, LnDebugInfo>,
) {
    let idx = ctx.idx;
    let ho = ctx.ho;
    let ln_duration = ctx.ln_duration;
    let tail_start = ctx.tail_start;
    let press_time = ctx.press_time;
    let head_was_hit = ctx.head_was_hit;
    let next_same_col_idx = ctx.next_same_col_idx;
    let w = ctx.windows;
    let has_early_rel = state.early.has_rel;
    let repr_after_rel = state.early.repr_after;
    let first_early_rel = state.early.first_rel;
    let first_repr_after_rel = state.early.first_repr;
    let last_repr_time = state.early.last_repr;
    let first_free_repr = state.early.first_free_repr;
    let rel_after_repr = state.early.rel_after_repr;
    let rescue_rel_near_end = state.rescue.near_end_rel;
    let last_repr_free = state.early.last_repr_free;
    let late_headless_rescue = state.rescue.late_headless;
    let rel_kind = state.pick.kind;
    let rel_time = state.pick.time;
    let end_diff = state.pick.diff;
    let force_kind = state.pick.force;
    let repr_hit_tail = state.early.hit_tail;
    let alt_head_press_time = state.rescue.alt_head_pt;
    let head_miss_pre_meta = state.rescue.miss_pre_meta;
    let hea_mis_next_ln_clai = state.rescue.miss_next_ln_claim;
    let late_body_claim = state.rescue.late_body_claim;
    let start_diff = press_time.map(|pt| (pt - ho.time).abs()).unwrap_or(0);
    let total_diff = start_diff + end_diff;
    let held_until_end = press_time.is_some() && !has_early_rel;
    let mis_wit_rep_pre_tail = rel_kind == ReleaseKind::Miss
        && has_early_rel
        && repr_after_rel
        && first_repr_after_rel
            .map(|rp| rp <= tail_start)
            .unwrap_or(false)
        && rel_after_repr.map(|rt| rt < tail_start).unwrap_or(false)
        && !head_miss_pre_meta;
    let alt_head_tail_res = alt_head_press_time.is_some()
        && has_early_rel
        && repr_after_rel
        && ln_duration > w.hit50 + w.hit100 + w.max
        && first_repr_after_rel
            .map(|rp| rp > tail_start && rp <= tail_start + w.max)
            .unwrap_or(false);
    ln_release_info.insert(
        idx,
        LnReleaseInfo {
            kind: rel_kind,
            time: rel_time,
            double_tap: false,
            rescued: repr_hit_tail
                || mis_wit_rep_pre_tail
                || (alt_head_press_time.is_some()
                    && has_early_rel
                    && repr_after_rel
                    && first_repr_after_rel
                        .map(|rp| rp <= tail_start)
                        .unwrap_or(false))
                || alt_head_tail_res
                || late_headless_rescue,
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
            branch: "stable_basic".to_string(),
            rule: RuleMeta::Unknown,
            authority: Authority::Derived,
            start_diff,
            end_diff,
            total_diff,
            effective_rel_time: rel_time,
            alt_head_used: alt_head_press_time.is_some() && !head_was_hit,
            alt_head_press_time,
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
    if hea_mis_next_ln_clai {
        if let Some((next_idx, rp, _rr)) = first_repr_after_rel
            .zip(rel_after_repr)
            .zip(next_same_col_idx)
            .map(|((rp, rr), next_idx)| (next_idx, rp, rr))
            .filter(|(_, _, _)| rel_time == first_early_rel)
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
