use super::judge;
use super::note::{PressNoteCtx, PressState};
use super::penalty;
use super::preserve;
use super::record;
use super::resolve;
use super::stale;
use crate::modes::mania::judgment::{
    build_column_timeline, build_events_by_col, collect_ln_ends, collect_pts_col,
    collect_sorted_notes, effective_key_count, ColumnTimeline, InternalJudgment, PressTracker,
};
use crate::types::replay::ManiaReplayData;
use crate::types::{Beatmap, Windows};
use std::collections::HashMap;
pub(crate) fn precompute_judgments(
    map: &Beatmap,
    replay: &ManiaReplayData,
    w: &Windows,
) -> Vec<InternalJudgment> {
    let key_count = effective_key_count(map);
    let events_by_col = build_events_by_col(&replay.key_actions, key_count);
    let presses_by_col = collect_pts_col(replay, key_count);
    let notes_idx = collect_sorted_notes(map, key_count);
    let last_note_idx_overall = notes_idx.last().map(|(idx, _)| *idx);
    let extreme_ln_ends = collect_ln_ends(&notes_idx, w);
    let mut tap_row_counts: HashMap<i32, usize> = HashMap::new();
    for (_, ho) in &notes_idx {
        if !ho.is_long_note() {
            *tap_row_counts.entry(ho.time).or_insert(0) += 1;
        }
    }
    let mut out: Vec<InternalJudgment> = Vec::with_capacity(notes_idx.len());
    for col in 0..key_count {
        let ColumnTimeline {
            notes: col_notes,
            presses,
            events,
        } = build_column_timeline(&notes_idx, &presses_by_col, &events_by_col, col);
        let mut tracker = PressTracker::default();
        for (note_pos, (idx, ho)) in col_notes.iter().enumerate() {
            let ctx = PressNoteCtx::new(
                note_pos,
                *idx,
                ho,
                &col_notes,
                tap_row_counts.get(&ho.time).copied().unwrap_or(0),
                presses,
                events,
                w,
                last_note_idx_overall,
                &extreme_ln_ends,
            );
            let mut state = PressState::from_tracker(&mut tracker);
            stale::scan(&ctx, &mut state);
            penalty::evaluate(&ctx, &mut state);
            resolve::resolve(&ctx, &mut state, &out);
            preserve::preserve(&ctx, &mut state);
            judge::finalize(&ctx, &mut state, &out);
            record::apply(&ctx, &mut state, &mut out, &mut tracker);
        }
    }
    out
}
