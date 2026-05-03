use super::types::{InternalJudgment, LnDebugInfo, LnReleaseInfo, ReleaseKind};
use crate::types::{HitObject, JudgmentKind, Windows};
use std::collections::HashMap;
#[derive(Debug, Clone)]
pub struct LnVisualState {
    pub consumed: bool,
    pub held: bool,
    pub miss_dim: bool,
    pub fifty_dim: bool,
    pub rel_kind: ReleaseKind,
    pub rel_time: Option<i32>,
    pub is_double_tap: bool,
    pub is_broken: bool,
    pub has_bad_judgment: bool,
    pub show_ln: bool,
    pub finished_normally: bool,
    pub press_after_end: bool,
    pub head_good_for_visuals: bool,
}
pub fn get_ln_state_at_time(
    ho: &HitObject,
    idx: usize,
    timestamp: i32,
    judgment: Option<&InternalJudgment>,
    is_down: bool,
    windows: &Windows,
    ln_releases: &HashMap<usize, (LnReleaseInfo, LnDebugInfo)>,
    fps: f32,
    is_score_v2: bool,
) -> LnVisualState {
    let frame_eps_ms = (1000.0 / fps.max(1.0)) as i32;
    let end_time = ho.end_time.unwrap_or(ho.time);
    const LATE_HOLD_TIMEOUT_MS: i32 = 200;
    let consumed = judgment
        .filter(|j| j.press_time.is_some())
        .map(|j| j.press_time.unwrap() <= timestamp && timestamp >= ho.time)
        .unwrap_or(false);
    let press_after_end = judgment
        .and_then(|j| j.press_time)
        .map(|pt| pt > end_time)
        .unwrap_or(false);
    let hea_kind_for_late_pt = if press_after_end {
        judgment
            .and_then(|j| j.press_time)
            .map(|pt| {
                let head_delta = (pt - ho.time).abs();
                if head_delta <= windows.hit100 {
                    "good"
                } else {
                    "bad"
                }
            })
            .unwrap_or("bad")
    } else {
        "bad"
    };
    let head_good = judgment
        .filter(|j| j.press_time.is_some())
        .map(|j| j.press_time.unwrap() <= timestamp && !press_after_end)
        .unwrap_or(false);
    let head_good_for_visuals = head_good
        || (press_after_end
            && hea_kind_for_late_pt == "good"
            && judgment
                .and_then(|j| j.press_time)
                .map(|pt| timestamp >= pt)
                .unwrap_or(false));
    let has_valid_press = judgment.and_then(|j| j.press_time).is_some();
    let rel_info = ln_releases.get(&idx);
    let (rel_kind, rel_time) = if head_good_for_visuals {
        rel_info
            .map(|(r, _)| (r.kind, r.time))
            .unwrap_or((ReleaseKind::None, None))
    } else {
        (ReleaseKind::None, None)
    };
    let mut effective_end_time = end_time;
    if let Some(rt) = rel_time {
        if rt > end_time {
            effective_end_time = rt.min(end_time + LATE_HOLD_TIMEOUT_MS);
        }
    }
    let rel_now = rel_time
        .map(|rt| timestamp >= rt - frame_eps_ms * 3 / 4)
        .unwrap_or(false);
    let is_double_tap = rel_info.map(|(r, _)| r.double_tap).unwrap_or(false);
    let past_rel_time = rel_time
        .map(|rt| timestamp >= rt - frame_eps_ms * 3 / 4)
        .unwrap_or(false);
    let held = if is_score_v2 && !head_good_for_visuals {
        false
    } else if is_double_tap {
        false
    } else {
        head_good_for_visuals
            && has_valid_press
            && is_down
            && timestamp >= ho.time
            && timestamp <= effective_end_time
            && !past_rel_time
    };
    let is_broken = consumed && !held && rel_kind == ReleaseKind::Miss;
    let miss_edge = judgment
        .map(|j| {
            if j.kind == JudgmentKind::Miss {
                ho.time + windows.hit50
            } else {
                (ho.time + windows.hit50).max(j.note_time)
            }
        })
        .unwrap_or(i32::MAX);
    let mut miss_dim = judgment
        .filter(|j| j.kind == JudgmentKind::Miss)
        .map(|_| timestamp >= miss_edge - frame_eps_ms * 3 / 4)
        .unwrap_or(false);
    let mut fifty_dim = judgment
        .filter(|j| j.kind == JudgmentKind::Hit50)
        .map(|_| timestamp >= ho.time + windows.hit50)
        .unwrap_or(false);
    if head_good_for_visuals {
        if let Some((rel, _)) = rel_info {
            if rel.double_tap {
                miss_dim = false;
                fifty_dim = false;
            } else if rel_kind == ReleaseKind::Miss {
                if let Some(rt) = rel_time {
                    miss_dim = timestamp >= rt - frame_eps_ms * 3 / 4;
                    fifty_dim = false;
                }
            } else if rel_kind == ReleaseKind::Hit50 {
                if let Some(rt) = rel_time {
                    miss_dim = false;
                    fifty_dim = timestamp >= rt - frame_eps_ms * 3 / 4;
                }
            } else if rel_kind != ReleaseKind::None {
                miss_dim = false;
                fifty_dim = false;
            }
        }
    }
    if press_after_end && hea_kind_for_late_pt == "bad" && !miss_dim {
        let dim_start = ho.time + windows.hit50;
        fifty_dim = timestamp >= dim_start - frame_eps_ms * 3 / 4;
    }
    let pass_whil_hold_other = judgment.and_then(|j| j.press_time).is_none()
        && is_down
        && timestamp >= ho.time
        && timestamp <= end_time;
    if pass_whil_hold_other {
        miss_dim = false;
        fifty_dim = false;
    }
    let release_active = rel_kind != ReleaseKind::None
        && rel_time
            .map(|rt| timestamp >= rt - frame_eps_ms * 3 / 4)
            .unwrap_or(false);
    let has_bad_judgment = miss_dim
        || (rel_kind == ReleaseKind::Miss && rel_now)
        || fifty_dim
        || (release_active && rel_kind == ReleaseKind::Hit50);
    let show_ln = held
        || !consumed
        || release_active
        || has_bad_judgment
        || is_broken
        || miss_dim
        || fifty_dim
        || is_double_tap;
    let rel_is_miss = rel_kind == ReleaseKind::Miss;
    let rel_is_good = rel_kind != ReleaseKind::None && !rel_is_miss;
    let rel_is_50 = rel_kind == ReleaseKind::Hit50;
    let finished_normally = (!is_double_tap
        && !rel_is_50
        && rel_is_good
        && rel_time.map(|rt| timestamp >= rt).unwrap_or(false))
        || (timestamp >= effective_end_time - frame_eps_ms * 3 / 4
            && !rel_is_miss
            && !miss_dim
            && !fifty_dim
            && !is_double_tap
            && !press_after_end
            && (!held || timestamp >= effective_end_time));
    LnVisualState {
        consumed,
        held,
        miss_dim,
        fifty_dim,
        rel_kind,
        rel_time,
        is_double_tap,
        is_broken,
        has_bad_judgment,
        show_ln,
        finished_normally,
        press_after_end,
        head_good_for_visuals,
    }
}
