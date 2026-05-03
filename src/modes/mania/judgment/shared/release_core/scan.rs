use super::super::InternalJudgment;
use crate::types::Beatmap;
use std::collections::HashSet;
pub fn build_last_idx(map: &Beatmap, key_count: u8) -> Option<usize> {
    map.hit_objects
        .iter()
        .enumerate()
        .filter(|(_, ho)| ho.column < key_count)
        .max_by_key(|(note_idx, ho)| (ho.time, *note_idx))
        .map(|(note_idx, _)| note_idx)
}
pub fn build_extreme_ln_ends(map: &Beatmap, hit50: i32) -> HashSet<i32> {
    map.hit_objects
        .iter()
        .filter_map(|ho| {
            if !ho.is_long_note() {
                return None;
            }
            let end_time = ho.end_time.unwrap_or(ho.time);
            let ln_duration = end_time - ho.time;
            if ln_duration >= hit50 * 50 {
                Some(end_time)
            } else {
                None
            }
        })
        .collect()
}
pub fn build_j_idx_lookup(
    judgments: &[InternalJudgment],
    hit_object_len: usize,
) -> Vec<Option<usize>> {
    let mut lookup = vec![None; hit_object_len];
    for (pos, judgment) in judgments.iter().enumerate() {
        if judgment.index < lookup.len() {
            lookup[judgment.index] = Some(pos);
        }
    }
    lookup
}
