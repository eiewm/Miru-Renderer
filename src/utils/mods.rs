#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModAbbr {
    NF,
    EZ,
    TD,
    HD,
    HR,
    SD,
    DT,
    RX,
    HT,
    NC,
    FL,
    AU,
    SO,
    AP,
    PF,
    K4,
    K5,
    K6,
    K7,
    K8,
    FI,
    RD,
    CM,
    TP,
    K9,
    COOP,
    K1,
    K3,
    K2,
    V2,
    MR,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptivePlaybackConfig {
    pub initial_rate: f64,
    pub adjust_pitch: bool,
}
#[derive(Debug, Clone, PartialEq)]
pub struct AdaptivePlaybackSegment {
    pub beatmap_start_ms: f64,
    pub beatmap_end_ms: Option<f64>,
    pub output_start_ms: f64,
    pub output_end_ms: Option<f64>,
    pub start_rate: f64,
    pub target_rate: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct AdaptivePlaybackProfile {
    pub initial_rate: f64,
    pub tail_rate: f64,
    pub segments: Vec<AdaptivePlaybackSegment>,
}
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackRateProfile {
    Constant {
        rate: f64,
    },
    LinearRamp {
        initial_rate: f64,
        final_rate: f64,
        begin_ms: i32,
        end_ms: i32,
    },
    Adaptive {
        profile: AdaptivePlaybackProfile,
    },
}
impl PlaybackRateProfile {
    #[inline]
    pub const fn constant(rate: f64) -> Self {
        Self::Constant { rate }
    }
    #[inline]
    pub fn initial_rate(&self) -> f64 {
        match self {
            Self::Constant { rate } => *rate,
            Self::LinearRamp { initial_rate, .. } => *initial_rate,
            Self::Adaptive { profile } => profile.initial_rate,
        }
    }
    #[inline]
    pub fn final_rate(&self) -> f64 {
        match self {
            Self::Constant { rate } => *rate,
            Self::LinearRamp { final_rate, .. } => *final_rate,
            Self::Adaptive { profile } => profile.tail_rate,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackAudioMode {
    StaticSplit,
    RateDriven { adjust_pitch: bool },
}
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackModSettings {
    pub clock_rate: f64,
    pub profile: PlaybackRateProfile,
    pub audio_mode: PlaybackAudioMode,
    pub audio_frequency_rate: f64,
    pub audio_tempo_rate: f64,
    pub nightcore: bool,
    pub adaptive_speed: Option<AdaptivePlaybackConfig>,
}
impl PlaybackModSettings {
    pub const fn normal() -> Self {
        Self {
            clock_rate: 1.0,
            profile: PlaybackRateProfile::Constant { rate: 1.0 },
            audio_mode: PlaybackAudioMode::StaticSplit,
            audio_frequency_rate: 1.0,
            audio_tempo_rate: 1.0,
            nightcore: false,
            adaptive_speed: None,
        }
    }
    #[inline]
    pub const fn from_static(
        clock_rate: f64,
        audio_frequency_rate: f64,
        audio_tempo_rate: f64,
        nightcore: bool,
    ) -> Self {
        Self {
            clock_rate,
            profile: PlaybackRateProfile::Constant { rate: clock_rate },
            audio_mode: PlaybackAudioMode::StaticSplit,
            audio_frequency_rate,
            audio_tempo_rate,
            nightcore,
            adaptive_speed: None,
        }
    }
}
const MOD_NOFAIL: u32 = 1 << 0;
const MOD_HIDDEN: u32 = 1 << 3;
const MOD_SUDDENDEATH: u32 = 1 << 5;
const MOD_DOUBLETIME: u32 = 1 << 6;
const MOD_HALFTIME: u32 = 1 << 8;
const MOD_NIGHTCORE: u32 = 1 << 9;
const MOD_FLASHLIGHT: u32 = 1 << 10;
const MOD_PERFECT: u32 = 1 << 14;
const MOD_FADEIN: u32 = 1 << 20;
// Legacy replay bit positions are fixed by osu!stable and include mania key-count mods.
const MOD_BITS: [(u32, ModAbbr); 31] = [
    (1 << 0, ModAbbr::NF),
    (1 << 1, ModAbbr::EZ),
    (1 << 2, ModAbbr::TD),
    (1 << 3, ModAbbr::HD),
    (1 << 4, ModAbbr::HR),
    (1 << 5, ModAbbr::SD),
    (1 << 6, ModAbbr::DT),
    (1 << 7, ModAbbr::RX),
    (1 << 8, ModAbbr::HT),
    (1 << 9, ModAbbr::NC),
    (1 << 10, ModAbbr::FL),
    (1 << 11, ModAbbr::AU),
    (1 << 12, ModAbbr::SO),
    (1 << 13, ModAbbr::AP),
    (1 << 14, ModAbbr::PF),
    (1 << 15, ModAbbr::K4),
    (1 << 16, ModAbbr::K5),
    (1 << 17, ModAbbr::K6),
    (1 << 18, ModAbbr::K7),
    (1 << 19, ModAbbr::K8),
    (1 << 20, ModAbbr::FI),
    (1 << 21, ModAbbr::RD),
    (1 << 22, ModAbbr::CM),
    (1 << 23, ModAbbr::TP),
    (1 << 24, ModAbbr::K9),
    (1 << 25, ModAbbr::COOP),
    (1 << 26, ModAbbr::K1),
    (1 << 27, ModAbbr::K3),
    (1 << 28, ModAbbr::K2),
    (1 << 29, ModAbbr::V2),
    (1 << 30, ModAbbr::MR),
];
pub fn decode_mods(bitmask: u32) -> Vec<ModAbbr> {
    MOD_BITS
        .iter()
        .filter(|(bit, _)| bitmask & bit != 0)
        .map(|(_, abbr)| *abbr)
        .collect()
}
pub fn default_mania_mod_multiplier(mods: &[ModAbbr]) -> f32 {
    let mut mult = 1.0;
    for m in mods {
        match m {
            ModAbbr::EZ => mult *= 0.5,
            ModAbbr::NF => mult *= 0.5,
            ModAbbr::HT => mult *= 0.3,
            _ => {}
        }
    }
    mult
}
#[inline]
pub fn has_mirror_mod(bitmask: u32) -> bool {
    bitmask & (1 << 30) != 0
}
#[inline]
pub fn has_fade_in_mod(bitmask: u32) -> bool {
    bitmask & MOD_FADEIN != 0
}
#[inline]
pub fn has_hidden_mod(bitmask: u32) -> bool {
    bitmask & MOD_HIDDEN != 0
}
#[inline]
pub fn has_flashlight_mod(bitmask: u32) -> bool {
    bitmask & MOD_FLASHLIGHT != 0
}
#[inline]
pub fn has_sudden_death_mod(bitmask: u32) -> bool {
    bitmask & MOD_SUDDENDEATH != 0
}
#[inline]
pub fn has_perfect_mod(bitmask: u32) -> bool {
    bitmask & MOD_PERFECT != 0
}
#[inline]
pub fn has_no_fail_mod(bitmask: u32) -> bool {
    bitmask & MOD_NOFAIL != 0
}
#[inline]
pub fn mirror_column(column: u8, key_count: u8) -> u8 {
    if key_count == 0 {
        return column;
    }
    let last_column = key_count.saturating_sub(1);
    last_column.saturating_sub(column.min(last_column))
}
#[inline]
pub fn has_scorev2(bitmask: u32) -> bool {
    bitmask & (1 << 29) != 0
}
pub fn get_rate_from_mods(mods: &[ModAbbr]) -> f32 {
    if mods.contains(&ModAbbr::DT) || mods.contains(&ModAbbr::NC) {
        1.5
    } else if mods.contains(&ModAbbr::HT) {
        0.75
    } else {
        1.0
    }
}
pub fn resolve_playback_mod_settings(bitmask: u32) -> PlaybackModSettings {
    if bitmask & MOD_NIGHTCORE != 0 {
        // Nightcore speeds gameplay but keeps chipmunk pitch through a separate overlay track.
        PlaybackModSettings::from_static(1.5, 1.5, 1.0, true)
    } else if bitmask & MOD_DOUBLETIME != 0 {
        // Double Time changes tempo without shifting pitch in the main audio stream.
        PlaybackModSettings::from_static(1.5, 1.0, 1.5, false)
    } else if bitmask & MOD_HALFTIME != 0 {
        PlaybackModSettings::from_static(0.75, 0.75, 1.0, false)
    } else {
        PlaybackModSettings::normal()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LazerRateModKind {
    DoubleTime,
    Nightcore,
    HalfTime,
    Daycore,
}
fn parse_positive_f64(raw: &serde_json::Value) -> Option<f64> {
    let parsed = match raw {
        serde_json::Value::Number(value) => value.as_f64(),
        serde_json::Value::String(value) => value.parse::<f64>().ok(),
        _ => None,
    }?;
    (parsed.is_finite() && parsed > 0.0).then_some(parsed)
}
fn parse_bool_like(raw: &serde_json::Value) -> Option<bool> {
    match raw {
        serde_json::Value::Bool(value) => Some(*value),
        // Lazer/API settings may arrive from user JSON where booleans were serialized loosely.
        serde_json::Value::Number(value) => value.as_i64().and_then(|value| match value {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }),
        serde_json::Value::String(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "true" | "1" => Some(true),
                "false" | "0" => Some(false),
                _ => None,
            }
        }
        _ => None,
    }
}
fn lazer_rate_mod_kind(acronym: &str) -> Option<LazerRateModKind> {
    if acronym.eq_ignore_ascii_case("NC") {
        Some(LazerRateModKind::Nightcore)
    } else if acronym.eq_ignore_ascii_case("DT") {
        Some(LazerRateModKind::DoubleTime)
    } else if acronym.eq_ignore_ascii_case("HT") {
        Some(LazerRateModKind::HalfTime)
    } else if acronym.eq_ignore_ascii_case("DC") {
        Some(LazerRateModKind::Daycore)
    } else {
        None
    }
}
pub fn resolve_lazer_playback_mod_settings(
    replay: &crate::types::ReplayData,
) -> Result<Option<PlaybackModSettings>, String> {
    if replay.origin != crate::types::ReplayOrigin::LazerExport
        || replay.mod_info.api_mods.is_empty()
    {
        return Ok(None);
    }
    let mut rate_mods = replay.mod_info.api_mods.iter().filter_map(|mod_entry| {
        lazer_rate_mod_kind(&mod_entry.acronym).map(|kind| (kind, &mod_entry.settings))
    });
    let Some((kind, settings)) = rate_mods.next() else {
        return Ok(None);
    };
    if rate_mods.next().is_some() {
        // Only one clock-changing mod can define the playback timeline.
        return Err("Mods de replay incompatibles".to_string());
    }
    let resolved = match kind {
        LazerRateModKind::DoubleTime => {
            let speed_change = settings
                .get("speed_change")
                .and_then(parse_positive_f64)
                .unwrap_or(1.5);
            let adjust_pitch = settings
                .get("adjust_pitch")
                .and_then(parse_bool_like)
                .unwrap_or(false);
            PlaybackModSettings::from_static(
                speed_change,
                if adjust_pitch { speed_change } else { 1.0 },
                if adjust_pitch { 1.0 } else { speed_change },
                false,
            )
        }
        LazerRateModKind::Nightcore => {
            let speed_change = settings
                .get("speed_change")
                .and_then(parse_positive_f64)
                .unwrap_or(1.5);
            // Lazer Nightcore keeps the 1.5x pitch profile while custom speed controls tempo.
            PlaybackModSettings::from_static(speed_change, 1.5, speed_change / 1.5, true)
        }
        LazerRateModKind::HalfTime => {
            let speed_change = settings
                .get("speed_change")
                .and_then(parse_positive_f64)
                .unwrap_or(0.75);
            let adjust_pitch = settings
                .get("adjust_pitch")
                .and_then(parse_bool_like)
                .unwrap_or(false);
            PlaybackModSettings::from_static(
                speed_change,
                if adjust_pitch { speed_change } else { 1.0 },
                if adjust_pitch { 1.0 } else { speed_change },
                false,
            )
        }
        LazerRateModKind::Daycore => {
            let speed_change = settings
                .get("speed_change")
                .and_then(parse_positive_f64)
                .unwrap_or(0.75);
            PlaybackModSettings::from_static(speed_change, 0.75, speed_change / 0.75, false)
        }
    };
    Ok(Some(resolved))
}
#[inline]
pub fn replay_has_api_mod(replay: &crate::types::ReplayData, acronym: &str) -> bool {
    replay
        .mod_info
        .api_mods
        .iter()
        .any(|mod_entry| mod_entry.acronym.eq_ignore_ascii_case(acronym))
}
