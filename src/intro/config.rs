use std::path::PathBuf;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntroModBadgeSpec {
    pub acronym: String,
    pub summary: Option<String>,
    pub summary_priority: u16,
}
impl IntroModBadgeSpec {
    pub fn new(acronym: impl Into<String>) -> Self {
        Self {
            acronym: acronym.into(),
            summary: None,
            summary_priority: 0,
        }
    }
    pub fn with_summary(
        acronym: impl Into<String>,
        summary: impl Into<String>,
        summary_priority: u16,
    ) -> Self {
        Self {
            acronym: acronym.into(),
            summary: Some(summary.into()),
            summary_priority,
        }
    }
}
#[derive(Debug, Clone)]
pub struct IntroConfig {
    pub duration_ms: u32,
    pub logo_path: PathBuf,
    pub background_path: Option<PathBuf>,
    pub background_blur_percent: Option<u8>,
    pub preview_time_ms: i32,
    pub bpm: f32,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub player_name: Option<String>,
    pub avatar_path: Option<PathBuf>,
    pub country_code: Option<String>,
    pub flag_path: Option<PathBuf>,
    pub team_badge_path: Option<PathBuf>,
    pub map_title: Option<String>,
    pub map_artist: Option<String>,
    pub map_difficulty: Option<String>,
    pub map_creator: Option<String>,
    pub key_count: u8,
    pub mods: u32,
    pub display_mods: Option<Vec<IntroModBadgeSpec>>,
    pub star_rating: Option<f32>,
    pub accuracy: f32,
    pub best_combo: u32,
    pub final_combo: u32,
    pub max_combo: u32,
    pub glow_enabled: bool,
}
impl Default for IntroConfig {
    fn default() -> Self {
        Self {
            duration_ms: 3500,
            logo_path: PathBuf::new(),
            background_path: None,
            background_blur_percent: None,
            preview_time_ms: 0,
            bpm: 120.0,
            width: 1920,
            height: 1080,
            fps: 60,
            player_name: None,
            avatar_path: None,
            country_code: None,
            flag_path: None,
            team_badge_path: None,
            map_title: None,
            map_artist: None,
            map_difficulty: None,
            map_creator: None,
            key_count: 4,
            mods: 0,
            display_mods: None,
            star_rating: None,
            accuracy: 100.0,
            best_combo: 0,
            final_combo: 0,
            max_combo: 0,
            glow_enabled: false,
        }
    }
}
#[derive(Debug, Clone)]
pub struct IntroUIConfig {
    pub width: u32,
    pub height: u32,
    pub player_name: String,
    pub avatar_path: Option<PathBuf>,
    pub country_code: Option<String>,
    pub flag_path: Option<PathBuf>,
    pub team_badge_path: Option<PathBuf>,
    pub map_title: String,
    pub map_artist: String,
    pub map_difficulty: String,
    pub map_creator: String,
    pub key_count: u8,
    pub mods: u32,
    pub display_mods: Option<Vec<IntroModBadgeSpec>>,
    pub star_rating: Option<f32>,
    pub ranked_status: Option<RankedStatus>,
    pub accuracy: f32,
    pub best_combo: u32,
    pub final_combo: u32,
    pub max_combo: u32,
    pub glow_enabled: bool,
    pub glow_color: [u8; 3],
}
impl Default for IntroUIConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            player_name: "Unknown".into(),
            avatar_path: None,
            country_code: None,
            flag_path: None,
            team_badge_path: None,
            map_title: "Unknown".into(),
            map_artist: String::new(),
            map_difficulty: String::new(),
            map_creator: String::new(),
            key_count: 4,
            mods: 0,
            display_mods: None,
            star_rating: None,
            ranked_status: None,
            accuracy: 100.0,
            best_combo: 0,
            final_combo: 0,
            max_combo: 0,
            glow_enabled: false,
            glow_color: [200, 200, 255],
        }
    }
}
impl IntroUIConfig {
    pub fn from_intro_config(cfg: &IntroConfig) -> Self {
        Self {
            width: cfg.width,
            height: cfg.height,
            player_name: cfg.player_name.clone().unwrap_or_else(|| "Unknown".into()),
            avatar_path: cfg.avatar_path.clone(),
            country_code: cfg.country_code.clone(),
            flag_path: cfg.flag_path.clone(),
            team_badge_path: cfg.team_badge_path.clone(),
            map_title: cfg.map_title.clone().unwrap_or_else(|| "Unknown".into()),
            map_artist: cfg.map_artist.clone().unwrap_or_default(),
            map_difficulty: cfg.map_difficulty.clone().unwrap_or_default(),
            map_creator: cfg.map_creator.clone().unwrap_or_default(),
            key_count: cfg.key_count,
            mods: cfg.mods,
            display_mods: cfg.display_mods.clone(),
            star_rating: cfg.star_rating,
            ranked_status: None,
            accuracy: cfg.accuracy,
            best_combo: cfg.best_combo,
            final_combo: cfg.final_combo,
            max_combo: cfg.max_combo,
            glow_enabled: cfg.glow_enabled,
            glow_color: [200, 200, 255],
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankedStatus {
    Ranked,
    Loved,
    Pending,
    Graveyard,
    Qualified,
}
#[derive(Debug)]
pub struct IntroFrame {
    pub data: Vec<u8>,
    pub timestamp_ms: f32,
    pub width: u32,
    pub height: u32,
}
impl IntroFrame {
    pub fn new(data: Vec<u8>, timestamp_ms: f32, width: u32, height: u32) -> Self {
        Self {
            data,
            timestamp_ms,
            width,
            height,
        }
    }
    #[inline]
    pub fn byte_len(&self) -> usize {
        // Intro frames are always raw RGBA8, so each pixel occupies four bytes.
        (self.width * self.height * 4) as usize
    }
}
pub fn star_rating_color(sr: f32) -> [u8; 3] {
    // Keep this in sync with osu!'s difficulty color ramp used by intro badges.
    const SCALE: &[(f32, [u8; 3])] = &[
        (0.0, [0xAA, 0xAA, 0xAA]),
        (0.1, [0x4F, 0xC0, 0xFF]),
        (1.25, [0x4F, 0xC0, 0xFF]),
        (2.0, [0x4F, 0xFF, 0xD5]),
        (2.5, [0x7C, 0xFF, 0x4F]),
        (3.25, [0xF6, 0xF0, 0x5C]),
        (4.5, [0xFF, 0x80, 0x68]),
        (6.0, [0xFF, 0x4E, 0x6F]),
        (6.75, [0xC6, 0x45, 0xB8]),
        (7.75, [0x65, 0x63, 0xDE]),
        (9.0, [0x18, 0x15, 0x8E]),
    ];
    for i in 0..SCALE.len() - 1 {
        let (sr0, c0) = SCALE[i];
        let (sr1, c1) = SCALE[i + 1];
        if sr >= sr0 && sr < sr1 {
            let t = (sr - sr0) / (sr1 - sr0);
            return lerp_color(c0, c1, t);
        }
    }
    SCALE.last().unwrap().1
}
#[inline]
fn lerp_color(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t).round() as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t).round() as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t).round() as u8,
    ]
}
pub fn calc_accuracy(c300: u32, geki: u32, c100: u32, katu: u32, c50: u32, miss: u32) -> f32 {
    let total = c300 + geki + c100 + katu + c50 + miss;
    if total == 0 {
        return 100.0;
    }
    // Mania treats geki as a 300 and katu as a 100 for the legacy accuracy formula.
    let score = (c300 + geki) * 300 + (c100 + katu) * 100 + c50 * 50;
    let max = total * 300;
    (score as f32 / max as f32) * 100.0
}
