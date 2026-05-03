use super::Storyboard;
#[derive(Debug, Clone, Default)]
pub struct BeatmapMetadata {
    pub title: String,
    pub title_unicode: String,
    pub artist: String,
    pub artist_unicode: String,
    pub creator: String,
    pub version: String,
    pub source: String,
    pub tags: String,
    pub beatmap_id: Option<u32>,
    pub beatmapset_id: Option<u32>,
    pub audio_filename: String,
    pub preview_time: i32,
    pub mode: u8,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct Difficulty {
    pub hp: f32,
    pub cs: f32,
    pub od: f32,
    pub ar: f32,
    pub slider_multiplier: f32,
    pub slider_tick_rate: f32,
}
impl Difficulty {
    #[inline]
    pub fn key_count(&self) -> u8 {
        // In mania beatmaps, CircleSize stores the column count.
        self.cs.round() as u8
    }
}
#[derive(Debug, Clone, Copy)]
pub struct TimingPoint {
    pub time: f64,
    pub beat_length: f64,
    pub meter: u8,
    pub sample_set: u8,
    pub sample_index: u8,
    pub volume: u8,
    pub uninherited: bool,
    pub effects: u8,
}
impl TimingPoint {
    #[inline]
    pub fn bpm(&self) -> Option<f64> {
        if self.uninherited && self.beat_length > 0.0 {
            Some(60000.0 / self.beat_length)
        } else {
            None
        }
    }
    #[inline]
    pub fn scroll_velocity(&self) -> f64 {
        if self.uninherited {
            1.0
        } else {
            let abs_bl = self.beat_length.abs();
            if abs_bl < 1e-6 {
                1.0
            } else {
                // Inherited timing points encode slider velocity as -100 / beat_length in osu! files.
                (100.0 / abs_bl).clamp(0.01, 100.0)
            }
        }
    }
}
impl Default for TimingPoint {
    fn default() -> Self {
        Self {
            time: 0.0,
            beat_length: 500.0,
            meter: 4,
            sample_set: 0,
            sample_index: 0,
            volume: 100,
            uninherited: true,
            effects: 0,
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct HitSample {
    pub normal_set: u8,
    pub addition_set: u8,
    pub index: u8,
    pub volume: u8,
    pub filename: String,
}
#[derive(Debug, Clone, Default)]
pub struct HitObject {
    pub x: i32,
    pub y: i32,
    pub time: i32,
    pub obj_type: u8,
    pub hit_sound: u8,
    pub end_time: Option<i32>,
    pub column: u8,
    pub hit_sample: HitSample,
}
impl HitObject {
    #[inline]
    pub fn has_positive_duration(&self) -> bool {
        self.end_time.is_some_and(|end_time| end_time > self.time)
    }
    #[inline]
    pub fn is_long_note(&self) -> bool {
        self.has_positive_duration()
    }
    #[inline]
    pub fn duration(&self) -> i32 {
        self.end_time.map_or(0, |e| (e - self.time).max(0))
    }
    #[inline]
    pub fn column_from_x(x: i32, key_count: u8) -> u8 {
        // osu! hit object x coordinates use a 512-wide playfield regardless of mania columns.
        ((x * key_count as i32) / 512).clamp(0, key_count as i32 - 1) as u8
    }
}
#[derive(Debug, Clone)]
pub enum BackgroundEvent {
    Image {
        filename: String,
        x_offset: i32,
        y_offset: i32,
    },
    Video {
        filename: String,
        start_time: i32,
        x_offset: i32,
        y_offset: i32,
    },
}
#[derive(Debug, Clone, Copy)]
pub struct BreakPeriod {
    pub start: i32,
    pub end: i32,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct ScrollVelocity {
    pub time: i32,
    pub multiplier: f32,
}
impl ScrollVelocity {
    pub fn new(time: i32, multiplier: f32) -> Self {
        Self { time, multiplier }
    }
}
#[derive(Debug, Clone, Default)]
pub struct BeatmapEvents {
    pub backgrounds: Vec<BackgroundEvent>,
    pub breaks: Vec<BreakPeriod>,
}
#[derive(Debug, Clone, Default)]
pub struct Beatmap {
    pub metadata: BeatmapMetadata,
    pub difficulty: Difficulty,
    pub timing_points: Vec<TimingPoint>,
    pub hit_objects: Vec<HitObject>,
    pub events: BeatmapEvents,
    pub storyboard: Storyboard,
}
impl Beatmap {
    #[inline]
    pub fn key_count(&self) -> u8 {
        self.difficulty.key_count()
    }
    #[inline]
    pub fn note_count(&self) -> usize {
        self.hit_objects.len()
    }
    #[inline]
    pub fn has_background_video(&self) -> bool {
        self.events
            .backgrounds
            .iter()
            .any(|event| matches!(event, BackgroundEvent::Video { .. }))
    }
    #[inline]
    pub fn has_storyboard(&self) -> bool {
        !self.storyboard.is_empty()
    }
    pub fn max_combo(&self) -> u32 {
        const TICK_INTERVAL: i32 = 100;
        // Legacy mania combo counts long-note ticks at fixed 100 ms intervals.
        self.hit_objects
            .iter()
            .map(|obj| {
                if obj.is_long_note() {
                    1 + (obj.duration() / TICK_INTERVAL) as u32
                } else {
                    1
                }
            })
            .sum()
    }
    pub fn uninherited_points(&self) -> impl Iterator<Item = &TimingPoint> {
        self.timing_points.iter().filter(|tp| tp.uninherited)
    }
    pub fn inherited_points(&self) -> impl Iterator<Item = &TimingPoint> {
        self.timing_points.iter().filter(|tp| !tp.uninherited)
    }
}
