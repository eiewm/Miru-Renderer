use crate::types::JudgmentKind;
const MAX_SCORE: u32 = 1_000_000;
#[derive(Debug, Clone)]
pub struct ComboBreakAnimation {
    pub start_combo: u32,
    pub break_time: i32,
}
#[derive(Debug, Clone)]
pub struct ComboCountdown {
    pub from_combo: u32,
    pub break_time: i32,
    pub current_value: u32,
    pub cancelled: bool,
}
#[derive(Debug, Clone)]
pub struct LastJudgment {
    pub kind: JudgmentKind,
    pub age: i32,
    pub column: u8,
}
#[derive(Debug, Clone)]
pub struct ComboEvent {
    pub event_type: ComboEventType,
    pub age: i32,
    pub combo: Option<u32>,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ComboEventType {
    Inc,
    Break,
}
#[derive(Debug, Clone)]
pub struct HudState {
    pub combo: u32,
    pub score: u32,
    pub accuracy: f32,
    pub progress: f32,
    pub last: Option<LastJudgment>,
    pub combo_event: Option<ComboEvent>,
    pub combo_break_anim: Option<ComboBreakAnimation>,
    pub combo_countdown: Option<ComboCountdown>,
}
#[derive(Debug, Clone)]
pub struct HudJudgment {
    pub time: i32,
    pub kind: JudgmentKind,
    pub column: u8,
    pub is_ln: bool,
}
#[derive(Debug, Clone)]
pub struct LnComboTick {
    pub time: i32,
    pub column: u8,
    pub ln_index: usize,
}
#[derive(Debug, Clone)]
pub struct LnComboBreak {
    pub time: i32,
    pub column: u8,
    pub ln_index: usize,
}
#[derive(Debug, Clone, Default)]
pub struct HudComputeOpts {
    pub mod_multiplier: Option<f32>,
    pub mod_divider: Option<f32>,
    pub anim_ms: Option<i32>,
    pub ln_combo_ticks: Vec<LnComboTick>,
    pub ln_combo_breaks: Vec<LnComboBreak>,
    pub is_score_v2: bool,
}
const fn hit_value(kind: JudgmentKind) -> u32 {
    match kind {
        JudgmentKind::Max => 320,
        JudgmentKind::Hit300 => 300,
        JudgmentKind::Hit200 => 200,
        JudgmentKind::Hit100 => 100,
        JudgmentKind::Hit50 => 50,
        JudgmentKind::Miss => 0,
    }
}
fn acc_value(kind: JudgmentKind, is_score_v2: bool) -> u32 {
    match kind {
        JudgmentKind::Max => {
            if is_score_v2 {
                305
            } else {
                300
            }
        }
        JudgmentKind::Hit300 => 300,
        JudgmentKind::Hit200 => 200,
        JudgmentKind::Hit100 => 100,
        JudgmentKind::Hit50 => 50,
        JudgmentKind::Miss => 0,
    }
}
const fn hit_bonus_value(kind: JudgmentKind) -> u32 {
    match kind {
        JudgmentKind::Max | JudgmentKind::Hit300 => 32,
        JudgmentKind::Hit200 => 16,
        JudgmentKind::Hit100 => 8,
        JudgmentKind::Hit50 => 4,
        JudgmentKind::Miss => 0,
    }
}
const fn hit_bonus_add(kind: JudgmentKind) -> i32 {
    match kind {
        JudgmentKind::Max => 2,
        JudgmentKind::Hit300 => 1,
        _ => 0,
    }
}
fn hit_punish(kind: JudgmentKind) -> Option<i32> {
    match kind {
        JudgmentKind::Max | JudgmentKind::Hit300 => Some(0),
        JudgmentKind::Hit200 => Some(8),
        JudgmentKind::Hit100 => Some(24),
        JudgmentKind::Hit50 => Some(44),
        JudgmentKind::Miss => None,
    }
}
pub fn compute_hud_metrics(
    judgments: &[HudJudgment],
    t0: i32,
    total_frames: usize,
    frame_time: f32,
    opts: &HudComputeOpts,
) -> Vec<HudState> {
    let mod_multiplier = opts.mod_multiplier.unwrap_or(1.0).max(0.0);
    let mod_divider = opts.mod_divider.unwrap_or(1.0).max(1e-6);
    let anim_ms = opts.anim_ms.unwrap_or(250).max(1);
    let is_score_v2 = opts.is_score_v2;
    let total_notes = judgments.len().max(1);
    // Stable score splits the million-point budget between hit value and bonus.
    let unit = (MAX_SCORE as f32 * mod_multiplier * 0.5) / total_notes as f32;
    let acc_max = if is_score_v2 { 305 } else { 300 };
    struct ScoreEvent {
        t: i32,
        delta: f32,
        _j: HudJudgment,
    }
    let mut events: Vec<ScoreEvent> = Vec::with_capacity(judgments.len());
    let mut bonus: f32 = 100.0;
    for j in judgments {
        let hv = hit_value(j.kind);
        let hbv = hit_bonus_value(j.kind);
        let base = unit * (hv as f32 / 320.0);
        let bonus_add = unit * ((hbv as f32 * bonus.clamp(0.0, 100.0).sqrt()) / 320.0);
        events.push(ScoreEvent {
            t: j.time,
            delta: base + bonus_add,
            _j: j.clone(),
        });
        match hit_punish(j.kind) {
            None => bonus = 0.0,
            Some(punish) => {
                bonus = (bonus + hit_bonus_add(j.kind) as f32 - punish as f32 / mod_divider)
                    .clamp(0.0, 100.0);
            }
        }
    }
    let mut hud: Vec<HudState> = Vec::with_capacity(total_frames);
    let mut combo: u32 = 0;
    let mut weighted_acc: u32 = 0;
    let mut judged_count: u32 = 0;
    let mut idx = 0;
    let mut last: Option<HudJudgment> = None;
    let mut combo_change: Option<(ComboEventType, i32, Option<u32>)> = None;
    let mut settle_ptr = 0;
    let mut settled_sum: f32 = 0.0;
    let ln_ticks = &opts.ln_combo_ticks;
    let ln_breaks = &opts.ln_combo_breaks;
    let mut tick_idx = 0;
    let mut break_idx = 0;
    let mut active_combo_break: Option<(u32, i32)> = None;
    const MIN_ANIM_INTERVAL: i32 = 50;
    const MAX_LAST_AGE: i32 = 200;
    const MAX_COMBO_ANIM_AGE: i32 = 120;
    const RED_POPUP_DURATION: i32 = 800;
    const COUNTDOWN_SPEED: f32 = 50.0;
    const COUNTDOWN_MAX_DURATION: i32 = 5000;
    for i in 0..total_frames {
        let t = t0 + (i as f32 * frame_time) as i32;
        while idx < judgments.len() && judgments[idx].time <= t {
            let j = &judgments[idx];
            // ScoreV2 counts LN heads and tails separately; legacy score keeps LN combo external.
            let should_affect_combo = !j.is_ln || is_score_v2;
            let breaks_combo =
                j.kind == JudgmentKind::Miss || (is_score_v2 && j.kind == JudgmentKind::Hit50);
            if breaks_combo {
                if should_affect_combo && combo > 0 {
                    active_combo_break = Some((combo, j.time));
                    combo_change = Some((ComboEventType::Break, j.time, Some(combo)));
                    combo = 0;
                }
            } else if should_affect_combo {
                combo += 1;
                let time_since_last = combo_change.map(|(_, t, _)| j.time - t).unwrap_or(999);
                let is_first_at_time = idx == 0 || judgments[idx - 1].time < j.time;
                if time_since_last >= MIN_ANIM_INTERVAL && is_first_at_time {
                    combo_change = Some((ComboEventType::Inc, j.time, None));
                }
            }
            weighted_acc += acc_value(j.kind, is_score_v2);
            judged_count += 1;
            last = Some(j.clone());
            idx += 1;
        }
        while tick_idx < ln_ticks.len() && ln_ticks[tick_idx].time <= t {
            // Legacy LN combo ticks arrive as synthetic events between head and tail judgments.
            combo += 1;
            let tick_time = ln_ticks[tick_idx].time;
            let time_since_last = combo_change.map(|(_, t, _)| tick_time - t).unwrap_or(999);
            if time_since_last >= MIN_ANIM_INTERVAL {
                combo_change = Some((ComboEventType::Inc, tick_time, None));
            }
            tick_idx += 1;
        }
        while break_idx < ln_breaks.len() && ln_breaks[break_idx].time <= t {
            if combo > 0 {
                let brk = &ln_breaks[break_idx];
                active_combo_break = Some((combo, brk.time));
                combo_change = Some((ComboEventType::Break, brk.time, Some(combo)));
                combo = 0;
            }
            break_idx += 1;
        }
        while settle_ptr < events.len() && t >= events[settle_ptr].t + anim_ms {
            // Score changes finish over anim_ms, so settled_sum holds fully animated points.
            settled_sum += events[settle_ptr].delta;
            settle_ptr += 1;
        }
        let mut partial: f32 = 0.0;
        for k in settle_ptr..events.len() {
            let e = &events[k];
            if e.t > t {
                break;
            }
            let f = ((t - e.t) as f32 / anim_ms as f32).clamp(0.0, 1.0);
            partial += e.delta * f;
        }
        let score = (settled_sum + partial).min(MAX_SCORE as f32) as u32;
        let acc = if judged_count > 0 {
            weighted_acc as f32 / (judged_count * acc_max) as f32
        } else {
            1.0
        };
        let lj = last
            .as_ref()
            .filter(|l| t - l.time <= MAX_LAST_AGE)
            .map(|l| LastJudgment {
                kind: l.kind,
                age: t - l.time,
                column: l.column,
            });
        let ce = combo_change
            .filter(|(_, ct, _)| t - *ct <= MAX_COMBO_ANIM_AGE)
            .map(|(ty, ct, c)| ComboEvent {
                event_type: ty,
                age: t - ct,
                combo: c,
            });
        let combo_break_anim = active_combo_break
            .filter(|(_, bt)| {
                let elapsed = t - *bt;
                (0..RED_POPUP_DURATION).contains(&elapsed)
            })
            .map(|(sc, bt)| ComboBreakAnimation {
                start_combo: sc,
                break_time: bt,
            });
        let combo_countdown = if let Some((start_combo, break_time)) = active_combo_break {
            if combo == 0 {
                let elapsed = t - break_time;
                if (0..COUNTDOWN_MAX_DURATION).contains(&elapsed) {
                    let countdown_progress = (elapsed as f32 / 1000.0) * COUNTDOWN_SPEED;
                    let current_value = (start_combo as f32 - countdown_progress).max(0.0) as u32;
                    if current_value > 0 {
                        Some(ComboCountdown {
                            from_combo: start_combo,
                            break_time,
                            current_value,
                            cancelled: false,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        let progress = if total_frames > 1 {
            (i as f32 / (total_frames - 1) as f32).clamp(0.0, 1.0)
        } else {
            1.0
        };
        hud.push(HudState {
            combo,
            score,
            accuracy: acc,
            progress,
            last: lj,
            combo_event: ce,
            combo_break_anim,
            combo_countdown,
        });
    }
    hud
}
