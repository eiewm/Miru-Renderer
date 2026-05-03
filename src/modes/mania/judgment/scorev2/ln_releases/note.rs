use crate::modes::mania::judgment::{InternalJudgment, KeyEvent, ReleaseKind};
use crate::types::{Beatmap, HitObject, JudgmentKind, Windows};
use std::collections::HashSet;
#[derive(Debug, Clone, Copy)]
pub(crate) struct ReleaseNoteCtx<'a> {
    pub idx: usize,
    pub ho: &'a HitObject,
    pub end_time: i32,
    pub ln_duration: i32,
    pub tail_window_scale: f32,
    pub tail_start: i32,
    pub late_repr_guard: i32,
    pub early_release_cutoff: i32,
    pub tail_end_exclusive: i32,
    pub press_time: Option<i32>,
    pub tail_only_pt: Option<i32>,
    pub tail_eval_press_time: Option<i32>,
    pub deep_ln_pen: bool,
    pub head_was_hit: bool,
    pub head_is_h100: bool,
    pub head_is_h50: bool,
    pub strong_head_hit: bool,
    pub post_end_hless: bool,
    pub prev_same_col_idx: Option<usize>,
    pub prev_same_col_ho: Option<&'a HitObject>,
    pub prev_same_col_is_ln: bool,
    pub prev_same_col_time: Option<i32>,
    pub prev_same_end: Option<i32>,
    pub next_same_col_idx: Option<usize>,
    pub next_same_col_time: Option<i32>,
    pub events: &'a [KeyEvent],
    pub windows: &'a Windows,
    pub last_note_idx_overall: Option<usize>,
    pub extreme_ln_ends: &'a HashSet<i32>,
}
impl<'a> ReleaseNoteCtx<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        idx: usize,
        ho: &'a HitObject,
        map: &'a Beatmap,
        events: &'a [KeyEvent],
        judgments: &[InternalJudgment],
        j_by_idx: &[Option<usize>],
        windows: &'a Windows,
        last_note_idx_overall: Option<usize>,
        extreme_ln_ends: &'a HashSet<i32>,
    ) -> Self {
        let end_time = ho.end_time.unwrap_or(ho.time);
        let ln_duration = end_time - ho.time;
        let tail_window_scale = 1.5;
        let tail_start = end_time - ((windows.hit50 as f32) * tail_window_scale).round() as i32;
        let late_repr_guard = if true {
            end_time - windows.hit50
        } else {
            tail_start
        };
        let early_release_cutoff = if true {
            end_time - windows.hit50
        } else {
            tail_start
        };
        let tail_end_exclusive =
            end_time + ((windows.hit100 as f32) * tail_window_scale).round() as i32;
        let j = j_by_idx[idx].and_then(|pos| judgments.get(pos));
        let press_time = j.and_then(|judgment| judgment.press_time);
        let deep_ln_pen = j.map(|judgment| judgment.deep_ln_pen).unwrap_or(false);
        let tail_only_pt = j.and_then(|judgment| {
            if judgment.kind == JudgmentKind::Miss {
                let short_miss_uses_h50 = ho.is_long_note()
                    && end_time - ho.time <= windows.hit100
                    && judgment
                        .early_press_idx
                        .map(|pt| pt > end_time + windows.hit100 && pt <= end_time + windows.hit50)
                        .unwrap_or(false);
                judgment.early_press_idx.filter(|pt| {
                    let miss_assigned_tail = judgment
                        .press_time
                        .map(|head_pt| {
                            *pt > head_pt
                                && *pt >= ho.time - windows.hit50
                                && *pt <= end_time + windows.hit100
                        })
                        .unwrap_or(false);
                    let mis_prs_tail_only_pt = judgment.press_time.is_none()
                        && ((*pt >= ho.time + windows.hit100 && *pt <= end_time + windows.hit100)
                            || short_miss_uses_h50);
                    miss_assigned_tail || mis_prs_tail_only_pt
                })
            } else {
                None
            }
        });
        let tail_eval_press_time = if matches!(j.map(|jj| jj.kind), Some(JudgmentKind::Miss)) {
            tail_only_pt.or(press_time)
        } else {
            press_time.or(tail_only_pt)
        };
        let head_was_hit = j
            .map(|judgment| judgment.kind != JudgmentKind::Miss && judgment.press_time.is_some())
            .unwrap_or(false);
        let head_is_h100 = matches!(j.map(|jj| jj.kind), Some(JudgmentKind::Hit100));
        let head_is_h50 = matches!(j.map(|jj| jj.kind), Some(JudgmentKind::Hit50));
        let strong_head_hit = matches!(
            j.map(|jj| jj.kind),
            Some(JudgmentKind::Hit200) | Some(JudgmentKind::Hit300) | Some(JudgmentKind::Max)
        );
        let post_end_hless = !head_was_hit
            && (end_time - ho.time) >= windows.hit50 * 2
            && press_time.map(|pt| pt > end_time).unwrap_or(false);
        let prev_same_col = map.hit_objects[..idx]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, prev_ho)| prev_ho.column == ho.column);
        let prev_same_col_idx = prev_same_col.map(|(prev_idx, _)| prev_idx);
        let prev_same_col_ho = prev_same_col.map(|(_, prev_ho)| prev_ho);
        let prev_same_col_is_ln = prev_same_col_ho
            .map(|prev_ho| prev_ho.is_long_note())
            .unwrap_or(false);
        let prev_same_col_time = prev_same_col_ho.map(|prev_ho| prev_ho.time);
        let prev_same_end =
            prev_same_col_ho.map(|prev_ho| prev_ho.end_time.unwrap_or(prev_ho.time));
        let next_same_col_idx = map.hit_objects[(idx + 1)..]
            .iter()
            .enumerate()
            .find(|(_, next_ho)| next_ho.column == ho.column)
            .map(|(offset, _)| idx + 1 + offset);
        let next_same_col_time = map.hit_objects[(idx + 1)..]
            .iter()
            .find(|next_ho| next_ho.column == ho.column)
            .map(|next_ho| next_ho.time);
        Self {
            idx,
            ho,
            end_time,
            ln_duration,
            tail_window_scale,
            tail_start,
            late_repr_guard,
            early_release_cutoff,
            tail_end_exclusive,
            press_time,
            tail_only_pt,
            tail_eval_press_time,
            deep_ln_pen,
            head_was_hit,
            head_is_h100,
            head_is_h50,
            strong_head_hit,
            post_end_hless,
            prev_same_col_idx,
            prev_same_col_ho,
            prev_same_col_is_ln,
            prev_same_col_time,
            prev_same_end,
            next_same_col_idx,
            next_same_col_time,
            events,
            windows,
            last_note_idx_overall,
            extreme_ln_ends,
        }
    }
}
pub(crate) type RelSeg = (i32, Option<i32>);
#[derive(Debug, Default)]
pub(crate) struct SegState {
    pub list: Vec<RelSeg>,
}
#[derive(Debug, Default)]
pub(crate) struct EarlyState {
    pub has_rel: bool,
    pub first_rel: Option<i32>,
    pub repr_after: bool,
    pub first_repr: Option<i32>,
    pub last_repr: Option<i32>,
    pub first_free_repr: Option<i32>,
    pub rel_after_repr: Option<i32>,
    pub last_repr_free: bool,
    pub hit_tail: bool,
}
#[derive(Debug, Default)]
pub(crate) struct TailPrefs {
    pub body: bool,
    pub bridge: bool,
    pub early: bool,
    pub pre_frag: bool,
    pub exact: bool,
}
#[derive(Debug)]
pub(crate) struct RelPick {
    pub kind: ReleaseKind,
    pub time: Option<i32>,
    pub diff: i32,
    pub force: bool,
}
impl Default for RelPick {
    fn default() -> Self {
        Self {
            kind: ReleaseKind::Miss,
            time: None,
            diff: 0,
            force: false,
        }
    }
}
#[derive(Debug, Default)]
pub(crate) struct RescueState {
    pub near_end_rel: Option<i32>,
    pub imm_rel_at_press: Option<i32>,
    pub late_headless: bool,
    pub init_first_repr: Option<i32>,
    pub init_rel_after_repr: Option<i32>,
    pub short_miss_bridge: bool,
    pub first_rel_after_press: Option<i32>,
    pub late_repr_dur: i32,
    pub tail_hold_hit: bool,
    pub miss_press_tail: bool,
    pub miss_repr_tail: bool,
    pub short_body_miss: bool,
    pub pre_frag_keep_rel: bool,
    pub miss_pre_meta: bool,
    pub miss_next_ln_claim: bool,
    pub late_body_claim: bool,
    pub alt_head_pt: Option<i32>,
    pub alt_prehold: bool,
    pub alt_cross_hold: bool,
}
#[derive(Debug, Default)]
pub(crate) struct ReclaimState {
    pub claimed_pt: Option<i32>,
    pub competing_idx: Option<usize>,
    pub competing_time: Option<i32>,
    pub followup_pt: Option<i32>,
    pub next_win_start: Option<i32>,
    pub competing_kind: Option<JudgmentKind>,
}
#[derive(Debug, Default)]
pub(crate) struct ReleaseState {
    pub segs: SegState,
    pub early: EarlyState,
    pub prefs: TailPrefs,
    pub pick: RelPick,
    pub rescue: RescueState,
    pub reclaim: ReclaimState,
}
