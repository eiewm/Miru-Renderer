use super::config::RendererConfig;
use super::cover::{
    resolve_playfield_cover_state, smooth_playfield_cover_state, PlayfieldCoverConfig,
    PlayfieldCoverMetrics, PlayfieldCoverRuntime, PlayfieldCoverState,
};
use super::debug_text::DebugTextCache;
use super::error::RendererError;
use super::layout::ManiaLayoutInfo;
use super::model::{LnReleaseInfo, RenderJudgment, ReplayModDisplay, Windows};
use super::sprites::{z_order, RenderCommand, SpritePlanner};
use super::state::{FailAnimationState, HudBeatmapMetadataState, HudFrameState};
use super::storyboard::StoryboardPlayer;
use super::textures::{HudDigitHeightCache, OpaqueBounds, TextureMeta};
use crate::hud::{HudConfig, HudElementConfig};
use crate::modes::mania::timing::StableScrollModel;
use crate::renderer::gpu::context::GpuPreference;
use crate::renderer::gpu::{
    GpuContext, GpuError, SpriteBlendMode, SpriteInstance, SpritePipeline, TextureSampling,
};
use crate::types::{SkinAssets, StoryboardLayer};
use crate::video::playback::PlaybackClock;
use ab_glyph::FontArc;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;
const RUNTIME_BIND_GROUP_CACHE_MAX: usize = 1024;
const RUNTIME_DEBUG_TEXT_CACHE_MAX: usize = 1024;
type RenderBatch = (
    Arc<wgpu::Texture>,
    TextureSampling,
    SpriteBlendMode,
    Vec<SpriteInstance>,
);
pub(super) const LEGACY_SCORE_LAYOUT_BASE_SCALE: f32 = 0.96;
pub(super) const LEGACY_ACCURACY_LAYOUT_BASE_SCALE: f32 = 0.576;
pub(super) const LEGACY_SONG_PROGRESS_LAYOUT_BASE_SIZE: f32 = 33.0;
pub(super) const LEGACY_SONG_PROGRESS_GAP_X: f32 = 18.0;
pub(super) const LEGACY_SCORE_MARGIN_X: f32 = 10.0;
pub(super) const LEGACY_ACCURACY_MARGIN_X: f32 = 17.0;
pub(super) const LEGACY_ACCURACY_MARGIN_Y: f32 = 9.0;
pub(super) const REPLAY_MOD_DEFAULT_ICON_SIZE: f32 = 48.0;
pub(super) const REPLAY_MOD_COLLAPSED_STEP_X: f32 = 23.0;
pub(super) const REPLAY_MOD_DEFAULT_MARGIN_RIGHT: f32 = 10.0;
pub(super) const REPLAY_MOD_DEFAULT_MARGIN_TOP: f32 = 10.0;
pub(super) const LEGACY_COMBO_LAYOUT_SCALE: f32 = 1.28;
const SCREEN_RIGHT_HUD_REFERENCE_HEIGHT: f32 = 768.0;
const STAGE_RELATIVE_HUD_REFERENCE_HEIGHT: f32 = 768.0;
const STAGE_RELATIVE_ACCURACY_LEFT_GAP_480: f32 = 91.925;
const STAGE_RELATIVE_ACCURACY_TOP_480: f32 = 28.74;
const STAGE_RELATIVE_PROGRESS_GAP_480: f32 = 5.13;
const LEGACY_STAGE_HINT_Y_SCALE: f32 = 0.9 * 1.6025;
#[derive(Debug, Clone)]
pub(super) struct HudAssetFrame {
    pub texture_id: String,
    pub width: u32,
    pub height: u32,
    pub delay_ms: u32,
}
#[derive(Debug, Clone, Copy)]
pub(super) struct HudKeyTailRelease {
    pub key_index: usize,
    pub released_at_ms: i32,
    pub duration_ms: i32,
}
#[derive(Debug, Clone, Copy)]
pub(super) struct HudJudgmentCounterAnimation {
    pub previous_value: u32,
    pub current_value: u32,
    pub changed_at_ms: i32,
}
#[derive(Debug, Clone, Copy)]
pub struct HudEditorPreviewRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
pub(super) fn legacy_scaled_hud_px(unscaled_px: f32, scale: f32) -> f32 {
    unscaled_px * scale
}
pub(super) fn legacy_component_right_edge(canvas_w: u32, margin_x: f32, scale_x: f32) -> f32 {
    canvas_w as f32 - legacy_scaled_hud_px(margin_x, scale_x)
}
pub(super) fn screen_right_hud_scale(canvas_h: u32) -> f32 {
    (canvas_h.max(1) as f32 / SCREEN_RIGHT_HUD_REFERENCE_HEIGHT).max(f32::EPSILON)
}
fn legacy_song_progress_default_left(
    canvas_w: u32,
    accuracy_width: f32,
    gap_x: f32,
    size: f32,
) -> f32 {
    canvas_w as f32 - accuracy_width - gap_x - size
}
fn mania_stage_scale(layout: &ManiaLayoutInfo) -> f32 {
    (layout.stage.height.max(1) as f32 / 480.0).max(f32::EPSILON)
}
fn stage_relative_hud_scale(layout: &ManiaLayoutInfo) -> f32 {
    (layout.stage.height.max(1) as f32 / STAGE_RELATIVE_HUD_REFERENCE_HEIGHT).max(f32::EPSILON)
}
fn stage_relative_accuracy_left(layout: &ManiaLayoutInfo) -> f32 {
    layout.stage.x as f32 - STAGE_RELATIVE_ACCURACY_LEFT_GAP_480 * mania_stage_scale(layout)
}
fn stage_relative_accuracy_top(layout: &ManiaLayoutInfo) -> f32 {
    layout.stage.top_y as f32 + STAGE_RELATIVE_ACCURACY_TOP_480 * mania_stage_scale(layout)
}
fn stage_relative_accuracy_right(layout: &ManiaLayoutInfo, accuracy_width: f32) -> f32 {
    stage_relative_accuracy_left(layout) + accuracy_width.max(0.0)
}
fn stage_relative_song_progress_left(
    layout: &ManiaLayoutInfo,
    accuracy_left: f32,
    size: f32,
) -> f32 {
    accuracy_left - STAGE_RELATIVE_PROGRESS_GAP_480 * mania_stage_scale(layout) - size.max(0.0)
}
fn legacy_editor_texture_display_height(
    texture_id: &str,
    texture_height: u32,
    layout: &ManiaLayoutInfo,
) -> f32 {
    // The legacy editor lays out skin assets in logical pixels, so @2x textures count as half height.
    let scale_adjust = if texture_id.contains("@2x") { 2.0 } else { 1.0 };
    let screen_scale = layout.stage.height.max(1) as f32 / STAGE_RELATIVE_HUD_REFERENCE_HEIGHT;
    texture_height.max(1) as f32 / scale_adjust * screen_scale
}
pub(super) fn stable_replay_mod_texture_stem(acronym: &str) -> Option<&'static str> {
    match acronym.trim().to_ascii_uppercase().as_str() {
        "NF" => Some("selection-mod-nofail"),
        "EZ" => Some("selection-mod-easy"),
        "HD" => Some("selection-mod-hidden"),
        "HR" => Some("selection-mod-hardrock"),
        "SD" => Some("selection-mod-suddendeath"),
        "PF" => Some("selection-mod-perfect"),
        "DT" => Some("selection-mod-doubletime"),
        "NC" => Some("selection-mod-nightcore"),
        "HT" => Some("selection-mod-halftime"),
        "FL" => Some("selection-mod-flashlight"),
        "FI" => Some("selection-mod-fadein"),
        "MR" => Some("selection-mod-mirror"),
        "V2" => Some("selection-mod-scorev2"),
        "AT" => Some("selection-mod-autoplay"),
        "CO" => Some("selection-mod-keycoop"),
        _ => None,
    }
}
pub(super) fn lazer_replay_mod_texture_name(acronym: &str) -> String {
    format!(
        "replay-mods/lazer/{}.png",
        acronym.trim().to_ascii_lowercase()
    )
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacyAccuracyDefaultLayoutMode {
    ScreenRight,
    #[allow(dead_code)]
    StageRelative,
}
#[derive(Debug, Clone, Copy)]
struct LegacyTextComponentTransform {
    right_x: f32,
    top_y: f32,
    scale_x: f32,
    scale_y: f32,
}
fn resolve_legacy_text_component_scale(
    layout: Option<&crate::types::LegacyHudDrawableLayout>,
    default_scale: f32,
) -> (f32, f32) {
    let Some(layout) = layout else {
        return (default_scale, default_scale);
    };
    let scale_x = if layout.scale_x.is_finite() && layout.scale_x > 0.0 {
        layout.scale_x
    } else {
        default_scale
    };
    let scale_y = if layout.scale_y.is_finite() && layout.scale_y > 0.0 {
        layout.scale_y
    } else {
        scale_x
    };
    (scale_x, scale_y)
}
fn resolve_legacy_text_scale_from_hud_config(
    cfg: &HudElementConfig,
    raw_width: f32,
    raw_height: f32,
    default_scale: f32,
) -> (f32, f32) {
    let default_scale = default_scale.max(f32::EPSILON);
    let raw_width = raw_width.max(1.0);
    let raw_height = raw_height.max(1.0);
    let (mut scale_x, mut scale_y) =
        if let Some(height) = cfg.height.filter(|v| v.is_finite() && *v > 0.0) {
            let height_scale = height / raw_height;
            if let Some(width) = cfg.width.filter(|v| v.is_finite() && *v > 0.0) {
                (width / raw_width, height_scale)
            } else {
                (height_scale, height_scale)
            }
        } else if let Some(width) = cfg.width.filter(|v| v.is_finite() && *v > 0.0) {
            let width_scale = width / raw_width;
            (width_scale, width_scale)
        } else {
            (default_scale, default_scale)
        };
    if cfg.height.is_none() && cfg.width.is_none() {
        if let Some(scale) = cfg.scale.or(cfg.size).filter(|v| v.is_finite() && *v > 0.0) {
            scale_x *= scale;
            scale_y *= scale;
        }
    } else if let Some(scale) = cfg.scale.or(cfg.size).filter(|v| v.is_finite() && *v > 0.0) {
        scale_x *= scale;
        scale_y *= scale;
    }
    (scale_x.max(f32::EPSILON), scale_y.max(f32::EPSILON))
}
fn legacy_alignment_factor(anchor: i32, axis: char) -> Option<f32> {
    match (anchor, axis) {
        (9 | 10 | 12, 'x') => Some(0.0),
        (17 | 18 | 20, 'x') => Some(0.5),
        (33 | 34 | 36, 'x') => Some(1.0),
        (9 | 17 | 33, 'y') => Some(0.0),
        (10 | 18 | 34, 'y') => Some(0.5),
        (12 | 20 | 36, 'y') => Some(1.0),
        _ => None,
    }
}
fn legacy_anchor_screen_point_f32(anchor: i32, canvas_w: f32, canvas_h: f32) -> Option<(f32, f32)> {
    match anchor {
        9 => Some((0.0, 0.0)),
        10 => Some((0.0, canvas_h / 2.0)),
        12 => Some((0.0, canvas_h)),
        17 => Some((canvas_w / 2.0, 0.0)),
        20 => Some((canvas_w / 2.0, canvas_h)),
        33 => Some((canvas_w, 0.0)),
        34 => Some((canvas_w, canvas_h / 2.0)),
        36 => Some((canvas_w, canvas_h)),
        _ => None,
    }
}
fn resolve_legacy_text_component_transform(
    canvas_w: u32,
    canvas_h: u32,
    draw_width: f32,
    draw_height: f32,
    layout: Option<&crate::types::LegacyHudDrawableLayout>,
    default_scale: f32,
    content_scale: f32,
    default_anchor: i32,
    default_origin: i32,
    default_x: f32,
    default_y: f32,
    margin_x: f32,
    margin_y: f32,
) -> LegacyTextComponentTransform {
    let (scale_x, scale_y) = resolve_legacy_text_component_scale(layout, default_scale);
    let content_scale = content_scale.max(f32::EPSILON);
    // Legacy HUD coordinates are authored in a virtual canvas; convert back after anchors and origins resolve.
    let virtual_canvas_w = canvas_w as f32 / content_scale;
    let virtual_canvas_h = canvas_h as f32 / content_scale;
    let scaled_draw_width = (draw_width.max(0.0) * scale_x).max(0.0);
    let scaled_draw_height = (draw_height.max(0.0) * scale_y).max(0.0);
    let anchor = layout.map(|l| l.anchor).unwrap_or(default_anchor);
    let origin = layout.map(|l| l.origin).unwrap_or(default_origin);
    let position_x = layout
        .map(|l| l.x)
        .filter(|v| v.is_finite())
        .unwrap_or(default_x);
    let position_y = layout
        .map(|l| l.y)
        .filter(|v| v.is_finite())
        .unwrap_or(default_y);
    let Some((anchor_x, anchor_y)) =
        legacy_anchor_screen_point_f32(anchor, virtual_canvas_w, virtual_canvas_h)
    else {
        return LegacyTextComponentTransform {
            right_x: canvas_w as f32,
            top_y: 0.0,
            scale_x: scale_x * content_scale,
            scale_y: scale_y * content_scale,
        };
    };
    let origin_factor_x = legacy_alignment_factor(origin, 'x').unwrap_or(1.0);
    let origin_factor_y = legacy_alignment_factor(origin, 'y').unwrap_or(0.0);
    let layout_w = scaled_draw_width + margin_x * 2.0 * scale_x;
    let layout_h = scaled_draw_height + margin_y * 2.0 * scale_y;
    let origin_x = origin_factor_x * layout_w;
    let origin_y = origin_factor_y * layout_h;
    let draw_offset_x = margin_x * scale_x;
    let draw_offset_y = margin_y * scale_y;
    let left = anchor_x + position_x - origin_x + draw_offset_x;
    let top = anchor_y + position_y - origin_y + draw_offset_y;
    let right = left + scaled_draw_width;
    LegacyTextComponentTransform {
        right_x: right * content_scale,
        top_y: top * content_scale,
        scale_x: scale_x * content_scale,
        scale_y: scale_y * content_scale,
    }
}
fn legacy_song_progress_layout_size(_canvas_w: u32, canvas_h: u32) -> u32 {
    (LEGACY_SONG_PROGRESS_LAYOUT_BASE_SIZE * screen_right_hud_scale(canvas_h))
        .round()
        .max(1.0) as u32
}

fn resolve_legacy_song_progress_layout(
    canvas_w: u32,
    canvas_h: u32,
    default_x: i32,
    default_y: i32,
    default_size: u32,
    layout: Option<&crate::types::LegacyHudDrawableLayout>,
    content_scale: f32,
) -> (i32, i32, u32) {
    let Some(layout) = layout else {
        return (default_x, default_y, default_size.max(1));
    };
    let content_scale = content_scale.max(f32::EPSILON);
    let virtual_canvas_w = canvas_w as f32 / content_scale;
    let virtual_canvas_h = canvas_h as f32 / content_scale;
    let scale = if layout.scale_x.is_finite() && layout.scale_x > 0.0 {
        layout.scale_x
    } else if layout.scale_y.is_finite() && layout.scale_y > 0.0 {
        layout.scale_y
    } else {
        1.0
    };
    let virtual_size = (default_size as f32 / content_scale * scale).max(1.0);
    let Some((anchor_x, anchor_y)) =
        legacy_anchor_screen_point_f32(layout.anchor, virtual_canvas_w, virtual_canvas_h)
    else {
        return (default_x, default_y, default_size.max(1));
    };
    let origin_factor_x = legacy_alignment_factor(layout.origin, 'x').unwrap_or(0.5);
    let origin_factor_y = legacy_alignment_factor(layout.origin, 'y').unwrap_or(0.5);
    let origin_x = origin_factor_x * virtual_size;
    let origin_y = origin_factor_y * virtual_size;
    let x = ((anchor_x + layout.x - origin_x) * content_scale).round() as i32;
    let y = ((anchor_y + layout.y - origin_y) * content_scale).round() as i32;
    let size = (virtual_size * content_scale).round().max(1.0) as u32;
    (x, y, size)
}
#[derive(Debug, Clone)]
pub(super) struct LegacyAnimationSpec {
    pub frames: Vec<String>,
    pub frame_length_ms: f64,
}
#[derive(Debug, Clone)]
pub(super) struct GameplayAnimationSpec {
    pub frames: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GameplayAnimationKind {
    Note,
    LongNoteBody,
    StageBottom,
    StageLight,
    LightingN,
    LightingL,
}
pub struct ReplayRenderer {
    pub(super) cfg: RendererConfig,
    pub(super) gpu_ctx: Option<Arc<GpuContext>>,
    pub(super) pipeline: Option<SpritePipeline>,
    pub(super) initialized: bool,
    pub(super) textures_loaded: bool,
    pub(super) loaded_textures: HashSet<String>,
    pub(super) texture_meta: HashMap<String, TextureMeta>,
    pub(super) texture_opaque_bounds: HashMap<String, OpaqueBounds>,
    pub(super) gpu_textures: HashMap<String, Arc<wgpu::Texture>>,
    pub(super) ln_body_scaled: HashSet<String>,
    pub(super) linear_sampled_textures: HashSet<String>,
    pub(super) sprite_scaled: HashMap<String, String>,
    pub(super) legacy_animation_cache: HashMap<String, Option<LegacyAnimationSpec>>,
    pub(super) gameplay_animation_cache: HashMap<String, Option<GameplayAnimationSpec>>,
    pub(super) gameplay_visibility_cache: HashMap<String, bool>,
    pub(super) hud_digit_cache: HudDigitHeightCache,
    pub(super) hud_text_cache: HashMap<String, String>,
    pub(super) hud_asset_cache: HashMap<String, Vec<HudAssetFrame>>,
    pub(super) hud_font_cache: HashMap<String, Option<FontArc>>,
    pub(super) hud_font_warning_cache: HashSet<String>,
    pub(super) hud_judgment_counter_animations: HashMap<String, HudJudgmentCounterAnimation>,
    pub(super) hud_key_last_mask: u32,
    pub(super) hud_key_press_times: VecDeque<(i32, u8)>,
    pub(super) hud_key_down_since: [Option<i32>; 32],
    pub(super) hud_key_tail_releases: VecDeque<HudKeyTailRelease>,
    pub(super) hud_kps_samples: VecDeque<(i32, f32)>,
    pub(super) hud_last_kps_sample_time: Option<i32>,
    pub(super) hud_pp_timeline: Vec<(i32, f32)>,
    pub(super) hud_pp_final: Option<f32>,
    pub(super) hud_pp_warning: Option<String>,
    pub(super) hud_unstable_rate: Option<f32>,
    pub(super) hud_beatmap_metadata: HudBeatmapMetadataState,
    pub(super) lead_in_ms: i32,
    pub(super) first_note_time_ms: Option<i32>,
    pub(super) random_seed: u64,
    pub(super) scroll_model: Option<StableScrollModel>,
    pub(super) scroll_playback_clock: Option<PlaybackClock>,
    pub(super) hud_enabled: bool,
    pub(super) editor_preview_base_only: bool,
    pub(super) lighting_enabled: bool,
    pub(super) barlines_enabled: bool,
    pub(super) sv_enabled: bool,
    pub(super) skin_animations_enabled: bool,
    pub(super) prefer_raw: bool,
    pub(super) ln_debug: bool,
    pub(super) stage_opaque_bg: bool,
    pub(super) storyboard: Option<StoryboardPlayer>,
    pub(super) storyboard_enabled: bool,
    pub(super) background_texture_id: Option<String>,
    pub(super) background_visible: f32,
    pub(super) _hud_log_printed: bool,
    pub(super) ln_bodies_prescaled: bool,
    pub(super) _key_debug_logged: bool,
    pub(super) life_bar_warned: bool,
    pub(super) playfield_cover_runtime: PlayfieldCoverRuntime,
    pub(super) combo_break_red_cache: HashMap<String, String>,
    pub(super) combo_break_textures_created: bool,
    pub(super) hud_config: Option<HudConfig>,
    pub(super) replay_mod_display: Option<ReplayModDisplay>,
    pub(super) progress_cb: Option<Box<dyn Fn(usize, usize) + Send + Sync>>,
    pub(super) debug_text_cache: DebugTextCache,
    pub(super) fallback_frame: Vec<u8>,
}
impl ReplayRenderer {
    pub fn new() -> Self {
        Self {
            cfg: RendererConfig::default(),
            gpu_ctx: None,
            pipeline: None,
            initialized: false,
            textures_loaded: false,
            loaded_textures: HashSet::new(),
            texture_meta: HashMap::new(),
            texture_opaque_bounds: HashMap::new(),
            gpu_textures: HashMap::new(),
            ln_body_scaled: HashSet::new(),
            linear_sampled_textures: HashSet::new(),
            sprite_scaled: HashMap::new(),
            legacy_animation_cache: HashMap::new(),
            gameplay_animation_cache: HashMap::new(),
            gameplay_visibility_cache: HashMap::new(),
            hud_digit_cache: HudDigitHeightCache::default(),
            hud_text_cache: HashMap::new(),
            hud_asset_cache: HashMap::new(),
            hud_font_cache: HashMap::new(),
            hud_font_warning_cache: HashSet::new(),
            hud_judgment_counter_animations: HashMap::new(),
            hud_key_last_mask: 0,
            hud_key_press_times: VecDeque::new(),
            hud_key_down_since: [None; 32],
            hud_key_tail_releases: VecDeque::new(),
            hud_kps_samples: VecDeque::new(),
            hud_last_kps_sample_time: None,
            hud_pp_timeline: Vec::new(),
            hud_pp_final: None,
            hud_pp_warning: None,
            hud_unstable_rate: None,
            hud_beatmap_metadata: HudBeatmapMetadataState::default(),
            lead_in_ms: 0,
            first_note_time_ms: None,
            random_seed: 0,
            scroll_model: None,
            scroll_playback_clock: None,
            hud_enabled: true,
            editor_preview_base_only: false,
            lighting_enabled: false,
            barlines_enabled: false,
            sv_enabled: true,
            skin_animations_enabled: true,
            prefer_raw: false,
            ln_debug: false,
            stage_opaque_bg: true,
            storyboard: None,
            storyboard_enabled: true,
            background_texture_id: None,
            background_visible: 1.0,
            _hud_log_printed: false,
            ln_bodies_prescaled: false,
            _key_debug_logged: false,
            life_bar_warned: false,
            playfield_cover_runtime: PlayfieldCoverRuntime::default(),
            combo_break_red_cache: HashMap::new(),
            combo_break_textures_created: false,
            hud_config: None,
            replay_mod_display: None,
            progress_cb: None,
            debug_text_cache: DebugTextCache::new(),
            fallback_frame: Vec::new(),
        }
    }
    #[inline]
    pub fn set_canvas_size(&mut self, w: u32, h: u32) {
        self.cfg.width = w;
        self.cfg.height = h;
    }
    #[inline]
    pub fn set_fps(&mut self, fps: u32) {
        self.cfg.fps = fps.max(1);
    }
    #[inline]
    pub fn set_scroll_speed(&mut self, ss: f32) {
        self.cfg.scroll_speed = ss;
    }
    #[inline]
    pub fn set_lead_in_ms(&mut self, ms: i32) {
        self.lead_in_ms = ms;
    }
    #[inline]
    pub fn set_first_note_time_ms(&mut self, time: Option<i32>) {
        self.first_note_time_ms = time;
    }
    fn legacy_accuracy_default_layout_mode(&self) -> LegacyAccuracyDefaultLayoutMode {
        // Keep the default accuracy block screen-right for HUD exports that predate stage-relative placement.
        LegacyAccuracyDefaultLayoutMode::ScreenRight
    }
    pub fn measure_hud_editor_preview_components(
        &self,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        hud_state: &HudFrameState,
    ) -> Vec<(&'static str, HudEditorPreviewRect)> {
        let mut components = Vec::new();
        components.push((
            "columns",
            HudEditorPreviewRect {
                x: layout.stage.x,
                y: layout.stage.top_y,
                width: layout.stage.width,
                height: layout.stage.height,
            },
        ));
        if let Some(stage_left) =
            skin.config.stage_left.as_ref().filter(|name| {
                self.has_texture(name) && self.texture_opaque_bounds.contains_key(*name)
            })
        {
            if let Some(meta) = self.texture_meta.get(stage_left) {
                let scale = layout.stage.height as f32 / meta.h.max(1) as f32;
                let width = (meta.w as f32 * scale).round().max(1.0) as u32;
                components.push((
                    "stageLeft",
                    HudEditorPreviewRect {
                        x: layout.stage.x - width as i32,
                        y: layout.stage.top_y,
                        width,
                        height: layout.stage.height,
                    },
                ));
            }
        }
        if let Some(stage_right) =
            skin.config.stage_right.as_ref().filter(|name| {
                self.has_texture(name) && self.texture_opaque_bounds.contains_key(*name)
            })
        {
            if let Some(meta) = self.texture_meta.get(stage_right) {
                let scale = layout.stage.height as f32 / meta.h.max(1) as f32;
                let width = (meta.w as f32 * scale).round().max(1.0) as u32;
                components.push((
                    "stageRight",
                    HudEditorPreviewRect {
                        x: layout.stage.x + layout.stage.width as i32,
                        y: layout.stage.top_y,
                        width,
                        height: layout.stage.height,
                    },
                ));
            }
        }
        if let Some(stage_bottom) =
            skin.config.stage_bottom.as_ref().filter(|name| {
                self.has_texture(name) && self.texture_opaque_bounds.contains_key(*name)
            })
        {
            if let Some(meta) = self.texture_meta.get(stage_bottom) {
                let original_w = meta.w as f32;
                let original_h = meta.h.max(1) as f32;
                let scaled_h = (original_h * layout.scale_y).round().max(1.0) as u32;
                let scaled_w = (scaled_h as f32 * (original_w / original_h))
                    .round()
                    .max(1.0) as u32;
                let stage_center_x = layout.stage.x + layout.stage.width as i32 / 2;
                components.push((
                    "stageBottom",
                    HudEditorPreviewRect {
                        x: stage_center_x - scaled_w as i32 / 2,
                        y: if layout.upside_down {
                            layout.stage.top_y
                        } else {
                            layout.stage.bottom_y - scaled_h as i32
                        },
                        width: scaled_w,
                        height: scaled_h,
                    },
                ));
            }
        }
        if let Some(stage_hint) = skin
            .config
            .stage_hint
            .as_deref()
            .filter(|name| self.has_texture(name) && self.texture_opaque_bounds.contains_key(*name))
            .or_else(|| {
                ["mania-stage-hint.png", "mania-stage-hint"]
                    .into_iter()
                    .find(|name| {
                        self.has_texture(name) && self.texture_opaque_bounds.contains_key(*name)
                    })
            })
        {
            if let Some(meta) = self.texture_meta.get(stage_hint) {
                let width = layout.stage.width.max(1);
                let height = (legacy_editor_texture_display_height(stage_hint, meta.h, layout)
                    * LEGACY_STAGE_HINT_Y_SCALE)
                    .round()
                    .max(1.0) as u32;
                components.push((
                    "stageHint",
                    HudEditorPreviewRect {
                        x: layout.stage.x,
                        y: layout.stage.hit_y - (height as i32 / 2),
                        width,
                        height,
                    },
                ));
            }
        }
        if let Some(rect) = self.measure_life_bar_editor_preview_rect(layout) {
            components.push(("lifeBar", rect));
        }
        let canvas_w = self.cfg.width;
        let canvas_h = self.cfg.height;
        let scale_y = layout.scale_y;
        let hud_elements = self.hud_config.as_ref().map(|cfg| &cfg.elements);
        let legacy_hud_layout = skin.legacy_hud_layout.as_ref();
        let default_screen_right_hud_scale = screen_right_hud_scale(canvas_h);
        let raw_score_layout = self.measure_score_raw_layout(skin, hud_state.score as u64);
        let mut score_scale_x = LEGACY_SCORE_LAYOUT_BASE_SCALE * default_screen_right_hud_scale;
        let mut score_scale_y = LEGACY_SCORE_LAYOUT_BASE_SCALE * default_screen_right_hud_scale;
        let mut score_right =
            legacy_component_right_edge(canvas_w, LEGACY_SCORE_MARGIN_X, score_scale_x);
        let mut score_top = 0.0f32;
        let score_cfg = hud_elements.and_then(|el| el.score.as_ref());
        let score_visible = score_cfg.and_then(|cfg| cfg.visible).unwrap_or(true);
        if let Some(cfg) = score_cfg {
            (score_scale_x, score_scale_y) = resolve_legacy_text_scale_from_hud_config(
                cfg,
                raw_score_layout.width,
                raw_score_layout.height,
                LEGACY_SCORE_LAYOUT_BASE_SCALE,
            );
            score_right =
                legacy_component_right_edge(canvas_w, LEGACY_SCORE_MARGIN_X, score_scale_x);
            if let Some(x) = cfg.x.filter(|v| v.is_finite()) {
                score_right = x;
            }
            if let Some(y) = cfg.y.filter(|v| v.is_finite()) {
                score_top = y;
            }
        } else if legacy_hud_layout.is_some() {
            let score_transform = resolve_legacy_text_component_transform(
                canvas_w,
                canvas_h,
                raw_score_layout.width,
                raw_score_layout.height,
                legacy_hud_layout.and_then(|layout| layout.score.as_ref()),
                LEGACY_SCORE_LAYOUT_BASE_SCALE,
                default_screen_right_hud_scale,
                33,
                33,
                0.0,
                0.0,
                LEGACY_SCORE_MARGIN_X,
                0.0,
            );
            score_right = score_transform.right_x;
            score_top = score_transform.top_y;
            score_scale_x = score_transform.scale_x;
            score_scale_y = score_transform.scale_y;
        }
        let score_screen_layout = self.measure_legacy_screen_layout(
            raw_score_layout,
            score_scale_x,
            score_scale_y,
            score_right,
            score_top,
        );
        components.push((
            "score",
            HudEditorPreviewRect {
                x: score_screen_layout.left.round() as i32,
                y: score_screen_layout.top.round() as i32,
                width: score_screen_layout.width.round().max(1.0) as u32,
                height: score_screen_layout.height.round().max(1.0) as u32,
            },
        ));
        let raw_accuracy_layout = self.measure_accuracy_raw_layout(skin, hud_state.accuracy);
        let accuracy_default_layout_mode = self.legacy_accuracy_default_layout_mode();
        let default_accuracy_scale = match accuracy_default_layout_mode {
            LegacyAccuracyDefaultLayoutMode::ScreenRight => {
                LEGACY_ACCURACY_LAYOUT_BASE_SCALE * default_screen_right_hud_scale
            }
            LegacyAccuracyDefaultLayoutMode::StageRelative => {
                LEGACY_ACCURACY_LAYOUT_BASE_SCALE * stage_relative_hud_scale(layout)
            }
        };
        let default_progress_size = match accuracy_default_layout_mode {
            LegacyAccuracyDefaultLayoutMode::ScreenRight => {
                legacy_song_progress_layout_size(canvas_w, canvas_h) as f32
            }
            LegacyAccuracyDefaultLayoutMode::StageRelative => {
                LEGACY_SONG_PROGRESS_LAYOUT_BASE_SIZE * stage_relative_hud_scale(layout)
            }
        };
        let mut accuracy_scale_x = default_accuracy_scale;
        let mut accuracy_scale_y = default_accuracy_scale;
        let mut accuracy_top = match accuracy_default_layout_mode {
            LegacyAccuracyDefaultLayoutMode::ScreenRight => {
                score_screen_layout.bottom()
                    + legacy_scaled_hud_px(LEGACY_ACCURACY_MARGIN_Y, accuracy_scale_y)
            }
            LegacyAccuracyDefaultLayoutMode::StageRelative => stage_relative_accuracy_top(layout),
        };
        let mut accuracy_right = match accuracy_default_layout_mode {
            LegacyAccuracyDefaultLayoutMode::ScreenRight => {
                legacy_component_right_edge(canvas_w, LEGACY_ACCURACY_MARGIN_X, accuracy_scale_x)
            }
            LegacyAccuracyDefaultLayoutMode::StageRelative => {
                stage_relative_accuracy_right(layout, raw_accuracy_layout.width * accuracy_scale_x)
            }
        };
        let accuracy_cfg = hud_elements.and_then(|el| el.accuracy.as_ref());
        let accuracy_visible = accuracy_cfg.and_then(|cfg| cfg.visible).unwrap_or(true);
        if let Some(cfg) = accuracy_cfg {
            (accuracy_scale_x, accuracy_scale_y) = resolve_legacy_text_scale_from_hud_config(
                cfg,
                raw_accuracy_layout.width,
                raw_accuracy_layout.height,
                default_accuracy_scale,
            );
            accuracy_top = match accuracy_default_layout_mode {
                LegacyAccuracyDefaultLayoutMode::ScreenRight => {
                    score_screen_layout.bottom()
                        + legacy_scaled_hud_px(LEGACY_ACCURACY_MARGIN_Y, accuracy_scale_y)
                }
                LegacyAccuracyDefaultLayoutMode::StageRelative => {
                    stage_relative_accuracy_top(layout)
                }
            };
            accuracy_right = match accuracy_default_layout_mode {
                LegacyAccuracyDefaultLayoutMode::ScreenRight => legacy_component_right_edge(
                    canvas_w,
                    LEGACY_ACCURACY_MARGIN_X,
                    accuracy_scale_x,
                ),
                LegacyAccuracyDefaultLayoutMode::StageRelative => stage_relative_accuracy_right(
                    layout,
                    raw_accuracy_layout.width * accuracy_scale_x,
                ),
            };
            if let Some(x) = cfg.x.filter(|v| v.is_finite()) {
                accuracy_right = x;
            }
            if let Some(y) = cfg.y.filter(|v| v.is_finite()) {
                accuracy_top = y;
            }
        } else if legacy_hud_layout.is_some() {
            let accuracy_transform = resolve_legacy_text_component_transform(
                canvas_w,
                canvas_h,
                raw_accuracy_layout.width,
                raw_accuracy_layout.height,
                legacy_hud_layout.and_then(|layout| layout.accuracy.as_ref()),
                LEGACY_ACCURACY_LAYOUT_BASE_SCALE,
                default_screen_right_hud_scale,
                33,
                33,
                0.0,
                score_screen_layout.bottom(),
                LEGACY_ACCURACY_MARGIN_X,
                LEGACY_ACCURACY_MARGIN_Y,
            );
            accuracy_right = accuracy_transform.right_x;
            accuracy_top = accuracy_transform.top_y;
            accuracy_scale_x = accuracy_transform.scale_x;
            accuracy_scale_y = accuracy_transform.scale_y;
        }
        let accuracy_screen_layout = self.measure_legacy_screen_layout(
            raw_accuracy_layout,
            accuracy_scale_x,
            accuracy_scale_y,
            accuracy_right,
            accuracy_top,
        );
        components.push((
            "accuracy",
            HudEditorPreviewRect {
                x: accuracy_screen_layout.left.round() as i32,
                y: accuracy_screen_layout.top.round() as i32,
                width: accuracy_screen_layout.width.round().max(1.0) as u32,
                height: accuracy_screen_layout.height.round().max(1.0) as u32,
            },
        ));
        let circle_left = match accuracy_default_layout_mode {
            LegacyAccuracyDefaultLayoutMode::ScreenRight => legacy_song_progress_default_left(
                canvas_w,
                accuracy_screen_layout.width,
                legacy_scaled_hud_px(LEGACY_SONG_PROGRESS_GAP_X, default_screen_right_hud_scale),
                default_progress_size,
            ),
            LegacyAccuracyDefaultLayoutMode::StageRelative => stage_relative_song_progress_left(
                layout,
                accuracy_screen_layout.left,
                default_progress_size,
            ),
        };
        let circle_top = accuracy_screen_layout.top + accuracy_screen_layout.height / 2.0
            - default_progress_size / 2.0;
        let progress_cfg = hud_elements.and_then(|el| el.progress_circle.as_ref());
        let mut progress_x = circle_left;
        let mut progress_y = circle_top;
        let mut progress_size = default_progress_size;
        if progress_cfg.is_none() && legacy_hud_layout.is_some() {
            let (x, y, size) = resolve_legacy_song_progress_layout(
                canvas_w,
                canvas_h,
                circle_left.round() as i32,
                circle_top.round() as i32,
                legacy_song_progress_layout_size(canvas_w, canvas_h),
                legacy_hud_layout.and_then(|layout| layout.song_progress.as_ref()),
                default_screen_right_hud_scale,
            );
            progress_x = x as f32;
            progress_y = y as f32;
            progress_size = size as f32;
        }
        if let Some(cfg) = progress_cfg {
            if let Some(x) = cfg.x.filter(|v| v.is_finite()) {
                progress_x = x;
            }
            if let Some(y) = cfg.y.filter(|v| v.is_finite()) {
                progress_y = y;
            }
            if let Some(size) = cfg.size.filter(|v| v.is_finite() && *v > 0.0) {
                progress_size = size;
            } else if let Some(width) = cfg.width.filter(|v| v.is_finite() && *v > 0.0) {
                progress_size = width;
            } else if let Some(height) = cfg.height.filter(|v| v.is_finite() && *v > 0.0) {
                progress_size = height;
            }
            if let Some(scale) = cfg.scale.filter(|v| v.is_finite() && *v > 0.0) {
                progress_size = (progress_size * scale).max(1.0);
            }
        }
        components.push((
            "progressCircle",
            HudEditorPreviewRect {
                x: progress_x.round() as i32,
                y: progress_y.round() as i32,
                width: progress_size.round().max(1.0) as u32,
                height: progress_size.round().max(1.0) as u32,
            },
        ));
        let mods_cfg = hud_elements.and_then(|el| el.mods.as_ref());
        let mods_visible = mods_cfg.and_then(|cfg| cfg.visible).unwrap_or(true);
        if mods_visible {
            let mods_top = if accuracy_visible {
                accuracy_screen_layout.bottom()
                    + legacy_scaled_hud_px(
                        REPLAY_MOD_DEFAULT_MARGIN_TOP,
                        default_screen_right_hud_scale,
                    )
            } else if score_visible {
                score_screen_layout.bottom()
                    + legacy_scaled_hud_px(
                        REPLAY_MOD_DEFAULT_MARGIN_TOP,
                        default_screen_right_hud_scale,
                    )
            } else {
                legacy_scaled_hud_px(
                    REPLAY_MOD_DEFAULT_MARGIN_TOP,
                    default_screen_right_hud_scale,
                )
            };
            let mods_right = canvas_w as f32
                - legacy_scaled_hud_px(
                    REPLAY_MOD_DEFAULT_MARGIN_RIGHT,
                    default_screen_right_hud_scale,
                );
            if let Some(layout) =
                self.measure_replay_mod_display_layout(mods_right, mods_top, mods_cfg)
            {
                components.push((
                    "mods",
                    HudEditorPreviewRect {
                        x: layout.left.round() as i32,
                        y: layout.top.round() as i32,
                        width: layout.width.round().max(1.0) as u32,
                        height: layout.height.round().max(1.0) as u32,
                    },
                ));
            }
        }
        let hit_error_meter_cfg = hud_elements.and_then(|el| el.hit_error_meter.as_ref());
        if hit_error_meter_cfg.and_then(|cfg| cfg.visible) != Some(false) {
            let canvas_w = canvas_w.max(1) as f32;
            let canvas_h = canvas_h.max(1) as f32;
            let meter_scale = (canvas_h / 720.0).max(0.5);
            let min_margin = 16.0 * meter_scale;
            let max_width = (canvas_w - min_margin * 2.0).max(96.0);
            let default_width = (canvas_w * 0.28)
                .clamp(230.0 * meter_scale, 420.0 * meter_scale)
                .min(max_width);
            let default_height = (22.0 * meter_scale).clamp(16.0, 32.0);
            let size_scale = hit_error_meter_cfg
                .and_then(|cfg| cfg.scale)
                .filter(|value| value.is_finite() && *value > 0.0)
                .unwrap_or(1.0);
            let width = (hit_error_meter_cfg
                .and_then(|cfg| cfg.width)
                .filter(|value| value.is_finite() && *value > 0.0)
                .unwrap_or(default_width)
                * size_scale)
                .min(max_width)
                .max(64.0);
            let height = (hit_error_meter_cfg
                .and_then(|cfg| cfg.height.or(cfg.size))
                .filter(|value| value.is_finite() && *value > 0.0)
                .unwrap_or(default_height)
                * size_scale)
                .min((canvas_h * 0.18).max(16.0))
                .max(10.0);
            let x = hit_error_meter_cfg
                .and_then(|cfg| cfg.x)
                .filter(|value| value.is_finite())
                .unwrap_or((canvas_w - width) * 0.5);
            let bottom_margin = (24.0 * meter_scale).clamp(16.0, 44.0);
            let y = hit_error_meter_cfg
                .and_then(|cfg| cfg.y)
                .filter(|value| value.is_finite())
                .unwrap_or(canvas_h - height - bottom_margin);
            components.push((
                "hitErrorMeter",
                HudEditorPreviewRect {
                    x: x.round() as i32,
                    y: y.round() as i32,
                    width: width.round().max(1.0) as u32,
                    height: height.round().max(1.0) as u32,
                },
            ));
        }
        let native_combo_h = self.hud_digit_cache.combo.unwrap_or(60);
        let combo_default_height = (native_combo_h as f32 * scale_y * LEGACY_COMBO_LAYOUT_SCALE)
            .round()
            .max(1.0) as u32;
        let combo_height = self.legacy_combo_height_for_current_hud_config(combo_default_height);
        let center_x = layout.stage.x + layout.stage.width as i32 / 2;
        let mut combo_center_x = center_x;
        let combo_y = if let Some(combo_pos) = skin.config.combo_pos_y {
            (combo_pos as f32 * scale_y).round() as i32
        } else {
            (layout.stage.hit_y as f32 - 96.0 * scale_y).round() as i32
        };
        let mut combo_pos_y = combo_y;
        let combo_cfg = hud_elements.and_then(|el| el.combo.as_ref());
        if let Some(cfg) = combo_cfg {
            if let Some(x) = cfg.x.filter(|v| v.is_finite()) {
                combo_center_x = x.round() as i32;
            }
            if let Some(y) = cfg.y.filter(|v| v.is_finite()) {
                combo_pos_y = y.round() as i32;
            }
        }
        let stretch = hud_state
            .combo_inc_anim
            .map(|a| Self::calc_combo_stretch(a.age_ms))
            .unwrap_or(1.0);
        if let Some(layout) = self.measure_combo_layout(
            skin,
            hud_state.combo,
            combo_height,
            combo_center_x,
            combo_pos_y,
            stretch,
        ) {
            components.push((
                "combo",
                HudEditorPreviewRect {
                    x: layout.left,
                    y: layout.top,
                    width: layout.width,
                    height: layout.height,
                },
            ));
        }
        if let Some(last) = &hud_state.last_judgment {
            let anim_fps = skin.config.animation_framerate.unwrap_or(60);
            let judgment_cfg = hud_elements.and_then(|el| el.judgment_pop.as_ref());
            let mut judgment_center_x = center_x;
            let mut judgment_center_y = None;
            let mut judgment_scale = 1.0f32;
            let mut judgment_target_height = None;
            if let Some(cfg) = judgment_cfg {
                if let Some(x) = cfg.x.filter(|v| v.is_finite()) {
                    judgment_center_x = x.round() as i32;
                }
                if let Some(y) = cfg.y.filter(|v| v.is_finite()) {
                    judgment_center_y = Some(y.round() as i32);
                }
                if let Some(height) = cfg.height.filter(|v| v.is_finite() && *v > 0.0) {
                    judgment_target_height = Some(height.round().max(1.0) as u32);
                }
                if let Some(scale) = cfg.scale.or(cfg.size).filter(|v| v.is_finite() && *v > 0.0) {
                    judgment_scale = scale;
                }
            }
            if let Some(layout) = self.measure_judgment_popup_layout(
                skin,
                last.kind,
                last.age_ms,
                judgment_center_x,
                layout.stage.hit_y,
                scale_y,
                anim_fps,
                judgment_center_y,
                judgment_scale,
                judgment_target_height,
            ) {
                components.push((
                    "judgmentPop",
                    HudEditorPreviewRect {
                        x: layout.left,
                        y: layout.top,
                        width: layout.width,
                        height: layout.height,
                    },
                ));
            }
        }
        components
    }
    #[inline]
    pub fn set_random_seed(&mut self, seed: u64) {
        self.random_seed = seed;
    }
    #[inline]
    pub fn set_stage_opaque_bg(&mut self, v: bool) {
        self.stage_opaque_bg = v;
    }
    #[inline]
    pub fn set_ln_debug(&mut self, v: bool) {
        self.ln_debug = v;
    }
    #[inline]
    pub fn set_hud_enabled(&mut self, v: bool) {
        self.hud_enabled = v;
    }
    #[inline]
    pub fn set_editor_preview_base_only(&mut self, v: bool) {
        self.editor_preview_base_only = v;
    }
    #[inline]
    pub fn set_lighting_enabled(&mut self, v: bool) {
        self.lighting_enabled = v;
    }
    #[inline]
    pub fn set_barlines_enabled(&mut self, v: bool) {
        self.barlines_enabled = v;
    }
    #[inline]
    pub fn set_sv_enabled(&mut self, v: bool) {
        self.sv_enabled = v;
    }
    #[inline]
    pub fn set_skin_animations_enabled(&mut self, v: bool) {
        self.skin_animations_enabled = v;
    }
    #[inline]
    pub fn set_scroll_timeline(&mut self, timeline: Option<StableScrollModel>) {
        self.scroll_model = timeline;
    }
    #[inline]
    pub fn set_scroll_playback_clock(&mut self, clock: Option<PlaybackClock>) {
        self.scroll_playback_clock = clock;
    }
    #[inline]
    pub fn set_prefer_raw(&mut self, v: bool) {
        self.prefer_raw = v;
    }
    #[inline]
    pub fn set_hud_config(&mut self, cfg: Option<HudConfig>) {
        self.hud_config = cfg;
    }
    fn hud_column_opacity_overrides(&self, count: usize) -> Option<Vec<f32>> {
        let columns = self.hud_config.as_ref()?.elements.columns.as_ref()?;
        let mut has_override = false;
        let global = columns
            .opacity
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(0.0, 1.0));
        let mut values = vec![global.unwrap_or(1.0); count];
        if global.is_some() {
            has_override = true;
        }
        for item in &columns.items {
            let Some(index) = item.index.filter(|index| *index < count) else {
                continue;
            };
            let Some(opacity) = item.opacity.filter(|value| value.is_finite()) else {
                continue;
            };
            values[index] = opacity.clamp(0.0, 1.0);
            has_override = true;
        }
        has_override.then_some(values)
    }
    #[inline]
    pub fn set_replay_mod_display(&mut self, display: Option<ReplayModDisplay>) {
        self.replay_mod_display = display.filter(|display| !display.is_empty());
    }
    #[inline]
    pub fn set_storyboard_enabled(&mut self, v: bool) {
        self.storyboard_enabled = v;
    }
    #[inline]
    pub fn set_storyboard(&mut self, sb: Option<StoryboardPlayer>) {
        self.storyboard = sb;
    }
    pub fn set_background_image(
        &mut self,
        path: &Path,
        dim: f32,
        blur_percent: u8,
        offset_x: f32,
        offset_y: f32,
    ) -> Result<(), RendererError> {
        if !self.initialized || self.gpu_ctx.is_none() {
            return Err(RendererError::NotInitialized);
        }
        let safe_dim = if dim.is_finite() { dim } else { 1.0 };
        let visible = (1.0 - safe_dim).clamp(0.0, 1.0);
        if visible <= 0.0 {
            self.background_texture_id = None;
            self.background_visible = 0.0;
            return Ok(());
        }
        let data = std::fs::read(path)
            .map_err(|e| RendererError::TextureLoad(format!("bg read failed: {e}")))?;
        let mut img = crate::utils::image_proc::load_rgba(&data)
            .ok_or_else(|| RendererError::TextureLoad("bg decode failed".into()))?;
        let target_w = self.cfg.width.max(1);
        let target_h = self.cfg.height.max(1);
        img = crate::utils::image_proc::resize_cover_with_offset(
            &img, target_w, target_h, offset_x, offset_y,
        );
        if blur_percent > 0 {
            let sigma = crate::utils::image_proc::background_blur_sigma_from_percent(blur_percent);
            img = crate::utils::image_proc::apply_gaussian_blur(&img, sigma);
        }
        let tex_id = "bg_static".to_string();
        if !self.load_texture_rgba(&tex_id, img.as_raw(), target_w, target_h) {
            return Err(RendererError::TextureLoad("bg upload failed".into()));
        }
        self.background_texture_id = Some(tex_id);
        self.background_visible = visible;
        Ok(())
    }
    pub fn set_progress_callback<F>(&mut self, cb: F)
    where
        F: Fn(usize, usize) + Send + Sync + 'static,
    {
        self.progress_cb = Some(Box::new(cb));
    }
    pub fn compact_runtime_memory(&mut self) {
        if let Some(pipeline) = self.pipeline.as_mut() {
            pipeline.prune_bind_group_cache(RUNTIME_BIND_GROUP_CACHE_MAX);
        }
        if self.ln_debug {
            self.debug_text_cache.compact(RUNTIME_DEBUG_TEXT_CACHE_MAX);
        }
    }
    pub fn runtime_memory_stats(&self) -> (usize, usize) {
        let bind_group_cache = self
            .pipeline
            .as_ref()
            .map(|p| p.cache_stats().0)
            .unwrap_or(0);
        (bind_group_cache, self.gpu_textures.len())
    }
    #[inline]
    pub fn fps(&self) -> u32 {
        self.cfg.fps
    }
    #[inline]
    pub fn size(&self) -> (u32, u32) {
        (self.cfg.width, self.cfg.height)
    }
    #[inline]
    pub fn config(&self) -> RendererConfig {
        self.cfg
    }
    pub fn renderer_options(&self) -> RendererConfig {
        self.cfg
    }
    pub(super) fn prefer_hidpi_texture_id(&self, texture_id: &str) -> String {
        let normalized = texture_id.to_ascii_lowercase();
        let ext_pos = normalized.rfind('.');
        let hidpi = if let Some(ext_pos) = ext_pos {
            format!("{}@2x{}", &normalized[..ext_pos], &normalized[ext_pos..])
        } else {
            format!("{normalized}@2x")
        };
        if self.loaded_textures.contains(&hidpi) {
            hidpi
        } else {
            normalized
        }
    }
    pub(super) fn gameplay_animation_base_name(texture_name: &str) -> String {
        let normalized = crate::types::SkinAssets::normalize_key(texture_name);
        let without_ext = normalized
            .trim_end_matches(".png")
            .trim_end_matches(".jpg")
            .trim_end_matches(".jpeg");
        // Skin animation frames are stored as name-0.png, name-1.png, and so on.
        if let Some((prefix, suffix)) = without_ext.rsplit_once('-') {
            if suffix.chars().all(|ch| ch.is_ascii_digit()) {
                return prefix.to_string();
            }
        }
        without_ext.to_string()
    }
    pub(super) fn gameplay_animation_spec(
        &mut self,
        texture_name: &str,
    ) -> Option<&GameplayAnimationSpec> {
        let base_name = Self::gameplay_animation_base_name(texture_name);
        if !self.gameplay_animation_cache.contains_key(&base_name) {
            let spec = self.detect_gameplay_animation_spec(&base_name);
            self.gameplay_animation_cache
                .insert(base_name.clone(), spec);
        }
        self.gameplay_animation_cache
            .get(&base_name)
            .and_then(Option::as_ref)
    }
    pub(super) fn gameplay_anchor_texture(&mut self, texture_name: &str) -> String {
        self.gameplay_animation_spec(texture_name)
            .and_then(|spec| spec.frames.first().cloned())
            .unwrap_or_else(|| crate::types::SkinAssets::normalize_key(texture_name))
    }
    pub(super) fn gameplay_frame_texture(
        &mut self,
        texture_name: &str,
        kind: GameplayAnimationKind,
        animation_time_ms: f64,
        local_start_time_ms: Option<i32>,
        local_end_time_ms: Option<i32>,
        frame_length_override_ms: Option<f64>,
    ) -> String {
        let fallback = self.prefer_hidpi_texture_id(texture_name);
        if !self.skin_animations_enabled {
            return fallback;
        }
        let Some(spec) = self.gameplay_animation_spec(texture_name).cloned() else {
            return fallback;
        };
        let frame_length_ms = frame_length_override_ms
            .unwrap_or_else(|| gameplay_animation_frame_length_ms(kind, spec.frames.len()));
        if frame_length_ms <= 0.0 {
            return fallback;
        }
        let start_time_ms = local_start_time_ms.map(f64::from).unwrap_or(0.0);
        let end_time_ms = local_end_time_ms.map(f64::from);
        let local_time_ms = (animation_time_ms - start_time_ms).max(0.0);
        // Each gameplay asset family follows a different osu! skin animation rule.
        let frame_index = match kind {
            GameplayAnimationKind::StageLight | GameplayAnimationKind::LightingL => {
                gameplay_loop_frame_index(local_time_ms, frame_length_ms, spec.frames.len())
            }
            GameplayAnimationKind::LightingN => {
                let total_duration_ms =
                    gameplay_lighting_n_duration_ms(frame_length_ms, spec.frames.len());
                if local_time_ms >= total_duration_ms {
                    spec.frames.len().saturating_sub(1)
                } else {
                    gameplay_clamped_frame_index(local_time_ms, frame_length_ms, spec.frames.len())
                }
            }
            GameplayAnimationKind::LongNoteBody => {
                if let Some(end_time_ms) = end_time_ms {
                    if animation_time_ms >= end_time_ms {
                        spec.frames.len().saturating_sub(1)
                    } else {
                        gameplay_loop_frame_index(local_time_ms, frame_length_ms, spec.frames.len())
                    }
                } else {
                    gameplay_loop_frame_index(local_time_ms, frame_length_ms, spec.frames.len())
                }
            }
            GameplayAnimationKind::Note | GameplayAnimationKind::StageBottom => {
                gameplay_loop_frame_index(
                    animation_time_ms.max(0.0),
                    frame_length_ms,
                    spec.frames.len(),
                )
            }
        };
        spec.frames
            .get(frame_index)
            .map(|frame| self.prefer_hidpi_texture_id(frame))
            .unwrap_or(fallback)
    }
    fn detect_gameplay_animation_spec(&self, base_name: &str) -> Option<GameplayAnimationSpec> {
        let mut frames = self.collect_gameplay_animation_frames(base_name, 0);
        if frames.len() <= 1 {
            frames = self.collect_gameplay_animation_frames(base_name, 1);
        }
        (frames.len() > 1).then_some(GameplayAnimationSpec { frames })
    }
    fn collect_gameplay_animation_frames(&self, base_name: &str, start_idx: usize) -> Vec<String> {
        let mut frames = Vec::new();
        for frame_idx in start_idx.. {
            let candidates = [
                format!("{base_name}-{frame_idx}.png"),
                format!("{base_name}-{frame_idx}.jpg"),
                format!("{base_name}-{frame_idx}.jpeg"),
                format!("{base_name}-{frame_idx}"),
            ];
            let Some(found) = candidates
                .iter()
                .find(|candidate| self.loaded_textures.contains(*candidate))
            else {
                break;
            };
            frames.push(found.clone());
        }
        frames
    }
    #[inline]
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
    pub async fn init_gpu(
        &mut self,
        preference: GpuPreference,
        adapter_hint: Option<&str>,
    ) -> Result<String, GpuError> {
        if self.initialized && self.gpu_ctx.is_some() {
            return Ok("already initialized".into());
        }
        let ctx = GpuContext::new(preference, adapter_hint).await?;
        let info = ctx.adapter_info();
        let info_str = format!("{} ({})", info.name, info.backend.to_str());
        let pipeline = SpritePipeline::new(&ctx, self.cfg.width, self.cfg.height);
        self.gpu_ctx = Some(Arc::new(ctx));
        self.pipeline = Some(pipeline);
        self.initialized = true;
        Ok(info_str)
    }
    pub fn resize(&mut self, w: u32, h: u32) {
        if self.cfg.width == w && self.cfg.height == h {
            return;
        }
        self.cfg.width = w;
        self.cfg.height = h;
        if let (Some(ctx), Some(ref mut pipeline)) = (&self.gpu_ctx, &mut self.pipeline) {
            pipeline.resize(ctx.as_ref(), w, h);
        }
    }
    #[inline]
    pub fn is_gpu_ready(&self) -> bool {
        self.initialized && self.textures_loaded && self.gpu_ctx.is_some()
    }
    pub fn gpu_context(&self) -> Option<&GpuContext> {
        self.gpu_ctx.as_ref().map(|arc| arc.as_ref())
    }
    fn build_render_batches(&self, planner: &SpritePlanner) -> Vec<RenderBatch> {
        let commands = planner.commands_ref();
        if commands.is_empty() {
            return Vec::new();
        }
        let mut sorted_commands: Vec<(usize, &RenderCommand)> =
            commands.iter().enumerate().collect();
        // Equal z layers keep planner order so storyboard, notes, and HUD elements remain stable.
        sorted_commands.sort_by_key(|(idx, c)| (c.z_order(), *idx));
        let mut batches: Vec<RenderBatch> = Vec::new();
        let mut current_texture_name: Option<String> = None;
        let mut current_sampling: TextureSampling = TextureSampling::Nearest;
        let mut current_blend_mode = SpriteBlendMode::Alpha;
        let mut current_instances: Vec<SpriteInstance> = Vec::new();
        let mut current_texture: Option<Arc<wgpu::Texture>> = None;
        for (_, cmd) in sorted_commands {
            match cmd {
                RenderCommand::Sprite(sprite) => {
                    let tex_name = &sprite.texture_id;
                    let sampling = self.texture_sampling_for_sprite(sprite);
                    let blend_mode = sprite.blend_mode;
                    let texture = match self.gpu_textures.get(tex_name) {
                        Some(t) => t.clone(),
                        None => continue,
                    };
                    if current_texture_name.as_ref() != Some(tex_name)
                        || current_sampling != sampling
                        || current_blend_mode != blend_mode
                    {
                        if let Some(tex) = current_texture.take() {
                            if !current_instances.is_empty() {
                                batches.push((
                                    tex,
                                    current_sampling,
                                    current_blend_mode,
                                    std::mem::take(&mut current_instances),
                                ));
                            }
                        }
                        current_texture_name = Some(tex_name.clone());
                        current_sampling = sampling;
                        current_blend_mode = blend_mode;
                        current_texture = Some(texture.clone());
                    }
                    let precise_position = sprite
                        .precise_position
                        .unwrap_or([sprite.x as f32, sprite.y as f32]);
                    let precise_size = sprite
                        .precise_size
                        .unwrap_or([sprite.width as f32, sprite.height as f32]);
                    let instance = SpriteInstance::new(
                        precise_position[0],
                        precise_position[1],
                        precise_size[0],
                        precise_size[1],
                    )
                    .with_uv(
                        sprite.uv_rect[0],
                        sprite.uv_rect[1],
                        sprite.uv_rect[2],
                        sprite.uv_rect[3],
                    )
                    .with_origin(sprite.origin[0], sprite.origin[1])
                    .with_rotation(sprite.rotation)
                    .with_color(
                        sprite.tint[0],
                        sprite.tint[1],
                        sprite.tint[2],
                        sprite.tint[3],
                    );
                    current_instances.push(instance);
                }
                RenderCommand::LnBody(ln_body) => {
                    // Long-note bodies are tiled vertically; clipped edge tiles use partial UVs.
                    let tex_name = &ln_body.texture_id;
                    let sampling = self.texture_sampling_for_texture(tex_name);
                    let texture = match self.gpu_textures.get(tex_name) {
                        Some(t) => t.clone(),
                        None => continue,
                    };
                    if current_texture_name.as_ref() != Some(tex_name)
                        || current_sampling != sampling
                        || current_blend_mode != SpriteBlendMode::Alpha
                    {
                        if let Some(tex) = current_texture.take() {
                            if !current_instances.is_empty() {
                                batches.push((
                                    tex,
                                    current_sampling,
                                    current_blend_mode,
                                    std::mem::take(&mut current_instances),
                                ));
                            }
                        }
                        current_texture_name = Some(tex_name.clone());
                        current_sampling = sampling;
                        current_blend_mode = SpriteBlendMode::Alpha;
                        current_texture = Some(texture.clone());
                    }
                    let body_height = ln_body.bottom_y - ln_body.top_y;
                    if body_height <= 0 {
                        continue;
                    }
                    let tile_h = ln_body.tile_h.max(1) as i32;
                    let mut y = ln_body.top_y - (ln_body.phase as i32 % tile_h);
                    while y < ln_body.bottom_y {
                        let draw_y = y.max(ln_body.top_y);
                        let draw_h = ((y + tile_h).min(ln_body.bottom_y) - draw_y).max(0) as u32;
                        if draw_h > 0 {
                            let v_start = if y < ln_body.top_y {
                                (ln_body.top_y - y) as f32 / tile_h as f32
                            } else {
                                0.0
                            };
                            let v_end = v_start + (draw_h as f32 / ln_body.tile_h as f32);
                            let instance = SpriteInstance::new(
                                ln_body.x as f32,
                                draw_y as f32,
                                ln_body.width as f32,
                                draw_h as f32,
                            )
                            .with_uv(0.0, v_start, 1.0, v_end.min(1.0))
                            .with_color(
                                ln_body.tint[0],
                                ln_body.tint[1],
                                ln_body.tint[2],
                                ln_body.tint[3],
                            );
                            current_instances.push(instance);
                        }
                        y += tile_h;
                    }
                }
            }
        }
        if let Some(tex) = current_texture.take() {
            if !current_instances.is_empty() {
                batches.push((tex, current_sampling, current_blend_mode, current_instances));
            }
        }
        batches
    }
    fn plan_storyboard_gameplay_layers(
        &self,
        before_notes: &mut SpritePlanner,
        after_notes: &mut SpritePlanner,
        timestamp: i32,
    ) {
        if !self.storyboard_enabled {
            return;
        }
        let Some(sb) = &self.storyboard else {
            return;
        };
        // osu! storyboard overlay is the only gameplay layer that renders after notes.
        sb.plan_layer(before_notes, timestamp, StoryboardLayer::Background);
        sb.plan_layer(before_notes, timestamp, StoryboardLayer::Fail);
        sb.plan_layer(before_notes, timestamp, StoryboardLayer::Pass);
        sb.plan_layer(before_notes, timestamp, StoryboardLayer::Foreground);
        sb.plan_layer(after_notes, timestamp, StoryboardLayer::Overlay);
    }
    pub(super) fn submit_render_gpu_layers(
        &mut self,
        before_notes: &SpritePlanner,
        notes_only: &SpritePlanner,
        after_notes: &SpritePlanner,
    ) -> bool {
        let ctx = match &self.gpu_ctx {
            Some(ctx) => ctx.clone(),
            None => return false,
        };
        if self.pipeline.is_none() {
            return false;
        }
        // Batch inside each render layer so GPU grouping never crosses the note ordering boundary.
        let before_batches = self.build_render_batches(before_notes);
        let note_batches = self.build_render_batches(notes_only);
        let after_batches = self.build_render_batches(after_notes);
        let clear_color = if self.stage_opaque_bg {
            [0.02, 0.02, 0.05, 1.0]
        } else {
            [0.0, 0.0, 0.0, 0.0]
        };
        let pipeline = self.pipeline.as_mut().expect("checked is_none");
        pipeline.submit_layered(
            &ctx,
            &before_batches,
            &note_batches,
            &after_batches,
            clear_color,
        );
        true
    }
    pub fn submit_results_backdrop_frame(&mut self, timestamp: i32) -> bool {
        if !self.initialized {
            return false;
        }
        let mut backdrop = SpritePlanner::new();
        let empty = SpritePlanner::new();
        self.plan_results_backdrop(&mut backdrop, timestamp);
        self.submit_render_gpu_layers(&backdrop, &empty, &empty)
    }
    pub fn render_frame_simple(
        &mut self,
        _time: i32,
        width: u32,
        height: u32,
        progress: f32,
    ) -> &[u8] {
        if !self.initialized {
            return self.fallback_frame(width, height);
        }
        let ctx = match &self.gpu_ctx {
            Some(ctx) => ctx.clone(),
            None => return self.fallback_frame(width, height),
        };
        if self.pipeline.is_none() {
            return self.fallback_frame(width, height);
        }
        let pipeline = self.pipeline.as_mut().expect("checked is_none");
        let base_r = 0.05;
        let base_g = 0.05;
        let base_b = 0.12;
        let pulse = (progress * std::f32::consts::PI * 2.0).sin() * 0.02;
        let clear_color = [
            (base_r + pulse).clamp(0.0, 1.0),
            (base_g + pulse * 0.5).clamp(0.0, 1.0),
            (base_b + pulse).clamp(0.0, 1.0),
            1.0,
        ];
        pipeline.draw_batched(&ctx, &[], clear_color)
    }
    pub fn poll_ready_frame(&mut self) -> Option<&[u8]> {
        let ctx = self.gpu_ctx.as_ref()?.clone();
        let pipeline = self.pipeline.as_mut()?;
        pipeline.poll_ready_frame(&ctx)
    }
    pub fn drain_ready_frame_blocking(&mut self) -> Option<&[u8]> {
        let ctx = self.gpu_ctx.as_ref()?.clone();
        let pipeline = self.pipeline.as_mut()?;
        pipeline.drain_ready_frame_blocking(&ctx)
    }
    pub fn dispose(&mut self) {
        self.pipeline = None;
        self.gpu_ctx = None;
        self.initialized = false;
        self.textures_loaded = false;
        self.loaded_textures.clear();
        self.texture_meta.clear();
        self.texture_opaque_bounds.clear();
        self.gpu_textures.clear();
        self.ln_body_scaled.clear();
        self.linear_sampled_textures.clear();
        self.sprite_scaled.clear();
        self.legacy_animation_cache.clear();
        self.gameplay_animation_cache.clear();
        self.gameplay_visibility_cache.clear();
        self.combo_break_red_cache.clear();
        self.hud_digit_cache = HudDigitHeightCache::default();
        self.hud_text_cache.clear();
        self.hud_asset_cache.clear();
        self.hud_font_cache.clear();
        self.hud_font_warning_cache.clear();
        self.hud_judgment_counter_animations.clear();
        self.hud_key_last_mask = 0;
        self.hud_key_press_times.clear();
        self.hud_key_down_since = [None; 32];
        self.hud_key_tail_releases.clear();
        self.hud_kps_samples.clear();
        self.hud_last_kps_sample_time = None;
        self.hud_pp_timeline.clear();
        self.hud_pp_final = None;
        self.hud_pp_warning = None;
        self.hud_unstable_rate = None;
        self.ln_bodies_prescaled = false;
        self.combo_break_textures_created = false;
        self.life_bar_warned = false;
        self.playfield_cover_runtime = PlayfieldCoverRuntime::default();
        self.first_note_time_ms = None;
        self.random_seed = 0;
        self.scroll_model = None;
        self.scroll_playback_clock = None;
        self.storyboard = None;
        if let Some(id) = self.background_texture_id.take() {
            self.gpu_textures.remove(&id);
            self.loaded_textures.remove(&id);
            self.texture_meta.remove(&id);
            self.texture_opaque_bounds.remove(&id);
        }
        self.background_visible = 1.0;
    }
    pub fn resolve_smoothed_playfield_cover(
        &mut self,
        frame_time_ms: i32,
        config: PlayfieldCoverConfig,
        metrics: PlayfieldCoverMetrics,
        hud_state: &HudFrameState,
    ) -> Option<PlayfieldCoverState> {
        let target = resolve_playfield_cover_state(config, metrics, hud_state);
        smooth_playfield_cover_state(target, &mut self.playfield_cover_runtime, frame_time_ms)
    }
}
fn gameplay_animation_frame_length_ms(kind: GameplayAnimationKind, frame_count: usize) -> f64 {
    match kind {
        GameplayAnimationKind::StageLight => 1000.0 / 24.0,
        GameplayAnimationKind::LightingN => gameplay_lighting_n_frame_length_ms(frame_count),
        GameplayAnimationKind::LightingL => 1000.0 / 60.0,
        GameplayAnimationKind::Note
        | GameplayAnimationKind::LongNoteBody
        | GameplayAnimationKind::StageBottom => 1000.0 / 60.0,
    }
}
fn gameplay_loop_frame_index(time_ms: f64, frame_length_ms: f64, frame_count: usize) -> usize {
    if frame_count == 0 {
        return 0;
    }
    ((time_ms / frame_length_ms).floor() as usize) % frame_count
}
fn gameplay_clamped_frame_index(time_ms: f64, frame_length_ms: f64, frame_count: usize) -> usize {
    if frame_count == 0 {
        return 0;
    }
    ((time_ms / frame_length_ms).floor() as usize).min(frame_count.saturating_sub(1))
}
fn gameplay_lighting_n_duration_ms(frame_length_ms: f64, frame_count: usize) -> f64 {
    (frame_count as f64 * frame_length_ms).max(170.0)
}
fn gameplay_lighting_n_frame_length_ms(frame_count: usize) -> f64 {
    if frame_count == 0 {
        return 1000.0 / 60.0;
    }
    (170.0 / frame_count as f64).max(1000.0 / 60.0)
}
impl Default for ReplayRenderer {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for ReplayRenderer {
    fn drop(&mut self) {
        self.dispose();
    }
}
impl ReplayRenderer {
    fn sprite_uses_exact_texture_region_size(
        &self,
        sprite: &super::sprites::SpriteCommand,
    ) -> bool {
        let Some(meta) = self.texture_meta.get(&sprite.texture_id) else {
            return false;
        };
        let draw_size = sprite
            .precise_size
            .unwrap_or([sprite.width as f32, sprite.height as f32]);
        let uv_span_u = (sprite.uv_rect[2] - sprite.uv_rect[0]).abs();
        let uv_span_v = (sprite.uv_rect[3] - sprite.uv_rect[1]).abs();
        let sampled_width = meta.w as f32 * uv_span_u;
        let sampled_height = meta.h as f32 * uv_span_v;
        (draw_size[0] - sampled_width).abs() <= f32::EPSILON
            && (draw_size[1] - sampled_height).abs() <= f32::EPSILON
    }
    pub(super) fn texture_sampling_for_sprite(
        &self,
        sprite: &super::sprites::SpriteCommand,
    ) -> TextureSampling {
        let texture_id = sprite.texture_id.as_str();
        // HUD textures and scaled skin pieces use linear sampling; exact-region draws stay pixel-sharp.
        if texture_id.starts_with("scorebar-") || texture_id.starts_with("hud_") {
            return TextureSampling::Linear;
        }
        if !self.linear_sampled_textures.contains(texture_id) {
            return TextureSampling::Nearest;
        }
        if self.sprite_uses_exact_texture_region_size(sprite) {
            TextureSampling::Nearest
        } else {
            TextureSampling::Linear
        }
    }
    pub(super) fn texture_sampling_for_texture(&self, texture_id: &str) -> TextureSampling {
        if texture_id.starts_with("scorebar-")
            || texture_id.starts_with("hud_")
            || self.linear_sampled_textures.contains(texture_id)
        {
            TextureSampling::Linear
        } else {
            TextureSampling::Nearest
        }
    }
    fn fallback_frame(&mut self, width: u32, height: u32) -> &[u8] {
        let size = (width * height * 4) as usize;
        if self.fallback_frame.len() != size {
            self.fallback_frame.resize(size, 0);
        }
        &self.fallback_frame
    }
    pub fn submit_frame(
        &mut self,
        timestamp: i32,
        animation_time_ms: f64,
        layout: &ManiaLayoutInfo,
        skin: &crate::types::SkinAssets,
        notes: &[crate::types::HitObject],
        active_indices: &[usize],
        judgments_by_idx: &[Option<RenderJudgment>],
        ln_releases_by_idx: &[Option<LnReleaseInfo>],
        hud_state: &HudFrameState,
        key_mask: u32,
        pps_base: f32,
        windows: Option<&Windows>,
        barlines: &[i32],
        fail_state: Option<&FailAnimationState>,
        playfield_cover: Option<&PlayfieldCoverState>,
    ) -> bool {
        if !self.initialized {
            return false;
        }
        let fail_state = fail_state.filter(|state| state.active);
        // Fail animation freezes gameplay inputs and note positions while HUD effects keep advancing from fail time.
        let state_time_ms = fail_state
            .map(|state| state.fail_started_at)
            .unwrap_or(timestamp);
        let position_time_ms = fail_state
            .map(|state| state.visual_time_ms)
            .unwrap_or(timestamp);
        let resolved_key_mask = fail_state
            .map(|state| state.frozen_key_mask)
            .unwrap_or(key_mask);
        let resolved_active_indices = fail_state
            .map(|state| state.active_note_indices.as_slice())
            .unwrap_or(active_indices);
        let mut hud_state_owned = hud_state.clone();
        self.enrich_hud_state(&mut hud_state_owned, state_time_ms, resolved_key_mask);
        let hud_state = &hud_state_owned;
        let mut before_notes = SpritePlanner::new();
        let mut notes_only = SpritePlanner::new();
        let mut after_notes = SpritePlanner::new();
        self.plan_background(&mut before_notes);
        self.plan_storyboard_gameplay_layers(&mut before_notes, &mut after_notes, timestamp);
        if !self.editor_preview_base_only {
            let column_opacities = self.hud_column_opacity_overrides(layout.columns.len());
            self.plan_column_backgrounds(
                &mut before_notes,
                layout,
                skin,
                column_opacities.as_deref(),
                state_time_ms,
            );
            if self.barlines_enabled {
                self.plan_barlines(
                    &mut before_notes,
                    layout,
                    skin,
                    barlines,
                    position_time_ms,
                    pps_base,
                    state_time_ms,
                );
            }
            self.plan_column_lines(&mut before_notes, layout, skin, state_time_ms);
            let keys_under_notes = skin.config.keys_under_notes;
            let key_planner = if keys_under_notes {
                &mut before_notes
            } else {
                &mut after_notes
            };
            self.plan_key_bases(
                key_planner,
                layout,
                skin,
                resolved_key_mask,
                keys_under_notes,
                state_time_ms,
            );
            self.plan_key_press_overlays(
                &mut after_notes,
                layout,
                skin,
                resolved_key_mask,
                state_time_ms,
            );
            self.plan_normal_notes(
                &mut notes_only,
                layout,
                skin,
                notes,
                resolved_active_indices,
                judgments_by_idx,
                state_time_ms,
                position_time_ms,
                animation_time_ms,
                pps_base,
                windows,
                fail_state,
            );
            self.plan_long_notes(
                &mut notes_only,
                layout,
                skin,
                notes,
                resolved_active_indices,
                judgments_by_idx,
                ln_releases_by_idx,
                state_time_ms,
                position_time_ms,
                animation_time_ms,
                resolved_key_mask,
                pps_base,
                windows,
                fail_state,
            );
            if self.lighting_enabled {
                self.plan_lighting(
                    &mut after_notes,
                    layout,
                    skin,
                    notes,
                    resolved_active_indices,
                    state_time_ms,
                    animation_time_ms,
                );
            }
            self.plan_stage_light(
                &mut after_notes,
                layout,
                skin,
                resolved_key_mask,
                animation_time_ms,
                state_time_ms,
            );
            let stage_left_cfg = self
                .hud_config
                .as_ref()
                .and_then(|cfg| cfg.elements.stage_left.clone());
            let stage_right_cfg = self
                .hud_config
                .as_ref()
                .and_then(|cfg| cfg.elements.stage_right.clone());
            let stage_bottom_cfg = self
                .hud_config
                .as_ref()
                .and_then(|cfg| cfg.elements.stage_bottom.clone());
            let stage_hint_cfg = self
                .hud_config
                .as_ref()
                .and_then(|cfg| cfg.elements.stage_hint.clone());
            self.plan_stage_hint(
                &mut after_notes,
                layout,
                skin,
                stage_hint_cfg.as_ref(),
                state_time_ms,
            );
            self.plan_stage_edges(
                &mut after_notes,
                layout,
                skin,
                stage_left_cfg.as_ref(),
                stage_right_cfg.as_ref(),
                state_time_ms,
            );
            self.plan_stage_bottom(
                &mut after_notes,
                layout,
                skin,
                animation_time_ms,
                stage_bottom_cfg.as_ref(),
                state_time_ms,
            );
            self.plan_warning_arrows(&mut after_notes, layout, skin, timestamp, state_time_ms);
            self.plan_playfield_cover_overlay(
                &mut after_notes,
                layout,
                playfield_cover,
                state_time_ms,
            );
        }
        if self.hud_enabled && hud_state.hud_visible {
            let canvas_w = self.cfg.width;
            let canvas_h = self.cfg.height;
            let scale_y = layout.scale_y;
            let life_bar_cfg = self
                .hud_config
                .as_ref()
                .and_then(|cfg| cfg.elements.life_bar.clone());
            self.plan_life_bar(
                &mut after_notes,
                layout,
                skin,
                hud_state.life,
                animation_time_ms,
                life_bar_cfg.as_ref(),
                state_time_ms,
            );
            let hud_elements = self.hud_config.as_ref().map(|cfg| &cfg.elements);
            let raw_score_layout = self.measure_score_raw_layout(skin, hud_state.score as u64);
            let default_screen_right_hud_scale = screen_right_hud_scale(canvas_h);
            let mut score_scale_x = LEGACY_SCORE_LAYOUT_BASE_SCALE * default_screen_right_hud_scale;
            let mut score_scale_y = LEGACY_SCORE_LAYOUT_BASE_SCALE * default_screen_right_hud_scale;
            let mut score_right =
                legacy_component_right_edge(canvas_w, LEGACY_SCORE_MARGIN_X, score_scale_x);
            let mut score_top = 0.0f32;
            let score_cfg = hud_elements.and_then(|el| el.score.as_ref());
            let score_visible = score_cfg.and_then(|cfg| cfg.visible).unwrap_or(true);
            let legacy_hud_layout = skin.legacy_hud_layout.as_ref();
            if let Some(cfg) = score_cfg {
                (score_scale_x, score_scale_y) = resolve_legacy_text_scale_from_hud_config(
                    cfg,
                    raw_score_layout.width,
                    raw_score_layout.height,
                    LEGACY_SCORE_LAYOUT_BASE_SCALE,
                );
                score_right =
                    legacy_component_right_edge(canvas_w, LEGACY_SCORE_MARGIN_X, score_scale_x);
                if let Some(x) = cfg.x.filter(|v| v.is_finite()) {
                    score_right = x;
                }
                if let Some(y) = cfg.y.filter(|v| v.is_finite()) {
                    score_top = y;
                }
            } else if legacy_hud_layout.is_some() {
                let score_transform = resolve_legacy_text_component_transform(
                    canvas_w,
                    canvas_h,
                    raw_score_layout.width,
                    raw_score_layout.height,
                    legacy_hud_layout.and_then(|layout| layout.score.as_ref()),
                    LEGACY_SCORE_LAYOUT_BASE_SCALE,
                    default_screen_right_hud_scale,
                    33,
                    33,
                    0.0,
                    0.0,
                    LEGACY_SCORE_MARGIN_X,
                    0.0,
                );
                score_right = score_transform.right_x;
                score_top = score_transform.top_y;
                score_scale_x = score_transform.scale_x;
                score_scale_y = score_transform.scale_y;
            }
            let score_screen_layout = self.measure_legacy_screen_layout(
                raw_score_layout,
                score_scale_x,
                score_scale_y,
                score_right,
                score_top,
            );
            let raw_accuracy_layout = self.measure_accuracy_raw_layout(skin, hud_state.accuracy);
            let accuracy_default_layout_mode = self.legacy_accuracy_default_layout_mode();
            let default_accuracy_scale = match accuracy_default_layout_mode {
                LegacyAccuracyDefaultLayoutMode::ScreenRight => {
                    LEGACY_ACCURACY_LAYOUT_BASE_SCALE * default_screen_right_hud_scale
                }
                LegacyAccuracyDefaultLayoutMode::StageRelative => {
                    LEGACY_ACCURACY_LAYOUT_BASE_SCALE * stage_relative_hud_scale(layout)
                }
            };
            let default_progress_size = match accuracy_default_layout_mode {
                LegacyAccuracyDefaultLayoutMode::ScreenRight => {
                    legacy_song_progress_layout_size(canvas_w, canvas_h) as f32
                }
                LegacyAccuracyDefaultLayoutMode::StageRelative => {
                    LEGACY_SONG_PROGRESS_LAYOUT_BASE_SIZE * stage_relative_hud_scale(layout)
                }
            };
            let mut accuracy_scale_x = default_accuracy_scale;
            let mut accuracy_scale_y = default_accuracy_scale;
            let mut accuracy_top = match accuracy_default_layout_mode {
                LegacyAccuracyDefaultLayoutMode::ScreenRight => {
                    score_screen_layout.bottom()
                        + legacy_scaled_hud_px(LEGACY_ACCURACY_MARGIN_Y, accuracy_scale_y)
                }
                LegacyAccuracyDefaultLayoutMode::StageRelative => {
                    stage_relative_accuracy_top(layout)
                }
            };
            let mut accuracy_right = match accuracy_default_layout_mode {
                LegacyAccuracyDefaultLayoutMode::ScreenRight => legacy_component_right_edge(
                    canvas_w,
                    LEGACY_ACCURACY_MARGIN_X,
                    accuracy_scale_x,
                ),
                LegacyAccuracyDefaultLayoutMode::StageRelative => stage_relative_accuracy_right(
                    layout,
                    raw_accuracy_layout.width * accuracy_scale_x,
                ),
            };
            let accuracy_cfg = hud_elements.and_then(|el| el.accuracy.as_ref());
            let accuracy_visible = accuracy_cfg.and_then(|cfg| cfg.visible).unwrap_or(true);
            let progress_cfg = hud_elements.and_then(|el| el.progress_circle.as_ref());
            let legacy_accuracy_layout =
                legacy_hud_layout.and_then(|layout| layout.accuracy.as_ref());
            let legacy_song_progress_layout =
                legacy_hud_layout.and_then(|layout| layout.song_progress.as_ref());
            if let Some(cfg) = accuracy_cfg {
                (accuracy_scale_x, accuracy_scale_y) = resolve_legacy_text_scale_from_hud_config(
                    cfg,
                    raw_accuracy_layout.width,
                    raw_accuracy_layout.height,
                    default_accuracy_scale,
                );
                accuracy_top = match accuracy_default_layout_mode {
                    LegacyAccuracyDefaultLayoutMode::ScreenRight => {
                        score_screen_layout.bottom()
                            + legacy_scaled_hud_px(LEGACY_ACCURACY_MARGIN_Y, accuracy_scale_y)
                    }
                    LegacyAccuracyDefaultLayoutMode::StageRelative => {
                        stage_relative_accuracy_top(layout)
                    }
                };
                accuracy_right = match accuracy_default_layout_mode {
                    LegacyAccuracyDefaultLayoutMode::ScreenRight => legacy_component_right_edge(
                        canvas_w,
                        LEGACY_ACCURACY_MARGIN_X,
                        accuracy_scale_x,
                    ),
                    LegacyAccuracyDefaultLayoutMode::StageRelative => {
                        stage_relative_accuracy_right(
                            layout,
                            raw_accuracy_layout.width * accuracy_scale_x,
                        )
                    }
                };
                if let Some(x) = cfg.x.filter(|v| v.is_finite()) {
                    accuracy_right = x;
                }
                if let Some(y) = cfg.y.filter(|v| v.is_finite()) {
                    accuracy_top = y;
                }
            } else if legacy_hud_layout.is_some() {
                let accuracy_transform = resolve_legacy_text_component_transform(
                    canvas_w,
                    canvas_h,
                    raw_accuracy_layout.width,
                    raw_accuracy_layout.height,
                    legacy_accuracy_layout,
                    LEGACY_ACCURACY_LAYOUT_BASE_SCALE,
                    default_screen_right_hud_scale,
                    33,
                    33,
                    0.0,
                    score_screen_layout.bottom(),
                    LEGACY_ACCURACY_MARGIN_X,
                    LEGACY_ACCURACY_MARGIN_Y,
                );
                accuracy_right = accuracy_transform.right_x;
                accuracy_top = accuracy_transform.top_y;
                accuracy_scale_x = accuracy_transform.scale_x;
                accuracy_scale_y = accuracy_transform.scale_y;
            }
            if score_visible {
                self.plan_score(
                    &mut after_notes,
                    skin,
                    hud_state.score as u64,
                    score_scale_x,
                    score_scale_y,
                    score_right,
                    score_top,
                    self.hud_element_runtime_rotation(score_cfg, state_time_ms),
                );
            }
            let measured_accuracy_layout = self.measure_accuracy_layout(
                skin,
                hud_state.accuracy,
                accuracy_scale_x,
                accuracy_scale_y,
                accuracy_right,
                accuracy_top,
            );
            let accuracy_screen_layout = self.measure_legacy_screen_layout(
                raw_accuracy_layout,
                accuracy_scale_x,
                accuracy_scale_y,
                accuracy_right,
                accuracy_top,
            );
            if accuracy_visible {
                self.plan_accuracy(
                    &mut after_notes,
                    skin,
                    hud_state.accuracy,
                    accuracy_scale_x,
                    accuracy_scale_y,
                    accuracy_right,
                    accuracy_top,
                    self.hud_element_runtime_rotation(accuracy_cfg, state_time_ms),
                )
            } else {
                measured_accuracy_layout
            };
            let circle_left = match accuracy_default_layout_mode {
                LegacyAccuracyDefaultLayoutMode::ScreenRight => legacy_song_progress_default_left(
                    canvas_w,
                    accuracy_screen_layout.width,
                    legacy_scaled_hud_px(
                        LEGACY_SONG_PROGRESS_GAP_X,
                        default_screen_right_hud_scale,
                    ),
                    default_progress_size,
                ),
                LegacyAccuracyDefaultLayoutMode::StageRelative => {
                    stage_relative_song_progress_left(
                        layout,
                        accuracy_screen_layout.left,
                        default_progress_size,
                    )
                }
            };
            let circle_top = accuracy_screen_layout.top + accuracy_screen_layout.height / 2.0
                - default_progress_size / 2.0;
            let progress_visible = progress_cfg.and_then(|cfg| cfg.visible).unwrap_or(true);
            let mut progress_x_f = circle_left;
            let mut progress_y_f = circle_top;
            let mut progress_size_f = default_progress_size;
            if progress_cfg.is_none() && legacy_hud_layout.is_some() {
                let (progress_x, progress_y, progress_size) = resolve_legacy_song_progress_layout(
                    canvas_w,
                    canvas_h,
                    circle_left.round() as i32,
                    circle_top.round() as i32,
                    legacy_song_progress_layout_size(canvas_w, canvas_h),
                    legacy_song_progress_layout,
                    default_screen_right_hud_scale,
                );
                progress_x_f = progress_x as f32;
                progress_y_f = progress_y as f32;
                progress_size_f = progress_size as f32;
            }
            if let Some(cfg) = progress_cfg {
                if let Some(x) = cfg.x.filter(|v| v.is_finite()) {
                    progress_x_f = x;
                }
                if let Some(y) = cfg.y.filter(|v| v.is_finite()) {
                    progress_y_f = y;
                }
                if let Some(size) = cfg.size.filter(|v| v.is_finite() && *v > 0.0) {
                    progress_size_f = size;
                } else if let Some(width) = cfg.width.filter(|v| v.is_finite() && *v > 0.0) {
                    progress_size_f = width;
                } else if let Some(height) = cfg.height.filter(|v| v.is_finite() && *v > 0.0) {
                    progress_size_f = height;
                }
                if let Some(scale) = cfg.scale.filter(|v| v.is_finite() && *v > 0.0) {
                    progress_size_f = (progress_size_f * scale).max(1.0);
                }
            }
            if progress_visible {
                self.plan_progress_circle(
                    &mut after_notes,
                    hud_state.progress as f64,
                    progress_x_f,
                    progress_y_f,
                    progress_size_f,
                    self.hud_element_runtime_rotation(progress_cfg, state_time_ms),
                );
            }
            let mods_cfg = hud_elements.and_then(|el| el.mods.as_ref());
            let mods_visible = mods_cfg.and_then(|cfg| cfg.visible).unwrap_or(true);
            if mods_visible {
                let mods_top = if accuracy_visible {
                    accuracy_screen_layout.bottom()
                        + legacy_scaled_hud_px(
                            REPLAY_MOD_DEFAULT_MARGIN_TOP,
                            default_screen_right_hud_scale,
                        )
                } else if score_visible {
                    score_screen_layout.bottom()
                        + legacy_scaled_hud_px(
                            REPLAY_MOD_DEFAULT_MARGIN_TOP,
                            default_screen_right_hud_scale,
                        )
                } else {
                    legacy_scaled_hud_px(
                        REPLAY_MOD_DEFAULT_MARGIN_TOP,
                        default_screen_right_hud_scale,
                    )
                };
                let mods_right = canvas_w as f32
                    - legacy_scaled_hud_px(
                        REPLAY_MOD_DEFAULT_MARGIN_RIGHT,
                        default_screen_right_hud_scale,
                    );
                self.plan_replay_mod_display(
                    &mut after_notes,
                    mods_right,
                    mods_top,
                    mods_cfg,
                    state_time_ms,
                );
            }
            let native_combo_h = self.hud_digit_cache.combo.unwrap_or(60);
            let combo_h = (native_combo_h as f32 * scale_y * LEGACY_COMBO_LAYOUT_SCALE)
                .round()
                .max(1.0) as u32;
            let combo_height = self.legacy_combo_height_for_current_hud_config(combo_h);
            let center_x = layout.stage.x + layout.stage.width as i32 / 2;
            let mut combo_center_x = center_x;
            let combo_y = if let Some(combo_pos) = skin.config.combo_pos_y {
                (combo_pos as f32 * scale_y).round() as i32
            } else {
                (layout.stage.hit_y as f32 - 96.0 * scale_y).round() as i32
            };
            let mut combo_pos_y = combo_y;
            let combo_cfg = hud_elements.and_then(|el| el.combo.as_ref());
            let combo_visible = combo_cfg.and_then(|cfg| cfg.visible).unwrap_or(true);
            if let Some(cfg) = combo_cfg {
                if let Some(x) = cfg.x.filter(|v| v.is_finite()) {
                    combo_center_x = x.round() as i32;
                }
                if let Some(y) = cfg.y.filter(|v| v.is_finite()) {
                    combo_pos_y = y.round() as i32;
                }
            }
            let stretch = hud_state
                .combo_inc_anim
                .map(|a| Self::calc_combo_stretch(a.age_ms))
                .unwrap_or(1.0);
            let combo_rotation = self.hud_element_runtime_rotation(combo_cfg, state_time_ms);
            if combo_visible {
                self.plan_combo(
                    &mut after_notes,
                    skin,
                    hud_state.combo,
                    combo_height,
                    combo_center_x,
                    combo_pos_y,
                    stretch,
                    combo_rotation,
                );
            }
            if let Some(anim) = &hud_state.combo_burst_anim {
                self.plan_combo_burst(&mut before_notes, layout, skin, anim.combo, anim.age_ms);
            }
            if let Some(last) = &hud_state.last_judgment {
                let anim_fps = skin.config.animation_framerate.unwrap_or(60);
                let judgment_cfg = hud_elements.and_then(|el| el.judgment_pop.as_ref());
                let judgment_visible = judgment_cfg.and_then(|cfg| cfg.visible).unwrap_or(true);
                let mut judgment_center_x = center_x;
                let mut judgment_center_y = None;
                let mut judgment_scale = 1.0f32;
                let mut judgment_target_height = None;
                if let Some(cfg) = judgment_cfg {
                    if let Some(x) = cfg.x.filter(|v| v.is_finite()) {
                        judgment_center_x = x.round() as i32;
                    }
                    if let Some(y) = cfg.y.filter(|v| v.is_finite()) {
                        judgment_center_y = Some(y.round() as i32);
                    }
                    if let Some(height) = cfg.height.filter(|v| v.is_finite() && *v > 0.0) {
                        judgment_target_height = Some(height.round().max(1.0) as u32);
                    }
                    if let Some(scale) =
                        cfg.scale.or(cfg.size).filter(|v| v.is_finite() && *v > 0.0)
                    {
                        judgment_scale = scale;
                    }
                }
                if judgment_visible {
                    self.plan_judgment_popup(
                        &mut after_notes,
                        skin,
                        last.kind,
                        last.age_ms,
                        judgment_center_x,
                        layout.stage.hit_y,
                        scale_y,
                        anim_fps,
                        judgment_center_y,
                        judgment_scale,
                        judgment_target_height,
                        self.hud_element_runtime_rotation(judgment_cfg, state_time_ms),
                    );
                }
            }
            if combo_visible {
                if let Some(anim) = &hud_state.combo_break_anim {
                    self.plan_combo_break(
                        &mut after_notes,
                        skin,
                        anim.start_combo,
                        anim.age_ms,
                        combo_height,
                        combo_center_x,
                        combo_pos_y,
                        scale_y,
                        combo_rotation,
                    );
                }
            }
            let hit_error_meter_cfg = self
                .hud_config
                .as_ref()
                .and_then(|cfg| cfg.elements.hit_error_meter.clone());
            self.plan_builtin_hit_error_meter(
                &mut after_notes,
                hud_state,
                hit_error_meter_cfg.as_ref(),
                state_time_ms,
                z_order::HUD + 5,
            );
            self.plan_hud_layers(
                &mut after_notes,
                hud_state,
                resolved_key_mask,
                state_time_ms,
            );
        }
        self.submit_render_gpu_layers(&before_notes, &notes_only, &after_notes)
    }
    pub fn render_frame(
        &mut self,
        timestamp: i32,
        layout: &ManiaLayoutInfo,
        skin: &crate::types::SkinAssets,
        notes: &[crate::types::HitObject],
        active_indices: &[usize],
        judgments_by_idx: &[Option<RenderJudgment>],
        ln_releases_by_idx: &[Option<LnReleaseInfo>],
        hud_state: &HudFrameState,
        key_mask: u32,
        pps_base: f32,
        windows: Option<&Windows>,
        barlines: &[i32],
        fail_state: Option<&FailAnimationState>,
        playfield_cover: Option<&PlayfieldCoverState>,
    ) -> &[u8] {
        if !self.submit_frame(
            timestamp,
            timestamp as f64,
            layout,
            skin,
            notes,
            active_indices,
            judgments_by_idx,
            ln_releases_by_idx,
            hud_state,
            key_mask,
            pps_base,
            windows,
            barlines,
            fail_state,
            playfield_cover,
        ) {
            return self.fallback_frame(self.cfg.width, self.cfg.height);
        }
        self.drain_ready_frame_blocking()
            .expect("frame should be available after render submit")
    }
}
