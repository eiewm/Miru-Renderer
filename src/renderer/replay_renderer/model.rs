use crate::modes::mania::judgment::ScoreMode;
use crate::types::{JudgmentKind, ReplayOrigin};
#[derive(Debug, Clone, Copy)]
pub struct RenderJudgment {
    pub idx: usize,
    pub column: u8,
    pub time: i32,
    pub kind: JudgmentKind,
    pub press_time: Option<i32>,
    pub rel_time: Option<i32>,
    pub is_ln: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayModDisplay {
    pub origin: ReplayOrigin,
    pub acronyms: Vec<String>,
}
impl ReplayModDisplay {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.acronyms.is_empty()
    }
}
impl RenderJudgment {
    #[inline]
    pub fn judgment_time(&self) -> i32 {
        if self.is_ln {
            // LN visuals announce the release result at tail time when that data exists.
            self.rel_time.or(self.press_time).unwrap_or(self.time)
        } else {
            self.press_time.unwrap_or(self.time)
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReleaseKind {
    #[default]
    None,
    Max,
    Hit300,
    Hit200,
    Hit100,
    Hit50,
    Miss,
}
impl ReleaseKind {
    pub fn from_judgment(kind: JudgmentKind) -> Self {
        match kind {
            JudgmentKind::Max => Self::Max,
            JudgmentKind::Hit300 => Self::Hit300,
            JudgmentKind::Hit200 => Self::Hit200,
            JudgmentKind::Hit100 => Self::Hit100,
            JudgmentKind::Hit50 => Self::Hit50,
            JudgmentKind::Miss => Self::Miss,
        }
    }
}
impl From<crate::modes::mania::judgment::ReleaseKind> for ReleaseKind {
    fn from(jk: crate::modes::mania::judgment::ReleaseKind) -> Self {
        use crate::modes::mania::judgment::ReleaseKind as JRK;
        match jk {
            JRK::Max => Self::Max,
            JRK::Hit300 => Self::Hit300,
            JRK::Hit200 => Self::Hit200,
            JRK::Hit100 => Self::Hit100,
            JRK::Hit50 => Self::Hit50,
            JRK::Miss => Self::Miss,
            JRK::None => Self::None,
        }
    }
}
#[derive(Debug, Clone, Copy, Default)]
pub struct LnReleaseInfo {
    pub kind: ReleaseKind,
    pub time: Option<i32>,
    pub rescued: bool,
    pub double_tap: bool,
}
impl From<&crate::modes::mania::judgment::LnReleaseInfo> for LnReleaseInfo {
    fn from(j: &crate::modes::mania::judgment::LnReleaseInfo) -> Self {
        Self {
            kind: j.kind.into(),
            time: j.time,
            rescued: j.rescued,
            double_tap: j.double_tap,
        }
    }
}
impl From<crate::modes::mania::judgment::LnReleaseInfo> for LnReleaseInfo {
    fn from(j: crate::modes::mania::judgment::LnReleaseInfo) -> Self {
        Self::from(&j)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComboEventType {
    Judgment,
    LnTick,
    LnBreak,
}
#[derive(Debug, Clone)]
pub struct ComboEvent {
    pub time: i32,
    pub event_type: ComboEventType,
    pub score_judgment_idx: Option<usize>,
    pub score_delta: f64,
    pub cumulative_score: f64,
    pub combo_after: u32,
    pub acc_hits: u32,
    pub acc_max_hits: u32,
    pub hit_error_offset_ms: Option<i32>,
    pub hit_error_moving_avg_ms: Option<f32>,
    pub combo_break_start: Option<u32>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawEventKind {
    Judgment,
    LnTick,
    LnBreak,
}
#[derive(Debug, Clone)]
pub struct RawEvent {
    pub time: i32,
    pub kind: RawEventKind,
    pub judgment_idx: Option<usize>,
    pub ln_idx: Option<usize>,
}
#[derive(Debug, Clone, Copy)]
pub struct LnComboTick {
    pub time: i32,
    pub ln_idx: usize,
}
#[derive(Debug, Clone, Copy)]
pub struct LnComboBreak {
    pub time: i32,
    pub ln_idx: usize,
}
#[derive(Debug, Clone)]
pub struct RenderPlan {
    pub timeline_start: i32,
    pub timeline_end: i32,
    pub frame_time: f64,
    pub total_frames: usize,
    pub travel_ms: f64,
}
impl RenderPlan {
    pub fn frame_at_time(&self, t: i32) -> usize {
        if t <= self.timeline_start {
            return 0;
        }
        let elapsed = (t - self.timeline_start) as f64;
        (elapsed / self.frame_time).floor() as usize
    }
    pub fn time_at_frame(&self, frame: usize) -> i32 {
        self.timeline_start + (frame as f64 * self.frame_time).round() as i32
    }
}
pub struct ScoreConstants {
    pub hit_value: [u32; 6],
    pub hit_bonus_value: [u32; 6],
    pub hit_bonus_add: [i32; 6],
    pub hit_punish: [i32; 6],
    pub acc_weights: [u32; 6],
    pub acc_max_per_hit: u32,
    pub v2_combo_portion: f64,
    pub v2_acc_portion: f64,
}
impl Default for ScoreConstants {
    fn default() -> Self {
        Self {
            // Arrays use ScoreConstants::kind_to_idx order: MAX, 300, 200, 100, 50, Miss.
            hit_value: [320, 300, 200, 100, 50, 0],
            hit_bonus_value: [32, 32, 16, 8, 4, 0],
            hit_bonus_add: [2, 1, 0, 0, 0, 0],
            hit_punish: [0, 0, 8, 24, 44, i32::MAX],
            acc_weights: [300, 300, 200, 100, 50, 0],
            acc_max_per_hit: 300,
            v2_combo_portion: 0.70,
            v2_acc_portion: 0.30,
        }
    }
}
impl ScoreConstants {
    #[inline]
    pub fn for_mode(mode: ScoreMode) -> Self {
        let mut out = Self::default();
        if mode.accuracy_max_per_hit() == 305 {
            // ScoreV2 gives MAX a 305 accuracy weight while keeping normal 300 at 300.
            out.acc_weights = [305, 300, 200, 100, 50, 0];
            out.acc_max_per_hit = 305;
        }
        out
    }
    #[inline]
    pub fn kind_to_idx(kind: JudgmentKind) -> usize {
        match kind {
            JudgmentKind::Max => 0,
            JudgmentKind::Hit300 => 1,
            JudgmentKind::Hit200 => 2,
            JudgmentKind::Hit100 => 3,
            JudgmentKind::Hit50 => 4,
            JudgmentKind::Miss => 5,
        }
    }
    pub fn hit_value(&self, kind: JudgmentKind) -> u32 {
        self.hit_value[Self::kind_to_idx(kind)]
    }
    pub fn hit_bonus_value(&self, kind: JudgmentKind) -> u32 {
        self.hit_bonus_value[Self::kind_to_idx(kind)]
    }
    pub fn hit_bonus_add(&self, kind: JudgmentKind) -> i32 {
        self.hit_bonus_add[Self::kind_to_idx(kind)]
    }
    pub fn hit_punish(&self, kind: JudgmentKind) -> i32 {
        self.hit_punish[Self::kind_to_idx(kind)]
    }
    pub fn acc_weight(&self, kind: JudgmentKind) -> u32 {
        self.acc_weights[Self::kind_to_idx(kind)]
    }
}
#[derive(Debug, Clone, Copy, Default)]
pub struct Windows {
    pub max: i32,
    pub hit300: i32,
    pub hit200: i32,
    pub hit100: i32,
    pub hit50: i32,
    pub miss: i32,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct NoteRenderState {
    pub consumed: bool,
    pub miss_dim: bool,
    pub fifty_dim: bool,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct LnRenderInfo {
    pub show_ln: bool,
    pub finished_normally: bool,
    pub head_top_y: i32,
    pub tail_top_y: i32,
    pub body_top_y: i32,
    pub body_bottom_y: i32,
    pub allow_pass: bool,
    pub head_tint: f32,
    pub is_broken: bool,
    pub held: bool,
}
