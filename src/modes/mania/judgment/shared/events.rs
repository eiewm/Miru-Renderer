use super::score_mode::ScoreMode;
use super::types::{InternalJudgment, LnReleaseInfo};
use crate::types::{HitObject, JudgmentKind};
use std::collections::HashMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreJudgmentPart {
    Tap,
    LnHead,
    LnTail,
}
#[derive(Debug, Clone, Copy)]
pub struct ScoreJudgmentEvent {
    pub note_index: usize,
    pub column: u8,
    pub part: ScoreJudgmentPart,
    pub kind: JudgmentKind,
    pub event_time: i32,
    pub hit_error_offset_ms: Option<i32>,
    pub is_ln: bool,
}
impl ScoreJudgmentEvent {
    #[inline]
    pub fn breaks_combo(self, mode: ScoreMode) -> bool {
        match mode {
            ScoreMode::ScoreV1 => self.kind.breaks_combo(),
            ScoreMode::ScoreV2 => self.kind.breaks_combo_v2(),
            ScoreMode::Lazer => self.kind.breaks_combo(),
        }
    }
}
pub fn build_score_judgments(
    hit_objects: &[HitObject],
    judgments: &[InternalJudgment],
    ln_releases: &HashMap<usize, LnReleaseInfo>,
    score_mode: ScoreMode,
) -> Vec<ScoreJudgmentEvent> {
    let mut out = Vec::with_capacity(judgments.len() * 2);
    for j in judgments {
        let ho = match hit_objects.get(j.index) {
            Some(v) => v,
            None => continue,
        };
        let is_ln = ho.is_long_note();
        match score_mode {
            ScoreMode::ScoreV1 => {
                let event_time = if is_ln {
                    ln_releases
                        .get(&j.index)
                        .and_then(|r| r.time)
                        .or(j.press_time)
                        .unwrap_or(j.note_time)
                } else {
                    j.press_time.unwrap_or(j.note_time)
                };
                out.push(ScoreJudgmentEvent {
                    note_index: j.index,
                    column: j.column,
                    part: ScoreJudgmentPart::Tap,
                    kind: j.kind,
                    event_time,
                    hit_error_offset_ms: if j.kind == JudgmentKind::Miss {
                        None
                    } else {
                        j.press_time.map(|press_time| press_time - ho.time)
                    },
                    is_ln,
                });
            }
            ScoreMode::ScoreV2 | ScoreMode::Lazer => {
                if !is_ln {
                    let event_time = j.press_time.unwrap_or(j.note_time);
                    out.push(ScoreJudgmentEvent {
                        note_index: j.index,
                        column: j.column,
                        part: ScoreJudgmentPart::Tap,
                        kind: j.kind,
                        event_time,
                        hit_error_offset_ms: if j.kind == JudgmentKind::Miss {
                            None
                        } else {
                            Some(event_time - ho.time)
                        },
                        is_ln: false,
                    });
                    continue;
                }
                let head_time = j.press_time.unwrap_or(j.note_time);
                out.push(ScoreJudgmentEvent {
                    note_index: j.index,
                    column: j.column,
                    part: ScoreJudgmentPart::LnHead,
                    kind: j.kind,
                    event_time: head_time,
                    hit_error_offset_ms: if j.kind == JudgmentKind::Miss {
                        None
                    } else {
                        Some(head_time - ho.time)
                    },
                    is_ln: true,
                });
                let end_time = ho.end_time.unwrap_or(ho.time);
                if let Some(rel) = ln_releases.get(&j.index) {
                    if let Some(tail_kind) = rel.kind.as_judgment_kind() {
                        let tail_time = rel.time.unwrap_or(end_time);
                        out.push(ScoreJudgmentEvent {
                            note_index: j.index,
                            column: j.column,
                            part: ScoreJudgmentPart::LnTail,
                            kind: tail_kind,
                            event_time: tail_time,
                            hit_error_offset_ms: if tail_kind == JudgmentKind::Miss {
                                None
                            } else {
                                Some(tail_time - end_time)
                            },
                            is_ln: true,
                        });
                    }
                } else {
                    out.push(ScoreJudgmentEvent {
                        note_index: j.index,
                        column: j.column,
                        part: ScoreJudgmentPart::LnTail,
                        kind: JudgmentKind::Miss,
                        event_time: end_time,
                        hit_error_offset_ms: None,
                        is_ln: true,
                    });
                }
            }
        }
    }
    out.sort_by_key(|e| (e.event_time, e.note_index, e.part as u8));
    out
}
