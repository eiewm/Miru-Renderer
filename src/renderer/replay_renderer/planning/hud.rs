use super::super::layout::ManiaLayoutInfo;
use super::super::render::{
    lazer_replay_mod_texture_name, screen_right_hud_scale, stable_replay_mod_texture_stem,
    HudEditorPreviewRect, LegacyAnimationSpec, ReplayRenderer, REPLAY_MOD_COLLAPSED_STEP_X,
    REPLAY_MOD_DEFAULT_ICON_SIZE,
};
use super::super::sprites::{z_order, SpriteCommand, SpritePlanner, Tint};
use super::super::textures::{OpaqueBounds, TextureMeta};
use crate::hud::HudElementConfig;
use crate::types::{JudgmentKind, SkinAssets};
// Stable scorebar assets are horizontal images rotated into the mania side lane.
const LIFE_BAR_LOGICAL_HEIGHT: f32 = 768.0;
const LIFE_BAR_REPLAY_SCALE: f32 = 0.7;
const LIFE_BAR_GAP_PX: f32 = 1.0;
const LEGACY_SCOREBAR_LAYOUT_SCALE: f32 = 1.6;
const QUARTER_TURN_CW: f32 = -std::f32::consts::FRAC_PI_2;
fn combo_burst_stage_scale(layout: &ManiaLayoutInfo) -> f32 {
    (layout.stage.height.max(1) as f32 / 768.0).max(f32::EPSILON)
}
#[derive(Debug, Clone, Copy)]
struct LifeBarLaneAnchor {
    visible_left_x: f32,
    bottom_y: f32,
}
#[derive(Debug, Clone, Copy)]
struct ScorebarContainerOrigin {
    x: f32,
    y: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacyScorebarStyle {
    Old,
    New,
}
#[derive(Debug, Clone, Copy)]
struct LegacyScorebarLayout {
    fill_offset_x: f32,
    fill_offset_y: f32,
    marker_center_y: bool,
}
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct RotatedScorebarPlacement {
    draw_x: i32,
    draw_y: i32,
    draw_w: u32,
    draw_h: u32,
    uv_rect: [f32; 4],
    visible_left: f32,
    visible_top: f32,
    visible_w: f32,
    visible_h: f32,
}
impl RotatedScorebarPlacement {
    #[allow(dead_code)]
    fn visible_y(self) -> i32 {
        self.visible_top.round() as i32
    }
}
#[derive(Debug, Clone)]
struct LegacyTextureSelection {
    texture_id: String,
    anchor_texture_id: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LegacyHudTextKind {
    Score,
    Accuracy,
}
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) struct LegacyHudTextLayout {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct LegacyHudRawTextLayout {
    pub width: f32,
    pub height: f32,
}
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) struct LegacyHudScreenTextLayout {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}
impl LegacyHudScreenTextLayout {
    pub(crate) fn bottom(self) -> f32 {
        self.top + self.height
    }
}
#[derive(Debug, Clone)]
struct LegacyHudRawGlyphPlacement {
    texture_id: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}
#[derive(Debug, Clone)]
struct LegacyHudRawTextPlan {
    layout: LegacyHudRawTextLayout,
    glyphs: Vec<LegacyHudRawGlyphPlacement>,
}
#[derive(Debug, Clone)]
struct LegacyHudGlyphPlacement {
    texture_id: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    precise_x: f32,
    precise_y: f32,
    precise_width: f32,
    precise_height: f32,
}
#[derive(Debug, Clone)]
struct LegacyHudTextPlan {
    layout: LegacyHudTextLayout,
    glyphs: Vec<LegacyHudGlyphPlacement>,
}
#[derive(Debug, Clone)]
struct ReplayModIconPlacement {
    texture_id: String,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}
#[derive(Debug, Clone)]
struct ReplayModDisplayPlan {
    layout: LegacyHudScreenTextLayout,
    icons: Vec<ReplayModIconPlacement>,
}
impl ReplayRenderer {
    pub(crate) fn measure_life_bar_editor_preview_rect(
        &self,
        layout: &ManiaLayoutInfo,
    ) -> Option<HudEditorPreviewRect> {
        let fill_texture = self.find_first_texture(&[
            "scorebar-colour.png",
            "scorebar-colour.jpg",
            "scorebar-colour",
            "scorebar-colour-0.png",
            "scorebar-colour-0.jpg",
            "scorebar-colour-0",
        ])?;
        let fill_meta = self.texture_meta.get(&fill_texture).copied()?;
        let fill_bounds = drawable_life_bar_bounds(
            fill_meta,
            self.texture_opaque_bounds
                .get(&fill_texture)
                .copied()
                .or_else(|| Some(default_opaque_bounds(fill_meta))),
        )?;
        let life_bar_cfg = self
            .hud_config
            .as_ref()
            .and_then(|cfg| cfg.elements.life_bar.as_ref());
        let lane_anchor = compute_life_bar_lane_anchor(layout, life_bar_cfg);
        let (scale_x, scale_y) =
            compute_life_bar_axis_scale(self.cfg.height, life_bar_cfg, fill_bounds);
        let width = life_bar_cfg
            .and_then(|cfg| cfg.width)
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or_else(|| (fill_bounds.height().max(1) as f32 * scale_y).max(1.0));
        let height = life_bar_cfg
            .and_then(|cfg| cfg.height)
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or_else(|| (fill_bounds.width().max(1) as f32 * scale_x).max(1.0));
        Some(HudEditorPreviewRect {
            x: lane_anchor.visible_left_x.round() as i32,
            y: (lane_anchor.bottom_y - height).round() as i32,
            width: width.round().max(1.0) as u32,
            height: height.round().max(1.0) as u32,
        })
    }
    pub fn plan_life_bar(
        &mut self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        life: f32,
        animation_time_ms: f64,
        config: Option<&HudElementConfig>,
        timestamp_ms: i32,
    ) {
        if config.and_then(|cfg| cfg.visible) == Some(false) {
            return;
        }
        if self.should_suppress_life_bar_for_storyboard() {
            return;
        }
        self.plan_legacy_life_bar(
            planner,
            layout,
            skin,
            life,
            animation_time_ms,
            config,
            timestamp_ms,
        );
    }
    fn plan_legacy_life_bar(
        &mut self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        life: f32,
        animation_time_ms: f64,
        config: Option<&HudElementConfig>,
        timestamp_ms: i32,
    ) {
        let animation_framerate = skin.config.animation_framerate;
        let new_style_marker = self.resolve_legacy_texture(
            "scorebar-marker",
            &[
                "scorebar-marker.png",
                "scorebar-marker.jpg",
                "scorebar-marker",
            ],
            animation_time_ms,
            true,
            animation_framerate,
        );
        let scorebar_style = if new_style_marker.is_some() {
            LegacyScorebarStyle::New
        } else {
            LegacyScorebarStyle::Old
        };
        // New scorebars use scorebar-marker; old scorebars swap scorebar-ki variants by life level.
        let background = self.resolve_legacy_texture(
            "scorebar-bg",
            &["scorebar-bg.png", "scorebar-bg.jpg", "scorebar-bg"],
            animation_time_ms,
            true,
            animation_framerate,
        );
        let fill = self.resolve_legacy_texture(
            "scorebar-colour",
            &[
                "scorebar-colour.png",
                "scorebar-colour.jpg",
                "scorebar-colour",
                "scorebar-colour-0.png",
                "scorebar-colour-0.jpg",
                "scorebar-colour-0",
            ],
            animation_time_ms,
            true,
            animation_framerate,
        );
        let Some(fill) = fill else {
            if !self.life_bar_warned {
                println!("   warn: scorebar-colour missing, life bar will not be drawn");
                self.life_bar_warned = true;
            }
            return;
        };
        let Some(fill_meta) = self.texture_meta.get(&fill.anchor_texture_id).copied() else {
            return;
        };
        let Some(fill_bounds) = drawable_life_bar_bounds(
            fill_meta,
            self.texture_opaque_bounds
                .get(&fill.anchor_texture_id)
                .copied()
                .or_else(|| Some(default_opaque_bounds(fill_meta))),
        ) else {
            return;
        };
        // Opaque bounds trim transparent padding before the horizontal asset is rotated into the side lane.
        let scorebar_layout = legacy_scorebar_layout(scorebar_style);
        let background_drawable = background.as_ref().and_then(|texture_id| {
            let meta = self
                .texture_meta
                .get(&texture_id.anchor_texture_id)
                .copied()?;
            let bounds = drawable_life_bar_bounds(
                meta,
                self.texture_opaque_bounds
                    .get(&texture_id.anchor_texture_id)
                    .copied()
                    .or_else(|| Some(default_opaque_bounds(meta))),
            )?;
            Some((texture_id.texture_id.clone(), meta, bounds))
        });
        let lane_anchor = compute_life_bar_lane_anchor(layout, config);
        let (anchor_bounds, anchor_offset_y) = match (scorebar_style, background_drawable.as_ref())
        {
            (LegacyScorebarStyle::Old, Some((_, _, bounds))) => (*bounds, 0.0),
            _ => (fill_bounds, scorebar_layout.fill_offset_y),
        };
        let (scale_x, scale_y) = compute_life_bar_axis_scale(self.cfg.height, config, fill_bounds);
        let container_origin = compute_scorebar_container_origin(
            lane_anchor,
            anchor_bounds,
            scorebar_layout.fill_offset_x,
            anchor_offset_y,
            scale_x,
            scale_y,
        );
        let clamped_life = life.clamp(0.0, 1.0);
        let fill_source_end = fill_meta.w as f32 * clamped_life;
        let fill_transform = compute_rotated_scorebar_piece(
            container_origin,
            scorebar_layout.fill_offset_x,
            scorebar_layout.fill_offset_y,
            fill_meta,
            fill_bounds,
            scale_x,
            scale_y,
            fill_source_end,
        );
        let life_bar_rotation = self.hud_element_runtime_rotation(config, timestamp_ms);
        let life_bar_bounds_width = config
            .and_then(|cfg| cfg.width)
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or_else(|| (fill_bounds.height().max(1) as f32 * scale_y).max(1.0));
        let life_bar_bounds_height = config
            .and_then(|cfg| cfg.height)
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or_else(|| (fill_bounds.width().max(1) as f32 * scale_x).max(1.0));
        let life_bar_bounds_left = lane_anchor.visible_left_x;
        let life_bar_bounds_top = lane_anchor.bottom_y - life_bar_bounds_height;
        if let Some((background, bg_meta, bg_bounds)) = background_drawable {
            let background_transform = compute_rotated_scorebar_piece(
                container_origin,
                0.0,
                0.0,
                bg_meta,
                bg_bounds,
                scale_x,
                scale_y,
                bg_meta.w as f32,
            );
            let mut sprite = SpriteCommand {
                texture_id: background,
                x: background_transform.draw_x,
                y: background_transform.draw_y,
                width: background_transform.draw_w,
                height: background_transform.draw_h,
                tint: [1.0, 1.0, 1.0, 1.0],
                uv_rect: background_transform.uv_rect,
                origin: [0.0, 0.0],
                rotation: QUARTER_TURN_CW,
                z_order: z_order::HUD,
                ..Default::default()
            };
            Self::rotate_legacy_sprite_around_bounds(
                &mut sprite,
                life_bar_bounds_left,
                life_bar_bounds_top,
                life_bar_bounds_width,
                life_bar_bounds_height,
                life_bar_rotation,
            );
            planner.add_sprite(sprite);
        }
        if clamped_life > 0.0 && fill_transform.visible_h > 0.0 {
            let mut sprite = SpriteCommand {
                texture_id: fill.texture_id,
                x: fill_transform.draw_x,
                y: fill_transform.draw_y,
                width: fill_transform.draw_w,
                height: fill_transform.draw_h,
                tint: [1.0, 1.0, 1.0, 1.0],
                uv_rect: fill_transform.uv_rect,
                origin: [0.0, 0.0],
                rotation: QUARTER_TURN_CW,
                z_order: z_order::HUD + 1,
                ..Default::default()
            };
            Self::rotate_legacy_sprite_around_bounds(
                &mut sprite,
                life_bar_bounds_left,
                life_bar_bounds_top,
                life_bar_bounds_width,
                life_bar_bounds_height,
                life_bar_rotation,
            );
            planner.add_sprite(sprite);
        }
        let marker_texture = match scorebar_style {
            LegacyScorebarStyle::New => new_style_marker,
            LegacyScorebarStyle::Old => self.resolve_old_style_scorebar_marker(life),
        };
        let Some(marker_texture) = marker_texture else {
            return;
        };
        let Some(marker_meta) = self
            .texture_meta
            .get(&marker_texture.anchor_texture_id)
            .copied()
        else {
            return;
        };
        let marker_w = (marker_meta.w.max(1) as f32 * scale_x).round().max(1.0) as u32;
        let marker_h = (marker_meta.h.max(1) as f32 * scale_y).round().max(1.0) as u32;
        let (marker_center_x, marker_center_y) = compute_scorebar_marker_center(
            container_origin,
            scorebar_layout,
            fill_transform,
            scale_x,
            scale_y,
        );
        let marker_x = (marker_center_x - marker_w as f32 / 2.0).round() as i32;
        let marker_y = (marker_center_y - marker_h as f32 / 2.0).round() as i32;
        let mut marker_sprite = SpriteCommand {
            texture_id: marker_texture.texture_id,
            x: marker_x,
            y: marker_y,
            width: marker_w,
            height: marker_h,
            tint: [1.0, 1.0, 1.0, 1.0],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            z_order: z_order::HUD + 2,
            ..Default::default()
        };
        Self::rotate_legacy_sprite_around_bounds(
            &mut marker_sprite,
            life_bar_bounds_left,
            life_bar_bounds_top,
            life_bar_bounds_width,
            life_bar_bounds_height,
            life_bar_rotation,
        );
        planner.add_sprite(marker_sprite);
    }
    fn should_suppress_life_bar_for_storyboard(&self) -> bool {
        // osu! overlay storyboard layers cover the scorebar area, so the replay HUD hides its life bar.
        self.storyboard_enabled
            && self
                .storyboard
                .as_ref()
                .map(|storyboard| storyboard.has_overlay_layer())
                .unwrap_or(false)
    }
    fn find_first_texture(&self, candidates: &[&str]) -> Option<String> {
        candidates
            .iter()
            .find(|candidate| self.has_texture(candidate))
            .map(|candidate| (*candidate).to_string())
    }
    fn find_first_texture_owned(&self, candidates: &[String]) -> Option<String> {
        candidates
            .iter()
            .find(|candidate| self.has_texture(candidate))
            .cloned()
    }
    fn resolve_old_style_scorebar_marker(&self, life: f32) -> Option<LegacyTextureSelection> {
        let life = life.clamp(0.0, 1.0);
        let groups: &[&[&str]] = if life < 0.2 {
            &[
                &[
                    "scorebar-kidanger2.png",
                    "scorebar-kidanger2.jpg",
                    "scorebar-kidanger2",
                ],
                &[
                    "scorebar-kidanger.png",
                    "scorebar-kidanger.jpg",
                    "scorebar-kidanger",
                ],
                &["scorebar-ki.png", "scorebar-ki.jpg", "scorebar-ki"],
            ]
        } else if life < 0.5 {
            &[
                &[
                    "scorebar-kidanger.png",
                    "scorebar-kidanger.jpg",
                    "scorebar-kidanger",
                ],
                &["scorebar-ki.png", "scorebar-ki.jpg", "scorebar-ki"],
            ]
        } else {
            &[&["scorebar-ki.png", "scorebar-ki.jpg", "scorebar-ki"]]
        };
        groups.iter().find_map(|group| {
            self.find_first_texture(group)
                .map(|texture_id| LegacyTextureSelection {
                    anchor_texture_id: texture_id.clone(),
                    texture_id,
                })
        })
    }
    fn resolve_legacy_texture(
        &mut self,
        base_name: &str,
        static_candidates: &[&str],
        animation_time_ms: f64,
        apply_config_frame_rate: bool,
        animation_framerate: Option<u32>,
    ) -> Option<LegacyTextureSelection> {
        let animations_enabled = self.skin_animations_enabled;
        let fallback_texture_id = self.find_first_texture(static_candidates);
        let spec = self
            .legacy_animation_spec(base_name, apply_config_frame_rate, animation_framerate)
            .cloned();
        let fallback = fallback_texture_id
            .or_else(|| spec.as_ref().and_then(|spec| spec.frames.first().cloned()))
            .map(|texture_id| LegacyTextureSelection {
                anchor_texture_id: texture_id.clone(),
                texture_id: self.prefer_hidpi_texture(&texture_id),
            });
        if !animations_enabled {
            return fallback;
        }
        let Some(spec) = spec else {
            return fallback;
        };
        let frame_idx = legacy_animation_frame_index(
            animation_time_ms.max(0.0),
            spec.frame_length_ms,
            spec.frames.len(),
        );
        let anchor_texture_id = spec.frames.first()?.clone();
        // Layout anchors stay on frame 0 while draw frames can switch to @2x variants.
        let texture_id = self.prefer_hidpi_texture(spec.frames.get(frame_idx)?);
        Some(LegacyTextureSelection {
            texture_id,
            anchor_texture_id,
        })
    }
    fn legacy_animation_spec(
        &mut self,
        base_name: &str,
        apply_config_frame_rate: bool,
        animation_framerate: Option<u32>,
    ) -> Option<&LegacyAnimationSpec> {
        if !self.legacy_animation_cache.contains_key(base_name) {
            let spec = self.detect_legacy_animation_spec(
                base_name,
                apply_config_frame_rate,
                animation_framerate,
            );
            self.legacy_animation_cache
                .insert(base_name.to_string(), spec);
        }
        self.legacy_animation_cache
            .get(base_name)
            .and_then(Option::as_ref)
    }
    fn detect_legacy_animation_spec(
        &self,
        base_name: &str,
        apply_config_frame_rate: bool,
        animation_framerate: Option<u32>,
    ) -> Option<LegacyAnimationSpec> {
        let mut frames = self.collect_legacy_animation_frames(base_name, 0);
        if frames.len() <= 1 {
            // Some stable skins start animation numbering at 1 instead of 0.
            frames = self.collect_legacy_animation_frames(base_name, 1);
        }
        if frames.len() <= 1 {
            return None;
        }
        Some(LegacyAnimationSpec {
            frame_length_ms: legacy_animation_frame_length_ms(
                apply_config_frame_rate,
                animation_framerate,
                frames.len(),
            ),
            frames,
        })
    }
    fn collect_legacy_animation_frames(
        &self,
        base_name: &str,
        first_frame_idx: usize,
    ) -> Vec<String> {
        let mut frames = Vec::new();
        for frame_idx in first_frame_idx.. {
            let frame_name = legacy_animation_frame_texture(base_name, frame_idx);
            let Some(texture_id) = self.find_first_texture_owned(&frame_name) else {
                break;
            };
            frames.push(texture_id);
        }
        frames
    }
    fn prefer_hidpi_texture(&self, texture_id: &str) -> String {
        if let Some(hidpi_texture_id) = hidpi_texture_name(texture_id) {
            if self.loaded_textures.contains(&hidpi_texture_id) {
                return hidpi_texture_id;
            }
        }
        texture_id.to_string()
    }
    fn legacy_texture_scale_adjust(texture_id: &str) -> f32 {
        if texture_id.contains("@2x") {
            2.0
        } else {
            1.0
        }
    }
    fn legacy_texture_display_size(meta: TextureMeta, texture_id: &str) -> (f32, f32) {
        let scale_adjust = Self::legacy_texture_scale_adjust(texture_id).max(1.0);
        (meta.w as f32 / scale_adjust, meta.h as f32 / scale_adjust)
    }
    fn is_invalid_accuracy_percent_glyph(
        draw_width: f32,
        draw_height: f32,
        digit_width: f32,
        digit_height: f32,
    ) -> bool {
        // Some skins ship non-score percent art; reject shapes far outside digit proportions.
        let digit_width = digit_width.max(1.0);
        let digit_height = digit_height.max(1.0);
        draw_width > digit_width * 6.0
            || draw_height > digit_height * 4.0
            || draw_width / draw_height.max(1.0) > 8.0
    }
    fn resolve_legacy_hud_glyph(&self, base_name: String) -> Option<(String, f32, f32)> {
        let preferred = self.prefer_hidpi_texture(&base_name);
        let (texture_id, meta) = if let Some(meta) = self.texture_meta.get(&preferred).copied() {
            (preferred, meta)
        } else {
            let meta = self.texture_meta.get(&base_name).copied()?;
            (base_name, meta)
        };
        let (display_width, display_height) = Self::legacy_texture_display_size(meta, &texture_id);
        Some((texture_id, display_width.max(1.0), display_height.max(1.0)))
    }
    fn replay_mod_texture_candidates(&self, acronym: &str) -> Option<Vec<String>> {
        let display = self.replay_mod_display.as_ref()?;
        match display.origin {
            crate::types::ReplayOrigin::StableLegacy => {
                // Stable mod icons come from selection-mod-* files; lazer exports use generated replay icons.
                let stem = stable_replay_mod_texture_stem(acronym)?;
                Some(vec![
                    format!("{stem}@2x.png"),
                    format!("{stem}.png"),
                    format!("{stem}@2x"),
                    stem.to_string(),
                ])
            }
            crate::types::ReplayOrigin::LazerExport => {
                Some(vec![lazer_replay_mod_texture_name(acronym)])
            }
        }
    }
    fn resolve_replay_mod_texture_id(&self, acronym: &str) -> Option<String> {
        let candidates = self.replay_mod_texture_candidates(acronym)?;
        self.find_first_texture_owned(&candidates)
    }
    pub(crate) fn resolved_replay_mod_display_acronyms(&self) -> Vec<String> {
        let Some(display) = self.replay_mod_display.as_ref() else {
            return Vec::new();
        };
        display
            .acronyms
            .iter()
            .filter(|acronym| self.resolve_replay_mod_texture_id(acronym).is_some())
            .cloned()
            .collect()
    }
    pub(crate) fn replay_mod_display_origin(&self) -> Option<crate::types::ReplayOrigin> {
        self.replay_mod_display
            .as_ref()
            .map(|display| display.origin)
    }
    fn resolve_replay_mod_icon_size(&self, config: Option<&HudElementConfig>) -> (f32, f32) {
        let default_icon_size =
            REPLAY_MOD_DEFAULT_ICON_SIZE * screen_right_hud_scale(self.cfg.height);
        let size_hint = config
            .and_then(|cfg| cfg.size.or(cfg.width).or(cfg.height))
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(default_icon_size);
        let mut width = config
            .and_then(|cfg| cfg.width)
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(size_hint);
        let mut height = config
            .and_then(|cfg| cfg.height)
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(size_hint);
        if let Some(scale) = config
            .and_then(|cfg| cfg.scale)
            .filter(|value| value.is_finite() && *value > 0.0)
        {
            width *= scale;
            height *= scale;
        }
        (width.max(1.0), height.max(1.0))
    }
    fn build_replay_mod_display_plan(
        &self,
        right_x: f32,
        top_y: f32,
        config: Option<&HudElementConfig>,
    ) -> Option<ReplayModDisplayPlan> {
        let display = self.replay_mod_display.as_ref()?;
        if display.acronyms.is_empty() {
            return None;
        }
        let (icon_width, icon_height) = self.resolve_replay_mod_icon_size(config);
        let step_x =
            (REPLAY_MOD_COLLAPSED_STEP_X * (icon_width / REPLAY_MOD_DEFAULT_ICON_SIZE)).max(1.0);
        let right_x = config
            .and_then(|cfg| cfg.x)
            .filter(|value| value.is_finite())
            .unwrap_or(right_x);
        let top_y = config
            .and_then(|cfg| cfg.y)
            .filter(|value| value.is_finite())
            .unwrap_or(top_y);
        let mut texture_ids = Vec::new();
        for acronym in &display.acronyms {
            if let Some(texture_id) = self.resolve_replay_mod_texture_id(acronym) {
                texture_ids.push(texture_id);
            }
        }
        if texture_ids.is_empty() {
            return None;
        }
        // Stable replay mod icons overlap horizontally instead of occupying full icon widths.
        let row_width = icon_width + step_x * texture_ids.len().saturating_sub(1) as f32;
        let left = right_x - row_width;
        let mut icons = Vec::with_capacity(texture_ids.len());
        for (index, texture_id) in texture_ids.into_iter().enumerate() {
            icons.push(ReplayModIconPlacement {
                texture_id,
                left: left + step_x * index as f32,
                top: top_y,
                width: icon_width,
                height: icon_height,
            });
        }
        Some(ReplayModDisplayPlan {
            layout: LegacyHudScreenTextLayout {
                left,
                top: top_y,
                width: row_width.max(0.0),
                height: icon_height.max(0.0),
            },
            icons,
        })
    }
    fn legacy_hud_base_name(kind: LegacyHudTextKind, prefix: &str, ch: char) -> String {
        match ch {
            '%' if kind == LegacyHudTextKind::Accuracy => format!("{prefix}-percent.png"),
            '.' => format!("{prefix}-dot.png"),
            ',' => format!("{prefix}-comma.png"),
            _ => format!("{prefix}-{ch}.png"),
        }
        .to_lowercase()
    }
    fn legacy_overlap_width(&self, overlap: i32) -> f32 {
        overlap.max(0) as f32
    }
    fn fixed_width_digit_metrics(&self, prefix: &str) -> (f32, f32) {
        // 5, 0, and 8 are reliable width samples for most score digit atlases.
        for digit in ['5', '0', '8'] {
            let base_name = format!("{}-{}.png", prefix, digit).to_lowercase();
            if let Some((_, width, height)) = self.resolve_legacy_hud_glyph(base_name) {
                return (width.max(1.0), height.max(1.0));
            }
        }
        let mut max_width = 0.0f32;
        let mut max_height = 0.0f32;
        for digit in '0'..='9' {
            let base_name = format!("{}-{}.png", prefix, digit).to_lowercase();
            if let Some((_, width, height)) = self.resolve_legacy_hud_glyph(base_name) {
                max_width = max_width.max(width);
                max_height = max_height.max(height);
            }
        }
        (max_width.max(1.0), max_height.max(1.0))
    }
    fn build_legacy_raw_text_plan(
        &self,
        text: &str,
        kind: LegacyHudTextKind,
        prefix: &str,
        overlap: f32,
    ) -> LegacyHudRawTextPlan {
        if text.is_empty() {
            return LegacyHudRawTextPlan {
                layout: LegacyHudRawTextLayout {
                    width: 0.0,
                    height: 0.0,
                },
                glyphs: Vec::new(),
            };
        }
        let (fixed_digit_advance, fixed_digit_height) = self.fixed_width_digit_metrics(prefix);
        let chars = text.chars().collect::<Vec<_>>();
        let char_count = chars.len();
        let mut cursor_x = 0.0f32;
        let mut max_height = 0.0f32;
        let mut glyphs = Vec::with_capacity(chars.len());
        for (index, ch) in chars.into_iter().enumerate() {
            let base_name = Self::legacy_hud_base_name(kind, prefix, ch);
            let (texture_id, draw_width, draw_height) = self
                .resolve_legacy_hud_glyph(base_name.clone())
                .unwrap_or((base_name, fixed_digit_advance.max(1.0), 1.0));
            let texture_id = if kind == LegacyHudTextKind::Accuracy
                && ch == '%'
                && Self::is_invalid_accuracy_percent_glyph(
                    draw_width,
                    draw_height,
                    fixed_digit_advance,
                    fixed_digit_height,
                ) {
                // Mark invalid percent glyphs as missing so layout stays stable without drawing wrong art.
                format!("__missing_{texture_id}")
            } else {
                texture_id
            };
            let advance_width = if ch.is_ascii_digit() {
                fixed_digit_advance.max(1.0)
            } else {
                draw_width.max(1.0)
            };
            let x_offset = if ch.is_ascii_digit() {
                (advance_width - draw_width) / 2.0
            } else {
                0.0
            };
            glyphs.push(LegacyHudRawGlyphPlacement {
                texture_id,
                x: cursor_x + x_offset,
                y: 0.0,
                width: draw_width.max(1.0),
                height: draw_height.max(1.0),
            });
            max_height = max_height.max(draw_height.max(1.0));
            cursor_x += advance_width;
            if index + 1 < char_count {
                cursor_x -= overlap;
            }
        }
        LegacyHudRawTextPlan {
            layout: LegacyHudRawTextLayout {
                width: cursor_x.max(0.0),
                height: max_height.max(1.0),
            },
            glyphs,
        }
    }
    fn compose_legacy_text_plan(
        &self,
        raw_plan: &LegacyHudRawTextPlan,
        scale_x: f32,
        scale_y: f32,
        right_x: f32,
        top: f32,
    ) -> LegacyHudTextPlan {
        let scaled_layout =
            self.measure_legacy_screen_layout(raw_plan.layout, scale_x, scale_y, right_x, top);
        let mut glyphs = Vec::with_capacity(raw_plan.glyphs.len());
        let mut min_x = right_x.round() as i32;
        let mut min_y = top.round() as i32;
        let mut max_x = scaled_layout.left.round() as i32;
        let mut max_y = scaled_layout.top.round() as i32;
        for glyph in &raw_plan.glyphs {
            let precise_width = (glyph.width * scale_x).max(1.0);
            let precise_height = (glyph.height * scale_y).max(1.0);
            let precise_x = scaled_layout.left + glyph.x * scale_x;
            let precise_y = scaled_layout.top + glyph.y * scale_y;
            let draw_width = (glyph.width * scale_x).round().max(1.0) as u32;
            let draw_height = (glyph.height * scale_y).round().max(1.0) as u32;
            let x = precise_x.round() as i32;
            let y = precise_y.round() as i32;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + draw_width as i32);
            max_y = max_y.max(y + draw_height as i32);
            glyphs.push(LegacyHudGlyphPlacement {
                texture_id: glyph.texture_id.clone(),
                x,
                y,
                width: draw_width,
                height: draw_height,
                precise_x,
                precise_y,
                precise_width,
                precise_height,
            });
        }
        LegacyHudTextPlan {
            layout: LegacyHudTextLayout {
                left: min_x,
                top: min_y,
                width: (max_x - min_x).max(0) as u32,
                height: (max_y - min_y).max(0) as u32,
            },
            glyphs,
        }
    }
    pub(crate) fn measure_legacy_screen_layout(
        &self,
        raw_layout: LegacyHudRawTextLayout,
        scale_x: f32,
        scale_y: f32,
        right_x: f32,
        top: f32,
    ) -> LegacyHudScreenTextLayout {
        let width = (raw_layout.width * scale_x).max(0.0);
        let height = (raw_layout.height * scale_y).max(0.0);
        let left = right_x - width;
        LegacyHudScreenTextLayout {
            left,
            top,
            width,
            height,
        }
    }
    pub(crate) fn hud_element_runtime_rotation(
        &self,
        config: Option<&HudElementConfig>,
        timestamp_ms: i32,
    ) -> f32 {
        let Some(config) = config else {
            return 0.0;
        };
        let base = config.rotation.unwrap_or(0.0);
        if config.spin_enabled.unwrap_or(false) {
            let speed = config
                .spin_speed
                .filter(|value| value.is_finite())
                .unwrap_or(90.0);
            base + timestamp_ms.max(0) as f32 / 1000.0 * speed
        } else {
            base
        }
    }
    pub(crate) fn rotate_legacy_sprite_around_bounds(
        sprite: &mut SpriteCommand,
        left: f32,
        top: f32,
        width: f32,
        height: f32,
        degrees: f32,
    ) {
        if degrees.abs() <= f32::EPSILON {
            return;
        }
        // Preserve each sprite's own rotation, then rotate its anchor around the HUD group bounds.
        let group_rotation = degrees.to_radians();
        let base_rotation = sprite.rotation;
        let total_rotation = base_rotation + group_rotation;
        let position = sprite
            .precise_position
            .unwrap_or([sprite.x as f32, sprite.y as f32]);
        let origin = sprite.origin;
        let center = [left + width * 0.5, top + height * 0.5];
        let rotate = |point: [f32; 2], radians: f32| -> [f32; 2] {
            let (sin, cos) = radians.sin_cos();
            [
                point[0] * cos - point[1] * sin,
                point[0] * sin + point[1] * cos,
            ]
        };
        let base_rotated_origin = rotate(origin, base_rotation);
        let base_anchor = [
            position[0] + origin[0] - base_rotated_origin[0],
            position[1] + origin[1] - base_rotated_origin[1],
        ];
        let group_offset = [base_anchor[0] - center[0], base_anchor[1] - center[1]];
        let rotated_group_offset = rotate(group_offset, group_rotation);
        let transformed_anchor = [
            center[0] + rotated_group_offset[0],
            center[1] + rotated_group_offset[1],
        ];
        let total_rotated_origin = rotate(origin, total_rotation);
        let transformed_position = [
            transformed_anchor[0] - origin[0] + total_rotated_origin[0],
            transformed_anchor[1] - origin[1] + total_rotated_origin[1],
        ];
        sprite.x = transformed_position[0].round() as i32;
        sprite.y = transformed_position[1].round() as i32;
        sprite.precise_position = Some(transformed_position);
        sprite.rotation = total_rotation;
    }
    fn emit_legacy_text_plan(
        &self,
        planner: &mut SpritePlanner,
        plan: &LegacyHudTextPlan,
        z_order: i32,
        rotation_degrees: f32,
    ) {
        for glyph in &plan.glyphs {
            if self.has_texture(&glyph.texture_id) {
                let mut sprite = SpriteCommand {
                    texture_id: glyph.texture_id.clone(),
                    x: glyph.x,
                    y: glyph.y,
                    width: glyph.width,
                    height: glyph.height,
                    precise_position: Some([glyph.precise_x, glyph.precise_y]),
                    precise_size: Some([glyph.precise_width, glyph.precise_height]),
                    tint: [1.0, 1.0, 1.0, 1.0],
                    z_order,
                    ..Default::default()
                };
                Self::rotate_legacy_sprite_around_bounds(
                    &mut sprite,
                    plan.layout.left as f32,
                    plan.layout.top as f32,
                    plan.layout.width as f32,
                    plan.layout.height as f32,
                    rotation_degrees,
                );
                planner.add_sprite(sprite);
            }
        }
    }
    pub(crate) fn measure_score_raw_layout(
        &self,
        skin: &SkinAssets,
        score: u64,
    ) -> LegacyHudRawTextLayout {
        let score_prefix = skin.config.score_prefix_or_default();
        let score_str = format!("{:07}", score);
        let overlap = self.legacy_overlap_width(skin.config.score_overlap.unwrap_or(0));
        self.build_legacy_raw_text_plan(&score_str, LegacyHudTextKind::Score, score_prefix, overlap)
            .layout
    }
    pub(crate) fn plan_score(
        &self,
        planner: &mut SpritePlanner,
        skin: &SkinAssets,
        score: u64,
        scale_x: f32,
        scale_y: f32,
        right_x: f32,
        top: f32,
        rotation_degrees: f32,
    ) -> LegacyHudTextLayout {
        let score_prefix = skin.config.score_prefix_or_default();
        let score_str = format!("{:07}", score);
        let overlap = self.legacy_overlap_width(skin.config.score_overlap.unwrap_or(0));
        let raw_plan = self.build_legacy_raw_text_plan(
            &score_str,
            LegacyHudTextKind::Score,
            score_prefix,
            overlap,
        );
        let plan = self.compose_legacy_text_plan(&raw_plan, scale_x, scale_y, right_x, top);
        self.emit_legacy_text_plan(planner, &plan, z_order::HUD, rotation_degrees);
        plan.layout
    }
    pub(crate) fn measure_accuracy_raw_layout(
        &self,
        skin: &SkinAssets,
        accuracy: f64,
    ) -> LegacyHudRawTextLayout {
        let score_prefix = skin.config.score_prefix_or_default();
        let acc_str = format!("{:.2}%", accuracy * 100.0);
        let overlap = self.legacy_overlap_width(skin.config.score_overlap.unwrap_or(0));
        self.build_legacy_raw_text_plan(
            &acc_str,
            LegacyHudTextKind::Accuracy,
            score_prefix,
            overlap,
        )
        .layout
    }
    pub(crate) fn measure_accuracy_layout(
        &self,
        skin: &SkinAssets,
        accuracy: f64,
        scale_x: f32,
        scale_y: f32,
        right_x: f32,
        acc_top: f32,
    ) -> LegacyHudTextLayout {
        let score_prefix = skin.config.score_prefix_or_default();
        let acc_str = format!("{:.2}%", accuracy * 100.0);
        let overlap = self.legacy_overlap_width(skin.config.score_overlap.unwrap_or(0));
        let raw_plan = self.build_legacy_raw_text_plan(
            &acc_str,
            LegacyHudTextKind::Accuracy,
            score_prefix,
            overlap,
        );
        self.compose_legacy_text_plan(&raw_plan, scale_x, scale_y, right_x, acc_top)
            .layout
    }
    pub(crate) fn plan_accuracy(
        &self,
        planner: &mut SpritePlanner,
        skin: &SkinAssets,
        accuracy: f64,
        scale_x: f32,
        scale_y: f32,
        right_x: f32,
        acc_top: f32,
        rotation_degrees: f32,
    ) -> LegacyHudTextLayout {
        let score_prefix = skin.config.score_prefix_or_default();
        let acc_str = format!("{:.2}%", accuracy * 100.0);
        let overlap = self.legacy_overlap_width(skin.config.score_overlap.unwrap_or(0));
        let raw_plan = self.build_legacy_raw_text_plan(
            &acc_str,
            LegacyHudTextKind::Accuracy,
            score_prefix,
            overlap,
        );
        let plan = self.compose_legacy_text_plan(&raw_plan, scale_x, scale_y, right_x, acc_top);
        self.emit_legacy_text_plan(planner, &plan, z_order::HUD + 1, rotation_degrees);
        plan.layout
    }
    pub fn plan_progress_circle(
        &self,
        planner: &mut SpritePlanner,
        progress: f64,
        x: f32,
        y: f32,
        size: f32,
        rotation_degrees: f32,
    ) {
        let pct = ((progress * 100.0).round() as u32).min(100);
        // Progress circles are pre-rendered in 1 percent increments by textures.rs.
        let tex_name = format!("progress_circle_{}", pct);
        if self.has_texture(&tex_name) {
            let mut sprite = SpriteCommand {
                texture_id: tex_name,
                x: x.round() as i32,
                y: y.round() as i32,
                width: size.round().max(1.0) as u32,
                height: size.round().max(1.0) as u32,
                precise_position: Some([x, y]),
                precise_size: Some([size.max(1.0), size.max(1.0)]),
                tint: [1.0, 1.0, 1.0, 1.0],
                z_order: z_order::HUD + 2,
                ..Default::default()
            };
            Self::rotate_legacy_sprite_around_bounds(
                &mut sprite,
                x,
                y,
                size.max(1.0),
                size.max(1.0),
                rotation_degrees,
            );
            planner.add_sprite(sprite);
        }
    }
    pub(crate) fn measure_replay_mod_display_layout(
        &self,
        right_x: f32,
        top_y: f32,
        config: Option<&HudElementConfig>,
    ) -> Option<LegacyHudScreenTextLayout> {
        self.build_replay_mod_display_plan(right_x, top_y, config)
            .map(|plan| plan.layout)
    }
    pub(crate) fn plan_replay_mod_display(
        &self,
        planner: &mut SpritePlanner,
        right_x: f32,
        top_y: f32,
        config: Option<&HudElementConfig>,
        timestamp_ms: i32,
    ) -> Option<LegacyHudScreenTextLayout> {
        let plan = self.build_replay_mod_display_plan(right_x, top_y, config)?;
        let rotation_degrees = self.hud_element_runtime_rotation(config, timestamp_ms);
        for icon in &plan.icons {
            let mut sprite = SpriteCommand {
                texture_id: icon.texture_id.clone(),
                x: icon.left.round() as i32,
                y: icon.top.round() as i32,
                width: icon.width.round().max(1.0) as u32,
                height: icon.height.round().max(1.0) as u32,
                precise_position: Some([icon.left, icon.top]),
                precise_size: Some([icon.width.max(1.0), icon.height.max(1.0)]),
                tint: [1.0, 1.0, 1.0, 1.0],
                z_order: z_order::HUD + 3,
                ..Default::default()
            };
            Self::rotate_legacy_sprite_around_bounds(
                &mut sprite,
                plan.layout.left,
                plan.layout.top,
                plan.layout.width,
                plan.layout.height,
                rotation_degrees,
            );
            planner.add_sprite(sprite);
        }
        Some(plan.layout)
    }
    pub fn plan_combo(
        &self,
        planner: &mut SpritePlanner,
        skin: &SkinAssets,
        combo: u32,
        combo_h: u32,
        center_x: i32,
        combo_pos_y: i32,
        stretch_scale_y: f32,
        rotation_degrees: f32,
    ) {
        if combo == 0 {
            return;
        }
        let combo_prefix = skin.config.combo_prefix_or_default();
        let combo_str = combo.to_string();
        let mut widths = Vec::with_capacity(combo_str.len());
        let mut texture_ids = Vec::with_capacity(combo_str.len());
        for ch in combo_str.chars() {
            let base_name = format!("{}-{}.png", combo_prefix, ch).to_lowercase();
            let scaled_name = format!("{}@hud_combo_{}", base_name, combo_h);
            let (tex_name, w) = if self.has_texture(&scaled_name) {
                let w = self
                    .texture_meta
                    .get(&scaled_name)
                    .map(|m| m.w)
                    .unwrap_or(combo_h);
                (scaled_name, w)
            } else {
                let w = self
                    .texture_meta
                    .get(&base_name)
                    .map(|m| scaled_hud_texture_width(*m, combo_h))
                    .unwrap_or((combo_h as f32 * 0.8).round() as u32);
                (base_name, w)
            };
            widths.push(w);
            texture_ids.push(tex_name);
        }
        let overlap = skin
            .config
            .combo_overlap
            .or(skin.config.score_overlap)
            .unwrap_or(0);
        let overlap_scaled = {
            let zero_name = format!("{}-0.png", combo_prefix).to_lowercase();
            self.texture_meta
                .get(&zero_name)
                .map(|m| (overlap as f32 * (combo_h as f32 / m.h as f32)).round() as i32)
                .unwrap_or(0)
        };
        let total_w: i32 = widths.iter().map(|&w| w as i32).sum::<i32>()
            - overlap_scaled * (widths.len().saturating_sub(1) as i32);
        let stretched_h = (combo_h as f32 * stretch_scale_y).round() as u32;
        let top = combo_pos_y - (stretched_h as i32 / 2);
        let left = center_x - (total_w / 2);
        let bounds_left = left as f32;
        let bounds_top = top as f32;
        let bounds_width = total_w.max(1) as f32;
        let bounds_height = stretched_h.max(1) as f32;
        let mut x = left;
        for (i, _) in combo_str.chars().enumerate() {
            let tex_name = &texture_ids[i];
            let w = widths[i];
            if self.has_texture(tex_name) {
                let mut sprite = SpriteCommand {
                    texture_id: tex_name.clone(),
                    x,
                    y: top,
                    width: w,
                    height: stretched_h,
                    precise_position: Some([x as f32, top as f32]),
                    precise_size: Some([w as f32, stretched_h.max(1) as f32]),
                    tint: [1.0, 1.0, 1.0, 1.0],
                    z_order: z_order::HUD + 2,
                    ..Default::default()
                };
                Self::rotate_legacy_sprite_around_bounds(
                    &mut sprite,
                    bounds_left,
                    bounds_top,
                    bounds_width,
                    bounds_height,
                    rotation_degrees,
                );
                planner.add_sprite(sprite);
            }
            x += w as i32 - overlap_scaled;
        }
    }
    pub(crate) fn measure_combo_layout(
        &self,
        skin: &SkinAssets,
        combo: u32,
        combo_h: u32,
        center_x: i32,
        combo_pos_y: i32,
        stretch_scale_y: f32,
    ) -> Option<LegacyHudTextLayout> {
        if combo == 0 {
            return None;
        }
        let combo_prefix = skin.config.combo_prefix_or_default();
        let combo_str = combo.to_string();
        let mut widths = Vec::with_capacity(combo_str.len());
        for ch in combo_str.chars() {
            let base_name = format!("{}-{}.png", combo_prefix, ch).to_lowercase();
            let scaled_name = format!("{}@hud_combo_{}", base_name, combo_h);
            let w = if self.has_texture(&scaled_name) {
                self.texture_meta
                    .get(&scaled_name)
                    .map(|m| m.w)
                    .unwrap_or(combo_h)
            } else {
                self.texture_meta
                    .get(&base_name)
                    .map(|m| scaled_hud_texture_width(*m, combo_h))
                    .unwrap_or((combo_h as f32 * 0.8).round() as u32)
            };
            widths.push(w);
        }
        let overlap = skin
            .config
            .combo_overlap
            .or(skin.config.score_overlap)
            .unwrap_or(0);
        let overlap_scaled = {
            let zero_name = format!("{}-0.png", combo_prefix).to_lowercase();
            self.texture_meta
                .get(&zero_name)
                .map(|m| (overlap as f32 * (combo_h as f32 / m.h as f32)).round() as i32)
                .unwrap_or(0)
        };
        let total_w: i32 = widths.iter().map(|&w| w as i32).sum::<i32>()
            - overlap_scaled * (widths.len().saturating_sub(1) as i32);
        let stretched_h = (combo_h as f32 * stretch_scale_y).round().max(1.0) as u32;
        Some(LegacyHudTextLayout {
            left: center_x - total_w / 2,
            top: combo_pos_y - stretched_h as i32 / 2,
            width: total_w.max(1) as u32,
            height: stretched_h,
        })
    }
    pub fn plan_combo_break(
        &self,
        planner: &mut SpritePlanner,
        skin: &SkinAssets,
        start_combo: u32,
        elapsed_ms: i32,
        combo_h: u32,
        center_x: i32,
        combo_pos_y: i32,
        _scale_y: f32,
        rotation_degrees: f32,
    ) {
        const ANIM_DURATION: i32 = 380;
        if !(0..ANIM_DURATION).contains(&elapsed_ms) {
            return;
        }
        let progress = elapsed_ms as f32 / ANIM_DURATION as f32;
        let peak = 0.35f32;
        let scale = if progress < peak {
            let p = progress / peak;
            let ease = if p < 0.5 {
                2.0 * p * p
            } else {
                1.0 - (-2.0 * p + 2.0).powi(2) / 2.0
            };
            1.0 + ease
        } else {
            let p = (progress - peak) / (1.0 - peak);
            2.0 - p * p
        };
        let opacity = 0.5 * (1.0 - progress);
        if opacity < 0.01 {
            return;
        }
        let combo_prefix = skin.config.combo_prefix_or_default();
        let combo_str = start_combo.to_string();
        let mut widths = Vec::with_capacity(combo_str.len());
        for ch in combo_str.chars() {
            let base_tex = format!("{}-{}.png", combo_prefix, ch).to_lowercase();
            let scaled_tex = format!("{}@hud_combo_{}", base_tex, combo_h);
            let tex_name = if self.has_texture(&scaled_tex) {
                scaled_tex.as_str()
            } else {
                base_tex.as_str()
            };
            let w = self
                .texture_meta
                .get(tex_name)
                .map(|m| scaled_hud_texture_width(*m, combo_h))
                .unwrap_or((combo_h as f32 * 0.8).round() as u32);
            widths.push(w);
        }
        let overlap = skin
            .config
            .combo_overlap
            .or(skin.config.score_overlap)
            .unwrap_or(0);
        let overlap_scaled = {
            let zero_name = format!("{}-0.png", combo_prefix).to_lowercase();
            let scaled_zero_name = format!("{}@hud_combo_{}", zero_name, combo_h);
            let zero_lookup = if self.has_texture(&scaled_zero_name) {
                scaled_zero_name.as_str()
            } else {
                zero_name.as_str()
            };
            self.texture_meta
                .get(zero_lookup)
                .map(|m| (overlap as f32 * (combo_h as f32 / m.h as f32)).round() as i32)
                .unwrap_or(0)
        };
        let total_w: i32 = widths.iter().map(|&w| w as i32).sum::<i32>()
            - overlap_scaled * (widths.len().saturating_sub(1) as i32);
        let scaled_h = (combo_h as f32 * scale).round() as u32;
        let scaled_w = (total_w as f32 * scale).round() as i32;
        let scaled_overlap = (overlap_scaled as f32 * scale).round() as i32;
        let top = combo_pos_y - (scaled_h as i32 / 2);
        let left = center_x - (scaled_w / 2);
        let bounds_left = left as f32;
        let bounds_top = top as f32;
        let bounds_width = scaled_w.max(1) as f32;
        let bounds_height = scaled_h.max(1) as f32;
        let mut x = left;
        for (i, ch) in combo_str.chars().enumerate() {
            let base_tex = format!("{}-{}.png", combo_prefix, ch).to_lowercase();
            let scaled_base_tex = format!("{}@hud_combo_{}", base_tex, combo_h);
            let preferred_base_tex = if self.has_texture(&scaled_base_tex) {
                scaled_base_tex
            } else {
                base_tex.clone()
            };
            let cached_red_tex = self
                .combo_break_red_cache
                .get(&preferred_base_tex)
                .or_else(|| self.combo_break_red_cache.get(&base_tex))
                .filter(|n| self.has_texture(n))
                .cloned();
            let (tex_name, tint): (String, Tint) = if let Some(red_tex) = cached_red_tex {
                (red_tex, [1.0, 1.0, 1.0, opacity])
            } else {
                (preferred_base_tex, [1.0, 0.0, 0.0, opacity])
            };
            let char_w = (widths[i] as f32 * scale).round() as u32;
            if self.has_texture(&tex_name) {
                let mut sprite = SpriteCommand {
                    texture_id: tex_name,
                    x,
                    y: top,
                    width: char_w,
                    height: scaled_h,
                    precise_position: Some([x as f32, top as f32]),
                    precise_size: Some([char_w as f32, scaled_h.max(1) as f32]),
                    tint,
                    z_order: z_order::HUD + 4,
                    ..Default::default()
                };
                Self::rotate_legacy_sprite_around_bounds(
                    &mut sprite,
                    bounds_left,
                    bounds_top,
                    bounds_width,
                    bounds_height,
                    rotation_degrees,
                );
                planner.add_sprite(sprite);
            }
            x += char_w as i32 - scaled_overlap;
        }
    }
    pub fn plan_combo_burst(
        &self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        combo: u32,
        elapsed_ms: i32,
    ) {
        if !(0..super::super::state::anim::COMBO_BURST_ANIM_MS).contains(&elapsed_ms) {
            return;
        }
        if skin.config.combo_burst.is_empty() {
            return;
        }
        let milestone_index = combo.saturating_div(100).saturating_sub(1) as usize;
        // Random combo bursts must be deterministic for a given replay render.
        let side_seed = hash_mix(self.random_seed, combo as u64);
        let image_seed = hash_mix(side_seed, milestone_index as u64);
        let texture_index = if skin.config.resolved_combo_burst_random() {
            (image_seed as usize) % skin.config.combo_burst.len()
        } else {
            milestone_index % skin.config.combo_burst.len()
        };
        let texture_name = skin
            .config
            .combo_burst
            .get(texture_index)
            .filter(|name| self.has_texture(name))
            .cloned();
        let Some(texture_name) = texture_name else {
            return;
        };
        let Some(meta) = self.texture_meta.get(&texture_name) else {
            return;
        };
        let side_left = match skin.config.resolved_combo_burst_style() {
            crate::types::ComboBurstStyle::Left => true,
            crate::types::ComboBurstStyle::Right => false,
            crate::types::ComboBurstStyle::Both => (side_seed & 1) == 0,
        };
        let draw_texture_id = self.prefer_hidpi_texture_id(&texture_name);
        let draw_meta = self
            .texture_meta
            .get(&draw_texture_id)
            .copied()
            .unwrap_or(*meta);
        let (display_width, display_height) =
            Self::legacy_texture_display_size(draw_meta, &draw_texture_id);
        let base_scale = combo_burst_stage_scale(layout);
        let width = (display_width * base_scale).round().max(1.0) as u32;
        let height = (display_height * base_scale).round().max(1.0) as u32;
        let bottom = layout.stage.bottom_y - height as i32;
        let move_duration_ms = 900.0f32;
        let move_t = (elapsed_ms as f32 / move_duration_ms).clamp(0.0, 1.0);
        let move_t_eased = 1.0 - (1.0 - move_t) * (1.0 - move_t);
        let fade_in = (elapsed_ms as f32 / 120.0).clamp(0.0, 1.0);
        let alpha = (1.0 - move_t) * fade_in;
        if alpha <= 0.0 {
            return;
        }
        let center_x = layout.stage.x + layout.stage.width as i32 / 2;
        let start_x = if side_left {
            center_x - width as i32
        } else {
            center_x
        };
        let target_x = if side_left {
            -(width as i32 / 2)
        } else {
            self.combo_burst_right_target_x(layout)
                .unwrap_or_else(|| self.cfg.width as i32 - width as i32 / 2)
        };
        let x = start_x + ((target_x - start_x) as f32 * move_t_eased).round() as i32;
        let uv_rect = if side_left {
            [0.0, 0.0, 1.0, 1.0]
        } else {
            [1.0, 0.0, 0.0, 1.0]
        };
        planner.add_sprite(SpriteCommand {
            texture_id: draw_texture_id,
            x,
            y: bottom,
            width,
            height,
            tint: [1.0, 1.0, 1.0, alpha],
            uv_rect,
            z_order: z_order::COMBO_BURST,
            ..Default::default()
        });
    }
    fn combo_burst_right_target_x(&self, layout: &ManiaLayoutInfo) -> Option<i32> {
        let fill_texture = self.find_first_texture(&[
            "scorebar-colour.png",
            "scorebar-colour.jpg",
            "scorebar-colour",
            "scorebar-colour-0.png",
            "scorebar-colour-0.jpg",
            "scorebar-colour-0",
        ])?;
        let fill_meta = self.texture_meta.get(&fill_texture).copied()?;
        let fill_bounds = drawable_life_bar_bounds(
            fill_meta,
            self.texture_opaque_bounds
                .get(&fill_texture)
                .copied()
                .or_else(|| Some(default_opaque_bounds(fill_meta))),
        )?;
        let life_bar_cfg = self
            .hud_config
            .as_ref()
            .and_then(|cfg| cfg.elements.life_bar.as_ref());
        let lane_anchor = compute_life_bar_lane_anchor(layout, life_bar_cfg);
        let new_style_marker = self.find_first_texture(&[
            "scorebar-marker.png",
            "scorebar-marker.jpg",
            "scorebar-marker",
        ]);
        let scorebar_style = if new_style_marker.is_some() {
            LegacyScorebarStyle::New
        } else {
            LegacyScorebarStyle::Old
        };
        let background_drawable = self
            .find_first_texture(&["scorebar-bg.png", "scorebar-bg.jpg", "scorebar-bg"])
            .and_then(|texture_id| {
                let meta = self.texture_meta.get(&texture_id).copied()?;
                let bounds = drawable_life_bar_bounds(
                    meta,
                    self.texture_opaque_bounds
                        .get(&texture_id)
                        .copied()
                        .or_else(|| Some(default_opaque_bounds(meta))),
                )?;
                Some((meta, bounds))
            });
        let scorebar_layout = legacy_scorebar_layout(scorebar_style);
        let (anchor_bounds, anchor_offset_y) = match (scorebar_style, background_drawable) {
            (LegacyScorebarStyle::Old, Some((_, bounds))) => (bounds, 0.0),
            _ => (fill_bounds, scorebar_layout.fill_offset_y),
        };
        let (scale_x, scale_y) =
            compute_life_bar_axis_scale(self.cfg.height, life_bar_cfg, fill_bounds);
        let container_origin = compute_scorebar_container_origin(
            lane_anchor,
            anchor_bounds,
            scorebar_layout.fill_offset_x,
            anchor_offset_y,
            scale_x,
            scale_y,
        );
        let (outer_meta, outer_bounds, local_y) = match (scorebar_style, background_drawable) {
            (LegacyScorebarStyle::Old, Some((meta, bounds))) => (meta, bounds, 0.0),
            _ => (fill_meta, fill_bounds, scorebar_layout.fill_offset_y),
        };
        let outer_transform = compute_rotated_scorebar_piece(
            container_origin,
            0.0,
            local_y,
            outer_meta,
            outer_bounds,
            scale_x,
            scale_y,
            outer_meta.w as f32,
        );
        Some((outer_transform.visible_left + outer_transform.visible_w).round() as i32)
    }
    pub fn plan_judgment_popup(
        &self,
        planner: &mut SpritePlanner,
        skin: &SkinAssets,
        kind: JudgmentKind,
        age_ms: i32,
        center_x: i32,
        hit_y: i32,
        scale_y: f32,
        anim_fps: u32,
        center_y_override: Option<i32>,
        popup_scale: f32,
        target_height: Option<u32>,
        rotation_degrees: f32,
    ) {
        let (configured_name, default_name) = match kind {
            JudgmentKind::Miss => (skin.config.hit_0.as_deref(), "mania-hit0.png"),
            JudgmentKind::Hit50 => (skin.config.hit_50.as_deref(), "mania-hit50.png"),
            JudgmentKind::Hit100 => (skin.config.hit_100.as_deref(), "mania-hit100.png"),
            JudgmentKind::Hit200 => (skin.config.hit_200.as_deref(), "mania-hit200.png"),
            JudgmentKind::Hit300 => (skin.config.hit_300.as_deref(), "mania-hit300.png"),
            JudgmentKind::Max => (skin.config.hit_300g.as_deref(), "mania-hit300g.png"),
        };
        let base_name = match configured_name {
            Some("") => return,
            Some(name) => name,
            None => default_name,
        };
        let base = base_name
            .trim_end_matches(".png")
            .trim_end_matches(".jpg")
            .to_lowercase();
        let frame_time = 1000.0 / anim_fps.max(1) as f32;
        let mut total_frames = 0u32;
        loop {
            let name = format!("{}-{}.png", base, total_frames);
            let scaled_check = format!("{}@judgment", name);
            if !self.has_texture(&scaled_check) && !self.has_texture(&name) {
                break;
            }
            total_frames += 1;
            // Cap frame probing so a malformed skin cannot keep the renderer scanning forever.
            if total_frames > 100 {
                break;
            }
        }
        let (tex_name, max_age) = if total_frames > 0 {
            let frame_idx = ((age_ms as f32 / frame_time) as u32).min(total_frames - 1);
            let name = format!("{}-{}.png", base, frame_idx);
            (name, (total_frames as f32 * frame_time) as i32)
        } else {
            (format!("{}.png", base), 200)
        };
        if age_ms >= max_age {
            return;
        }
        let scaled_name = format!("{}@judgment", tex_name);
        let final_tex = if self.has_texture(&scaled_name) {
            scaled_name
        } else if self.has_texture(&tex_name) {
            tex_name.clone()
        } else {
            return;
        };
        let meta = match self.texture_meta.get(&final_tex) {
            Some(m) => m,
            None => return,
        };
        let age = age_ms as f32;
        let max = max_age as f32;
        let half = max / 2.0;
        let s = if age < half {
            1.0 + 0.15 * (age / half)
        } else {
            1.15 - 0.15 * ((age - half) / half)
        };
        let size_scale = if popup_scale.is_finite() && popup_scale > 0.0 {
            popup_scale
        } else {
            1.0
        };
        let mut effective_scale = s * size_scale;
        if let Some(target_h) = target_height.filter(|h| *h > 0) {
            effective_scale *= target_h as f32 / meta.h.max(1) as f32;
        }
        let w = (meta.w as f32 * effective_scale).round().max(1.0) as u32;
        let h = (meta.h as f32 * effective_scale).round().max(1.0) as u32;
        let left = center_x - (w as i32 / 2);
        let top = if let Some(center_y) = center_y_override {
            center_y - (h as i32 / 2)
        } else if let Some(score_y) = skin.config.score_y {
            (score_y as f32 * scale_y).round() as i32 - (h as i32 / 2)
        } else {
            (hit_y as f32 - h as f32 * 0.6).round() as i32
        };
        let opacity = 1.0;
        let mut sprite = SpriteCommand {
            texture_id: final_tex,
            x: left,
            y: top,
            width: w,
            height: h,
            precise_position: Some([left as f32, top as f32]),
            precise_size: Some([w as f32, h as f32]),
            tint: [1.0, 1.0, 1.0, opacity],
            z_order: z_order::JUDGMENT_POPUP,
            ..Default::default()
        };
        Self::rotate_legacy_sprite_around_bounds(
            &mut sprite,
            left as f32,
            top as f32,
            w as f32,
            h as f32,
            rotation_degrees,
        );
        planner.add_sprite(sprite);
    }
    pub(crate) fn measure_judgment_popup_layout(
        &self,
        skin: &SkinAssets,
        kind: JudgmentKind,
        age_ms: i32,
        center_x: i32,
        hit_y: i32,
        scale_y: f32,
        anim_fps: u32,
        center_y_override: Option<i32>,
        popup_scale: f32,
        target_height: Option<u32>,
    ) -> Option<LegacyHudTextLayout> {
        let (configured_name, default_name) = match kind {
            JudgmentKind::Miss => (skin.config.hit_0.as_deref(), "mania-hit0.png"),
            JudgmentKind::Hit50 => (skin.config.hit_50.as_deref(), "mania-hit50.png"),
            JudgmentKind::Hit100 => (skin.config.hit_100.as_deref(), "mania-hit100.png"),
            JudgmentKind::Hit200 => (skin.config.hit_200.as_deref(), "mania-hit200.png"),
            JudgmentKind::Hit300 => (skin.config.hit_300.as_deref(), "mania-hit300.png"),
            JudgmentKind::Max => (skin.config.hit_300g.as_deref(), "mania-hit300g.png"),
        };
        let base_name = match configured_name {
            Some("") => return None,
            Some(name) => name,
            None => default_name,
        };
        let base = base_name
            .trim_end_matches(".png")
            .trim_end_matches(".jpg")
            .to_lowercase();
        let frame_time = 1000.0 / anim_fps.max(1) as f32;
        let mut total_frames = 0u32;
        loop {
            let name = format!("{}-{}.png", base, total_frames);
            let scaled_check = format!("{}@judgment", name);
            if !self.has_texture(&scaled_check) && !self.has_texture(&name) {
                break;
            }
            total_frames += 1;
            if total_frames > 100 {
                break;
            }
        }
        let (tex_name, max_age) = if total_frames > 0 {
            let frame_idx = ((age_ms as f32 / frame_time) as u32).min(total_frames - 1);
            (
                format!("{}-{}.png", base, frame_idx),
                (total_frames as f32 * frame_time) as i32,
            )
        } else {
            (format!("{}.png", base), 200)
        };
        if age_ms >= max_age {
            return None;
        }
        let scaled_name = format!("{}@judgment", tex_name);
        let final_tex = if self.has_texture(&scaled_name) {
            scaled_name
        } else if self.has_texture(&tex_name) {
            tex_name
        } else {
            return None;
        };
        let meta = self.texture_meta.get(&final_tex)?;
        let age = age_ms as f32;
        let max = max_age as f32;
        let half = max / 2.0;
        let s = if age < half {
            1.0 + 0.15 * (age / half)
        } else {
            1.15 - 0.15 * ((age - half) / half)
        };
        let size_scale = if popup_scale.is_finite() && popup_scale > 0.0 {
            popup_scale
        } else {
            1.0
        };
        let mut effective_scale = s * size_scale;
        if let Some(target_h) = target_height.filter(|h| *h > 0) {
            effective_scale *= target_h as f32 / meta.h.max(1) as f32;
        }
        let width = (meta.w as f32 * effective_scale).round().max(1.0) as u32;
        let height = (meta.h as f32 * effective_scale).round().max(1.0) as u32;
        let left = center_x - width as i32 / 2;
        let top = if let Some(center_y) = center_y_override {
            center_y - height as i32 / 2
        } else if let Some(score_y) = skin.config.score_y {
            (score_y as f32 * scale_y).round() as i32 - height as i32 / 2
        } else {
            (hit_y as f32 - height as f32 * 0.6).round() as i32
        };
        Some(LegacyHudTextLayout {
            left,
            top,
            width,
            height,
        })
    }
    pub fn calc_combo_stretch(elapsed_ms: i32) -> f32 {
        const DURATION: i32 = 120;
        if !(0..DURATION).contains(&elapsed_ms) {
            return 1.0;
        }
        let progress = elapsed_ms as f32 / DURATION as f32;
        1.0 + 0.14 * (progress * std::f32::consts::PI).sin()
    }
}
fn default_opaque_bounds(meta: TextureMeta) -> OpaqueBounds {
    OpaqueBounds {
        min_x: 0,
        min_y: 0,
        max_x: meta.w.saturating_sub(1),
        max_y: meta.h.saturating_sub(1),
    }
}
fn drawable_life_bar_bounds(
    meta: TextureMeta,
    bounds: Option<OpaqueBounds>,
) -> Option<OpaqueBounds> {
    if meta.w <= 2 && meta.h <= 2 {
        return None;
    }
    bounds
}
fn scaled_hud_texture_width(meta: TextureMeta, target_h: u32) -> u32 {
    if meta.h == 0 || target_h == 0 {
        return 0;
    }
    ((meta.w as f64 / meta.h as f64) * target_h as f64)
        .round()
        .clamp(1.0, crate::utils::image_proc::MAX_TEXTURE_DIM as f64) as u32
}
fn legacy_animation_frame_texture(base_name: &str, frame_idx: usize) -> [String; 2] {
    [
        format!("{base_name}-{frame_idx}.png"),
        format!("{base_name}-{frame_idx}.jpg"),
    ]
}
fn hidpi_texture_name(texture_id: &str) -> Option<String> {
    if texture_id.contains("@2x") {
        return None;
    }
    let (stem, ext) = texture_id.rsplit_once('.')?;
    Some(format!("{stem}@2x.{ext}"))
}
fn hash_mix(seed: u64, value: u64) -> u64 {
    let mut mixed = seed ^ value.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    mixed ^= mixed >> 30;
    mixed = mixed.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed ^= mixed >> 27;
    mixed = mixed.wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^ (mixed >> 31)
}
fn legacy_animation_frame_length_ms(
    apply_config_frame_rate: bool,
    animation_framerate: Option<u32>,
    frame_count: usize,
) -> f64 {
    if apply_config_frame_rate {
        if let Some(framerate) = animation_framerate.filter(|framerate| *framerate > 0) {
            return 1000.0 / framerate as f64;
        }
        return 1000.0 / frame_count.max(1) as f64;
    }
    1000.0 / 60.0
}
fn legacy_animation_frame_index(
    timestamp_ms: f64,
    frame_length_ms: f64,
    frame_count: usize,
) -> usize {
    if frame_count == 0 {
        return 0;
    }
    let frame_length_ms = frame_length_ms.max(1.0);
    let epsilon_ms = frame_length_ms * 0.0001;
    // Tiny epsilon keeps exact frame-boundary timestamps from flickering to the previous frame.
    (((timestamp_ms.max(0.0) + epsilon_ms) / frame_length_ms).floor() as usize) % frame_count
}
fn compute_life_bar_axis_scale(
    canvas_h: u32,
    config: Option<&HudElementConfig>,
    bounds: OpaqueBounds,
) -> (f32, f32) {
    let thickness_override = config
        .and_then(|cfg| cfg.width)
        .filter(|value| value.is_finite() && *value > 0.0);
    let length_override = config
        .and_then(|cfg| cfg.height)
        .filter(|value| value.is_finite() && *value > 0.0);
    let base_scale = canvas_h as f32 / LIFE_BAR_LOGICAL_HEIGHT * LIFE_BAR_REPLAY_SCALE;
    let scale_x = length_override
        .map(|height| height / bounds.width().max(1) as f32)
        .unwrap_or(base_scale);
    let scale_y = thickness_override
        .map(|width| width / bounds.height().max(1) as f32)
        .unwrap_or(base_scale);
    let extra_scale = config
        .and_then(|cfg| cfg.scale.or(cfg.size))
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(1.0);
    (
        (scale_x * extra_scale).max(0.0001),
        (scale_y * extra_scale).max(0.0001),
    )
}
fn compute_life_bar_lane_anchor(
    layout: &ManiaLayoutInfo,
    config: Option<&HudElementConfig>,
) -> LifeBarLaneAnchor {
    let last_column_right = layout
        .columns
        .last()
        .map(|col| col.x + col.width as i32)
        .unwrap_or(layout.stage.x + layout.stage.width as i32);
    let visible_left_x = config
        .and_then(|cfg| cfg.x.filter(|value| value.is_finite()))
        .unwrap_or(last_column_right as f32 + LIFE_BAR_GAP_PX);
    let bottom_y = config
        .and_then(|cfg| cfg.y.filter(|value| value.is_finite()))
        .unwrap_or(layout.stage.bottom_y as f32);
    LifeBarLaneAnchor {
        visible_left_x,
        bottom_y,
    }
}
fn legacy_scorebar_layout(style: LegacyScorebarStyle) -> LegacyScorebarLayout {
    match style {
        LegacyScorebarStyle::Old => LegacyScorebarLayout {
            fill_offset_x: 3.0 * LEGACY_SCOREBAR_LAYOUT_SCALE,
            fill_offset_y: 10.0 * LEGACY_SCOREBAR_LAYOUT_SCALE,
            marker_center_y: false,
        },
        LegacyScorebarStyle::New => LegacyScorebarLayout {
            fill_offset_x: 7.5 * LEGACY_SCOREBAR_LAYOUT_SCALE,
            fill_offset_y: 7.8 * LEGACY_SCOREBAR_LAYOUT_SCALE,
            marker_center_y: true,
        },
    }
}
fn compute_scorebar_container_origin(
    lane_anchor: LifeBarLaneAnchor,
    anchor_bounds: OpaqueBounds,
    fill_offset_x: f32,
    anchor_offset_y: f32,
    scale_x: f32,
    scale_y: f32,
) -> ScorebarContainerOrigin {
    // The scorebar's local x axis becomes vertical after the quarter-turn rotation.
    ScorebarContainerOrigin {
        x: lane_anchor.visible_left_x - (anchor_offset_y + anchor_bounds.min_y as f32) * scale_y,
        y: lane_anchor.bottom_y + fill_offset_x * scale_x,
    }
}
fn compute_rotated_scorebar_piece(
    container_origin: ScorebarContainerOrigin,
    local_x: f32,
    local_y: f32,
    meta: TextureMeta,
    bounds: OpaqueBounds,
    scale_x: f32,
    scale_y: f32,
    source_x_end: f32,
) -> RotatedScorebarPlacement {
    let clamped_source_end = source_x_end.max(0.0).min(meta.w.max(1) as f32);
    let draw_x = container_origin.x + local_y * scale_y;
    let draw_y = container_origin.y - local_x * scale_x;
    let draw_w = (clamped_source_end * scale_x).round().max(1.0) as u32;
    let draw_h = (meta.h.max(1) as f32 * scale_y).round().max(1.0) as u32;
    let visible_source_end = clamped_source_end
        .max(bounds.min_x as f32)
        .min((bounds.max_x + 1) as f32);
    let visible_h = ((visible_source_end - bounds.min_x as f32).max(0.0) * scale_x).max(0.0);
    let visible_w = (bounds.height() as f32 * scale_y).max(1.0);
    let visible_left = draw_x + bounds.min_y as f32 * scale_y;
    let visible_top = draw_y - visible_source_end * scale_x;
    RotatedScorebarPlacement {
        draw_x: draw_x.round() as i32,
        draw_y: draw_y.round() as i32,
        draw_w,
        draw_h,
        uv_rect: [0.0, 0.0, clamped_source_end / meta.w.max(1) as f32, 1.0],
        visible_left,
        visible_top,
        visible_w,
        visible_h,
    }
}
fn compute_scorebar_marker_center(
    container_origin: ScorebarContainerOrigin,
    scorebar_layout: LegacyScorebarLayout,
    fill_transform: RotatedScorebarPlacement,
    scale_x: f32,
    scale_y: f32,
) -> (f32, f32) {
    let local_x = scorebar_layout.fill_offset_x * scale_x + fill_transform.draw_w as f32;
    let local_y = scorebar_layout.fill_offset_y * scale_y
        + if scorebar_layout.marker_center_y {
            fill_transform.draw_h as f32 / 2.0
        } else {
            0.0
        };
    (container_origin.x + local_y, container_origin.y - local_x)
}
