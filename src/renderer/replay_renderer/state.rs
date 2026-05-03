use crate::types::JudgmentKind;
#[derive(Debug, Clone, Copy)]
pub struct LastJudgment {
    pub kind: JudgmentKind,
    pub age_ms: i32,
    pub column: u8,
    pub hit_offset_ms: Option<i32>,
}
#[derive(Debug, Clone, Copy)]
pub struct HitErrorJudgment {
    pub kind: JudgmentKind,
    pub offset_ms: i32,
    pub age_ms: i32,
}
#[derive(Debug, Clone, Copy)]
pub struct HitErrorWindows {
    pub max: i32,
    pub hit300: i32,
    pub hit200: i32,
    pub hit100: i32,
    pub hit50: i32,
}
#[derive(Debug, Clone, Copy)]
pub struct ComboBreakAnimation {
    pub start_combo: u32,
    pub break_time: i32,
    pub age_ms: i32,
    pub column: u8,
}
#[derive(Debug, Clone, Copy)]
pub struct ComboIncAnimation {
    pub time: i32,
    pub age_ms: i32,
}
#[derive(Debug, Clone, Copy)]
pub struct ComboBurstAnimation {
    pub combo: u32,
    pub time: i32,
    pub age_ms: i32,
}
#[derive(Debug, Clone, Default)]
pub struct HudBeatmapMetadataState {
    pub title: String,
    pub title_romanized: String,
    pub artist: String,
    pub artist_romanized: String,
    pub difficulty: String,
    pub mapper: String,
    pub source: String,
    pub tags: String,
    pub beatmap_id: Option<u32>,
    pub beatmapset_id: Option<u32>,
    pub key_count: u8,
    pub cs: f32,
    pub od: f32,
    pub hp: f32,
    pub bpm: f32,
    pub bpm_text: String,
    pub note_count: u32,
    pub max_combo: u32,
    pub duration_ms: i32,
}
#[derive(Debug, Clone, Default)]
pub struct HudFrameState {
    pub hud_visible: bool,
    pub score: u32,
    pub accuracy: f64,
    pub combo: u32,
    // Order is Max, 300, 200, 100, 50, Miss to match custom HUD variable indexes.
    pub judgment_counts: [u32; 6],
    pub progress: f32,
    pub song_elapsed_ms: i32,
    pub song_duration_ms: i32,
    pub beatmap: HudBeatmapMetadataState,
    pub life: f32,
    pub key_down_mask: u32,
    pub key_kps: [f32; 32],
    pub key_press_duration_ms: [i32; 32],
    pub total_kps: f32,
    pub pp_current: Option<f32>,
    pub pp_final: Option<f32>,
    pub pp_available: bool,
    pub unstable_rate: Option<f32>,
    pub hit_error_judgments: Vec<HitErrorJudgment>,
    pub hit_error_moving_avg_ms: Option<f32>,
    pub hit_error_windows: Option<HitErrorWindows>,
    pub is_break_time: bool,
    pub has_failed: bool,
    pub fail_started_at: Option<i32>,
    pub last_judgment: Option<LastJudgment>,
    pub combo_break_anim: Option<ComboBreakAnimation>,
    pub combo_inc_anim: Option<ComboIncAnimation>,
    pub combo_burst_anim: Option<ComboBurstAnimation>,
}
#[derive(Debug, Clone, Default)]
pub struct FailAnimationState {
    pub active: bool,
    pub fail_started_at: i32,
    pub visual_time_ms: i32,
    pub progress: f32,
    pub active_note_indices: Vec<usize>,
    pub frozen_key_mask: u32,
}
#[derive(Debug, Clone, Default)]
pub struct NoteWindow {
    pub start: usize,
    pub end: usize,
}
impl NoteWindow {
    #[inline]
    pub fn update(
        &mut self,
        time: i32,
        note_times: &[i32],
        effective_ends: &[i32],
        look_ahead: i32,
        look_behind: i32,
    ) {
        // Hit objects are sorted by start time, so the visible window can advance without rescanning old notes.
        while self.start < effective_ends.len() && effective_ends[self.start] < time - look_behind {
            self.start += 1;
        }
        while self.end < note_times.len() && note_times[self.end] <= time + look_ahead {
            self.end += 1;
        }
    }
    #[inline]
    pub fn range(&self) -> std::ops::Range<usize> {
        self.start..self.end
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
}
pub mod anim {
    pub const SCORE_ANIM_MS: i32 = 300;
    pub const LAST_JUDGMENT_AGE_MS: i32 = 500;
    pub const COMBO_BREAK_DURATION_MS: i32 = 800;
    pub const COMBO_INC_ANIM_MS: i32 = 80;
    pub const COMBO_BURST_ANIM_MS: i32 = 1400;
    pub const FAIL_ANIM_MS: i32 = 900;
    pub const FAIL_HOLD_MS: i32 = 400;
    pub const LOOK_AHEAD_BUFFER_MS: i32 = 500;
    pub const LOOK_BEHIND_MS: i32 = 400;
    pub const WARMUP_FRAMES: usize = 30;
}
#[derive(Debug)]
pub struct FrameOutput {
    pub data: Vec<u8>,
    pub timestamp: i32,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, Copy)]
pub struct RenderProgress {
    pub current_frame: usize,
    pub total_frames: usize,
    pub elapsed_ms: f64,
}
impl RenderProgress {
    pub fn percent(&self) -> f32 {
        if self.total_frames == 0 {
            return 100.0;
        }
        (self.current_frame as f32 / self.total_frames as f32) * 100.0
    }
    pub fn eta_ms(&self) -> f64 {
        if self.current_frame == 0 {
            return 0.0;
        }
        let ms_per_frame = self.elapsed_ms / self.current_frame as f64;
        let remaining = self.total_frames.saturating_sub(self.current_frame);
        ms_per_frame * remaining as f64
    }
    pub fn fps(&self) -> f64 {
        if self.elapsed_ms < 1.0 {
            return 0.0;
        }
        self.current_frame as f64 / (self.elapsed_ms / 1000.0)
    }
}
