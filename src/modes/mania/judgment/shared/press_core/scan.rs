use super::super::KeyEvent;
use super::context::ColumnTimeline;
use crate::types::replay::ManiaReplayData;
use crate::types::{Beatmap, HitObject, Windows};
use std::collections::HashSet;
pub fn collect_pts_col(replay: &ManiaReplayData, key_count: u8) -> Vec<Vec<i32>> {
    let mut presses_by_col: Vec<Vec<i32>> = (0..key_count)
        .map(|col| {
            replay
                .key_actions
                .iter()
                .filter(|a| a.column == col && a.pressed)
                .map(|a| a.time)
                .collect()
        })
        .collect();
    for presses in &mut presses_by_col {
        presses.sort();
    }
    presses_by_col
}
pub fn collect_sorted_notes<'a>(map: &'a Beatmap, key_count: u8) -> Vec<(usize, &'a HitObject)> {
    let mut notes_idx: Vec<(usize, &'a HitObject)> = map
        .hit_objects
        .iter()
        .enumerate()
        .filter(|(_, ho)| ho.column < key_count)
        .collect();
    notes_idx.sort_by_key(|(idx, ho)| (ho.time, *idx));
    notes_idx
}
pub fn collect_ln_ends(notes_idx: &[(usize, &HitObject)], windows: &Windows) -> HashSet<i32> {
    notes_idx
        .iter()
        .filter_map(|(_, ho)| {
            if !ho.is_long_note() {
                return None;
            }
            let end_time = ho.end_time.unwrap_or(ho.time);
            let ln_duration = end_time - ho.time;
            if ln_duration >= windows.hit50 * 50 {
                Some(end_time)
            } else {
                None
            }
        })
        .collect()
}
pub fn build_column_timeline<'a>(
    notes_idx: &[(usize, &'a HitObject)],
    presses_by_col: &'a [Vec<i32>],
    events_by_col: &'a [Vec<KeyEvent>],
    col: u8,
) -> ColumnTimeline<'a> {
    ColumnTimeline {
        notes: notes_idx
            .iter()
            .filter(|(_, ho)| ho.column == col)
            .map(|(i, ho)| (*i, *ho))
            .collect(),
        presses: &presses_by_col[col as usize],
        events: &events_by_col[col as usize],
    }
}
