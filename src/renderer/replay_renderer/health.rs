use super::model::{LnComboBreak, LnComboTick, LnReleaseInfo, ReleaseKind, RenderJudgment};
use super::render::ReplayRenderer;
use crate::types::{BreakPeriod, HitObject, JudgmentKind};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthEventKind {
    Tap,
    LnHead,
    LnTail,
    LnBodyTick,
    LnBodyBreak,
}
#[derive(Debug, Clone, Copy)]
pub struct HealthEvent {
    pub time: i32,
    pub note_index: usize,
    pub kind: HealthEventKind,
    pub delta: f32,
    pub life_after: f32,
}
#[derive(Debug, Clone, Default)]
pub struct HealthTimeline {
    pub initial_life: f32,
    pub events: Vec<HealthEvent>,
    pub hp_multiplier_normal: f32,
    pub fail_time_ms: Option<i32>,
}
impl HealthTimeline {
    pub fn life_at_time(&self, time: i32) -> f32 {
        // Events are sorted by time, so partition_point gives the last applied life state.
        let idx = self.events.partition_point(|event| event.time <= time);
        if idx == 0 {
            self.initial_life
        } else {
            self.events[idx - 1].life_after
        }
    }
}
#[derive(Debug, Clone, Copy)]
struct PendingHealthEvent {
    time: i32,
    note_index: usize,
    kind: HealthEventKind,
    judgment: Option<JudgmentKind>,
}
impl ReplayRenderer {
    pub fn precompute_health_timeline(
        &self,
        hit_objects: &[HitObject],
        breaks: &[BreakPeriod],
        judgments_by_idx: &[Option<RenderJudgment>],
        ln_releases_by_idx: &[Option<LnReleaseInfo>],
        _ln_ticks: &[LnComboTick],
        _ln_breaks: &[LnComboBreak],
        drain_rate: f32,
    ) -> HealthTimeline {
        let hp_multiplier_normal = compute_hp_multiplier_normal(hit_objects, breaks, drain_rate);
        let mut pending = Vec::with_capacity(hit_objects.len() * 2);
        for (idx, hit_object) in hit_objects.iter().enumerate() {
            if hit_object.is_long_note() {
                let head = judgments_by_idx.get(idx).copied().flatten();
                // LN head and tail contribute separate health events in mania.
                pending.push(PendingHealthEvent {
                    time: head
                        .and_then(|judgment| judgment.press_time)
                        .unwrap_or(hit_object.time),
                    note_index: idx,
                    kind: HealthEventKind::LnHead,
                    judgment: Some(
                        head.map(|judgment| judgment.kind)
                            .unwrap_or(JudgmentKind::Miss),
                    ),
                });
                let end_time = hit_object.end_time.unwrap_or(hit_object.time);
                let release = ln_releases_by_idx.get(idx).copied().flatten();
                pending.push(PendingHealthEvent {
                    time: release.and_then(|info| info.time).unwrap_or(end_time),
                    note_index: idx,
                    kind: HealthEventKind::LnTail,
                    judgment: Some(
                        release
                            .map(|info| rel_kind_to_judgment(info.kind))
                            .unwrap_or(JudgmentKind::Miss),
                    ),
                });
            } else {
                let judgment = judgments_by_idx.get(idx).copied().flatten();
                pending.push(PendingHealthEvent {
                    time: judgment
                        .and_then(|render_judgment| render_judgment.press_time)
                        .unwrap_or(hit_object.time),
                    note_index: idx,
                    kind: HealthEventKind::Tap,
                    judgment: Some(
                        judgment
                            .map(|render_judgment| render_judgment.kind)
                            .unwrap_or(JudgmentKind::Miss),
                    ),
                });
            }
        }
        build_health_timeline_from_pending(&pending, drain_rate, hp_multiplier_normal, 1.0)
    }
    pub fn health_at_time(timeline: &HealthTimeline, time: i32) -> f32 {
        timeline.life_at_time(time)
    }
}
fn build_health_timeline_from_pending(
    pending: &[PendingHealthEvent],
    drain_rate: f32,
    hp_multiplier_normal: f32,
    starting_life: f32,
) -> HealthTimeline {
    let mut sorted = pending.to_vec();
    sorted.sort_by_key(|event| {
        (
            event.time,
            event.note_index,
            // Same-time events must apply in gameplay order so tails do not precede heads.
            health_event_priority(event.kind),
        )
    });
    let mut life = starting_life.clamp(0.0, 1.0);
    let mut fail_time_ms = None;
    let mut events = Vec::with_capacity(sorted.len());
    for event in sorted {
        let delta = event
            .judgment
            .map(|judgment| {
                health_delta_for_judgment(event.kind, judgment, drain_rate, hp_multiplier_normal)
            })
            .unwrap_or_else(|| {
                health_delta_for_body_event(event.kind, drain_rate, hp_multiplier_normal)
            });
        life = (life + delta).clamp(0.0, 1.0);
        if fail_time_ms.is_none() && life <= 0.0 {
            fail_time_ms = Some(event.time);
        }
        events.push(HealthEvent {
            time: event.time,
            note_index: event.note_index,
            kind: event.kind,
            delta,
            life_after: life,
        });
    }
    HealthTimeline {
        initial_life: starting_life.clamp(0.0, 1.0),
        events,
        hp_multiplier_normal,
        fail_time_ms,
    }
}
fn compute_hp_multiplier_normal(
    hit_objects: &[HitObject],
    breaks: &[BreakPeriod],
    drain_rate: f32,
) -> f32 {
    if hit_objects.is_empty() {
        return 1.0;
    }
    let lowest_hp_ever = difficulty_range(drain_rate, 0.975, 0.8, 0.3);
    let lowest_hp_end = difficulty_range(drain_rate, 0.99, 0.9, 0.4);
    let hp_recovery_available = difficulty_range(drain_rate, 0.04, 0.02, 0.0);
    let mut hp_multiplier_normal = 1.0f32;
    let mut test_drop = 0.00025f32;
    // Stable derives HP gain by simulating a perfect play until drain and recovery constraints pass.
    for _ in 0..10_000 {
        let mut current_hp = 1.0f32;
        let mut current_hp_uncapped = 1.0f32;
        let mut last_time = 0i32;
        let mut current_break = 0usize;
        let mut fail = false;
        for hit_object in hit_objects {
            while current_break < breaks.len() && breaks[current_break].end <= hit_object.time {
                // Breaks suspend passive drain; resume from the next hit object's time.
                last_time = hit_object.time;
                current_break += 1;
            }
            let gap_ms = (hit_object.time - last_time).max(0) as f32;
            reduce_hp(
                test_drop * gap_ms,
                &mut current_hp,
                &mut current_hp_uncapped,
            );
            if current_hp <= lowest_hp_ever {
                fail = true;
                test_drop *= 0.96;
                break;
            }
            let sustain_ms = hit_object
                .end_time
                .filter(|end_time| *end_time > hit_object.time)
                .unwrap_or(hit_object.time)
                - hit_object.time;
            let hp_reduction = test_drop * sustain_ms.max(0) as f32;
            let hp_overkill = (hp_reduction - current_hp).max(0.0);
            reduce_hp(hp_reduction, &mut current_hp, &mut current_hp_uncapped);
            if hit_object.is_long_note() {
                increase_hp(
                    health_delta_for_judgment(
                        HealthEventKind::LnHead,
                        JudgmentKind::Max,
                        drain_rate,
                        hp_multiplier_normal,
                    ),
                    &mut current_hp,
                    &mut current_hp_uncapped,
                );
                increase_hp(
                    health_delta_for_judgment(
                        HealthEventKind::LnTail,
                        JudgmentKind::Max,
                        drain_rate,
                        hp_multiplier_normal,
                    ),
                    &mut current_hp,
                    &mut current_hp_uncapped,
                );
            } else {
                increase_hp(
                    health_delta_for_judgment(
                        HealthEventKind::Tap,
                        JudgmentKind::Max,
                        drain_rate,
                        hp_multiplier_normal,
                    ),
                    &mut current_hp,
                    &mut current_hp_uncapped,
                );
            }
            if hp_overkill > 0.0 && current_hp - hp_overkill <= lowest_hp_ever {
                fail = true;
                test_drop *= 0.96;
                break;
            }
            last_time = hit_object.end_time.unwrap_or(hit_object.time);
        }
        if !fail && current_hp < lowest_hp_end {
            fail = true;
            test_drop *= 0.94;
            hp_multiplier_normal *= 1.01;
        }
        let recovery = (current_hp_uncapped - 1.0) / hit_objects.len().max(1) as f32;
        if !fail && recovery < hp_recovery_available {
            fail = true;
            test_drop *= 0.96;
            hp_multiplier_normal *= 1.01;
        }
        if !fail {
            return if hp_multiplier_normal.is_finite() {
                hp_multiplier_normal
            } else {
                1.0
            };
        }
    }
    hp_multiplier_normal
}
fn reduce_hp(amount: f32, current_hp: &mut f32, current_hp_uncapped: &mut f32) {
    *current_hp_uncapped = (*current_hp_uncapped - amount).max(0.0);
    *current_hp = (*current_hp - amount).max(0.0);
}
fn increase_hp(amount: f32, current_hp: &mut f32, current_hp_uncapped: &mut f32) {
    *current_hp_uncapped += amount;
    *current_hp = (*current_hp + amount).clamp(0.0, 1.0);
}
fn health_event_priority(kind: HealthEventKind) -> u8 {
    match kind {
        HealthEventKind::Tap | HealthEventKind::LnHead => 0,
        HealthEventKind::LnBodyBreak => 1,
        HealthEventKind::LnBodyTick => 2,
        HealthEventKind::LnTail => 3,
    }
}
fn health_delta_for_judgment(
    event_kind: HealthEventKind,
    judgment: JudgmentKind,
    drain_rate: f32,
    hp_multiplier_normal: f32,
) -> f32 {
    let drain_factor = drain_rate + 1.0;
    match judgment {
        JudgmentKind::Miss => {
            if matches!(
                event_kind,
                HealthEventKind::LnHead | HealthEventKind::LnTail
            ) {
                -(drain_factor * 0.00375)
            } else {
                -(drain_factor * 0.0075)
            }
        }
        JudgmentKind::Hit50 => -(drain_factor * 0.0016),
        JudgmentKind::Hit100 => 0.0,
        JudgmentKind::Hit200 => hp_multiplier_normal * (0.004 - drain_rate * 0.0004),
        JudgmentKind::Hit300 => hp_multiplier_normal * (0.005 - drain_rate * 0.0005),
        JudgmentKind::Max => hp_multiplier_normal * (0.0055 - drain_rate * 0.0005),
    }
}
fn health_delta_for_body_event(
    event_kind: HealthEventKind,
    _drain_rate: f32,
    _hp_multiplier_normal: f32,
) -> f32 {
    match event_kind {
        HealthEventKind::LnBodyTick | HealthEventKind::LnBodyBreak => 0.0,
        _ => 0.0,
    }
}
fn rel_kind_to_judgment(kind: ReleaseKind) -> JudgmentKind {
    match kind {
        ReleaseKind::Max => JudgmentKind::Max,
        ReleaseKind::Hit300 => JudgmentKind::Hit300,
        ReleaseKind::Hit200 => JudgmentKind::Hit200,
        ReleaseKind::Hit100 => JudgmentKind::Hit100,
        ReleaseKind::Hit50 => JudgmentKind::Hit50,
        ReleaseKind::Miss | ReleaseKind::None => JudgmentKind::Miss,
    }
}
fn difficulty_range(value: f32, min: f32, mid: f32, max: f32) -> f32 {
    let clamped = value.clamp(0.0, 10.0);
    if clamped > 5.0 {
        mid + (max - mid) * (clamped - 5.0) / 5.0
    } else if clamped < 5.0 {
        mid - (mid - min) * (5.0 - clamped) / 5.0
    } else {
        mid
    }
}
