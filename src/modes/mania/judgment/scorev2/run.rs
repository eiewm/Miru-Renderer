use super::super::EngineOutput;
use super::ln_releases::pre_ln_rels;
use super::presses::precompute_judgments;
use super::score::finalize_judgments;
use crate::types::replay::ManiaReplayData;
use crate::types::{Beatmap, Windows};
pub(crate) fn compute(
    beatmap: &Beatmap,
    replay: &ManiaReplayData,
    windows: &Windows,
) -> EngineOutput {
    let mut judgments = precompute_judgments(beatmap, replay, windows);
    let (mut ln_releases, ln_debug) = pre_ln_rels(beatmap, replay, &mut judgments, windows);
    finalize_judgments(
        &mut judgments,
        &beatmap.hit_objects,
        &mut ln_releases,
        &ln_debug,
        windows,
    );
    EngineOutput {
        judgments,
        ln_releases,
        ln_debug,
    }
}
