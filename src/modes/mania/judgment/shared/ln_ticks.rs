use super::{LnDebugInfo, LnReleaseInfo, ReleaseKind, ScoreMode};
use crate::types::{HitObject, Windows};
use std::collections::HashMap;
const TICK_INTERVAL_MS: i32 = 100;
#[derive(Debug, Clone)]
pub struct ComboTick {
    pub time: i32,
    pub column: u8,
    pub ln_index: usize,
}
#[derive(Debug, Clone)]
pub struct ComboBreak {
    pub time: i32,
    pub column: u8,
    pub ln_index: usize,
}
pub fn gen_ln_combo(
    hit_objects: &[HitObject],
    head_judgments: &HashMap<usize, (ReleaseKind, Option<i32>)>,
    ln_releases: &HashMap<usize, LnReleaseInfo>,
    ln_debug: &HashMap<usize, LnDebugInfo>,
    _windows: &Windows,
    score_mode: ScoreMode,
) -> (Vec<ComboTick>, Vec<ComboBreak>) {
    if matches!(score_mode, ScoreMode::ScoreV2) {
        return (Vec::new(), Vec::new());
    }
    let mut ticks = Vec::new();
    let mut breaks = Vec::new();
    for (idx, ho) in hit_objects.iter().enumerate() {
        let is_ln = ho.end_time.map(|et| et > ho.time + 2).unwrap_or(false);
        if !is_ln {
            continue;
        }
        let end_time = ho.end_time.unwrap();
        let Some((head_kind, press_time)) = head_judgments.get(&idx) else {
            continue;
        };
        let release_info = ln_releases.get(&idx);
        let effective_press_time = press_time.or(release_info.and_then(|r| r.alt_head_press_time));
        if score_mode.uses_prgrss_ln_ticks() {
            if let Some(pt) = effective_press_time {
                if *head_kind != ReleaseKind::Miss {
                    let rel_time = release_info.and_then(|r| r.time).unwrap_or(end_time);
                    let tick_start = pt + TICK_INTERVAL_MS;
                    let tick_end = end_time.min(rel_time);
                    if tick_end > tick_start {
                        let mut tick_time = tick_start;
                        while tick_time < tick_end {
                            ticks.push(ComboTick {
                                time: tick_time,
                                column: ho.column,
                                ln_index: idx,
                            });
                            tick_time += TICK_INTERVAL_MS;
                        }
                    }
                }
            }
        }
        if *head_kind != ReleaseKind::Miss {
            if let Some(dbg) = ln_debug.get(&idx) {
                if dbg.has_early_rel {
                    if let Some(t) = dbg.first_early_rel {
                        breaks.push(ComboBreak {
                            time: t,
                            column: ho.column,
                            ln_index: idx,
                        });
                    }
                }
            }
        }
        if score_mode.uses_prgrss_ln_ticks() {
            if let Some(rel) = release_info {
                let rel_time = rel.time.unwrap_or(end_time);
                if rel.kind != ReleaseKind::Miss {
                    if let Some(pt) = effective_press_time {
                        let first_tick = pt + TICK_INTERVAL_MS;
                        let end_matches_first = end_time == first_tick;
                        if end_matches_first && rel_time > end_time {
                            ticks.push(ComboTick {
                                time: end_time,
                                column: ho.column,
                                ln_index: idx,
                            });
                        }
                    }
                    ticks.push(ComboTick {
                        time: rel_time,
                        column: ho.column,
                        ln_index: idx,
                    });
                } else if *head_kind != ReleaseKind::Miss {
                    breaks.push(ComboBreak {
                        time: rel_time,
                        column: ho.column,
                        ln_index: idx,
                    });
                }
            }
        }
    }
    ticks.sort_by_key(|t| t.time);
    breaks.sort_by_key(|b| b.time);
    (ticks, breaks)
}
