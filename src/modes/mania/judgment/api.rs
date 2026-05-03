use super::shared::{
    build_score_judgments, InternalJudgment, LnDebugInfo, LnReleaseInfo, ScoreJudgmentEvent,
    ScoreMode, ScoreModeContext, WindowProfile,
};
use crate::types::replay::{ManiaReplayData, ReplayData, ReplayOrigin};
use crate::types::{Beatmap, HitObject, KeyAction, Windows};
use crate::utils::mods::resolve_lazer_playback_mod_settings;
use std::collections::HashMap;
pub type HitWindows = Windows;
#[derive(Debug, Clone)]
pub struct Judgment {
    pub index: usize,
    pub kind: String,
    pub press_time: Option<i32>,
    pub delta: i32,
}
impl From<&InternalJudgment> for Judgment {
    fn from(ij: &InternalJudgment) -> Self {
        Self {
            index: ij.index,
            kind: format!("{:?}", ij.kind),
            press_time: ij.press_time,
            delta: ij.delta,
        }
    }
}
#[inline]
pub fn calc_hit_windows(od: f32) -> HitWindows {
    Windows::from_od(od)
}
#[inline]
pub fn calc_hit_win_mode(od: f32, mods: u32, mode: ScoreMode) -> HitWindows {
    let profile = match mode {
        ScoreMode::ScoreV1 => WindowProfile::StableScoreV1,
        ScoreMode::ScoreV2 | ScoreMode::Lazer => WindowProfile::StableScoreV2,
    };
    calc_hit_win_prof(od, mods, profile)
}
#[inline]
pub fn calc_hit_win_prof(od: f32, mods: u32, profile: WindowProfile) -> HitWindows {
    calc_hit_win_prof_with_clock_rate(od, mods, profile, None)
}
#[inline]
fn calc_hit_win_prof_with_clock_rate(
    od: f32,
    mods: u32,
    profile: WindowProfile,
    clock_rate_override: Option<f32>,
) -> HitWindows {
    let difficulty_range = |minimum: f32, midpoint: f32, maximum: f32| -> f32 {
        let v = od.clamp(0.0, 10.0);
        if v > 5.0 {
            midpoint + (maximum - midpoint) * (v - 5.0) / 5.0
        } else if v < 5.0 {
            midpoint - (midpoint - minimum) * (5.0 - v) / 5.0
        } else {
            midpoint
        }
    };
    match profile {
        WindowProfile::StableScoreV1 | WindowProfile::LazerClassic => {
            let num = (10.0 - od).clamp(0.0, 10.0);
            Windows {
                max: crate::types::scale_window_for_mods_with_clock_rate(
                    16.0,
                    mods,
                    clock_rate_override,
                ),
                hit300: crate::types::scale_window_for_mods_with_clock_rate(
                    34.0 + 3.0 * num,
                    mods,
                    clock_rate_override,
                ),
                hit200: crate::types::scale_window_for_mods_with_clock_rate(
                    67.0 + 3.0 * num,
                    mods,
                    clock_rate_override,
                ),
                hit100: crate::types::scale_window_for_mods_with_clock_rate(
                    97.0 + 3.0 * num,
                    mods,
                    clock_rate_override,
                ),
                hit50: crate::types::scale_window_for_mods_with_clock_rate(
                    121.0 + 3.0 * num,
                    mods,
                    clock_rate_override,
                ),
            }
        }
        WindowProfile::StableScoreV2 | WindowProfile::LazerModern => Windows {
            max: crate::types::scale_window_for_mods_with_clock_rate(
                difficulty_range(22.4, 19.4, 13.9),
                mods,
                clock_rate_override,
            ),
            hit300: crate::types::scale_window_for_mods_with_clock_rate(
                difficulty_range(64.0, 49.0, 34.0),
                mods,
                clock_rate_override,
            ),
            hit200: crate::types::scale_window_for_mods_with_clock_rate(
                difficulty_range(97.0, 82.0, 67.0),
                mods,
                clock_rate_override,
            ),
            hit100: crate::types::scale_window_for_mods_with_clock_rate(
                difficulty_range(127.0, 112.0, 97.0),
                mods,
                clock_rate_override,
            ),
            hit50: crate::types::scale_window_for_mods_with_clock_rate(
                difficulty_range(151.0, 136.0, 121.0),
                mods,
                clock_rate_override,
            ),
        },
    }
}
#[inline]
fn calc_miss_win_prof(od: f32, mods: u32, profile: WindowProfile) -> i32 {
    calc_miss_win_prof_with_clock_rate(od, mods, profile, None)
}
#[inline]
fn calc_miss_win_prof_with_clock_rate(
    od: f32,
    mods: u32,
    profile: WindowProfile,
    clock_rate_override: Option<f32>,
) -> i32 {
    let difficulty_range = |minimum: f32, midpoint: f32, maximum: f32| -> f32 {
        let v = od.clamp(0.0, 10.0);
        if v > 5.0 {
            midpoint + (maximum - midpoint) * (v - 5.0) / 5.0
        } else if v < 5.0 {
            midpoint - (midpoint - minimum) * (5.0 - v) / 5.0
        } else {
            midpoint
        }
    };
    match profile {
        WindowProfile::StableScoreV1 | WindowProfile::LazerClassic => {
            let num = (10.0 - od).clamp(0.0, 10.0);
            crate::types::scale_window_for_mods_with_clock_rate(
                158.0 + 3.0 * num,
                mods,
                clock_rate_override,
            )
        }
        WindowProfile::StableScoreV2 | WindowProfile::LazerModern => {
            crate::types::scale_window_for_mods_with_clock_rate(
                difficulty_range(188.0, 173.0, 158.0),
                mods,
                clock_rate_override,
            )
        }
    }
}
#[inline]
fn replay_clock_rate_override(replay: &ReplayData) -> Option<f32> {
    if replay.origin != ReplayOrigin::LazerExport {
        return None;
    }
    resolve_lazer_playback_mod_settings(replay)
        .ok()
        .flatten()
        .map(|settings| settings.clock_rate as f32)
}
#[inline]
pub fn resolve_score_mode(od: f32, mods: u32) -> (HitWindows, ScoreModeContext) {
    let mut mode_ctx = ScoreModeContext::from_mods(mods);
    mode_ctx.miss_window = calc_miss_win_prof(od, mods, mode_ctx.window_profile);
    let windows = calc_hit_win_prof(od, mods, mode_ctx.window_profile);
    (windows, mode_ctx)
}
#[inline]
pub fn res_sco_mod_for_repl(
    od: f32,
    replay: &crate::types::ReplayData,
) -> (HitWindows, ScoreModeContext) {
    let mut mode_ctx = ScoreModeContext::from_replay(replay);
    let clock_rate_override = replay_clock_rate_override(replay);
    mode_ctx.miss_window = calc_miss_win_prof_with_clock_rate(
        od,
        replay.mods,
        mode_ctx.window_profile,
        clock_rate_override,
    );
    let windows = calc_hit_win_prof_with_clock_rate(
        od,
        replay.mods,
        mode_ctx.window_profile,
        clock_rate_override,
    );
    (windows, mode_ctx)
}
pub struct JudgmentResult {
    pub judgments: Vec<Judgment>,
    pub ln_releases: HashMap<usize, LnReleaseInfo>,
    pub ln_debug: HashMap<usize, LnDebugInfo>,
    pub score_judgments: Vec<ScoreJudgmentEvent>,
}
pub fn compute_judgments(
    hit_objects: &[HitObject],
    key_actions: &[KeyAction],
    windows: &HitWindows,
) -> JudgmentResult {
    calc_judg_mode(
        hit_objects,
        key_actions,
        windows,
        ScoreModeContext::from_mods(0),
    )
}
fn build_replay_for_judgment(
    replay_data: Option<&crate::types::ReplayData>,
    key_actions: &[KeyAction],
) -> ManiaReplayData {
    ManiaReplayData {
        replay: replay_data.cloned().unwrap_or_default(),
        frames: Vec::new(),
        key_actions: key_actions.to_vec(),
        beatmap_file: None,
    }
}
#[inline]
fn calc_judg_mode_internal(
    replay_data: Option<&crate::types::ReplayData>,
    hit_objects: &[HitObject],
    key_actions: &[KeyAction],
    windows: &HitWindows,
    mode_ctx: ScoreModeContext,
) -> JudgmentResult {
    let mut beatmap = Beatmap::default();
    beatmap.hit_objects = hit_objects.to_vec();
    beatmap.difficulty.cs = infer_key_count(hit_objects) as f32;
    let replay = build_replay_for_judgment(replay_data, key_actions);
    let engine_output = match mode_ctx.effective_mode {
        ScoreMode::ScoreV1 => super::scorev1::compute(&beatmap, &replay, windows),
        ScoreMode::ScoreV2 => super::scorev2::compute(&beatmap, &replay, windows),
        ScoreMode::Lazer => super::lazer::compute(&beatmap, &replay, windows, mode_ctx),
    };
    let score_judgments = build_score_judgments(
        &beatmap.hit_objects,
        &engine_output.judgments,
        &engine_output.ln_releases,
        mode_ctx.effective_mode,
    );
    JudgmentResult {
        judgments: engine_output.judgments.iter().map(Judgment::from).collect(),
        ln_releases: engine_output.ln_releases,
        ln_debug: engine_output.ln_debug,
        score_judgments,
    }
}
#[inline]
pub fn calc_judg_mode(
    hit_objects: &[HitObject],
    key_actions: &[KeyAction],
    windows: &HitWindows,
    mode_ctx: ScoreModeContext,
) -> JudgmentResult {
    calc_judg_mode_internal(None, hit_objects, key_actions, windows, mode_ctx)
}
#[inline]
pub fn calc_judg_mode_for_replay(
    replay_data: &crate::types::ReplayData,
    hit_objects: &[HitObject],
    key_actions: &[KeyAction],
    windows: &HitWindows,
    mode_ctx: ScoreModeContext,
) -> JudgmentResult {
    calc_judg_mode_internal(
        Some(replay_data),
        hit_objects,
        key_actions,
        windows,
        mode_ctx,
    )
}
pub struct JudgmentDebugResult {
    pub judgments: Vec<InternalJudgment>,
    pub ln_releases: HashMap<usize, LnReleaseInfo>,
    pub ln_debug: HashMap<usize, LnDebugInfo>,
    pub score_judgments: Vec<ScoreJudgmentEvent>,
    pub press_status: Vec<PressStatus>,
}
#[derive(Debug, Clone)]
pub struct PressStatus {
    pub time: i32,
    pub column: u8,
    pub assigned_to: Option<usize>,
    pub status: String,
}
pub fn calc_judg_dbg(
    hit_objects: &[HitObject],
    key_actions: &[KeyAction],
    windows: &HitWindows,
) -> JudgmentDebugResult {
    calc_jdbg_mode(
        hit_objects,
        key_actions,
        windows,
        ScoreModeContext::from_mods(0),
    )
}
#[inline]
pub fn calc_jdbg_mode(
    hit_objects: &[HitObject],
    key_actions: &[KeyAction],
    windows: &HitWindows,
    mode_ctx: ScoreModeContext,
) -> JudgmentDebugResult {
    calc_jdbg_mode_internal(None, hit_objects, key_actions, windows, mode_ctx)
}
#[inline]
pub fn calc_jdbg_mode_for_replay(
    replay_data: &crate::types::ReplayData,
    hit_objects: &[HitObject],
    key_actions: &[KeyAction],
    windows: &HitWindows,
    mode_ctx: ScoreModeContext,
) -> JudgmentDebugResult {
    calc_jdbg_mode_internal(
        Some(replay_data),
        hit_objects,
        key_actions,
        windows,
        mode_ctx,
    )
}
#[inline]
fn calc_jdbg_mode_internal(
    replay_data: Option<&crate::types::ReplayData>,
    hit_objects: &[HitObject],
    key_actions: &[KeyAction],
    windows: &HitWindows,
    mode_ctx: ScoreModeContext,
) -> JudgmentDebugResult {
    let mut beatmap = Beatmap::default();
    beatmap.hit_objects = hit_objects.to_vec();
    beatmap.difficulty.cs = infer_key_count(hit_objects) as f32;
    let replay = build_replay_for_judgment(replay_data, key_actions);
    let engine_output = match mode_ctx.effective_mode {
        ScoreMode::ScoreV1 => super::scorev1::compute(&beatmap, &replay, windows),
        ScoreMode::ScoreV2 => super::scorev2::compute(&beatmap, &replay, windows),
        ScoreMode::Lazer => super::lazer::compute(&beatmap, &replay, windows, mode_ctx),
    };
    let score_judgments = build_score_judgments(
        &beatmap.hit_objects,
        &engine_output.judgments,
        &engine_output.ln_releases,
        mode_ctx.effective_mode,
    );
    let press_status = build_press_status(
        &engine_output.judgments,
        key_actions,
        hit_objects,
        windows,
        &engine_output.ln_releases,
        &engine_output.ln_debug,
    );
    JudgmentDebugResult {
        judgments: engine_output.judgments,
        ln_releases: engine_output.ln_releases,
        ln_debug: engine_output.ln_debug,
        score_judgments,
        press_status,
    }
}
fn infer_key_count(hit_objects: &[HitObject]) -> u8 {
    let max_col = hit_objects.iter().map(|h| h.column).max();
    max_col.map(|c| c.saturating_add(1)).unwrap_or(1)
}
fn build_press_status(
    judgments: &[InternalJudgment],
    key_actions: &[KeyAction],
    hit_objects: &[HitObject],
    windows: &HitWindows,
    ln_releases: &HashMap<usize, LnReleaseInfo>,
    ln_debug: &HashMap<usize, LnDebugInfo>,
) -> Vec<PressStatus> {
    let mut result = Vec::new();
    let key_count = infer_key_count(hit_objects) as usize;
    let mut ln_tails_by_col: Vec<Vec<(usize, i32)>> = vec![Vec::new(); key_count];
    for (idx, ho) in hit_objects.iter().enumerate() {
        if !ho.is_long_note() {
            continue;
        }
        let col = ho.column as usize;
        if col >= ln_tails_by_col.len() {
            continue;
        }
        let end = ho.end_time.unwrap_or(ho.time);
        ln_tails_by_col[col].push((idx, end));
    }
    let mut ln_rel_by_col_time: HashMap<(u8, i32), usize> = HashMap::new();
    for (idx, info) in ln_releases {
        if let Some(t) = info.time {
            if let Some(ho) = hit_objects.get(*idx) {
                if ho.column as usize >= key_count {
                    continue;
                }
                ln_rel_by_col_time.entry((ho.column, t)).or_insert(*idx);
            }
        }
    }
    let mut resc_pt_by_col_time: HashMap<(u8, i32), usize> = HashMap::new();
    for (idx, dbg) in ln_debug {
        let rescued = ln_releases.get(idx).map(|i| i.rescued).unwrap_or(false);
        if !rescued {
            continue;
        }
        let rescue_press_time = dbg.first_repr_after_rel.or(dbg.last_repr_time);
        if let Some(t) = rescue_press_time {
            if let Some(ho) = hit_objects.get(*idx) {
                if ho.column as usize >= key_count {
                    continue;
                }
                resc_pt_by_col_time.entry((ho.column, t)).or_insert(*idx);
            }
        }
    }
    for action in key_actions {
        let col = action.column;
        let time = action.time;
        if action.pressed {
            let assigned = judgments
                .iter()
                .find(|j| j.press_time == Some(time) && j.column == col);
            let rescue_owner = resc_pt_by_col_time.get(&(col, time)).copied();
            let status = if let Some(j) = assigned {
                let note = &hit_objects[j.index];
                let is_ln = note.is_long_note();
                let base = if is_ln {
                    format!("PRESS ASSIGNED (LN #{}, {:?})", j.index, j.kind)
                } else {
                    format!("PRESS ASSIGNED (#{}, {:?})", j.index, j.kind)
                };
                if let Some(rescue_idx) = rescue_owner.filter(|rescue_idx| *rescue_idx != j.index) {
                    format!("{} + RESCUE CONFLICT (LN #{})", base, rescue_idx)
                } else {
                    base
                }
            } else if let Some(rescue_idx) = rescue_owner {
                if let Some(ho) = hit_objects.get(rescue_idx) {
                    format!(
                        "PRESS RESCUE (LN #{}, delta {:+}ms)",
                        rescue_idx,
                        time - ho.time
                    )
                } else {
                    format!("PRESS RESCUE (LN #{})", rescue_idx)
                }
            } else {
                let nearby = hit_objects
                    .iter()
                    .enumerate()
                    .filter(|(_, ho)| ho.column == col)
                    .min_by_key(|(_, ho)| (ho.time - time).abs());
                if let Some((idx, ho)) = nearby {
                    let delta = (time - ho.time).abs();
                    if delta <= windows.hit50 + 100 {
                        format!("PRESS GHOST (near #{}, delta {}ms)", idx, time - ho.time)
                    } else {
                        "PRESS EXTRA".to_string()
                    }
                } else {
                    "PRESS EXTRA".to_string()
                }
            };
            result.push(PressStatus {
                time,
                column: col,
                assigned_to: assigned.map(|j| j.index),
                status,
            });
        } else {
            let status = if let Some(&idx) = ln_rel_by_col_time.get(&(col, time)) {
                let ho = &hit_objects[idx];
                let end = ho.end_time.unwrap_or(ho.time);
                let delta = time - end;
                let kind = ln_releases.get(&idx).map(|i| i.kind).unwrap_or_default();
                format!("RELEASE (LN #{}, {:?}, delta {:+}ms)", idx, kind, delta)
            } else {
                let col_idx = col as usize;
                let mut nearest: Option<(usize, i32, i32)> = None;
                if col_idx < ln_tails_by_col.len() {
                    for (idx, end) in &ln_tails_by_col[col_idx] {
                        let win_start = *end - windows.hit50;
                        let win_end = *end + windows.hit100;
                        if time < win_start || time > win_end {
                            continue;
                        }
                        let abs = (time - *end).abs();
                        if nearest.map(|(_, _, best)| abs < best).unwrap_or(true) {
                            nearest = Some((*idx, *end, abs));
                        }
                    }
                }
                if let Some((idx, end, _)) = nearest {
                    let delta = time - end;
                    format!("RELEASE (near LN #{}, delta {:+}ms)", idx, delta)
                } else {
                    "RELEASE EXTRA".to_string()
                }
            };
            result.push(PressStatus {
                time,
                column: col,
                assigned_to: None,
                status,
            });
        }
    }
    result.sort_by_key(|p| (p.column, p.time));
    result
}
