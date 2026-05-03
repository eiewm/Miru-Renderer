use super::conflicts;
use super::finish;
use super::note::{ReleaseNoteCtx, ReleaseState};
use super::resolve;
use super::segments;
use crate::modes::mania::judgment::{
    build_events_by_col, build_extreme_ln_ends, build_j_idx_lookup, build_last_idx,
    effective_key_count, InternalJudgment, LnDebugInfo, LnReleaseInfo,
};
use crate::types::replay::ManiaReplayData;
use crate::types::{Beatmap, JudgmentKind, Windows};
use std::collections::{HashMap, HashSet};
pub(crate) fn pre_ln_rels(
    map: &Beatmap,
    replay: &ManiaReplayData,
    judgments: &mut [InternalJudgment],
    w: &Windows,
) -> (HashMap<usize, LnReleaseInfo>, HashMap<usize, LnDebugInfo>) {
    let key_count = effective_key_count(map);
    let events_by_col = build_events_by_col(&replay.key_actions, key_count);
    let last_note_idx_overall = build_last_idx(map, key_count);
    let extreme_ln_ends: HashSet<i32> = build_extreme_ln_ends(map, w.hit50);
    let j_by_idx = build_j_idx_lookup(judgments, map.hit_objects.len());
    let mut ln_release_info: HashMap<usize, LnReleaseInfo> = HashMap::new();
    let mut ln_debug_info: HashMap<usize, LnDebugInfo> = HashMap::new();
    let mut headless_meta_clear: Vec<(usize, i32)> = Vec::new();
    for (idx, ho) in map.hit_objects.iter().enumerate() {
        if ho.column >= key_count || !ho.is_long_note() {
            continue;
        }
        let events = &events_by_col[ho.column as usize];
        let ctx = ReleaseNoteCtx::new(
            idx,
            ho,
            map,
            events,
            judgments,
            &j_by_idx,
            w,
            last_note_idx_overall,
            &extreme_ln_ends,
        );
        let mut state = ReleaseState::default();
        segments::scan(
            &ctx,
            &mut state,
            map,
            judgments,
            w,
            &j_by_idx,
            &ln_debug_info,
        );
        resolve::resolve(&ctx, &mut state, map, judgments, w);
        let handled_by_conflicts = conflicts::resolve(
            &ctx,
            &mut state,
            map,
            judgments,
            w,
            &j_by_idx,
            &ln_debug_info,
            &ln_release_info,
            &mut headless_meta_clear,
        );
        if !handled_by_conflicts {
            finish::finalize(
                &ctx,
                &mut state,
                map,
                judgments,
                &j_by_idx,
                &mut ln_release_info,
                &mut ln_debug_info,
            );
        }
    }
    apply_metadata_clears(judgments, &j_by_idx, headless_meta_clear);
    (ln_release_info, ln_debug_info)
}
fn apply_metadata_clears(
    judgments: &mut [InternalJudgment],
    j_by_idx: &[Option<usize>],
    metadata_clears: Vec<(usize, i32)>,
) {
    for (idx, pt) in metadata_clears {
        let Some(pos) = j_by_idx.get(idx).and_then(|pos| *pos) else {
            continue;
        };
        let Some(jj) = judgments.get_mut(pos) else {
            continue;
        };
        if jj.kind == JudgmentKind::Miss && jj.press_time == Some(pt) {
            jj.press_time = None;
            jj.early_press_idx = jj.early_press_idx.or(Some(pt));
        }
    }
}
