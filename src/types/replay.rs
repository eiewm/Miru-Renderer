use std::collections::HashMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReplayOrigin {
    #[default]
    StableLegacy,
    LazerExport,
}
#[derive(Debug, Clone, Default)]
pub struct ApiModEntry {
    pub acronym: String,
    pub settings: serde_json::Value,
}
#[derive(Debug, Clone, Default)]
pub struct ReplayModInfo {
    pub legacy_bits: u32,
    pub api_mods: Vec<ApiModEntry>,
    pub has_classic: bool,
    pub has_score_v2: bool,
    pub display_mods: Option<Vec<String>>,
}
#[derive(Debug, Clone, Default)]
pub struct ReplayScoreInfo {
    pub statistics: HashMap<String, i32>,
    pub maximum_statistics: HashMap<String, i32>,
    pub client_version: Option<String>,
    pub solo_score_online_id: Option<i64>,
    pub pauses: Vec<i32>,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReplayBasicStatistics {
    pub max: u32,
    pub hit300: u32,
    pub hit200: u32,
    pub hit100: u32,
    pub hit50: u32,
    pub miss: u32,
}
impl ReplayBasicStatistics {
    #[inline]
    pub const fn total(self) -> u32 {
        self.max + self.hit300 + self.hit200 + self.hit100 + self.hit50 + self.miss
    }
    #[inline]
    pub const fn weighted_hits(self, max_weight: u32) -> u32 {
        self.max * max_weight
            + self.hit300 * 300
            + self.hit200 * 200
            + self.hit100 * 100
            + self.hit50 * 50
    }
    fn from_statistics_map(stats: &HashMap<String, i32>) -> Option<Self> {
        // Lazer exports, API payloads, and stable counters use different names for the same judgments.
        let lookup = |aliases: &[&str]| -> Option<u32> {
            aliases.iter().find_map(|alias| {
                stats.iter().find_map(|(key, value)| {
                    (canonical_stat_key(key) == *alias && *value >= 0).then_some(*value as u32)
                })
            })
        };
        let parsed = Self {
            max: lookup(&["perfect", "max", "geki", "6"]).unwrap_or(0),
            hit300: lookup(&["great", "hit300", "300", "5"]).unwrap_or(0),
            hit200: lookup(&["good", "hit200", "200", "katu", "4"]).unwrap_or(0),
            hit100: lookup(&["ok", "hit100", "100", "3"]).unwrap_or(0),
            hit50: lookup(&["meh", "hit50", "50", "2"]).unwrap_or(0),
            miss: lookup(&["miss", "1"]).unwrap_or(0),
        };
        (parsed.total() > 0).then_some(parsed)
    }
}
#[derive(Debug, Clone, Default)]
pub struct ReplayData {
    pub game_mode: u8,
    pub version: u32,
    pub beatmap_hash: String,
    pub player_name: String,
    pub replay_hash: String,
    pub count_300: u16,
    pub count_100: u16,
    pub count_50: u16,
    pub count_geki: u16,
    pub count_katu: u16,
    pub count_miss: u16,
    pub total_score: u32,
    pub max_combo: u16,
    pub perfect_combo: bool,
    pub mods: u32,
    pub life_bar: String,
    pub timestamp: i64,
    pub online_score_id: i64,
    pub origin: ReplayOrigin,
    pub mod_info: ReplayModInfo,
    pub score_info: Option<ReplayScoreInfo>,
}
impl ReplayData {
    pub fn basic_statistics(&self) -> ReplayBasicStatistics {
        // API statistics are more expressive than legacy replay counters, especially for lazer mania.
        if let Some(score_info) = &self.score_info {
            if let Some(stats) = ReplayBasicStatistics::from_statistics_map(&score_info.statistics)
            {
                return stats;
            }
        }
        ReplayBasicStatistics {
            max: self.count_geki as u32,
            hit300: self.count_300 as u32,
            hit200: self.count_katu as u32,
            hit100: self.count_100 as u32,
            hit50: self.count_50 as u32,
            miss: self.count_miss as u32,
        }
    }
}
fn canonical_stat_key(key: &str) -> String {
    key.chars()
        .flat_map(|ch| ch.to_lowercase())
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect()
}
#[derive(Debug, Clone, Copy)]
pub struct ReplayFrame {
    pub time: i32,
    pub x: f32,
    pub y: f32,
    pub keys: u32,
}
impl ReplayFrame {
    #[inline]
    pub fn is_key_pressed(&self, key: u8) -> bool {
        (self.keys & (1 << key)) != 0
    }
    #[inline]
    pub fn pressed_count(&self) -> u32 {
        self.keys.count_ones()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    Press,
    Release,
}
#[derive(Debug, Clone)]
pub struct KeyAction {
    pub time: i32,
    pub column: u8,
    pub pressed: bool,
    pub keys_mask: u32,
}
impl KeyAction {
    #[inline]
    pub fn action_type(&self) -> ActionType {
        if self.pressed {
            ActionType::Press
        } else {
            ActionType::Release
        }
    }
}
#[derive(Debug, Clone)]
pub struct ManiaReplayData {
    pub replay: ReplayData,
    pub frames: Vec<ReplayFrame>,
    pub key_actions: Vec<KeyAction>,
    pub beatmap_file: Option<String>,
}
impl ManiaReplayData {
    pub fn derive_key_actions(frames: &[ReplayFrame], key_count: u8) -> Vec<KeyAction> {
        if frames.is_empty() {
            return Vec::new();
        }
        let mut actions = Vec::with_capacity(frames.len() * 2);
        let mut prev_keys: u32 = 0;
        for frame in frames.iter().filter(|frame| frame.time >= 0) {
            let curr = action_keys(frame, key_count);
            for col in 0..key_count {
                let mask = 1u32 << col;
                let was = (prev_keys & mask) != 0;
                let is = (curr & mask) != 0;
                // Releases come before same-frame presses so transitions do not create phantom holds.
                if was && !is {
                    actions.push(KeyAction {
                        time: frame.time,
                        column: col,
                        pressed: false,
                        keys_mask: curr,
                    });
                }
            }
            for col in 0..key_count {
                let mask = 1u32 << col;
                let was = (prev_keys & mask) != 0;
                let is = (curr & mask) != 0;
                if !was && is {
                    actions.push(KeyAction {
                        time: frame.time,
                        column: col,
                        pressed: true,
                        keys_mask: curr,
                    });
                }
            }
            prev_keys = curr;
        }
        actions
    }
    pub fn actions_by_column(&self, key_count: u8) -> Vec<Vec<&KeyAction>> {
        let mut by_col: Vec<Vec<&KeyAction>> = (0..key_count).map(|_| Vec::new()).collect();
        for action in &self.key_actions {
            if (action.column as usize) < by_col.len() {
                by_col[action.column as usize].push(action);
            }
        }
        by_col
    }
}
fn action_keys(frame: &ReplayFrame, key_count: u8) -> u32 {
    if key_count == 10 && is_legacy_dummy_frame(frame) {
        // Stable encodes 10K spacer frames at x=256,y=-500; map them to the dummy ninth bit.
        return 1 << 8;
    }
    frame.keys
}
fn is_legacy_dummy_frame(frame: &ReplayFrame) -> bool {
    (frame.x - 256.0).abs() < f32::EPSILON && (frame.y + 500.0).abs() < f32::EPSILON
}
