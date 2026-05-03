use super::super::{InternalJudgment, LnDebugInfo, LnReleaseInfo, ReleaseKind};
use crate::modes::mania::judgment::{calc_hit_kind, KeyEvent};
use crate::types::{JudgmentKind, Windows};
use std::collections::HashMap;
#[inline]
fn ln_judged_with(start_diff: i32, total_diff: i32, window: i32, rate: f32) -> bool {
    let threshold = window as f32 * rate;
    (start_diff as f32) <= threshold && (total_diff as f32) <= threshold * 2.0
}
pub fn merge_head_tail(
    judgments: &mut [InternalJudgment],
    hit_objects: &[crate::types::HitObject],
    ln_releases: &mut HashMap<usize, LnReleaseInfo>,
    ln_debug: &HashMap<usize, LnDebugInfo>,
    events_by_col: &[Vec<KeyEvent>],
    windows: &Windows,
) {
    for (idx, ho) in hit_objects.iter().enumerate() {
        if !ho.is_long_note() {
            continue;
        }
        let Some(_col_events) = events_by_col.get(ho.column as usize) else {
            continue;
        };
        let end_time = ho.end_time.unwrap_or(ho.time);
        let rel_info = match ln_releases.get(&idx) {
            Some(v) => v.clone(),
            None => continue,
        };
        let dbg = match ln_debug.get(&idx) {
            Some(v) => v,
            None => continue,
        };
        let judgment_pos = match judgments.iter().position(|j| j.index == idx) {
            Some(v) => v,
            None => continue,
        };
        let pt_already_assigned = judgments[judgment_pos]
            .press_time
            .map(|pt| {
                judgments.iter().enumerate().any(|(other_pos, other)| {
                    other_pos != judgment_pos
                        && other.press_time == Some(pt)
                        && hit_objects
                            .get(other.index)
                            .map(|other_ho| other_ho.column == ho.column)
                            .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        let current_kind = judgments[judgment_pos].kind;
        let current_press_time = judgments[judgment_pos].press_time;
        let rel_time = rel_info.time.unwrap_or(end_time);
        judgments[judgment_pos].note_time = rel_time.max(end_time);
        let next_same_col = hit_objects[(idx + 1)..]
            .iter()
            .enumerate()
            .find(|(_, next_ho)| next_ho.column == ho.column)
            .map(|(offset, next_ho)| (idx + 1 + offset, next_ho));
        let next_same_col_note = next_same_col.map(|(_, next_ho)| next_ho);
        let prev_same_end = hit_objects[..idx]
            .iter()
            .rev()
            .find(|prev_ho| prev_ho.column == ho.column)
            .map(|prev_ho| prev_ho.end_time.unwrap_or(prev_ho.time));
        let no_next_same_col_note = next_same_col_note.is_none();
        let early_rel_before_head = dbg.first_early_rel.map(|t| t <= ho.time).unwrap_or(false);
        let rescue_repr_for_head = if early_rel_before_head
            && dbg.repr_after_rel
            && dbg.repr_hit_tail
            && matches!(
                current_kind,
                JudgmentKind::Hit50
                    | JudgmentKind::Hit100
                    | JudgmentKind::Hit200
                    | JudgmentKind::Hit300
            ) {
            let head_window_start = ho.time - windows.hit50;
            let near_head_repress_end = if rel_info.rescued
                && rel_info.kind == ReleaseKind::Hit50
                && matches!(
                    current_kind,
                    JudgmentKind::Hit50 | JudgmentKind::Hit200 | JudgmentKind::Miss
                ) {
                ho.time + windows.hit100
            } else {
                ho.time + windows.hit200
            };
            dbg.first_repr_after_rel.filter(|t| {
                *t >= head_window_start && *t <= near_head_repress_end && *t <= end_time
            })
        } else {
            None
        };
        let resc_repr_body_break =
            if !early_rel_before_head && dbg.repr_after_rel && dbg.repr_hit_tail {
                let near_head_repress_end = ho.time + windows.hit100;
                dbg.first_repr_after_rel
                    .filter(|t| *t > ho.time && *t <= near_head_repress_end && *t <= end_time)
            } else {
                None
            };
        let ln_duration = end_time - ho.time;
        let tail_start = end_time - windows.hit50;
        let short_frag_alt = !rel_info.rescued
            && pt_already_assigned
            && current_kind == JudgmentKind::Miss
            && rel_info.alt_head_press_time.is_some()
            && dbg.has_early_rel
            && dbg.repr_after_rel
            && current_press_time
                .zip(dbg.first_early_rel)
                .map(|(pt, er)| pt < er)
                .unwrap_or(false)
            && (end_time - ho.time) <= windows.hit100 + windows.max;
        let sho_pre_gap_alt_head = !rel_info.rescued
            && current_kind == JudgmentKind::Miss
            && !matches!(rel_info.kind, ReleaseKind::Miss | ReleaseKind::None)
            && rel_info.alt_head_press_time.is_some()
            && dbg.has_early_rel
            && dbg.repr_after_rel
            && !dbg.repr_hit_tail
            && current_press_time
                .zip(dbg.first_early_rel)
                .map(|(pt, er)| pt < er)
                .unwrap_or(false)
            && dbg
                .raw_rel_from_press
                .map(|raw_rt| raw_rt < ho.time && Some(raw_rt) != rel_info.time)
                .unwrap_or(false)
            && dbg
                .first_early_rel
                .zip(dbg.first_repr_after_rel)
                .map(|(er, rp)| {
                    er < ho.time
                        && rp == rel_info.alt_head_press_time.unwrap_or(rp)
                        && rp > er
                        && rp < ho.time
                        && rp - er <= windows.hit300
                })
                .unwrap_or(false)
            && rel_info
                .time
                .zip(dbg.rel_after_repr)
                .map(|(rt, rr)| rt == rr && rr >= ho.time && rr <= end_time)
                .unwrap_or(false)
            && ln_duration > windows.hit300 + windows.max
            && ln_duration <= windows.hit100 + windows.max;
        let first_repr_alt_head = dbg.fir_rep_yiel_next_ln
            && !rel_info.rescued
            && current_kind == JudgmentKind::Miss
            && rel_info.kind == ReleaseKind::Hit50
            && !dbg.head_was_hit
            && dbg.has_early_rel
            && dbg.repr_after_rel
            && !dbg.repr_hit_tail
            && current_press_time
                .zip(dbg.first_early_rel)
                .map(|(pt, er)| pt < er && er < ho.time)
                .unwrap_or(false)
            && rel_info.alt_head_press_time == dbg.first_repr_after_rel
            && rel_info.time == dbg.rel_after_repr
            && rel_info
                .alt_head_press_time
                .map(|pt| {
                    pt >= ho.time - windows.hit50
                        && pt < ho.time + windows.hit50
                        && pt <= end_time
                        && calc_hit_kind((pt - ho.time).abs(), windows) == JudgmentKind::Hit200
                })
                .unwrap_or(false)
            && dbg
                .rel_after_repr
                .zip(rel_info.alt_head_press_time)
                .map(|(rr, rp)| rr >= tail_start && rr <= end_time && rr > rp)
                .unwrap_or(false)
            && ln_duration <= windows.hit100;
        let palt = rel_info.rescued
            && pt_already_assigned
            && !dbg.head_was_hit
            && rel_info.kind == ReleaseKind::Hit50
            && rel_info.alt_head_press_time == dbg.first_repr_after_rel
            && rel_info.time == dbg.rel_after_repr
            && rel_info.time.map(|rt| rt < end_time).unwrap_or(false)
            && prev_same_end.map(|pe| pe < ho.time).unwrap_or(false)
            && dbg
                .first_early_rel
                .zip(prev_same_end)
                .map(|(er, pe)| er <= pe)
                .unwrap_or(false)
            && rel_info
                .alt_head_press_time
                .zip(prev_same_end)
                .map(|(ap, pe)| ap > pe && (ap - ho.time).abs() <= windows.max)
                .unwrap_or(false);
        let effective_press_time = if matches!(current_kind, JudgmentKind::Miss) {
            if rel_info.rescued || palt {
                rel_info
                    .alt_head_press_time
                    .or(rescue_repr_for_head)
                    .or(resc_repr_body_break)
                    .or(current_press_time)
            } else if short_frag_alt || sho_pre_gap_alt_head {
                rel_info
                    .alt_head_press_time
                    .or(current_press_time)
                    .or(rescue_repr_for_head)
                    .or(resc_repr_body_break)
            } else {
                current_press_time
                    .or(rescue_repr_for_head)
                    .or(resc_repr_body_break)
                    .or(rel_info.alt_head_press_time)
            }
        } else {
            rescue_repr_for_head
                .or(resc_repr_body_break)
                .or(current_press_time)
        };
        let press_time = match effective_press_time {
            Some(pt) => pt,
            None => {
                judgments[judgment_pos].press_time = None;
                judgments[judgment_pos].kind = JudgmentKind::Miss;
                continue;
            }
        };
        if matches!(rel_info.kind, ReleaseKind::Miss | ReleaseKind::None) {
            if rel_info.rescued {
                judgments[judgment_pos].press_time =
                    dbg.first_repr_after_rel.or(current_press_time);
            } else if !dbg.head_was_hit
                && rel_info.alt_head_press_time.is_some()
                && rel_info.time.is_none()
                && current_press_time
                    .zip(prev_same_end)
                    .map(|(pt, pe)| pt <= pe)
                    .unwrap_or(false)
                && dbg.rel_after_repr.map(|rr| rr <= ho.time).unwrap_or(false)
                && rel_info
                    .alt_head_press_time
                    .map(|ap| (ap - ho.time).abs() <= windows.hit300)
                    .unwrap_or(false)
            {
                judgments[judgment_pos].press_time =
                    rel_info.alt_head_press_time.or(current_press_time);
            }
            judgments[judgment_pos].kind = JudgmentKind::Miss;
            continue;
        }
        let start_diff = (press_time - ho.time).abs();
        let end_diff = (rel_time - end_time).abs();
        let hles_prhd_stbl_trnsf =
            !dbg.head_was_hit && !dbg.has_early_rel && press_time < ho.time - windows.hit50;
        let hles_prhd_force_tail =
            hles_prhd_stbl_trnsf && rel_info.force_kind && rel_info.kind == ReleaseKind::Hit100;
        let scoring_start_diff = if (rel_info.alt_head_press_time.is_some()
            || (hles_prhd_stbl_trnsf && dbg.held_until_end)
            || hles_prhd_force_tail)
            && !dbg.has_early_rel
        {
            if press_time < ho.time - windows.hit50 {
                (ln_duration - 1).max(0)
            } else if press_time < ho.time {
                (ho.time - press_time).saturating_mul(2)
            } else {
                start_diff
            }
        } else {
            start_diff
        };
        let total_diff = scoring_start_diff + end_diff;
        let assigned_start_diff = current_press_time.map(|pt| (pt - ho.time).abs());
        let rescued_short_h100 = rel_info.rescued
            && early_rel_before_head
            && rel_info.kind == ReleaseKind::Hit50
            && ln_duration <= windows.hit100
            && start_diff <= windows.hit100
            && dbg
                .first_early_rel
                .map(|er| {
                    let initial_start_diff = (er - ho.time).abs();
                    initial_start_diff > start_diff && initial_start_diff >= windows.hit100
                })
                .unwrap_or(false);
        let rescued_short_head = rel_info.rescued
            && early_rel_before_head
            && rel_info.kind == ReleaseKind::Hit50
            && ln_duration <= windows.hit100
            && dbg
                .first_repr_after_rel
                .map(|rp| rp <= tail_start)
                .unwrap_or(false)
            && (rel_info.alt_head_press_time.is_some() || dbg.head_was_hit);
        let short_prehead_keeps = !dbg.head_was_hit
            && rel_info.alt_head_press_time.is_some()
            && early_rel_before_head
            && dbg.repr_after_rel
            && ln_duration <= windows.hit100
            && start_diff <= windows.max
            && dbg
                .first_repr_after_rel
                .map(|rp| rp <= ho.time)
                .unwrap_or(false)
            && dbg
                .rel_after_repr
                .map(|rt| rt > end_time && rt <= end_time + windows.hit300)
                .unwrap_or(false);
        let j = &mut judgments[judgment_pos];
        let mut final_kind = if ln_judged_with(scoring_start_diff, total_diff, windows.max, 1.2) {
            JudgmentKind::Max
        } else if ln_judged_with(scoring_start_diff, total_diff, windows.hit300, 1.1) {
            JudgmentKind::Hit300
        } else if ln_judged_with(scoring_start_diff, total_diff, windows.hit200, 1.0) {
            JudgmentKind::Hit200
        } else if ln_judged_with(scoring_start_diff, total_diff, windows.hit100, 1.0) {
            JudgmentKind::Hit100
        } else {
            JudgmentKind::Hit50
        };
        let body_break =
            dbg.has_early_rel && !early_rel_before_head && rel_info.alt_head_press_time.is_none();
        if body_break && resc_repr_body_break.is_none() && final_kind != JudgmentKind::Miss {
            final_kind = JudgmentKind::Hit50;
        }
        if body_break
            && resc_repr_body_break.is_some()
            && matches!(final_kind, JudgmentKind::Max | JudgmentKind::Hit300)
        {
            final_kind = JudgmentKind::Hit200;
        }
        let rescued_body_h50 = rel_info.rescued
            && body_break
            && dbg.repr_after_rel
            && dbg.repr_hit_tail
            && rel_info.kind == ReleaseKind::Hit50
            && rel_info.force_kind
            && final_kind == JudgmentKind::Hit100
            && rel_time > end_time
            && start_diff >= windows.hit100.saturating_sub(2);
        if rescued_body_h50 {
            final_kind = JudgmentKind::Hit50;
        }
        let rescued_body_h100 = rel_info.rescued
            && body_break
            && dbg.head_was_hit
            && dbg.repr_after_rel
            && dbg.repr_hit_tail
            && rel_info.kind == ReleaseKind::Hit50
            && rel_info.force_kind
            && final_kind == JudgmentKind::Hit200
            && ln_duration >= windows.hit50 * 2
            && dbg
                .first_early_rel
                .map(|er| er > ho.time && er < tail_start)
                .unwrap_or(false)
            && dbg
                .first_repr_after_rel
                .zip(dbg.last_repr_time)
                .map(|(rp, last_rp)| {
                    rp == last_rp
                        && rp > ho.time
                        && rp <= ho.time + windows.hit100
                        && rp <= end_time
                })
                .unwrap_or(false)
            && dbg.rel_after_repr.map(|rr| rr > rel_time).unwrap_or(false);
        if rescued_body_h100 {
            final_kind = JudgmentKind::Hit100;
        }
        if dbg.has_early_rel
            && rel_info.alt_head_press_time.is_none()
            && !rel_info.rescued
            && start_diff > windows.hit100
        {
            final_kind = JudgmentKind::Miss;
        }
        if early_rel_before_head
            && dbg.head_was_hit
            && dbg.repr_after_rel
            && dbg.repr_hit_tail
            && j.kind == JudgmentKind::Hit100
            && ln_duration <= windows.hit100
            && !rescued_short_head
            && !rescued_short_h100
            && assigned_start_diff
                .map(|d| d >= windows.hit100)
                .unwrap_or(false)
        {
            final_kind = JudgmentKind::Miss;
        }
        let late_pre_head_repress = early_rel_before_head
            && dbg.repr_after_rel
            && dbg.repr_hit_tail
            && dbg
                .first_repr_after_rel
                .map(|t| t > ho.time + windows.hit300)
                .unwrap_or(false);
        if late_pre_head_repress
            && j.kind == JudgmentKind::Hit100
            && rel_info.kind == ReleaseKind::Hit50
            && ln_duration <= windows.hit100
        {
            final_kind = JudgmentKind::Miss;
        }
        if early_rel_before_head
            && dbg.repr_after_rel
            && dbg.repr_hit_tail
            && !matches!(j.kind, JudgmentKind::Miss)
            && matches!(final_kind, JudgmentKind::Max | JudgmentKind::Hit300)
        {
            final_kind = JudgmentKind::Hit200;
        }
        if rel_info.alt_head_press_time.is_some()
            && matches!(final_kind, JudgmentKind::Max | JudgmentKind::Hit300)
            && !short_prehead_keeps
            && !sho_pre_gap_alt_head
            && !palt
        {
            final_kind = JudgmentKind::Hit200;
        }
        if short_prehead_keeps {
            final_kind = JudgmentKind::Max;
        }
        if rel_info.alt_head_press_time.is_some()
            && !dbg.has_early_rel
            && ln_duration <= windows.hit300.saturating_sub(3)
            && final_kind == JudgmentKind::Hit100
            && matches!(
                rel_info.kind,
                ReleaseKind::Hit300 | ReleaseKind::Hit200 | ReleaseKind::Hit100
            )
        {
            final_kind = JudgmentKind::Hit200;
        }
        let late_altrn_head_repr = early_rel_before_head
            && !dbg.head_was_hit
            && !short_frag_alt
            && rel_info
                .alt_head_press_time
                .map(|pt| pt - ho.time > windows.max + 4)
                .unwrap_or(false)
            && ln_duration <= windows.hit100;
        if late_altrn_head_repr {
            final_kind = JudgmentKind::Miss;
        }
        let rescued_prehead_h50 = rel_info.rescued
            && early_rel_before_head
            && rel_info.kind == ReleaseKind::Hit50
            && ln_duration > windows.hit100
            && matches!(final_kind, JudgmentKind::Hit200 | JudgmentKind::Hit100)
            && dbg
                .first_repr_after_rel
                .map(|rp| rp > tail_start || rp > ho.time + windows.hit100)
                .unwrap_or(false);
        if rescued_prehead_h50 {
            final_kind = JudgmentKind::Hit50;
        }
        let rescued_prehead_fol = rel_info.rescued
            && !dbg.head_was_hit
            && matches!(j.kind, JudgmentKind::Miss)
            && early_rel_before_head
            && rel_info.kind == ReleaseKind::Hit50
            && dbg
                .first_repr_after_rel
                .map(|rp| rp < ho.time)
                .unwrap_or(false)
            && dbg.rel_after_repr.map(|rt| rt < ho.time).unwrap_or(false)
            && dbg
                .last_repr_time
                .map(|rp| rp > ho.time + windows.hit300)
                .unwrap_or(false)
            && !dbg
                .last_repr_time
                .zip(next_same_col_note)
                .map(|(rp, next_ho)| {
                    let next_window_start = next_ho.time - windows.hit50;
                    let next_win_end = next_ho.time + windows.hit100;
                    rp >= next_window_start && rp < next_win_end
                })
                .unwrap_or(false);
        if rescued_prehead_fol {
            final_kind = JudgmentKind::Miss;
        }
        let rescd_zero_head_fol = rel_info.rescued
            && early_rel_before_head
            && dbg.head_was_hit
            && rel_info.kind == ReleaseKind::Hit50
            && ln_duration <= windows.hit100
            && dbg
                .first_repr_after_rel
                .map(|rp| rp < ho.time)
                .unwrap_or(false)
            && dbg.raw_rel_from_press == dbg.first_repr_after_rel
            && dbg
                .rel_after_repr
                .map(|rt| rt > ho.time && rt < end_time)
                .unwrap_or(false)
            && dbg
                .last_repr_time
                .map(|rp| rp > ho.time + windows.max)
                .unwrap_or(false);
        if rescd_zero_head_fol {
            final_kind = JudgmentKind::Miss;
        }
        let alt_head_caps_h50 = !dbg.head_was_hit
            && dbg.has_early_rel
            && !early_rel_before_head
            && rel_info.alt_head_press_time.is_some()
            && rel_info.kind == ReleaseKind::Hit50
            && dbg.rel_after_repr.map(|rt| rt > end_time).unwrap_or(false);
        let rescued_alt_h200 = alt_head_caps_h50
            && rel_info.rescued
            && final_kind == JudgmentKind::Hit200
            && rel_time <= end_time + windows.hit100;
        if alt_head_caps_h50 && !rescued_alt_h200 {
            final_kind = JudgmentKind::Hit50;
        }
        let nea_tai_sta_alt_resc = !dbg.head_was_hit
            && rel_info.alt_head_press_time.is_some()
            && ln_duration > windows.hit50 + windows.hit100 + windows.max
            && dbg
                .first_repr_after_rel
                .map(|rp| rp > tail_start && rp <= tail_start + windows.max)
                .unwrap_or(false);
        let alt_repr_after_tail = !dbg.head_was_hit
            && dbg.has_early_rel
            && rel_info.alt_head_press_time.is_some()
            && !rel_info.rescued
            && !short_frag_alt
            && !sho_pre_gap_alt_head
            && !dbg.last_repr_free
            && dbg
                .first_repr_after_rel
                .map(|rp| rp > tail_start)
                .unwrap_or(false)
            && !nea_tai_sta_alt_resc;
        if alt_repr_after_tail {
            final_kind = JudgmentKind::Miss;
        }
        let is_short_ln = ln_duration <= windows.hit100;
        let low_od_rel_h100_tail = windows.hit100 >= 127 && !rel_info.force_kind;
        if rel_info.kind == ReleaseKind::Hit100
            && start_diff <= windows.max
            && total_diff > windows.hit100
            && matches!(final_kind, JudgmentKind::Max | JudgmentKind::Hit300)
            && !low_od_rel_h100_tail
        {
            final_kind = JudgmentKind::Hit200;
        }
        let sho_ln_hea_h100_edge = start_diff >= windows.hit100.saturating_sub(windows.max);
        let sho_ln_tai_h100_edge = end_diff >= windows.hit100.saturating_sub(1);
        let short_tail_h100 = sho_ln_tai_h100_edge && (rel_time >= end_time || rel_info.force_kind);
        let med_short_hit100 = ln_duration > windows.hit300 + 9
            && ln_duration <= windows.hit300 + windows.max
            && rel_info.kind == ReleaseKind::Hit100
            && end_diff == windows.hit100 - 1
            && start_diff >= windows.hit100 - 9
            && !dbg.has_early_rel;
        if is_short_ln
            && rel_info.kind == ReleaseKind::Hit100
            && final_kind == JudgmentKind::Hit100
            && sho_ln_hea_h100_edge
            && short_tail_h100
            && (rel_info.force_kind || start_diff >= windows.hit100)
            && rel_info.alt_head_press_time.is_none()
            && !med_short_hit100
        {
            final_kind = JudgmentKind::Hit50;
        }
        if is_short_ln
            && ln_duration <= windows.hit300
            && rel_info.kind == ReleaseKind::Hit100
            && final_kind == JudgmentKind::Hit100
            && end_diff == windows.hit100 - 1
            && start_diff >= windows.hit200 + 8
            && !dbg.has_early_rel
        {
            final_kind = JudgmentKind::Hit50;
        }
        if is_short_ln
            && rel_info.force_kind
            && rel_info.kind == ReleaseKind::Hit100
            && final_kind == JudgmentKind::Hit100
            && end_diff == windows.hit100 - 1
            && rel_time >= end_time
            && start_diff >= windows.hit200 + 7
            && !dbg.has_early_rel
        {
            final_kind = JudgmentKind::Hit50;
        }
        let ultra_short_h100 = is_short_ln
            && ln_duration <= windows.hit300
            && !dbg.has_early_rel
            && !rel_info.force_kind
            && rel_info.alt_head_press_time.is_none()
            && rel_info.kind == ReleaseKind::Hit100
            && j.kind == JudgmentKind::Hit100
            && final_kind == JudgmentKind::Hit50
            && end_diff == windows.hit100 - 1;
        if ultra_short_h100 {
            final_kind = JudgmentKind::Hit100;
        }
        let rescue_pre_h50 = rescued_short_head;
        if is_short_ln
            && dbg.has_early_rel
            && start_diff > windows.hit200
            && !rescue_pre_h50
            && !rescued_short_h100
        {
            final_kind = JudgmentKind::Miss;
        }
        if rel_info.force_kind
            && rel_info.kind == ReleaseKind::Hit100
            && matches!(final_kind, JudgmentKind::Max | JudgmentKind::Hit300)
        {
            final_kind = JudgmentKind::Hit200;
        }
        if rel_info.force_kind
            && rel_info.kind == ReleaseKind::Hit100
            && final_kind == JudgmentKind::Hit200
            && end_diff == windows.hit100 - 1
            && dbg.held_until_end
            && !dbg.has_early_rel
            && total_diff + windows.hit50.saturating_sub(windows.hit100) >= windows.hit200 * 2
        {
            final_kind = JudgmentKind::Hit100;
        }
        let rescue_h50_weak = matches!(j.kind, JudgmentKind::Hit50 | JudgmentKind::Miss)
            && assigned_start_diff
                .map(|d| d >= windows.hit100)
                .unwrap_or(false);
        let rescue_h50_over = dbg.has_early_rel
            && dbg.repr_after_rel
            && dbg.repr_hit_tail
            && dbg.total_diff >= windows.hit200 * 2;
        if rel_info.force_kind
            && rel_info.rescued
            && early_rel_before_head
            && rel_info.kind == ReleaseKind::Hit50
            && final_kind == JudgmentKind::Hit200
            && (rescue_h50_weak || rescue_h50_over)
            && total_diff + windows.hit50.saturating_sub(windows.hit100) >= windows.hit200 * 2
        {
            final_kind = JudgmentKind::Hit100;
        }
        let rescued_h50_press = rel_info.force_kind
            && rel_info.rescued
            && early_rel_before_head
            && rel_info.kind == ReleaseKind::Hit50
            && final_kind == JudgmentKind::Hit200
            && ln_duration <= windows.hit50 * 2
            && press_time > ho.time + windows.max;
        if rescued_h50_press {
            final_kind = JudgmentKind::Hit100;
        }
        if is_short_ln
            && ln_duration <= windows.hit300 + windows.max
            && rel_info.kind == ReleaseKind::Hit100
            && final_kind == JudgmentKind::Hit200
            && windows.hit100 < 120
            && start_diff > windows.max + 7
            && start_diff > windows.hit300 + windows.max / 2
            && end_diff == windows.hit100 - 1
            && dbg.held_until_end
            && !dbg.has_early_rel
            && rel_info.alt_head_press_time.is_none()
        {
            final_kind = JudgmentKind::Hit100;
        }
        if is_short_ln
            && ln_duration > windows.hit300 + windows.max
            && rel_info.kind == ReleaseKind::Hit100
            && rel_info.force_kind
            && final_kind == JudgmentKind::Hit200
            && windows.hit100 < 120
            && start_diff >= windows.hit300 - 8
            && end_diff == windows.hit100 - 1
            && dbg.held_until_end
            && !dbg.has_early_rel
        {
            final_kind = JudgmentKind::Hit100;
        }
        if ln_duration >= windows.hit50 * 16
            && rel_info.kind == ReleaseKind::Hit100
            && final_kind == JudgmentKind::Hit200
            && end_diff == windows.hit100 - 1
            && start_diff >= windows.max * 2 + 12
            && dbg.held_until_end
            && !dbg.has_early_rel
        {
            final_kind = JudgmentKind::Hit100;
        }
        if rel_info.alt_head_press_time.is_some()
            && !dbg.has_early_rel
            && ln_duration > windows.hit300
            && ln_duration <= windows.hit300 + 4
            && final_kind == JudgmentKind::Hit100
            && matches!(
                rel_info.kind,
                ReleaseKind::Hit300 | ReleaseKind::Hit200 | ReleaseKind::Hit100
            )
            && start_diff >= windows.hit100 - 1
            && assigned_start_diff
                .map(|d| d > windows.hit50)
                .unwrap_or(false)
        {
            final_kind = JudgmentKind::Hit200;
        }
        let term_long_caps_h200 = no_next_same_col_note
            && !dbg.has_early_rel
            && dbg.held_until_end
            && ln_duration >= windows.hit50 * 50
            && start_diff <= windows.max + 2
            && j.kind == JudgmentKind::Hit300
            && rel_info.kind == ReleaseKind::Max
            && rel_time > end_time
            && final_kind == JudgmentKind::Max;
        if term_long_caps_h200 {
            final_kind = JudgmentKind::Hit200;
        }
        let first_repr_prom_200 = first_repr_alt_head
            && rel_info
                .alt_head_press_time
                .map(|pt| pt < ho.time)
                .unwrap_or(false);
        let display_pt_override = if first_repr_prom_200 {
            final_kind = JudgmentKind::Hit200;
            rel_info.alt_head_press_time
        } else {
            None
        };
        let display_press_time = if rel_info.rescued {
            dbg.first_repr_after_rel.or(Some(press_time))
        } else if display_pt_override.is_some() {
            display_pt_override
        } else {
            Some(press_time)
        };
        j.press_time = display_press_time;
        if let Some(override_press_time) = display_pt_override {
            j.delta = override_press_time - ho.time;
            j.early_press_idx = None;
            j.early_pen_win = None;
        }
        j.kind = final_kind;
        if palt {
            if let Some(r) = ln_releases.get_mut(&idx) {
                r.rescued = false;
            }
        }
    }
}
