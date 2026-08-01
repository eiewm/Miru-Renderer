use crate::beatmaps::{self, ResolveOptions};
use crate::hud::{resolve_hud_config, HudConfig};
use crate::intro::{
    get_bpm_at_time, GpuIntroRenderer, IntroConfig, IntroModBadgeSpec, INTRO_DURATION_MS,
};
use crate::modes::mania::assets::{map_column_family, ManiaFamily};
use crate::modes::mania::layout::ScrollDirection;
use crate::modes::mania::judgment::{self, HitWindows, Judgment};
use crate::modes::mania::timing;
use crate::parser;
use crate::renderer::gpu::context::GpuPreference;
use crate::renderer::{
    anim, ColumnLayout, FailAnimationState, HudBeatmapMetadataState, HudFrameState, LastJudgment,
    LnReleaseInfo, ManiaLayoutInfo, NoteWindow, PlayfieldCoverConfig, PlayfieldCoverDirection,
    PlayfieldCoverMode, RenderJudgment, ReplayRenderer, StageLayout, Windows,
};
use crate::results::{EndSequencePlan, ResultsScreenData};
use crate::types::replay::{
    KeyAction, ManiaReplayData, ReplayBasicStatistics, ReplayData, ReplayModInfo,
};
use crate::types::{BackgroundEvent, Beatmap, JudgmentKind, SkinAssets, TimingPoint};
use crate::utils::mods::{
    has_fade_in_mod, has_flashlight_mod, has_hidden_mod, has_no_fail_mod, has_perfect_mod,
    has_sudden_death_mod, resolve_lazer_playback_mod_settings, resolve_playback_mod_settings,
    AdaptivePlaybackConfig, PlaybackAudioMode, PlaybackModSettings, PlaybackRateProfile,
};
use crate::video::{
    BackgroundInput, BackgroundKind as VideoBackgroundKind, BgComposeMode, ComposeError,
    ComposeFailure, ComposeOpts, FrameComposer, PlaybackClock, VideoEncoder,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
mod analyze;
mod api;
mod layout;
mod prepare;
mod render;
mod resolve;
mod skin;
mod temp;
pub use api::*;
pub(crate) use layout::KeyMaskEvent;
pub(crate) use prepare::{ComboComputation, PreparedReplayRender};
pub(crate) use resolve::finalize_health_timeline_for_replay;
pub(crate) use temp::{
    retry_encoder_for_failure, AttemptOutputFile, IntroUserDataGuard, TempDirGuard, TempFileGuard,
};
pub type ProgressCallback = Box<dyn Fn(u32, &str) + Send + Sync>;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ReplayAccuracyChallengeMode {
    #[default]
    MaximumAchievable,
    Standard,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ReplayPerfectFailCondition {
    pub(crate) require_perfect_hits: bool,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ReplayAccuracyChallengeFailCondition {
    pub(crate) minimum_accuracy: f64,
    pub(crate) mode: ReplayAccuracyChallengeMode,
}
impl Default for ReplayAccuracyChallengeFailCondition {
    fn default() -> Self {
        Self {
            minimum_accuracy: 0.90,
            mode: ReplayAccuracyChallengeMode::MaximumAchievable,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct ReplayFailConditions {
    pub(crate) sudden_death: bool,
    pub(crate) perfect: Option<ReplayPerfectFailCondition>,
    pub(crate) accuracy_challenge: Option<ReplayAccuracyChallengeFailCondition>,
}
impl ReplayFailConditions {
    #[inline]
    pub(crate) const fn is_empty(self) -> bool {
        !self.sudden_death && self.perfect.is_none() && self.accuracy_challenge.is_none()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ReplayBeatmapConversionMods {
    pub(crate) mirror: bool,
    pub(crate) invert: bool,
    pub(crate) hold_off: bool,
}
impl ReplayBeatmapConversionMods {
    #[inline]
    pub(crate) const fn is_empty(self) -> bool {
        !self.mirror && !self.invert && !self.hold_off
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ReplayScrollVisualisationMods {
    pub(crate) constant_speed: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReplayMutedConfig {
    pub(crate) inverse_muting: bool,
    pub(crate) enable_metronome: bool,
    pub(crate) mute_combo_count: u32,
    pub(crate) affects_hit_sounds: bool,
}
impl Default for ReplayMutedConfig {
    fn default() -> Self {
        Self {
            inverse_muting: false,
            enable_metronome: true,
            mute_combo_count: 100,
            affects_hit_sounds: true,
        }
    }
}
pub struct ManiaVideoConverter {
    pub settings: ConverterSettings,
    progress_cb: Option<ProgressCallback>,
}
impl ManiaVideoConverter {
    pub fn new(settings: ConverterSettings) -> Self {
        Self {
            settings,
            progress_cb: None,
        }
    }
    pub fn set_progress_callback(&mut self, cb: ProgressCallback) {
        self.progress_cb = Some(cb);
    }
    fn progress(&self, pct: u32, msg: &str) {
        if let Some(ref cb) = self.progress_cb {
            cb(pct, msg);
        }
    }
    pub fn set_scroll_speed(&mut self, v: f32) {
        self.settings.scroll_speed = v;
    }
    pub fn set_lead_in_ms(&mut self, v: i32) {
        self.settings.lead_in_ms = v;
    }
    pub fn set_fps(&mut self, v: u32) {
        self.settings.fps = v;
    }
    pub fn set_canvas_size(&mut self, w: u32, h: u32) {
        self.settings.width = w;
        self.settings.height = h;
    }
    pub fn set_hud_enabled(&mut self, v: bool) {
        self.settings.enable_hud = v;
    }
    pub fn set_lighting_enabled(&mut self, v: bool) {
        self.settings.enable_lighting = v;
    }
    pub fn set_ln_debug(&mut self, v: bool) {
        self.settings.ln_debug = v;
    }
    pub fn set_sv_enabled(&mut self, v: bool) {
        self.settings.sv_enabled = v;
    }
    pub fn set_skin_animations_enabled(&mut self, v: bool) {
        self.settings.skin_animations_enabled = v;
    }
    pub fn set_intro_enabled(&mut self, v: bool) {
        self.settings.intro_enabled = v;
    }
    pub fn set_hud_config(&mut self, cfg: Option<HudConfig>) {
        self.settings.hud_config = cfg;
    }
    /// The user HUD in canvas pixels, over the portrait layout when portrait.
    pub(crate) fn resolve_hud_config_for_canvas(&self) -> Option<HudConfig> {
        let width = self.settings.width;
        let height = self.settings.height;
        let resolved = self
            .settings
            .hud_config
            .as_ref()
            .map(|cfg| resolve_hud_config(cfg, width as f32, height as f32));
        crate::hud::with_vertical_defaults(resolved, width, height)
    }
    pub fn set_storyboard_enabled(&mut self, v: bool) {
        self.settings.storyboard_enabled = v;
    }
    pub fn set_note_debug(&mut self, v: bool) {
        self.settings.note_debug = v;
    }
    pub fn set_all_presses(&mut self, v: bool) {
        self.settings.all_presses = v;
    }
}
impl Default for ManiaVideoConverter {
    fn default() -> Self {
        Self::new(ConverterSettings::default())
    }
}
