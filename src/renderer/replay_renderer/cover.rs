use super::{HudFrameState, ManiaLayoutInfo};
use crate::types::SkinAssets;
const LEGACY_SKIN_REFERENCE_HEIGHT: f32 = 480.0;
const REFERENCE_PLAYFIELD_HEIGHT: f32 = 768.0;
const MIN_COVERAGE_PX: f32 = 160.0;
const MAX_COVERAGE_PX: f32 = 400.0;
const COVERAGE_PER_COMBO_PX: f32 = 0.5;
const FLASHLIGHT_BASE_WINDOW_HEIGHT_PX: f32 = 50.0;
const FLASHLIGHT_BREAK_SIZE_MULTIPLIER: f32 = 2.5;
const FLASHLIGHT_EDGE_FADE_BASE_PX: f32 = 12.0;
const DEFAULT_FLASHLIGHT_SIZE_MULTIPLIER: f32 = 1.0;
const DEFAULT_COVERAGE_RATIO: f32 = 0.5;
const GRADIENT_HEIGHT_RATIO: f32 = 0.25;
const DAMPING_RATE_PER_SECOND: f32 = 25.0;
const FLASHLIGHT_BREAK_TRANSITION_MS: f32 = 800.0;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayfieldCoverMode {
    #[default]
    None,
    FadeIn,
    Hidden,
    Cover,
    Flashlight,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayfieldCoverDirection {
    #[default]
    Downwards,
    Upwards,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayfieldCoverConfig {
    pub mode: PlayfieldCoverMode,
    pub direction: PlayfieldCoverDirection,
    pub coverage_ratio: f32,
    pub flashlight_size_multiplier: f32,
    pub flashlight_combo_based_size: bool,
}
impl Default for PlayfieldCoverConfig {
    fn default() -> Self {
        Self {
            mode: PlayfieldCoverMode::None,
            direction: PlayfieldCoverDirection::Downwards,
            coverage_ratio: DEFAULT_COVERAGE_RATIO,
            flashlight_size_multiplier: DEFAULT_FLASHLIGHT_SIZE_MULTIPLIER,
            flashlight_combo_based_size: false,
        }
    }
}
impl From<PlayfieldCoverMode> for PlayfieldCoverConfig {
    fn from(mode: PlayfieldCoverMode) -> Self {
        Self {
            // Hidden covers from the receptor upward; FadeIn/Cover default from the top downward.
            mode,
            direction: if mode == PlayfieldCoverMode::Hidden {
                PlayfieldCoverDirection::Upwards
            } else {
                PlayfieldCoverDirection::Downwards
            },
            ..Default::default()
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlayfieldCoverState {
    pub mode: PlayfieldCoverMode,
    pub direction: PlayfieldCoverDirection,
    pub top_y: f32,
    pub bottom_y: f32,
    pub total_height_px: f32,
    pub opaque_height_px: f32,
    pub fade_height_px: f32,
    pub window_top_y: f32,
    pub window_bottom_y: f32,
    pub transition_span_px: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlayfieldCoverMetrics {
    pub top_y: f32,
    pub hit_y: f32,
    pub stage_bottom_y: f32,
    pub logical_height_px: f32,
    pub gradient_height_px: f32,
    pub fill_height_min_px: f32,
    pub fill_height_max_px: f32,
    pub fill_height_per_combo_px: f32,
    pub flashlight_window_height_px: f32,
    pub flashlight_break_window_height_px: f32,
    pub flashlight_edge_fade_height_px: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlayfieldCoverRuntime {
    pub displayed_primary_size_px: Option<f32>,
    pub last_frame_time_ms: Option<i32>,
}
impl PlayfieldCoverState {
    #[inline]
    pub fn is_active(&self) -> bool {
        self.mode != PlayfieldCoverMode::None && self.total_height_px > 0.0
    }
    #[inline]
    pub fn bottom_y(&self) -> f32 {
        self.bottom_y
    }
    #[inline]
    pub fn opaque_start_y(&self) -> f32 {
        match self.direction {
            PlayfieldCoverDirection::Downwards => self.top_y,
            PlayfieldCoverDirection::Upwards => self.bottom_y - self.opaque_height_px.max(0.0),
        }
    }
    #[inline]
    pub fn opaque_end_y(&self) -> f32 {
        match self.direction {
            PlayfieldCoverDirection::Downwards => self.top_y + self.opaque_height_px.max(0.0),
            PlayfieldCoverDirection::Upwards => self.bottom_y,
        }
    }
    #[inline]
    pub fn fade_start_y(&self) -> f32 {
        match self.direction {
            PlayfieldCoverDirection::Downwards => self.opaque_end_y(),
            PlayfieldCoverDirection::Upwards => self.top_y,
        }
    }
    #[inline]
    pub fn fade_end_y(&self) -> f32 {
        match self.direction {
            PlayfieldCoverDirection::Downwards => self.bottom_y,
            PlayfieldCoverDirection::Upwards => self.top_y + self.fade_height_px.max(0.0),
        }
    }
    pub fn opacity_at_y(&self, y: f32) -> f32 {
        if !self.is_active() {
            return 0.0;
        }
        if y < self.top_y || y >= self.bottom_y() {
            return 0.0;
        }
        if self.mode == PlayfieldCoverMode::Flashlight {
            let top_fade_start = (self.window_top_y - self.fade_height_px).max(self.top_y);
            let bottom_fade_end = (self.window_bottom_y + self.fade_height_px).min(self.bottom_y);
            if y < top_fade_start {
                return 1.0;
            }
            if y < self.window_top_y {
                let fade_span = (self.window_top_y - top_fade_start).max(1.0);
                return (1.0 - (y - top_fade_start) / fade_span).clamp(0.0, 1.0);
            }
            if y < self.window_bottom_y {
                return 0.0;
            }
            if y < bottom_fade_end {
                let fade_span = (bottom_fade_end - self.window_bottom_y).max(1.0);
                return ((y - self.window_bottom_y) / fade_span).clamp(0.0, 1.0);
            }
            return 1.0;
        }
        match self.direction {
            PlayfieldCoverDirection::Downwards => {
                let opaque_end = self.opaque_end_y();
                if y < opaque_end || self.fade_height_px <= 0.0 {
                    return 1.0;
                }
                (1.0 - ((y - self.fade_start_y()) / self.fade_height_px)).clamp(0.0, 1.0)
            }
            PlayfieldCoverDirection::Upwards => {
                let fade_end = self.fade_end_y();
                if y >= fade_end || self.fade_height_px <= 0.0 {
                    return 1.0;
                }
                ((y - self.top_y) / self.fade_height_px).clamp(0.0, 1.0)
            }
        }
    }
}
pub fn resolve_playfield_cover_state(
    config: impl Into<PlayfieldCoverConfig>,
    metrics: PlayfieldCoverMetrics,
    hud_state: &HudFrameState,
) -> Option<PlayfieldCoverState> {
    let config = config.into();
    let mode = config.mode;
    if mode == PlayfieldCoverMode::None {
        return None;
    }
    if hud_state.is_break_time
        && mode != PlayfieldCoverMode::Flashlight
        && mode != PlayfieldCoverMode::Cover
    {
        // Stable disables Hidden/FadeIn style covers during breaks, but Cover and Flashlight remain visible.
        return None;
    }
    if mode == PlayfieldCoverMode::Flashlight {
        let size_multiplier = if config.flashlight_size_multiplier.is_finite() {
            config.flashlight_size_multiplier.clamp(0.5, 3.0)
        } else {
            DEFAULT_FLASHLIGHT_SIZE_MULTIPLIER
        };
        let combo_scale = if config.flashlight_combo_based_size && !hud_state.is_break_time {
            // Combo-based flashlight shrinks the visible window as combo increases.
            flashlight_combo_scale_for(hud_state.combo)
        } else {
            1.0
        };
        let target_window_height_px = if hud_state.is_break_time {
            metrics.flashlight_break_window_height_px * size_multiplier
        } else {
            metrics.flashlight_window_height_px * size_multiplier * combo_scale
        }
        .min((metrics.stage_bottom_y - metrics.top_y).max(0.0))
        .max(0.0);
        let stage_center_y = (metrics.top_y + metrics.stage_bottom_y) * 0.5;
        let half_window = target_window_height_px * 0.5;
        let window_top_y = (stage_center_y - half_window).max(metrics.top_y);
        let window_bottom_y = (stage_center_y + half_window).min(metrics.stage_bottom_y);
        let window_height_px = (window_bottom_y - window_top_y).max(0.0);
        return Some(PlayfieldCoverState {
            mode,
            direction: PlayfieldCoverDirection::Downwards,
            top_y: metrics.top_y,
            bottom_y: metrics.stage_bottom_y,
            total_height_px: window_height_px,
            opaque_height_px: 0.0,
            fade_height_px: metrics
                .flashlight_edge_fade_height_px
                .min(window_height_px * 0.5)
                .max(0.0),
            window_top_y,
            window_bottom_y,
            transition_span_px: (metrics.flashlight_break_window_height_px * size_multiplier
                - metrics.flashlight_window_height_px * size_multiplier)
                .abs(),
        });
    }
    let (fill_height_px, fade_height_px, direction, top_y, bottom_y, total_height_px) = if mode
        == PlayfieldCoverMode::Cover
    {
        let fill_height_px =
            (metrics.logical_height_px * config.coverage_ratio.clamp(0.0, 1.0)).max(0.0);
        let fade_height_px = metrics.gradient_height_px.max(0.0);
        let total_height_px = fill_height_px + fade_height_px;
        let direction = config.direction;
        let (top_y, bottom_y) = match direction {
            PlayfieldCoverDirection::Downwards => (metrics.top_y, metrics.top_y + total_height_px),
            PlayfieldCoverDirection::Upwards => (metrics.hit_y - total_height_px, metrics.hit_y),
        };
        (
            fill_height_px,
            fade_height_px,
            direction,
            top_y,
            bottom_y,
            total_height_px,
        )
    } else {
        let fill_height_px = (metrics.fill_height_min_px
            + hud_state.combo as f32 * metrics.fill_height_per_combo_px)
            .min(metrics.fill_height_max_px)
            .max(0.0);
        let fade_height_px = metrics.gradient_height_px.max(0.0);
        let total_height_px = fill_height_px + fade_height_px;
        let (direction, top_y, bottom_y) = match mode {
            PlayfieldCoverMode::FadeIn => (
                PlayfieldCoverDirection::Downwards,
                metrics.top_y,
                metrics.top_y + total_height_px,
            ),
            PlayfieldCoverMode::Hidden => (
                PlayfieldCoverDirection::Upwards,
                metrics.hit_y - total_height_px,
                metrics.hit_y,
            ),
            PlayfieldCoverMode::Cover => unreachable!("cover handled in dedicated branch"),
            PlayfieldCoverMode::Flashlight => {
                unreachable!("flashlight cover state handled earlier")
            }
            PlayfieldCoverMode::None => return None,
        };
        (
            fill_height_px,
            fade_height_px,
            direction,
            top_y,
            bottom_y,
            total_height_px,
        )
    };
    Some(PlayfieldCoverState {
        mode,
        direction,
        top_y,
        bottom_y,
        total_height_px,
        opaque_height_px: fill_height_px,
        fade_height_px,
        window_top_y: 0.0,
        window_bottom_y: 0.0,
        transition_span_px: 0.0,
    })
}
pub fn smooth_playfield_cover_state(
    target: Option<PlayfieldCoverState>,
    runtime: &mut PlayfieldCoverRuntime,
    frame_time_ms: i32,
) -> Option<PlayfieldCoverState> {
    let Some(mut target) = target else {
        *runtime = PlayfieldCoverRuntime::default();
        return None;
    };
    let dt_ms = runtime
        .last_frame_time_ms
        .map(|last| (frame_time_ms - last).max(0) as f32)
        .unwrap_or(0.0);
    if target.mode == PlayfieldCoverMode::Flashlight {
        let displayed_window_height_px = match runtime.displayed_primary_size_px {
            None => target.total_height_px,
            Some(previous) if dt_ms <= 0.0 => previous,
            Some(previous) => {
                // Break-time flashlight size changes linearly to match osu!'s slower window expansion.
                let max_step = ((target.total_height_px - previous).abs()
                    / FLASHLIGHT_BREAK_TRANSITION_MS)
                    * dt_ms;
                let delta = target.total_height_px - previous;
                if delta.abs() <= max_step {
                    target.total_height_px
                } else {
                    previous + delta.signum() * max_step
                }
            }
        };
        runtime.displayed_primary_size_px = Some(displayed_window_height_px);
        runtime.last_frame_time_ms = Some(frame_time_ms);
        let stage_center_y = (target.top_y + target.bottom_y) * 0.5;
        let half_window = displayed_window_height_px.max(0.0) * 0.5;
        target.total_height_px = displayed_window_height_px.max(0.0);
        target.window_top_y = (stage_center_y - half_window).max(target.top_y);
        target.window_bottom_y = (stage_center_y + half_window).min(target.bottom_y);
        target.fade_height_px = target
            .fade_height_px
            .min(((target.window_bottom_y - target.window_top_y) * 0.5).max(0.0));
        return Some(target);
    }
    let displayed_fill_height_px = match runtime.displayed_primary_size_px {
        None => target.opaque_height_px,
        Some(previous) if dt_ms <= 0.0 => previous,
        Some(previous) => {
            let dt_seconds = dt_ms / 1000.0;
            let blend = 1.0 - (-DAMPING_RATE_PER_SECOND * dt_seconds).exp();
            previous + (target.opaque_height_px - previous) * blend.clamp(0.0, 1.0)
        }
    };
    runtime.displayed_primary_size_px = Some(displayed_fill_height_px);
    runtime.last_frame_time_ms = Some(frame_time_ms);
    target.opaque_height_px = displayed_fill_height_px.max(0.0);
    target.total_height_px = target.opaque_height_px + target.fade_height_px;
    match target.direction {
        PlayfieldCoverDirection::Downwards => {
            target.bottom_y = target.top_y + target.total_height_px;
        }
        PlayfieldCoverDirection::Upwards => {
            target.top_y = target.bottom_y - target.total_height_px;
        }
    }
    Some(target)
}
fn flashlight_combo_scale_for(combo: u32) -> f32 {
    if combo >= 200 {
        0.625
    } else if combo >= 100 {
        0.8125
    } else {
        1.0
    }
}
pub fn resolve_playfield_cover_metrics(
    _mode: PlayfieldCoverMode,
    layout: &ManiaLayoutInfo,
    skin: &SkinAssets,
    _canvas_height: u32,
) -> PlayfieldCoverMetrics {
    let logical_height = (layout.stage.hit_y - layout.stage.top_y).max(0) as f32;
    let hit_position = skin.config.hit_position.unwrap_or(402).max(1) as f32;
    let legacy_available_height =
        (hit_position * (REFERENCE_PLAYFIELD_HEIGHT / LEGACY_SKIN_REFERENCE_HEIGHT)).max(1.0);
    // Stable authored cover distances against 480p skin coordinates and 768p gameplay height.
    let fill_scale = logical_height / legacy_available_height;
    PlayfieldCoverMetrics {
        top_y: layout.stage.top_y as f32,
        hit_y: layout.stage.hit_y as f32,
        stage_bottom_y: layout.stage.bottom_y as f32,
        logical_height_px: logical_height,
        gradient_height_px: logical_height * GRADIENT_HEIGHT_RATIO,
        fill_height_min_px: MIN_COVERAGE_PX * fill_scale,
        fill_height_max_px: MAX_COVERAGE_PX * fill_scale,
        fill_height_per_combo_px: COVERAGE_PER_COMBO_PX * fill_scale,
        flashlight_window_height_px: FLASHLIGHT_BASE_WINDOW_HEIGHT_PX * layout.scale_y,
        flashlight_break_window_height_px: FLASHLIGHT_BASE_WINDOW_HEIGHT_PX
            * FLASHLIGHT_BREAK_SIZE_MULTIPLIER
            * layout.scale_y,
        flashlight_edge_fade_height_px: FLASHLIGHT_EDGE_FADE_BASE_PX * layout.scale_y,
    }
}
