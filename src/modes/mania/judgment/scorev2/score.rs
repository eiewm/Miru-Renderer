use super::super::{InternalJudgment, LnDebugInfo, LnReleaseInfo, ReleaseKind};
use crate::modes::mania::judgment::calc_hit_kind;
use crate::types::{HitObject, JudgmentKind, Windows};
use std::collections::HashMap;
const TAIL_WINDOW_SCALE: f32 = 1.5;
fn calc_rel_kind(abs_diff: i32, windows: &Windows) -> ReleaseKind {
    let scaled_diff = (abs_diff as f32) / TAIL_WINDOW_SCALE;
    if scaled_diff <= windows.max as f32 {
        ReleaseKind::Max
    } else if scaled_diff <= windows.hit300 as f32 {
        ReleaseKind::Hit300
    } else if scaled_diff <= windows.hit200 as f32 {
        ReleaseKind::Hit200
    } else if scaled_diff <= windows.hit100 as f32 {
        ReleaseKind::Hit100
    } else if scaled_diff <= windows.hit50 as f32 {
        ReleaseKind::Hit50
    } else {
        ReleaseKind::Miss
    }
}
pub(crate) fn finalize_judgments(
    judgments: &mut [InternalJudgment],
    hit_objects: &[HitObject],
    ln_releases: &mut HashMap<usize, LnReleaseInfo>,
    ln_debug: &HashMap<usize, LnDebugInfo>,
    windows: &Windows,
) {
    for (idx, ho) in hit_objects.iter().enumerate() {
        if !ho.is_long_note() {
            continue;
        }
        let Some(rel_info) = ln_releases.get_mut(&idx) else {
            continue;
        };
        let Some(dbg) = ln_debug.get(&idx) else {
            continue;
        };
        let Some(judgment_pos) = judgments.iter().position(|j| j.index == idx) else {
            continue;
        };
        let Some(end_time) = ho.end_time else {
            continue;
        };
        let prev_same_col = hit_objects[..idx]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, prev_ho)| prev_ho.column == ho.column);
        let prev_same_col_is_ln = prev_same_col
            .map(|(_, prev_ho)| prev_ho.is_long_note())
            .unwrap_or(false);
        let prev_hless_miss = prev_same_col
            .and_then(|(prev_idx, prev_ho)| {
                prev_ho.is_long_note().then(|| {
                    judgments
                        .iter()
                        .find(|j| j.index == prev_idx)
                        .map(|j| j.kind == JudgmentKind::Miss && j.press_time.is_none())
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        let next_same_col_time = hit_objects[idx + 1..]
            .iter()
            .find(|next_ho| next_ho.column == ho.column)
            .map(|next_ho| next_ho.time);
        let prehead_h100_prom_cur = !prev_same_col_is_ln
            && rel_info.alt_head_press_time.is_none()
            && dbg.head_was_hit
            && judgments[judgment_pos].kind == JudgmentKind::Hit100
            && rel_info.kind == ReleaseKind::Miss
            && judgments[judgment_pos]
                .press_time
                .map(|pt| pt < ho.time)
                .unwrap_or(false)
            && dbg.has_early_rel
            && dbg
                .first_early_rel
                .zip(rel_info.time)
                .map(|(first_rt, current_rt)| first_rt == current_rt && first_rt < ho.time)
                .unwrap_or(false)
            && dbg.repr_after_rel
            && !dbg.repr_hit_tail
            && (end_time - ho.time) <= windows.hit50 + windows.max
            && dbg
                .first_repr_after_rel
                .zip(dbg.rel_after_repr)
                .map(|(rp, rr)| {
                    rp > ho.time
                        && rp <= end_time
                        && rr > rp
                        && calc_hit_kind((rp - ho.time).abs(), windows) == JudgmentKind::Max
                        && calc_rel_kind((rr - end_time).abs(), windows) == ReleaseKind::Max
                })
                .unwrap_or(false);
        if prehead_h100_prom_cur {
            let Some((new_press_time, new_rel_time)) =
                dbg.first_repr_after_rel.zip(dbg.rel_after_repr)
            else {
                continue;
            };
            let press_time_taken = judgments.iter().enumerate().any(|(other_pos, other)| {
                other_pos != judgment_pos
                    && other.press_time == Some(new_press_time)
                    && hit_objects
                        .get(other.index)
                        .map(|other_ho| other_ho.column == ho.column)
                        .unwrap_or(false)
            });
            if press_time_taken {
                continue;
            }
            judgments[judgment_pos].press_time = Some(new_press_time);
            judgments[judgment_pos].kind = JudgmentKind::Max;
            judgments[judgment_pos].delta = new_press_time - ho.time;
            judgments[judgment_pos].early_press_idx = None;
            judgments[judgment_pos].early_pen_win = None;
            rel_info.time = Some(new_rel_time);
            rel_info.kind = ReleaseKind::Max;
            rel_info.force_kind = false;
            rel_info.rescued = false;
            rel_info.alt_head_press_time = Some(new_press_time);
            continue;
        }
        let start_diff = judgments[judgment_pos]
            .press_time
            .map(|pt| (pt - ho.time).abs())
            .unwrap_or(i32::MAX);
        let end_diff = rel_info
            .time
            .map(|rt| (rt - end_time).abs())
            .unwrap_or(i32::MAX);
        let duration = end_time - ho.time;
        let late_tai_head = judgments[judgment_pos].press_time.and_then(|pt| {
            let d = pt - ho.time;
            let k = judgments[judgment_pos].kind;
            let tail_ok = rel_info
                .time
                .map(|rt| {
                    rt > pt && !matches!(rel_info.kind, ReleaseKind::Miss | ReleaseKind::None)
                })
                .unwrap_or(false);
            let h50 = k == JudgmentKind::Hit50 && d > windows.hit100;
            let h100_post = k == JudgmentKind::Hit100 && pt > end_time && d >= windows.hit100;
            let h100_tail =
                k == JudgmentKind::Hit100 && pt >= ho.time + windows.hit100 && pt <= end_time;
            (tail_ok && (h50 || h100_post || h100_tail)).then_some(pt)
        });
        if let Some(pt) = late_tai_head {
            judgments[judgment_pos].press_time = None;
            judgments[judgment_pos].kind = JudgmentKind::Miss;
            judgments[judgment_pos].delta = 0;
            judgments[judgment_pos].early_press_idx = Some(pt);
            judgments[judgment_pos].early_pen_win = None;
            rel_info.alt_head_press_time = None;
            continue;
        }
        let sta_tap_tail = judgments[judgment_pos].press_time.and_then(|pt| {
            let (pi, ph) = prev_same_col?;
            if ph.is_long_note() || pt - ph.time != windows.hit100 {
                return None;
            }
            let pm = judgments
                .iter()
                .find(|j| j.index == pi)
                .map(|j| j.kind == JudgmentKind::Miss && j.press_time.is_none())
                .unwrap_or(false);
            let nm = hit_objects[(idx + 1)..]
                .iter()
                .enumerate()
                .find(|(_, nh)| nh.column == ho.column)
                .and_then(|(off, nh)| (!nh.is_long_note()).then_some(idx + 1 + off))
                .and_then(|ni| judgments.iter().find(|j| j.index == ni))
                .map(|nj| nj.kind == JudgmentKind::Miss && nj.press_time.is_none())
                .unwrap_or(false);
            let tk = rel_info
                .time
                .map(|rt| {
                    rt > end_time
                        && rt > pt
                        && !matches!(rel_info.kind, ReleaseKind::Miss | ReleaseKind::None)
                })
                .unwrap_or(false);
            (pm && nm
                && tk
                && duration <= windows.hit100
                && judgments[judgment_pos].kind == JudgmentKind::Hit200
                && pt < ho.time)
                .then_some(pt)
        });
        if sta_tap_tail.is_some() {
            judgments[judgment_pos].press_time = None;
            judgments[judgment_pos].kind = JudgmentKind::Miss;
            judgments[judgment_pos].delta = 0;
            judgments[judgment_pos].early_press_idx = None;
            judgments[judgment_pos].early_pen_win = None;
            rel_info.alt_head_press_time = None;
            continue;
        }
        let ralt_pt = if prev_hless_miss
            && rel_info.rescued
            && rel_info.kind == ReleaseKind::Hit50
            && !dbg.head_was_hit
            && judgments[judgment_pos].kind == JudgmentKind::Miss
            && judgments[judgment_pos]
                .press_time
                .map(|pt| pt < ho.time)
                .unwrap_or(false)
            && rel_info
                .time
                .map(|rt| rt > ho.time && rt < end_time)
                .unwrap_or(false)
            && next_same_col_time
                .map(|next_time| next_time - end_time > windows.hit50)
                .unwrap_or(true)
            && duration <= windows.hit50 + windows.hit100
        {
            rel_info
                .alt_head_press_time
                .or(dbg.first_repr_after_rel)
                .or(dbg.last_repr_time)
                .filter(|pt| *pt >= ho.time && *pt <= end_time)
        } else {
            None
        };
        let Some(new_press_time) = rel_info.alt_head_press_time.or(ralt_pt) else {
            continue;
        };
        let press_time_taken = judgments.iter().enumerate().any(|(other_pos, other)| {
            other_pos != judgment_pos
                && other.press_time == Some(new_press_time)
                && hit_objects
                    .get(other.index)
                    .map(|other_ho| other_ho.column == ho.column)
                    .unwrap_or(false)
        });
        if press_time_taken {
            continue;
        }
        let alternate_head_kind = calc_hit_kind((new_press_time - ho.time).abs(), windows);
        let alternate_tail_kind = calc_rel_kind(end_diff, windows);
        let exis_clean_head_swap = dbg.head_was_hit
            && !dbg.has_early_rel
            && !dbg.repr_after_rel
            && !rel_info.rescued
            && judgments[judgment_pos].kind != JudgmentKind::Miss;
        let short_pro_alt = !dbg.head_was_hit
            && dbg.has_early_rel
            && dbg.repr_after_rel
            && judgments[judgment_pos].kind == JudgmentKind::Miss
            && judgments[judgment_pos]
                .press_time
                .map(|pt| pt < ho.time)
                .unwrap_or(false)
            && judgments[judgment_pos].early_pen_win.is_none()
            && dbg.first_early_rel.map(|rt| rt < ho.time).unwrap_or(false)
            && dbg.first_repr_after_rel == Some(new_press_time)
            && dbg.rel_after_repr == rel_info.time
            && duration <= windows.hit100
            && start_diff > windows.hit50 + windows.hit300
            && start_diff <= windows.hit50 + windows.hit100 + windows.max
            && alternate_head_kind == JudgmentKind::Max
            && alternate_tail_kind == ReleaseKind::Max;
        let short_h50_alt = !dbg.head_was_hit
            && dbg.has_early_rel
            && dbg.repr_after_rel
            && judgments[judgment_pos].kind == JudgmentKind::Miss
            && judgments[judgment_pos]
                .press_time
                .map(|pt| pt < ho.time)
                .unwrap_or(false)
            && judgments[judgment_pos].early_pen_win.is_none()
            && dbg.first_early_rel.map(|rt| rt < ho.time).unwrap_or(false)
            && dbg.first_repr_after_rel == Some(new_press_time)
            && dbg.rel_after_repr == rel_info.time
            && duration <= windows.hit50
            && start_diff > windows.hit50 + windows.hit300
            && start_diff <= windows.hit50 + windows.hit100 + windows.max
            && alternate_head_kind == JudgmentKind::Max
            && alternate_tail_kind == ReleaseKind::Hit300;
        let ultra_stale_alt = !dbg.head_was_hit
            && dbg.has_early_rel
            && dbg.repr_after_rel
            && judgments[judgment_pos].kind == JudgmentKind::Miss
            && judgments[judgment_pos]
                .press_time
                .map(|pt| pt < ho.time)
                .unwrap_or(false)
            && judgments[judgment_pos].early_pen_win.is_none()
            && dbg.first_early_rel.map(|rt| rt < ho.time).unwrap_or(false)
            && dbg.first_repr_after_rel == Some(new_press_time)
            && dbg.rel_after_repr == rel_info.time
            && duration <= windows.max + windows.hit300
            && start_diff >= windows.hit50 * 3
            && alternate_head_kind == JudgmentKind::Max
            && alternate_tail_kind == ReleaseKind::Max;
        let rescued_single_alt = !dbg.head_was_hit
            && dbg.has_early_rel
            && dbg.repr_after_rel
            && dbg.repr_hit_tail
            && rel_info.rescued
            && judgments[judgment_pos].kind == JudgmentKind::Miss
            && judgments[judgment_pos]
                .press_time
                .map(|pt| pt < ho.time)
                .unwrap_or(false)
            && judgments[judgment_pos].early_pen_win.is_some()
            && dbg.first_early_rel.map(|rt| rt < ho.time).unwrap_or(false)
            && dbg
                .first_early_rel
                .map(|rt| ho.time - rt > windows.hit100)
                .unwrap_or(false)
            && dbg.first_repr_after_rel == Some(new_press_time)
            && dbg.last_repr_time == Some(new_press_time)
            && dbg.rel_after_repr == rel_info.time
            && dbg.rel_after_repr.map(|rt| rt < end_time).unwrap_or(false)
            && dbg
                .first_repr_after_rel
                .map(|pt| pt >= ho.time && pt - ho.time <= windows.max)
                .unwrap_or(false)
            && next_same_col_time
                .map(|next_time| next_time - end_time > windows.hit50 + windows.hit100)
                .unwrap_or(true)
            && duration <= windows.hit50 + windows.hit100
            && start_diff > windows.hit50 + windows.max
            && start_diff <= windows.hit50 + windows.hit200
            && alternate_head_kind == JudgmentKind::Max
            && alternate_tail_kind == ReleaseKind::Hit300;
        let hls_alt = !dbg.head_was_hit
            && judgments[judgment_pos].kind == JudgmentKind::Miss
            && judgments[judgment_pos].press_time.is_none()
            && rel_info.alt_head_press_time == Some(new_press_time)
            && duration <= windows.hit100
            && new_press_time >= ho.time
            && new_press_time <= end_time
            && rel_info.time.map(|rt| rt > end_time).unwrap_or(false)
            && matches!(
                alternate_head_kind,
                JudgmentKind::Max | JudgmentKind::Hit300
            )
            && !matches!(alternate_tail_kind, ReleaseKind::Miss | ReleaseKind::None);
        let ralt = ralt_pt == Some(new_press_time)
            && rel_info
                .time
                .map(|rt| rt > new_press_time && rt < end_time)
                .unwrap_or(false)
            && alternate_head_kind == JudgmentKind::Max
            && alternate_tail_kind == ReleaseKind::Hit300;
        if !exis_clean_head_swap
            && !short_pro_alt
            && !short_h50_alt
            && !ultra_stale_alt
            && !rescued_single_alt
            && !hls_alt
            && !ralt
        {
            continue;
        }
        judgments[judgment_pos].press_time = Some(new_press_time);
        if short_pro_alt
            || short_h50_alt
            || ultra_stale_alt
            || rescued_single_alt
            || hls_alt
            || ralt
        {
            judgments[judgment_pos].kind = alternate_head_kind;
            judgments[judgment_pos].delta = new_press_time - ho.time;
            judgments[judgment_pos].early_press_idx = None;
            judgments[judgment_pos].early_pen_win = None;
            rel_info.kind = alternate_tail_kind;
            rel_info.force_kind = false;
            if ralt {
                rel_info.rescued = false;
                rel_info.alt_head_press_time = Some(new_press_time);
            }
        }
    }
}
