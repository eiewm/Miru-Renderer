use super::super::EngineOutput;
use super::ln_releases::pre_ln_rels;
use super::presses::precompute_judgments;
use super::score::merge_head_tail;
use crate::modes::mania::judgment::{build_events_by_col, effective_key_count};
use crate::types::replay::ManiaReplayData;
use crate::types::{Beatmap, Windows};
pub(crate) fn compute(
    beatmap: &Beatmap,
    replay: &ManiaReplayData,
    windows: &Windows,
) -> EngineOutput {
    let mut judgments = precompute_judgments(beatmap, replay, windows);
    let (mut ln_releases, ln_debug) = pre_ln_rels(beatmap, replay, &mut judgments, windows);
    let events_by_col = build_events_by_col(&replay.key_actions, effective_key_count(beatmap));
    merge_head_tail(
        &mut judgments,
        &beatmap.hit_objects,
        &mut ln_releases,
        &ln_debug,
        &events_by_col,
        windows,
    );
    EngineOutput {
        judgments,
        ln_releases,
        ln_debug,
    }
}
