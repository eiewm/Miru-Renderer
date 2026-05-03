use crate::types::JudgmentKind;
use std::collections::HashMap;
#[derive(Debug, Clone)]
pub struct InternalJudgment {
    pub index: usize,
    pub column: u8,
    pub note_time: i32,
    pub kind: JudgmentKind,
    pub delta: i32,
    pub press_time: Option<i32>,
    pub is_ln: bool,
    pub end_time: Option<i32>,
    pub early_press_idx: Option<i32>,
    pub early_pen_win: Option<i32>,
    pub deep_ln_pen: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum RuleMeta {
    #[default]
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum ReleaseKind {
    Max,
    Hit300,
    Hit200,
    Hit100,
    Hit50,
    Miss,
    #[default]
    None,
}
impl ReleaseKind {
    pub fn from_str(s: &str) -> Self {
        match s {
            "MAX" | "max" => Self::Max,
            "300" => Self::Hit300,
            "200" => Self::Hit200,
            "100" => Self::Hit100,
            "50" => Self::Hit50,
            "miss" | "MISS" => Self::Miss,
            _ => Self::None,
        }
    }
    #[inline]
    pub const fn rank(self) -> i32 {
        match self {
            Self::Max => 5,
            Self::Hit300 => 4,
            Self::Hit200 => 3,
            Self::Hit100 => 2,
            Self::Hit50 => 1,
            Self::Miss => 0,
            Self::None => -1,
        }
    }
    pub fn as_judgment_kind(&self) -> Option<JudgmentKind> {
        match self {
            Self::Max => Some(JudgmentKind::Max),
            Self::Hit300 => Some(JudgmentKind::Hit300),
            Self::Hit200 => Some(JudgmentKind::Hit200),
            Self::Hit100 => Some(JudgmentKind::Hit100),
            Self::Hit50 => Some(JudgmentKind::Hit50),
            Self::Miss => Some(JudgmentKind::Miss),
            Self::None => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Authority {
    #[default]
    Derived,
    FinalOverride,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LnLengthBucket {
    VeryShort,
    Short,
    Medium,
    Long,
    VeryLong,
    #[default]
    Unknown,
}
impl LnLengthBucket {
    pub fn from_duration(ln_dur: i32) -> Self {
        if ln_dur < 60 {
            Self::VeryShort
        } else if ln_dur < 100 {
            Self::Short
        } else if ln_dur < 200 {
            Self::Medium
        } else if ln_dur < 400 {
            Self::Long
        } else {
            Self::VeryLong
        }
    }
    pub fn as_legacy_str(&self) -> Option<&'static str> {
        match self {
            Self::VeryShort => Some("VERY_SHORT_LN"),
            Self::Short => Some("SHORT_LN"),
            Self::Medium => None,
            Self::Long => Some("LONG_LN"),
            Self::VeryLong => Some("VERY_LONG_LN"),
            Self::Unknown => None,
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct LnReleaseInfo {
    pub kind: ReleaseKind,
    pub time: Option<i32>,
    pub double_tap: bool,
    pub rescued: bool,
    pub force_kind: bool,
    pub alt_head_press_time: Option<i32>,
}
#[derive(Debug, Clone, Default)]
pub struct LnDebugInfo {
    pub head_was_hit: bool,
    pub held_until_end: bool,
    pub has_early_rel: bool,
    pub repr_after_rel: bool,
    pub repr_hit_tail: bool,
    pub had_head_dt: bool,
    pub had_pre_head_dt: bool,
    pub pre_hea_far_sho_miss: bool,
    pub first_early_rel: Option<i32>,
    pub first_repr_after_rel: Option<i32>,
    pub last_repr_time: Option<i32>,
    pub first_free_repr: Option<i32>,
    pub rel_after_repr: Option<i32>,
    pub rescue_rel_near_end: Option<i32>,
    pub last_repr_free: bool,
    pub branch: String,
    pub rule: RuleMeta,
    pub authority: Authority,
    pub start_diff: i32,
    pub end_diff: i32,
    pub total_diff: i32,
    pub head_orphan_press_idx: Option<i32>,
    pub head_early_press_idx: Option<i32>,
    pub head_best_idx: Option<i32>,
    pub head_win_left: Option<i32>,
    pub head_win_right: Option<i32>,
    pub head_assign_reason: Option<String>,
    pub effective_rel_time: Option<i32>,
    pub raw_rel_from_press: Option<i32>,
    pub alt_head_used: bool,
    pub alt_head_press_time: Option<i32>,
    pub first_repr_owner_idx: Option<usize>,
    pub first_repr_owner_time: Option<i32>,
    pub fir_rep_yiel_next_ln: bool,
}
#[derive(Debug, Default)]
pub struct EngineOutput {
    pub judgments: Vec<InternalJudgment>,
    pub ln_releases: HashMap<usize, LnReleaseInfo>,
    pub ln_debug: HashMap<usize, LnDebugInfo>,
}
