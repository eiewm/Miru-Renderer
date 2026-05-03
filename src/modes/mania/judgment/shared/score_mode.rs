use crate::types::replay::{ReplayData, ReplayOrigin};
use crate::utils::mods::has_scorev2;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreMode {
    ScoreV1,
    ScoreV2,
    Lazer,
}
impl ScoreMode {
    #[inline]
    pub const fn accuracy_max_per_hit(self) -> u32 {
        match self {
            Self::ScoreV1 => 300,
            Self::ScoreV2 | Self::Lazer => 305,
        }
    }
    #[inline]
    pub const fn uses_split_ln_events(self) -> bool {
        !matches!(self, Self::ScoreV1)
    }
    #[inline]
    pub const fn use_ext_ln_comb_evnt(self) -> bool {
        matches!(self, Self::Lazer)
    }
    #[inline]
    pub const fn uses_prgrss_ln_ticks(self) -> bool {
        matches!(self, Self::ScoreV1)
    }
    #[inline]
    pub const fn hit50_breaks_combo(self) -> bool {
        matches!(self, Self::ScoreV2)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowProfile {
    StableScoreV1,
    StableScoreV2,
    LazerClassic,
    LazerModern,
}
#[derive(Debug, Clone, Copy)]
pub struct ScoreModeContext {
    pub requested_mode: ScoreMode,
    pub effective_mode: ScoreMode,
    pub fallback_to_v1: bool,
    pub mods: u32,
    pub origin: ReplayOrigin,
    pub window_profile: WindowProfile,
    pub miss_window: i32,
}
impl ScoreModeContext {
    #[inline]
    pub fn from_mods(mods: u32) -> Self {
        let requested_mode = if has_scorev2(mods) {
            ScoreMode::ScoreV2
        } else {
            ScoreMode::ScoreV1
        };
        let effective_mode = requested_mode;
        let window_profile = match effective_mode {
            ScoreMode::ScoreV1 => WindowProfile::StableScoreV1,
            ScoreMode::ScoreV2 => WindowProfile::StableScoreV2,
            ScoreMode::Lazer => WindowProfile::LazerModern,
        };
        Self {
            requested_mode,
            effective_mode,
            fallback_to_v1: requested_mode != effective_mode,
            mods,
            origin: ReplayOrigin::StableLegacy,
            window_profile,
            miss_window: 0,
        }
    }
    #[inline]
    pub fn from_replay(replay: &ReplayData) -> Self {
        let explicit_profile = replay.mod_info.api_mods.iter().find_map(|entry| {
            if entry.acronym.eq_ignore_ascii_case("SV1") {
                Some((
                    ScoreMode::ScoreV1,
                    WindowProfile::StableScoreV1,
                    ReplayOrigin::StableLegacy,
                ))
            } else if entry.acronym.eq_ignore_ascii_case("SV2") {
                Some((
                    ScoreMode::ScoreV2,
                    WindowProfile::StableScoreV2,
                    ReplayOrigin::StableLegacy,
                ))
            } else if entry.acronym.eq_ignore_ascii_case("CL") {
                Some((
                    ScoreMode::Lazer,
                    WindowProfile::LazerClassic,
                    ReplayOrigin::LazerExport,
                ))
            } else if entry.acronym.eq_ignore_ascii_case("LZ") {
                Some((
                    ScoreMode::Lazer,
                    WindowProfile::LazerModern,
                    ReplayOrigin::LazerExport,
                ))
            } else {
                None
            }
        });
        if let Some((mode, window_profile, origin)) = explicit_profile {
            return Self {
                requested_mode: mode,
                effective_mode: mode,
                fallback_to_v1: false,
                mods: replay.mods,
                origin,
                window_profile,
                miss_window: 0,
            };
        }
        if replay.origin == ReplayOrigin::LazerExport {
            let has_classic = replay.mod_info.has_classic;
            let has_score_v2 = replay.mod_info.has_score_v2;
            let window_profile = if has_classic && !has_score_v2 {
                WindowProfile::LazerClassic
            } else {
                WindowProfile::LazerModern
            };
            return Self {
                requested_mode: ScoreMode::Lazer,
                effective_mode: ScoreMode::Lazer,
                fallback_to_v1: false,
                mods: replay.mods,
                origin: ReplayOrigin::LazerExport,
                window_profile,
                miss_window: 0,
            };
        }
        Self::from_mods(replay.mods)
    }
}
