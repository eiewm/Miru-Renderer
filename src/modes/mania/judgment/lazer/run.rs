use super::state::{HoldState, LaneObjectState, PressDispatch, TapState};
use crate::modes::mania::judgment::{
    EngineOutput, InternalJudgment, LnDebugInfo, LnReleaseInfo, ReleaseKind, ScoreModeContext,
    WindowProfile,
};
use crate::types::replay::ManiaReplayData;
use crate::types::{Beatmap, HitObject, JudgmentKind, KeyAction, Windows};
use crate::utils::mods::replay_has_api_mod;
use std::collections::HashMap;
const RELEASE_WINDOW_LENIENCE_NUM: i32 = 3;
const RELEASE_WINDOW_LENIENCE_DEN: i32 = 2;
#[derive(Debug, Clone, Copy)]
struct WindowThresholds {
    max: f64,
    hit300: f64,
    hit200: f64,
    hit100: f64,
    hit50: f64,
    miss: f64,
}
impl WindowThresholds {
    #[inline]
    fn from_windows(windows: &Windows, miss_window: i32) -> Self {
        Self {
            max: f64::from(windows.max) + 0.5,
            hit300: f64::from(windows.hit300) + 0.5,
            hit200: f64::from(windows.hit200) + 0.5,
            hit100: f64::from(windows.hit100) + 0.5,
            hit50: f64::from(windows.hit50) + 0.5,
            miss: f64::from(miss_window) + 0.5,
        }
    }
    #[inline]
    fn judgment_for_delta(self, delta: i32) -> Option<JudgmentKind> {
        let abs_delta = f64::from(delta.abs());
        if abs_delta <= self.max {
            Some(JudgmentKind::Max)
        } else if abs_delta <= self.hit300 {
            Some(JudgmentKind::Hit300)
        } else if abs_delta <= self.hit200 {
            Some(JudgmentKind::Hit200)
        } else if abs_delta <= self.hit100 {
            Some(JudgmentKind::Hit100)
        } else if abs_delta <= self.hit50 {
            Some(JudgmentKind::Hit50)
        } else if abs_delta <= self.miss {
            Some(JudgmentKind::Miss)
        } else {
            None
        }
    }
    #[inline]
    fn rel_kind_for_delta(self, delta: i32) -> Option<ReleaseKind> {
        let abs_delta = f64::from(delta.abs());
        let within = |window: f64| {
            abs_delta * f64::from(RELEASE_WINDOW_LENIENCE_DEN)
                <= window * f64::from(RELEASE_WINDOW_LENIENCE_NUM)
        };
        if within(self.max) {
            Some(ReleaseKind::Max)
        } else if within(self.hit300) {
            Some(ReleaseKind::Hit300)
        } else if within(self.hit200) {
            Some(ReleaseKind::Hit200)
        } else if within(self.hit100) {
            Some(ReleaseKind::Hit100)
        } else if within(self.hit50) {
            Some(ReleaseKind::Hit50)
        } else if within(self.miss) {
            Some(ReleaseKind::Miss)
        } else {
            None
        }
    }
    #[inline]
    fn release_miss_window(self) -> f64 {
        self.miss * f64::from(RELEASE_WINDOW_LENIENCE_NUM) / f64::from(RELEASE_WINDOW_LENIENCE_DEN)
    }
}
pub(crate) fn compute(
    beatmap: &Beatmap,
    replay: &ManiaReplayData,
    windows: &Windows,
    mode_ctx: ScoreModeContext,
) -> EngineOutput {
    let hit_objects = &beatmap.hit_objects;
    let no_release = replay.replay.origin == crate::types::ReplayOrigin::LazerExport
        && replay_has_api_mod(&replay.replay, "NR");
    let miss_window = if mode_ctx.miss_window > 0 {
        mode_ctx.miss_window
    } else {
        miss_win_for_prof(beatmap.difficulty.od, mode_ctx.mods, mode_ctx)
    };
    let thresholds = WindowThresholds::from_windows(windows, miss_window);
    let key_count = beatmap.key_count() as usize;
    let mut states: Vec<LaneObjectState> = hit_objects
        .iter()
        .map(|ho| LaneObjectState::new(ho.is_long_note()))
        .collect();
    let mut next_same_col_start: Vec<Option<i32>> = vec![None; hit_objects.len()];
    let mut last_seen_by_col: Vec<Option<usize>> = vec![None; key_count.max(1)];
    for (idx, ho) in hit_objects.iter().enumerate().rev() {
        let col = ho.column as usize;
        if col >= last_seen_by_col.len() {
            continue;
        }
        next_same_col_start[idx] = last_seen_by_col[col].map(|next_idx| hit_objects[next_idx].time);
        last_seen_by_col[col] = Some(idx);
    }
    let mut lane_objects: Vec<Vec<usize>> = vec![Vec::new(); key_count.max(1)];
    for (idx, ho) in hit_objects.iter().enumerate() {
        let col = ho.column as usize;
        if col < lane_objects.len() {
            lane_objects[col].push(idx);
        }
    }
    let mut actions_by_lane: Vec<Vec<&KeyAction>> = vec![Vec::new(); key_count.max(1)];
    for action in &replay.key_actions {
        let col = action.column as usize;
        if col < actions_by_lane.len() {
            actions_by_lane[col].push(action);
        }
    }
    for (col, object_indices) in lane_objects.iter().enumerate() {
        process_lane(
            &actions_by_lane[col],
            object_indices,
            hit_objects,
            &next_same_col_start,
            &mut states,
            windows,
            miss_window,
            thresholds,
            no_release,
        );
    }
    build_output(hit_objects, states)
}
fn process_lane(
    actions: &[&KeyAction],
    object_indices: &[usize],
    hit_objects: &[HitObject],
    next_same_col_start: &[Option<i32>],
    states: &mut [LaneObjectState],
    windows: &Windows,
    miss_window: i32,
    thresholds: WindowThresholds,
    no_release: bool,
) {
    for action in actions {
        advance_lane_to(
            action.time,
            object_indices,
            hit_objects,
            states,
            windows,
            miss_window,
            thresholds,
            no_release,
        );
        if action.pressed {
            dispatch_press(
                action.time,
                object_indices,
                hit_objects,
                next_same_col_start,
                states,
                windows,
                miss_window,
                thresholds,
            );
        } else {
            dispatch_release(
                action.time,
                object_indices,
                hit_objects,
                states,
                windows,
                miss_window,
                thresholds,
            );
        }
    }
    advance_lane_to(
        i32::MAX / 4,
        object_indices,
        hit_objects,
        states,
        windows,
        miss_window,
        thresholds,
        no_release,
    );
}
fn advance_lane_to(
    time: i32,
    object_indices: &[usize],
    hit_objects: &[HitObject],
    states: &mut [LaneObjectState],
    windows: &Windows,
    _miss_window: i32,
    thresholds: WindowThresholds,
    no_release: bool,
) {
    let tail_deadline_pad = thresholds.release_miss_window();
    for &idx in object_indices {
        let ho = &hit_objects[idx];
        match &mut states[idx] {
            LaneObjectState::Tap(state) => {
                if !state.is_resolved() && ho.time + windows.hit50 < time {
                    state.kind = Some(JudgmentKind::Miss);
                }
            }
            LaneObjectState::Hold(state) => {
                if !state.head_resolved() && ho.time + windows.hit50 < time {
                    state.head_kind = Some(JudgmentKind::Miss);
                }
                let end_time = ho.end_time.unwrap_or(ho.time);
                if no_release && !state.is_resolved() && state.holding && time >= end_time {
                    state.tail_kind = Some(cap_tail_kind(ReleaseKind::Max, state));
                    state.tail_time = Some(end_time);
                    state.holding = false;
                    continue;
                }
                if !state.is_resolved()
                    && f64::from(time.saturating_sub(end_time)) > tail_deadline_pad
                {
                    state.tail_kind = Some(ReleaseKind::Miss);
                    state.tail_time = Some(end_time + tail_deadline_pad.floor() as i32);
                    state.holding = false;
                }
            }
        }
    }
}
fn dispatch_press(
    time: i32,
    object_indices: &[usize],
    hit_objects: &[HitObject],
    next_same_col_start: &[Option<i32>],
    states: &mut [LaneObjectState],
    windows: &Windows,
    miss_window: i32,
    thresholds: WindowThresholds,
) {
    for (pos, &idx) in object_indices.iter().enumerate() {
        if !is_hittable_at(time, next_same_col_start[idx]) {
            continue;
        }
        let outcome = match (&hit_objects[idx], &mut states[idx]) {
            (ho, LaneObjectState::Tap(state)) => {
                press_tap(ho, state, time, windows, miss_window, thresholds)
            }
            (ho, LaneObjectState::Hold(state)) => {
                press_hold(ho, state, time, windows, miss_window, thresholds)
            }
        };
        if !outcome.consumed() {
            continue;
        }
        if outcome.is_hit() {
            let cutoff = hit_objects[idx].time;
            for &earlier_idx in &object_indices[..pos] {
                if object_end_time(&hit_objects[earlier_idx]) < cutoff {
                    force_miss_state(&hit_objects[earlier_idx], &mut states[earlier_idx], time);
                }
            }
        }
        break;
    }
}
fn dispatch_release(
    time: i32,
    object_indices: &[usize],
    hit_objects: &[HitObject],
    states: &mut [LaneObjectState],
    _windows: &Windows,
    _miss_window: i32,
    thresholds: WindowThresholds,
) {
    for &idx in object_indices {
        let ho = &hit_objects[idx];
        let LaneObjectState::Hold(state) = &mut states[idx] else {
            continue;
        };
        if !state.holding || state.is_resolved() {
            continue;
        }
        let end_time = ho.end_time.unwrap_or(ho.time);
        let raw_tail = thresholds.rel_kind_for_delta(time - end_time);
        let mut tail_hit = false;
        if let Some(kind) = raw_tail {
            let capped = cap_tail_kind(kind, state);
            tail_hit = capped != ReleaseKind::Miss;
            state.tail_kind = Some(capped);
            state.tail_time = Some(time);
        }
        if time < end_time && !tail_hit {
            if state.first_early_rel.is_none() {
                state.first_early_rel = Some(time);
            }
            state.body_broken = true;
        }
        if state.firs_repr_post_break.is_some()
            && state.rel_post_first_repr.is_none()
            && state
                .firs_repr_post_break
                .map(|pt| pt < time)
                .unwrap_or(false)
        {
            state.rel_post_first_repr = Some(time);
        }
        state.holding = false;
    }
}
fn press_tap(
    ho: &HitObject,
    state: &mut TapState,
    time: i32,
    windows: &Windows,
    miss_window: i32,
    thresholds: WindowThresholds,
) -> PressDispatch {
    if state.is_resolved() {
        return PressDispatch::Ignored;
    }
    let _ = (windows, miss_window);
    let Some(kind) = thresholds.judgment_for_delta(time - ho.time) else {
        return PressDispatch::Ignored;
    };
    state.kind = Some(kind);
    state.press_time = Some(time);
    if kind == JudgmentKind::Miss {
        PressDispatch::ConsumedMiss
    } else {
        PressDispatch::ConsumedHit
    }
}
fn press_hold(
    ho: &HitObject,
    state: &mut HoldState,
    time: i32,
    windows: &Windows,
    miss_window: i32,
    thresholds: WindowThresholds,
) -> PressDispatch {
    if state.is_resolved() {
        return PressDispatch::Ignored;
    }
    let end_time = ho.end_time.unwrap_or(ho.time);
    if time < ho.time - miss_window || time > end_time + windows.hit50 {
        return PressDispatch::Ignored;
    }
    if !state.holding {
        state.holding = true;
        if state.head_kind == Some(JudgmentKind::Miss) && state.late_hold_start.is_none() {
            state.late_hold_start = Some(time);
        }
        state.mark_repress(time);
    }
    if state.head_resolved() {
        return PressDispatch::Ignored;
    }
    let Some(kind) = thresholds.judgment_for_delta(time - ho.time) else {
        return PressDispatch::Ignored;
    };
    state.head_kind = Some(kind);
    state.head_press_time = Some(time);
    if kind == JudgmentKind::Miss {
        PressDispatch::ConsumedMiss
    } else {
        PressDispatch::ConsumedHit
    }
}
fn force_miss_state(ho: &HitObject, state: &mut LaneObjectState, trigger_time: i32) {
    match state {
        LaneObjectState::Tap(tap) => {
            if !tap.is_resolved() {
                tap.kind = Some(JudgmentKind::Miss);
            }
        }
        LaneObjectState::Hold(hold) => {
            if !hold.head_resolved() {
                hold.head_kind = Some(JudgmentKind::Miss);
            }
            if !hold.is_resolved() {
                hold.tail_kind = Some(ReleaseKind::Miss);
                hold.tail_time = Some(trigger_time.max(ho.end_time.unwrap_or(ho.time)));
            }
            hold.holding = false;
        }
    }
}
fn build_output(hit_objects: &[HitObject], states: Vec<LaneObjectState>) -> EngineOutput {
    let mut judgments = Vec::with_capacity(hit_objects.len());
    let mut ln_releases = HashMap::new();
    let mut ln_debug = HashMap::new();
    for (idx, ho) in hit_objects.iter().enumerate() {
        match &states[idx] {
            LaneObjectState::Tap(state) => {
                let kind = state.kind.unwrap_or(JudgmentKind::Miss);
                judgments.push(InternalJudgment {
                    index: idx,
                    column: ho.column,
                    note_time: ho.time,
                    kind,
                    delta: state.press_time.map(|pt| pt - ho.time).unwrap_or(0),
                    press_time: state.press_time,
                    is_ln: false,
                    end_time: None,
                    early_press_idx: None,
                    early_pen_win: None,
                    deep_ln_pen: false,
                });
            }
            LaneObjectState::Hold(state) => {
                let head_kind = state.head_kind.unwrap_or(JudgmentKind::Miss);
                judgments.push(InternalJudgment {
                    index: idx,
                    column: ho.column,
                    note_time: ho.time,
                    kind: head_kind,
                    delta: state.head_press_time.map(|pt| pt - ho.time).unwrap_or(0),
                    press_time: state.head_press_time,
                    is_ln: true,
                    end_time: ho.end_time,
                    early_press_idx: None,
                    early_pen_win: None,
                    deep_ln_pen: false,
                });
                ln_releases.insert(
                    idx,
                    LnReleaseInfo {
                        kind: state.tail_kind.unwrap_or(ReleaseKind::Miss),
                        time: state.tail_time.or(ho.end_time),
                        double_tap: false,
                        rescued: false,
                        force_kind: false,
                        alt_head_press_time: state.late_hold_start,
                    },
                );
                ln_debug.insert(
                    idx,
                    LnDebugInfo {
                        head_was_hit: head_kind != JudgmentKind::Miss,
                        held_until_end: !state.body_broken
                            && state
                                .tail_kind
                                .map(|kind| kind != ReleaseKind::Miss)
                                .unwrap_or(false),
                        has_early_rel: state.body_broken,
                        repr_after_rel: state.firs_repr_post_break.is_some(),
                        repr_hit_tail: state
                            .last_repr_time
                            .zip(ho.end_time)
                            .map(|(press_time, end_time)| press_time >= end_time - 1)
                            .unwrap_or(false),
                        first_early_rel: state.first_early_rel,
                        first_repr_after_rel: state.firs_repr_post_break,
                        last_repr_time: state.last_repr_time,
                        rel_after_repr: state.rel_post_first_repr,
                        effective_rel_time: state.tail_time,
                        raw_rel_from_press: state.tail_time,
                        start_diff: state
                            .effective_press_time()
                            .map(|pt| pt - ho.time)
                            .unwrap_or(0),
                        end_diff: state
                            .tail_time
                            .zip(ho.end_time)
                            .map(|(tail_time, end_time)| tail_time - end_time)
                            .unwrap_or(0),
                        total_diff: state
                            .effective_press_time()
                            .zip(state.tail_time.zip(ho.end_time))
                            .map(|(press_time, (tail_time, end_time))| {
                                (press_time - ho.time).abs() + (tail_time - end_time).abs()
                            })
                            .unwrap_or(0),
                        ..Default::default()
                    },
                );
            }
        }
    }
    EngineOutput {
        judgments,
        ln_releases,
        ln_debug,
    }
}
fn is_hittable_at(time: i32, next_start: Option<i32>) -> bool {
    next_start.map(|next_time| time < next_time).unwrap_or(true)
}
fn object_end_time(ho: &HitObject) -> i32 {
    ho.end_time.unwrap_or(ho.time)
}
fn cap_tail_kind(kind: ReleaseKind, state: &HoldState) -> ReleaseKind {
    let head_missed = matches!(state.head_kind, Some(JudgmentKind::Miss));
    if (head_missed || state.body_broken)
        && matches!(
            kind,
            ReleaseKind::Max | ReleaseKind::Hit300 | ReleaseKind::Hit200 | ReleaseKind::Hit100
        )
    {
        ReleaseKind::Hit50
    } else {
        kind
    }
}
fn miss_win_for_prof(od: f32, mods: u32, mode_ctx: ScoreModeContext) -> i32 {
    match mode_ctx.window_profile {
        WindowProfile::StableScoreV1 | WindowProfile::LazerClassic => {
            Windows::miss_window_v1_mods(od, mods)
        }
        WindowProfile::StableScoreV2 | WindowProfile::LazerModern => {
            Windows::mis_win_v2_mods_stbl(od, mods)
        }
    }
}
