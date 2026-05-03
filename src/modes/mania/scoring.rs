use crate::utils::mods::{default_mania_mod_multiplier, ModAbbr};
const MAX_SCORE: f64 = 1_000_000.0;
#[derive(Debug, Clone, Copy, Default)]
pub struct JudgmentCounts {
    pub max: u32,
    pub hit300: u32,
    pub hit200: u32,
    pub hit100: u32,
    pub hit50: u32,
    pub miss: u32,
}
impl JudgmentCounts {
    pub fn total(&self) -> u32 {
        self.max + self.hit300 + self.hit200 + self.hit100 + self.hit50 + self.miss
    }
}
#[derive(Debug, Clone)]
pub struct ScoreInput {
    pub total_notes: u32,
    pub counts: JudgmentCounts,
    pub mods: Vec<ModAbbr>,
    pub initial_bonus: f64,
    pub map_od: f32,
}
impl Default for ScoreInput {
    fn default() -> Self {
        Self {
            total_notes: 0,
            counts: JudgmentCounts::default(),
            mods: Vec::new(),
            initial_bonus: 100.0,
            map_od: 8.0,
        }
    }
}
#[derive(Debug, Clone)]
pub struct ScoreReport {
    pub score_v1: ScoreV1,
    pub score_v2: ScoreV2,
}
#[derive(Debug, Clone)]
pub struct ScoreV1 {
    pub base_per_note: f64,
    pub base_total: f64,
    pub bonus_total: f64,
    pub total: f64,
}
#[derive(Debug, Clone)]
pub struct ScoreV2 {
    pub combo_portion: f64,
    pub accuracy_portion: f64,
    pub combo_ratio: f64,
    pub accuracy_ratio: f64,
    pub estimated: f64,
}
const HIT_VALUE: [f64; 6] = [320.0, 300.0, 200.0, 100.0, 50.0, 0.0];
const HIT_BONUS_ADD: [f64; 6] = [2.0, 1.6, 1.0, 0.4, 0.2, 0.0];
const HIT_PUNISH: [f64; 6] = [0.0, 0.0, 0.0, 1.5, 2.5, 5.0];
pub fn compute_mania_score_report(input: &ScoreInput) -> ScoreReport {
    let total = input.total_notes.max(1) as f64;
    let mod_mult = default_mania_mod_multiplier(&input.mods) as f64;
    let unit = (MAX_SCORE * mod_mult * 0.5) / total;
    // Arrays use Max, 300, 200, 100, 50, Miss order.
    let counts = [
        input.counts.max,
        input.counts.hit300,
        input.counts.hit200,
        input.counts.hit100,
        input.counts.hit50,
        input.counts.miss,
    ];
    let mut base_total = 0.0;
    for (i, &count) in counts.iter().enumerate() {
        let per = unit * (HIT_VALUE[i] / 320.0);
        base_total += per * count as f64;
    }
    let mut bonus = input.initial_bonus.clamp(0.0, 100.0);
    let mut bonus_total = 0.0;
    // With only aggregate counts, ScoreV1 bonus is an estimate because real bonus depends on hit order.
    for (i, &count) in counts.iter().enumerate() {
        for _ in 0..count {
            let val = HIT_VALUE[i];
            let add = unit * (val * bonus.sqrt() / 320.0);
            bonus_total += add;
            let inc = HIT_BONUS_ADD[i];
            let pun = HIT_PUNISH[i];
            bonus = (bonus + inc - pun).clamp(0.0, 100.0);
        }
    }
    let total_v1 = base_total + bonus_total;
    let combo_portion = 0.70;
    let accuracy_portion = 0.30;
    let hits = input.counts.total();
    // ScoreV2 estimate mirrors the public 70/30 combo/accuracy split.
    let acc_numer = input.counts.max as f64 * 305.0
        + input.counts.hit300 as f64 * 300.0
        + input.counts.hit200 as f64 * 200.0
        + input.counts.hit100 as f64 * 100.0
        + input.counts.hit50 as f64 * 50.0;
    let acc_denom = (hits as f64 * 305.0).max(1.0);
    let accuracy_ratio = acc_numer / acc_denom;
    let player_combo = hits - input.counts.miss;
    let max_combo = hits;
    let combo_ratio = if max_combo > 0 {
        player_combo as f64 / max_combo as f64
    } else {
        0.0
    };
    let est_v2 = MAX_SCORE * (combo_portion * combo_ratio + accuracy_portion * accuracy_ratio);
    ScoreReport {
        score_v1: ScoreV1 {
            base_per_note: unit,
            base_total,
            bonus_total,
            total: total_v1,
        },
        score_v2: ScoreV2 {
            combo_portion,
            accuracy_portion,
            combo_ratio,
            accuracy_ratio,
            estimated: est_v2,
        },
    }
}
pub fn calculate_accuracy_v1(counts: &JudgmentCounts) -> f64 {
    let total = counts.total();
    if total == 0 {
        return 0.0;
    }
    let weighted = counts.max as f64 * 300.0
        + counts.hit300 as f64 * 300.0
        + counts.hit200 as f64 * 200.0
        + counts.hit100 as f64 * 100.0
        + counts.hit50 as f64 * 50.0;
    weighted / (total as f64 * 300.0)
}
pub fn calculate_accuracy_v2(counts: &JudgmentCounts) -> f64 {
    let total = counts.total();
    if total == 0 {
        return 0.0;
    }
    let weighted = counts.max as f64 * 305.0
        + counts.hit300 as f64 * 300.0
        + counts.hit200 as f64 * 200.0
        + counts.hit100 as f64 * 100.0
        + counts.hit50 as f64 * 50.0;
    weighted / (total as f64 * 305.0)
}
