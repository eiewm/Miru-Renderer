#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum JudgmentKind {
    Max = 0,
    Hit300 = 1,
    Hit200 = 2,
    Hit100 = 3,
    Hit50 = 4,
    #[default]
    Miss = 5,
}
impl JudgmentKind {
    pub fn from_str(s: &str) -> Self {
        match s {
            "Max" | "MAX" | "max" => Self::Max,
            "Hit300" | "300" => Self::Hit300,
            "Hit200" | "200" => Self::Hit200,
            "Hit100" | "100" => Self::Hit100,
            "Hit50" | "50" => Self::Hit50,
            _ => Self::Miss,
        }
    }
    #[inline]
    pub const fn score_value(self) -> u32 {
        match self {
            // Mania MAX is worth 320 in score calculations, even though it displays as a 300g.
            Self::Max => 320,
            Self::Hit300 => 300,
            Self::Hit200 => 200,
            Self::Hit100 => 100,
            Self::Hit50 => 50,
            Self::Miss => 0,
        }
    }
    #[inline]
    pub const fn accuracy_v1(self) -> u32 {
        match self {
            Self::Max | Self::Hit300 => 300,
            Self::Hit200 => 200,
            Self::Hit100 => 100,
            Self::Hit50 => 50,
            Self::Miss => 0,
        }
    }
    #[inline]
    pub const fn accuracy_v2(self) -> u32 {
        match self {
            // ScoreV2 gives MAX a small accuracy bonus over a normal 300.
            Self::Max => 305,
            Self::Hit300 => 300,
            Self::Hit200 => 200,
            Self::Hit100 => 100,
            Self::Hit50 => 50,
            Self::Miss => 0,
        }
    }
    #[inline]
    pub const fn bonus_add(self) -> i32 {
        match self {
            Self::Max => 2,
            Self::Hit300 => 1,
            _ => 0,
        }
    }
    #[inline]
    pub const fn bonus_punish(self) -> Option<i32> {
        match self {
            Self::Max | Self::Hit300 => Some(0),
            Self::Hit200 => Some(8),
            Self::Hit100 => Some(24),
            Self::Hit50 => Some(44),
            // Misses reset bonus instead of applying a finite penalty.
            Self::Miss => None,
        }
    }
    #[inline]
    pub const fn bonus_hit_value(self) -> u32 {
        match self {
            Self::Max | Self::Hit300 => 32,
            Self::Hit200 => 16,
            Self::Hit100 => 8,
            Self::Hit50 => 4,
            Self::Miss => 0,
        }
    }
    #[inline]
    pub const fn breaks_combo(self) -> bool {
        matches!(self, Self::Miss)
    }
    #[inline]
    pub const fn breaks_combo_v2(self) -> bool {
        matches!(self, Self::Miss | Self::Hit50)
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Windows {
    pub max: i32,
    pub hit300: i32,
    pub hit200: i32,
    pub hit100: i32,
    pub hit50: i32,
}
impl Windows {
    #[inline]
    pub fn from_od(od: f32) -> Self {
        Self::from_od_v1_mods_stbl(od, 0)
    }
    #[inline]
    pub fn from_od_v1_mods(od: f32, mods: u32) -> Self {
        Self::from_od_v1_mods_stbl(od, mods)
    }
    #[inline]
    pub fn from_od_v1_mods_stbl(od: f32, mods: u32) -> Self {
        let num = (10.0 - od).clamp(0.0, 10.0);
        // Stable ScoreV1 mania windows use linear OD formulas from osu!stable.
        Self {
            max: scale_window_for_mods(16.0, mods),
            hit300: scale_window_for_mods(34.0 + 3.0 * num, mods),
            hit200: scale_window_for_mods(67.0 + 3.0 * num, mods),
            hit100: scale_window_for_mods(97.0 + 3.0 * num, mods),
            hit50: scale_window_for_mods(121.0 + 3.0 * num, mods),
        }
    }
    #[inline]
    pub fn from_od_v2_mods_stbl(od: f32, mods: u32) -> Self {
        let difficulty_range = |minimum: f32, midpoint: f32, maximum: f32| -> f32 {
            let v = od.clamp(0.0, 10.0);
            if v > 5.0 {
                midpoint + (maximum - midpoint) * (v - 5.0) / 5.0
            } else if v < 5.0 {
                midpoint - (midpoint - minimum) * (5.0 - v) / 5.0
            } else {
                midpoint
            }
        };
        // ScoreV2 uses osu!'s difficulty_range interpolation instead of the ScoreV1 linear formula.
        Self {
            max: scale_window_for_mods(difficulty_range(22.4, 19.4, 13.9), mods),
            hit300: scale_window_for_mods(difficulty_range(64.0, 49.0, 34.0), mods),
            hit200: scale_window_for_mods(difficulty_range(97.0, 82.0, 67.0), mods),
            hit100: scale_window_for_mods(difficulty_range(127.0, 112.0, 97.0), mods),
            hit50: scale_window_for_mods(difficulty_range(151.0, 136.0, 121.0), mods),
        }
    }
    #[inline]
    pub fn miss_window_v1_mods(od: f32, mods: u32) -> i32 {
        let num = (10.0 - od).clamp(0.0, 10.0);
        scale_window_for_mods(158.0 + 3.0 * num, mods)
    }
    #[inline]
    pub fn mis_win_v2_mods_stbl(od: f32, mods: u32) -> i32 {
        let difficulty_range = |minimum: f32, midpoint: f32, maximum: f32| -> f32 {
            let v = od.clamp(0.0, 10.0);
            if v > 5.0 {
                midpoint + (maximum - midpoint) * (v - 5.0) / 5.0
            } else if v < 5.0 {
                midpoint - (midpoint - minimum) * (5.0 - v) / 5.0
            } else {
                midpoint
            }
        };
        scale_window_for_mods(difficulty_range(188.0, 173.0, 158.0), mods)
    }
    #[inline]
    pub fn judgment_for_delta(&self, abs_delta: i32) -> Option<JudgmentKind> {
        if abs_delta <= self.max {
            Some(JudgmentKind::Max)
        } else if abs_delta <= self.hit300 {
            Some(JudgmentKind::Hit300)
        } else if abs_delta <= self.hit200 {
            Some(JudgmentKind::Hit200)
        } else if abs_delta <= self.hit100 {
            Some(JudgmentKind::Hit100)
        } else if abs_delta <= self.hit50 {
            Some(JudgmentKind::Hit50)
        } else {
            None
        }
    }
    #[inline]
    pub fn judgment_for_signed_delta_stable(&self, delta: i32) -> Option<JudgmentKind> {
        let abs_delta = delta.abs();
        let is_late = delta > 0;
        if abs_delta <= self.max {
            Some(JudgmentKind::Max)
        } else if abs_delta <= self.hit300 {
            Some(JudgmentKind::Hit300)
        } else if abs_delta <= self.hit200 {
            Some(JudgmentKind::Hit200)
        } else if abs_delta <= self.hit100 {
            Some(JudgmentKind::Hit100)
        } else if (is_late && abs_delta < self.hit50) || (!is_late && abs_delta <= self.hit50) {
            // osu!stable excludes the exact late Hit50 boundary but includes the early boundary.
            Some(JudgmentKind::Hit50)
        } else {
            None
        }
    }
}
#[inline]
pub fn get_windows(od: f32) -> Windows {
    Windows::from_od(od)
}
const MOD_EASY: u32 = 1 << 1;
const MOD_HARDROCK: u32 = 1 << 4;
const MOD_DOUBLETIME: u32 = 1 << 6;
const MOD_HALFTIME: u32 = 1 << 8;
const MOD_NIGHTCORE: u32 = 1 << 9;
#[inline]
fn scale_window_for_mods(value: f32, mods: u32) -> i32 {
    scale_window_for_mods_with_clock_rate(value, mods, None)
}
#[inline]
pub fn scale_window_for_mods_with_clock_rate(
    value: f32,
    mods: u32,
    clock_rate_override: Option<f32>,
) -> i32 {
    let mut scaled = value;
    if mods & MOD_HARDROCK != 0 {
        scaled /= 1.4;
    } else if mods & MOD_EASY != 0 {
        scaled *= 1.4;
    }
    if let Some(clock_rate) = clock_rate_override {
        scaled *= clock_rate;
    } else if mods & MOD_DOUBLETIME != 0 || mods & MOD_NIGHTCORE != 0 {
        // Deltas are judged in beatmap time, so faster playback widens the beatmap-time window.
        scaled *= 1.5;
    } else if mods & MOD_HALFTIME != 0 {
        scaled *= 0.75;
    }
    scaled as i32
}
