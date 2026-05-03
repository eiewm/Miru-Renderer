use super::super::layout::ManiaLayoutInfo;
use super::super::render::{GameplayAnimationKind, ReplayRenderer};
use super::super::sprites::{z_order, SpriteCommand, SpritePlanner};
use super::super::PlayfieldCoverState;
use crate::hud::HudElementConfig;
use crate::renderer::gpu::SpriteBlendMode;
use crate::types::{HitObject, SkinAssets};
// The stage hint multiplier preserves the legacy osu!mania skin proportions at 768p.
const LEGACY_STAGE_HINT_Y_SCALE: f32 = 0.9 * 1.6025;
const LEGACY_KEY_RELEASE_HOLD_MS: i32 = 80;
const LEGACY_STAGE_LIGHT_RELEASE_MS: i32 = 250;
const LEGACY_HIT_EXPLOSION_FADE_IN_MS: i32 = 80;
const LEGACY_HIT_EXPLOSION_FADE_OUT_MS: i32 = 120;
const LEGACY_HIT_EXPLOSION_MIN_ANIM_MS: f64 = 170.0;
impl ReplayRenderer {
    pub fn plan_background(&self, planner: &mut SpritePlanner) {
        if let Some(tex_id) = &self.background_texture_id {
            if self.has_texture(tex_id) {
                let v = self.background_visible.clamp(0.0, 1.0);
                planner.add_sprite(SpriteCommand {
                    texture_id: tex_id.clone(),
                    x: 0,
                    y: 0,
                    width: self.cfg.width,
                    height: self.cfg.height,
                    tint: [v, v, v, 1.0],
                    z_order: z_order::BACKGROUND,
                    ..Default::default()
                });
                return;
            }
        }
        if self.stage_opaque_bg {
            planner.add_sprite(SpriteCommand {
                texture_id: "solid_black".into(),
                x: 0,
                y: 0,
                width: self.cfg.width,
                height: self.cfg.height,
                tint: [0.0, 0.0, 0.0, 1.0],
                z_order: z_order::BACKGROUND,
                ..Default::default()
            });
        }
    }
    fn hud_columns_runtime_rotation(&self, timestamp_ms: i32) -> f32 {
        self.hud_config
            .as_ref()
            .and_then(|cfg| cfg.elements.columns.as_ref())
            .map(|columns| self.hud_element_runtime_rotation(Some(&columns.base), timestamp_ms))
            .unwrap_or(0.0)
    }
    pub(crate) fn rotate_columns_sprite(
        &self,
        sprite: &mut SpriteCommand,
        layout: &ManiaLayoutInfo,
        timestamp_ms: i32,
    ) {
        let rotation = self.hud_columns_runtime_rotation(timestamp_ms);
        if rotation.abs() <= f32::EPSILON {
            return;
        }
        // Column rotation is applied as one group transform around the playfield bounds.
        Self::rotate_legacy_sprite_around_bounds(
            sprite,
            layout.stage.x as f32,
            layout.stage.top_y as f32,
            layout.stage.width as f32,
            layout.stage.height as f32,
            rotation,
        );
    }
    pub fn plan_results_backdrop(&self, planner: &mut SpritePlanner, timestamp: i32) {
        self.plan_background(planner);
        if !self.storyboard_enabled {
            return;
        }
        let Some(sb) = &self.storyboard else {
            return;
        };
        sb.plan_layer(
            planner,
            timestamp,
            crate::types::StoryboardLayer::Background,
        );
        sb.plan_layer(planner, timestamp, crate::types::StoryboardLayer::Fail);
        sb.plan_layer(planner, timestamp, crate::types::StoryboardLayer::Pass);
        sb.plan_layer(
            planner,
            timestamp,
            crate::types::StoryboardLayer::Foreground,
        );
        sb.plan_layer(planner, timestamp, crate::types::StoryboardLayer::Overlay);
    }
    pub fn plan_column_backgrounds(
        &self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        column_alphas: Option<&[f32]>,
        timestamp_ms: i32,
    ) {
        for (c, col) in layout.columns.iter().enumerate() {
            let mut tint = skin
                .config
                .column_colors
                .get(c)
                .and_then(|color| *color)
                .unwrap_or([0.0, 0.0, 0.0, 1.0]);
            if let Some(alpha) = column_alphas.and_then(|a| a.get(c)).copied() {
                tint[3] = alpha.clamp(0.0, 1.0);
            }
            if tint[3] <= 0.0 {
                continue;
            }
            let mut sprite = SpriteCommand {
                texture_id: "solid_white".into(),
                x: col.x,
                y: layout.stage.y,
                width: col.width,
                height: layout.stage.height,
                tint,
                z_order: z_order::COLUMN_BG,
                ..Default::default()
            };
            self.rotate_columns_sprite(&mut sprite, layout, timestamp_ms);
            planner.add_sprite(sprite);
        }
    }
    pub fn plan_barlines(
        &self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        barlines: &[i32],
        timestamp: i32,
        pps_base: f32,
        transform_timestamp_ms: i32,
    ) {
        let judgment_y = layout.stage.hit_y;
        let top_y = layout.stage.top_y;
        let bottom_y = layout.stage.bottom_y;
        for &bt in barlines {
            // Barlines use the grid SV lane, which can differ from object SV under inherited timing.
            if bt < timestamp - 400 || bt > timestamp + 8000 {
                continue;
            }
            let dist_px = self.sv_grid_distance_px(timestamp, bt, pps_base);
            let y = scroll_target_y(layout, judgment_y, dist_px);
            if y < top_y || y > bottom_y {
                continue;
            }
            let height = (skin.config.resolved_barline_height() * layout.scale_y)
                .round()
                .max(1.0) as u32;
            let mut tint = skin.config.resolved_colour_barline();
            tint[3] *= 0.35;
            let mut sprite = SpriteCommand {
                texture_id: "solid_white".into(),
                x: layout.stage.x,
                y,
                width: layout.stage.width,
                height,
                tint,
                z_order: z_order::BARLINE,
                ..Default::default()
            };
            self.rotate_columns_sprite(&mut sprite, layout, transform_timestamp_ms);
            planner.add_sprite(sprite);
        }
    }
    pub fn plan_column_lines(
        &self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        timestamp_ms: i32,
    ) {
        if layout.columns.len() < 2 {
            return;
        }
        let tint = skin.config.resolved_colour_column_line();
        for idx in 0..layout.columns.len().saturating_sub(1) {
            let left = &layout.columns[idx];
            let right = &layout.columns[idx + 1];
            let separator_x = left.x + left.width as i32;
            let gap_right = right.x;
            let requested_width =
                (skin.config.resolved_column_line_width(idx) * layout.scale_y).round();
            if requested_width <= 0.0 {
                continue;
            }
            let max_width = (gap_right - separator_x).max(1) as u32;
            let width = (requested_width as u32).min(max_width);
            let mut sprite = SpriteCommand {
                texture_id: "solid_white".into(),
                x: separator_x,
                y: layout.stage.top_y,
                width,
                height: layout.stage.height,
                tint,
                z_order: z_order::COLUMN_LINE,
                ..Default::default()
            };
            self.rotate_columns_sprite(&mut sprite, layout, timestamp_ms);
            planner.add_sprite(sprite);
        }
    }
    pub fn plan_key_bases(
        &mut self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        key_mask: u32,
        keys_under_notes: bool,
        timestamp_ms: i32,
    ) {
        for (c, col) in layout.columns.iter().enumerate() {
            let padding = self.get_column_padding(skin, layout, c);
            let target_w = (col.width as i32 - padding * 2).max(1) as u32;
            let key_x = col.x + padding;
            let base_img = skin
                .config
                .key_image
                .get(c)
                .filter(|name| !name.is_empty())
                .cloned();
            let down_img = skin
                .config
                .key_image_d
                .get(c)
                .filter(|name| !name.is_empty())
                .cloned();
            let base_texture = base_img.filter(|name| self.has_texture(name));
            let down_texture = down_img.filter(|name| self.has_texture(name));
            let down_visible = self.legacy_key_down_visible(c, key_mask, timestamp_ms);
            // If a skin has no base key, draw the down texture only while the key is visible.
            let draw_base = base_texture.is_some() && !(down_visible && down_texture.is_some());
            let fallback_texture = if draw_base {
                base_texture.as_ref()
            } else if base_texture.is_none() && down_visible {
                down_texture.as_ref()
            } else {
                None
            };
            if let Some(texture_name) = fallback_texture {
                self.plan_key_texture(
                    planner,
                    layout,
                    skin,
                    c,
                    texture_name,
                    key_x,
                    target_w,
                    if keys_under_notes {
                        z_order::KEYS
                    } else {
                        z_order::KEYS_OVER_NOTES
                    },
                    false,
                    timestamp_ms,
                );
            }
        }
    }
    pub fn plan_key_press_overlays(
        &mut self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        key_mask: u32,
        timestamp_ms: i32,
    ) {
        for (c, col) in layout.columns.iter().enumerate() {
            if !self.legacy_key_down_visible(c, key_mask, timestamp_ms) {
                continue;
            }
            let Some(down_texture) = skin
                .config
                .key_image_d
                .get(c)
                .filter(|name| !name.is_empty() && self.has_texture(name))
            else {
                continue;
            };
            let base_texture = skin
                .config
                .key_image
                .get(c)
                .filter(|name| !name.is_empty() && self.has_texture(name));
            if base_texture.is_none() {
                continue;
            }
            let padding = self.get_column_padding(skin, layout, c);
            let target_w = (col.width as i32 - padding * 2).max(1) as u32;
            self.plan_key_texture(
                planner,
                layout,
                skin,
                c,
                down_texture,
                col.x + padding,
                target_w,
                z_order::KEY_PRESS_OVERLAY,
                true,
                timestamp_ms,
            );
        }
    }
    fn legacy_key_down_visible(
        &self,
        column_index: usize,
        key_mask: u32,
        timestamp_ms: i32,
    ) -> bool {
        if key_mask & (1_u32 << column_index) != 0 {
            return true;
        }
        // Stable keeps key-down art visible briefly after release to avoid one-frame flicker.
        self.hud_key_tail_releases
            .iter()
            .rev()
            .find(|release| release.key_index == column_index)
            .is_some_and(|release| {
                let age_ms = timestamp_ms - release.released_at_ms;
                (0..LEGACY_KEY_RELEASE_HOLD_MS).contains(&age_ms)
            })
    }
    fn plan_key_texture(
        &mut self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        column_index: usize,
        texture_name: &str,
        key_x: i32,
        target_w: u32,
        z_order: i32,
        pressed: bool,
        timestamp_ms: i32,
    ) {
        let anchor_texture = self.gameplay_anchor_texture(texture_name);
        let key_h = self
            .texture_meta
            .get(&anchor_texture)
            .map(|meta| legacy_key_texture_display_height(&anchor_texture, meta.h, layout))
            .unwrap_or(target_w);
        let key_top = self.key_sprite_top_for_stage_edge(layout, key_h);
        let flip_uv = if layout.upside_down
            && skin
                .config
                .resolved_key_flip_when_upside_down(column_index, pressed)
        {
            [0.0, 1.0, 1.0, 0.0]
        } else {
            [0.0, 0.0, 1.0, 1.0]
        };
        let texture_id = self.prefer_hidpi_texture_id(texture_name);
        let cache_key = format!("{}|{}|{}", texture_id, target_w, key_h);
        let final_texture = self
            .sprite_scaled
            .get(&cache_key)
            .filter(|name| self.has_texture(name))
            .cloned()
            .unwrap_or(texture_id);
        let mut sprite = SpriteCommand {
            texture_id: final_texture,
            x: key_x,
            y: key_top,
            width: target_w,
            height: key_h,
            tint: [1.0, 1.0, 1.0, 1.0],
            uv_rect: flip_uv,
            z_order,
            ..Default::default()
        };
        self.rotate_columns_sprite(&mut sprite, layout, timestamp_ms);
        planner.add_sprite(sprite);
    }
    fn key_sprite_top_for_stage_edge(&self, layout: &ManiaLayoutInfo, key_h: u32) -> i32 {
        if layout.upside_down {
            layout.stage.top_y
        } else {
            layout.stage.bottom_y - key_h as i32
        }
    }
    pub fn get_column_padding(
        &self,
        skin: &SkinAssets,
        _layout: &ManiaLayoutInfo,
        _column: usize,
    ) -> i32 {
        skin.config.note_padding_x
    }
    pub fn sv_distance_px(&self, from_time: i32, to_time: i32, pps_base: f32) -> f32 {
        self.sv_object_distance_px(from_time, to_time, pps_base)
    }
    pub fn sv_object_distance_px(&self, from_time: i32, to_time: i32, pps_base: f32) -> f32 {
        self.sv_distance_px_for_lane(from_time, to_time, pps_base, false)
    }
    pub fn sv_grid_distance_px(&self, from_time: i32, to_time: i32, pps_base: f32) -> f32 {
        self.sv_distance_px_for_lane(from_time, to_time, pps_base, true)
    }
    fn sv_distance_px_for_lane(
        &self,
        from_time: i32,
        to_time: i32,
        pps_base: f32,
        use_grid_lane: bool,
    ) -> f32 {
        let remap_time = |time_ms: i32| {
            self.scroll_playback_clock
                .as_ref()
                .map(|clock| {
                    clock
                        .output_elapsed_ms_for_beatmap_time(time_ms as f64)
                        .round() as i32
                })
                .unwrap_or(time_ms)
        };
        let from_time = remap_time(from_time);
        let to_time = remap_time(to_time);
        if !self.sv_enabled {
            return (to_time - from_time) as f32 * pps_base / 1000.0;
        }
        if let Some(model) = self.scroll_model.as_ref() {
            // Grid lines and hit objects have separate SV integrals in osu!mania.
            return if use_grid_lane {
                model.grid_distance_px(from_time, to_time)
            } else {
                model.object_distance_px(from_time, to_time)
            };
        }
        (to_time - from_time) as f32 * pps_base / 1000.0
    }
    pub fn plan_lighting(
        &mut self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        notes: &[HitObject],
        active_indices: &[usize],
        timestamp: i32,
        animation_time_ms: f64,
    ) {
        let judgment_y = layout.stage.hit_y;
        let top_y = layout.stage.top_y;
        let bottom_y = layout.stage.bottom_y;
        let note_lighting_base = skin
            .config
            .lighting_n
            .first()
            .filter(|name| self.has_texture(name))
            .cloned();
        let hold_lighting_base = skin
            .config
            .lighting_l
            .first()
            .or(skin.config.lighting_n.first())
            .filter(|name| self.has_texture(name))
            .cloned();
        for (c, column) in layout.columns.iter().enumerate() {
            let Some(note_lighting_base) = note_lighting_base.as_ref() else {
                break;
            };
            // Multiple hits in a column use the latest active one-shot light.
            let one_shot_frames = self
                .gameplay_animation_spec(note_lighting_base)
                .map(|spec| spec.frames.len())
                .unwrap_or(1);
            let one_shot_duration_ms = lighting_one_shot_duration_ms(one_shot_frames);
            let mut one_shot_start = None;
            let mut hold_start = None;
            let mut hold_end = None;
            for &idx in active_indices {
                let ho = &notes[idx];
                if ho.column as usize != c {
                    continue;
                }
                if !ho.is_long_note() {
                    if timestamp >= ho.time && timestamp <= ho.time + one_shot_duration_ms {
                        one_shot_start =
                            Some(one_shot_start.map_or(ho.time, |prev: i32| prev.max(ho.time)));
                    }
                    continue;
                }
                let end_time = ho.end_time.unwrap_or(ho.time);
                if timestamp >= ho.time && timestamp <= end_time {
                    hold_start = Some(hold_start.map_or(ho.time, |prev: i32| prev.max(ho.time)));
                    hold_end = Some(end_time);
                }
                if timestamp >= end_time && timestamp <= end_time + one_shot_duration_ms {
                    one_shot_start =
                        Some(one_shot_start.map_or(end_time, |prev: i32| prev.max(end_time)));
                }
            }
            if let Some(start_time) = one_shot_start {
                let age_ms = timestamp - start_time;
                let alpha = legacy_lighting_n_alpha(age_ms);
                if alpha <= 0.0 {
                    continue;
                }
                let texture_id = self.gameplay_frame_texture(
                    note_lighting_base,
                    GameplayAnimationKind::LightingN,
                    animation_time_ms,
                    Some(start_time),
                    Some(start_time + one_shot_duration_ms),
                    None,
                );
                self.plan_lane_light_sprite(
                    planner,
                    layout,
                    column.x + column.width as i32 / 2,
                    judgment_y,
                    legacy_lighting_draw_scale(skin, layout, c, column.width, false),
                    &texture_id,
                    [1.0, 1.0, 1.0, 0.9 * alpha],
                    z_order::LIGHTING_N,
                    top_y,
                    bottom_y,
                    timestamp,
                );
            }
            if let (Some(hold_lighting_base), Some(start_time), Some(end_time)) =
                (hold_lighting_base.as_ref(), hold_start, hold_end)
            {
                let texture_id = self.gameplay_frame_texture(
                    hold_lighting_base,
                    GameplayAnimationKind::LightingL,
                    animation_time_ms,
                    Some(start_time),
                    Some(end_time),
                    None,
                );
                self.plan_lane_light_sprite(
                    planner,
                    layout,
                    column.x + column.width as i32 / 2,
                    judgment_y,
                    legacy_lighting_draw_scale(skin, layout, c, column.width, true),
                    &texture_id,
                    [1.0, 1.0, 1.0, 0.7],
                    z_order::LIGHTING_L,
                    top_y,
                    bottom_y,
                    timestamp,
                );
            }
        }
    }
    pub fn plan_stage_bottom(
        &mut self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        animation_time_ms: f64,
        config: Option<&HudElementConfig>,
        timestamp_ms: i32,
    ) {
        if config.and_then(|cfg| cfg.visible) == Some(false) {
            return;
        }
        let stage_bottom = skin
            .config
            .stage_bottom
            .as_ref()
            .filter(|name| self.has_texture(name) && self.texture_opaque_bounds.contains_key(*name))
            .cloned()
            .or_else(|| {
                (self.has_texture("mania-stage-bottom.png")
                    && self
                        .texture_opaque_bounds
                        .contains_key("mania-stage-bottom.png"))
                .then(|| "mania-stage-bottom.png".to_string())
            });
        let Some(stage_bottom) = stage_bottom else {
            return;
        };
        let anchor_texture = self.gameplay_anchor_texture(&stage_bottom);
        let meta = match self.texture_meta.get(&anchor_texture) {
            Some(m) => m,
            None => return,
        };
        let original_w = meta.w as f32;
        let original_h = meta.h as f32;
        if original_h <= 0.0 || original_w <= 0.0 {
            return;
        }
        let scaled_h = (original_h * layout.scale_y).round() as u32;
        let aspect_ratio = original_w / original_h;
        let scaled_w = (scaled_h as f32 * aspect_ratio).round() as u32;
        let stage_center_x = layout.stage.x + layout.stage.width as i32 / 2;
        let mut image_left = (stage_center_x - scaled_w as i32 / 2) as f32;
        let mut image_top = if layout.upside_down {
            layout.stage.top_y
        } else {
            layout.stage.bottom_y - scaled_h as i32
        } as f32;
        let mut image_width = scaled_w as f32;
        let mut image_height = scaled_h as f32;
        if let Some(cfg) = config {
            if let Some(width) = cfg.width.filter(|v| v.is_finite() && *v > 0.0) {
                image_width = width;
                if cfg.height.is_none() {
                    image_height = (image_width / aspect_ratio).max(1.0);
                }
            }
            if let Some(height) = cfg.height.filter(|v| v.is_finite() && *v > 0.0) {
                image_height = height;
                if cfg.width.is_none() {
                    image_width = (image_height * aspect_ratio).max(1.0);
                }
            }
            if let Some(x) = cfg.x.filter(|v| v.is_finite()) {
                image_left = x;
            }
            if let Some(y) = cfg.y.filter(|v| v.is_finite()) {
                image_top = y;
            }
        }
        let texture_id = self.gameplay_frame_texture(
            &stage_bottom,
            GameplayAnimationKind::StageBottom,
            animation_time_ms,
            None,
            None,
            None,
        );
        let mut sprite = SpriteCommand {
            texture_id,
            x: image_left.round() as i32,
            y: image_top.round() as i32,
            width: image_width.round().max(1.0) as u32,
            height: image_height.round().max(1.0) as u32,
            tint: [1.0, 1.0, 1.0, 1.0],
            z_order: z_order::STAGE_BOTTOM,
            ..Default::default()
        };
        let rotation = self.hud_element_runtime_rotation(config, timestamp_ms);
        Self::rotate_legacy_sprite_around_bounds(
            &mut sprite,
            image_left,
            image_top,
            image_width,
            image_height,
            rotation,
        );
        planner.add_sprite(sprite);
    }
    pub fn plan_stage_light(
        &mut self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        key_mask: u32,
        animation_time_ms: f64,
        timestamp_ms: i32,
    ) {
        let Some(stage_light) = skin
            .config
            .stage_light
            .as_ref()
            .filter(|name| self.has_texture(name) && self.texture_opaque_bounds.contains_key(*name))
            .cloned()
        else {
            return;
        };
        let logical_y = skin.config.resolved_light_position() as f32;
        let anchor_y = mania_logical_y_to_screen(layout, skin, logical_y).round() as i32;
        for (c, column) in layout.columns.iter().enumerate() {
            let Some((alpha, vertical_scale)) =
                self.legacy_stage_light_visual_state(c, key_mask, timestamp_ms)
            else {
                continue;
            };
            let texture_id = self.gameplay_frame_texture(
                &stage_light,
                GameplayAnimationKind::StageLight,
                animation_time_ms,
                None,
                None,
                Some(1000.0 / skin.config.resolved_light_frame_rate().max(1) as f64),
            );
            let mut tint = skin.config.resolved_colour_light(c);
            tint[3] *= alpha;
            self.plan_lane_anchored_sprite(
                planner,
                layout,
                column.x,
                column.width,
                anchor_y,
                layout.upside_down,
                &texture_id,
                tint,
                z_order::STAGE_LIGHT,
                vertical_scale,
                timestamp_ms,
            );
        }
    }
    fn legacy_stage_light_visual_state(
        &self,
        column_index: usize,
        key_mask: u32,
        timestamp_ms: i32,
    ) -> Option<(f32, f32)> {
        if key_mask & (1_u32 << column_index) != 0 {
            return Some((1.0, 1.0));
        }
        // Stage lights fade and shrink after key release, matching the legacy skin behavior.
        self.hud_key_tail_releases
            .iter()
            .rev()
            .find(|release| release.key_index == column_index)
            .and_then(|release| {
                let age_ms = timestamp_ms - release.released_at_ms;
                if !(0..LEGACY_STAGE_LIGHT_RELEASE_MS).contains(&age_ms) {
                    return None;
                }
                let amount = 1.0 - (age_ms as f32 / LEGACY_STAGE_LIGHT_RELEASE_MS as f32);
                Some((amount, amount))
            })
    }
    pub fn plan_stage_edges(
        &mut self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        left_config: Option<&HudElementConfig>,
        right_config: Option<&HudElementConfig>,
        timestamp_ms: i32,
    ) {
        for (texture_name, align_left, config) in [
            (skin.config.stage_left.as_ref(), true, left_config),
            (skin.config.stage_right.as_ref(), false, right_config),
        ] {
            if config.and_then(|cfg| cfg.visible) == Some(false) {
                continue;
            }
            let Some(texture_name) = texture_name.filter(|name| {
                self.has_texture(name) && self.texture_opaque_bounds.contains_key(*name)
            }) else {
                continue;
            };
            let Some(meta) = self.texture_meta.get(texture_name) else {
                continue;
            };
            let scale = layout.stage.height as f32 / meta.h.max(1) as f32;
            let mut width = (meta.w as f32 * scale).round().max(1.0);
            let mut height = layout.stage.height as f32;
            let aspect_ratio = meta.w.max(1) as f32 / meta.h.max(1) as f32;
            let mut x = if align_left {
                layout.stage.x as f32 - width
            } else {
                (layout.stage.x + layout.stage.width as i32) as f32
            };
            let mut y = layout.stage.top_y as f32;
            if let Some(cfg) = config {
                if let Some(next_width) = cfg.width.filter(|v| v.is_finite() && *v > 0.0) {
                    width = next_width;
                    if cfg.height.is_none() {
                        height = (width / aspect_ratio).max(1.0);
                    }
                }
                if let Some(next_height) = cfg.height.filter(|v| v.is_finite() && *v > 0.0) {
                    height = next_height;
                    if cfg.width.is_none() {
                        width = (height * aspect_ratio).max(1.0);
                    }
                }
                if let Some(next_x) = cfg.x.filter(|v| v.is_finite()) {
                    x = next_x;
                }
                if let Some(next_y) = cfg.y.filter(|v| v.is_finite()) {
                    y = next_y;
                }
            }
            let texture_id = self.prefer_hidpi_texture_id(texture_name);
            let mut sprite = SpriteCommand {
                texture_id,
                x: x.round() as i32,
                y: y.round() as i32,
                width: width.round().max(1.0) as u32,
                height: height.round().max(1.0) as u32,
                tint: [1.0, 1.0, 1.0, 1.0],
                z_order: z_order::STAGE_EDGE,
                ..Default::default()
            };
            let rotation = self.hud_element_runtime_rotation(config, timestamp_ms);
            Self::rotate_legacy_sprite_around_bounds(&mut sprite, x, y, width, height, rotation);
            planner.add_sprite(sprite);
        }
    }
    pub fn plan_stage_hint(
        &mut self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        config: Option<&HudElementConfig>,
        timestamp_ms: i32,
    ) {
        if config.and_then(|cfg| cfg.visible) != Some(false) {
            let stage_hint = skin
                .config
                .stage_hint
                .as_deref()
                .filter(|name| {
                    self.has_texture(name) && self.texture_opaque_bounds.contains_key(*name)
                })
                .or_else(|| {
                    ["mania-stage-hint.png", "mania-stage-hint"]
                        .into_iter()
                        .find(|name| {
                            self.has_texture(name) && self.texture_opaque_bounds.contains_key(*name)
                        })
                });
            if let Some(stage_hint) = stage_hint {
                if let Some(meta) = self.texture_meta.get(stage_hint) {
                    let aspect_ratio = meta.w.max(1) as f32 / meta.h.max(1) as f32;
                    let mut width = layout.stage.width.max(1) as f32;
                    let mut height = legacy_texture_display_height(stage_hint, meta.h, layout)
                        as f32
                        * LEGACY_STAGE_HINT_Y_SCALE;
                    let mut left = layout.stage.x as f32;
                    let mut top_override = None;
                    if let Some(cfg) = config {
                        if let Some(next_width) = cfg.width.filter(|v| v.is_finite() && *v > 0.0) {
                            width = next_width;
                            if cfg.height.is_none() {
                                height =
                                    (width / aspect_ratio * LEGACY_STAGE_HINT_Y_SCALE).max(1.0);
                            }
                        }
                        if let Some(next_height) = cfg.height.filter(|v| v.is_finite() && *v > 0.0)
                        {
                            height = next_height;
                            if cfg.width.is_none() {
                                width = (height * aspect_ratio).max(1.0);
                            }
                        }
                        if let Some(x) = cfg.x.filter(|v| v.is_finite()) {
                            left = x;
                        }
                        if let Some(y) = cfg.y.filter(|v| v.is_finite()) {
                            top_override = Some(y);
                        }
                    }
                    let top = top_override.unwrap_or(layout.stage.hit_y as f32 - height * 0.5);
                    let texture_id = self.prefer_hidpi_texture_id(stage_hint);
                    let mut sprite = SpriteCommand {
                        texture_id,
                        x: left.round() as i32,
                        y: top.round() as i32,
                        width: width.round().max(1.0) as u32,
                        height: height.round().max(1.0) as u32,
                        tint: [1.0, 1.0, 1.0, 1.0],
                        z_order: z_order::STAGE_HINT,
                        ..Default::default()
                    };
                    let rotation = self.hud_element_runtime_rotation(config, timestamp_ms);
                    if rotation.abs() > f32::EPSILON {
                        Self::rotate_legacy_sprite_around_bounds(
                            &mut sprite,
                            left,
                            top,
                            width,
                            height,
                            rotation,
                        );
                    } else {
                        self.rotate_columns_sprite(&mut sprite, layout, timestamp_ms);
                    }
                    planner.add_sprite(sprite);
                }
            }
        }
        if skin.config.resolved_judgement_line() {
            let tint = skin.config.resolved_colour_judgement_line();
            let thickness = layout.scale_y.round().max(1.0) as u32;
            let mut sprite = SpriteCommand {
                texture_id: "solid_white".into(),
                x: layout.stage.x,
                y: layout.stage.hit_y - thickness as i32 / 2,
                width: layout.stage.width,
                height: thickness,
                tint,
                z_order: z_order::STAGE_HINT + 1,
                ..Default::default()
            };
            self.rotate_columns_sprite(&mut sprite, layout, timestamp_ms);
            planner.add_sprite(sprite);
        }
    }
    pub fn plan_warning_arrows(
        &self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        timestamp: i32,
        transform_timestamp_ms: i32,
    ) {
        let Some(first_note_time_ms) = self.first_note_time_ms else {
            return;
        };
        let Some(warning_arrow) = skin.config.warning_arrow.as_ref().filter(|name| {
            self.has_texture(name) && self.texture_opaque_bounds.contains_key(*name)
        }) else {
            return;
        };
        let Some(meta) = self.texture_meta.get(warning_arrow) else {
            return;
        };
        for wave_idx in 0..3 {
            // Warning arrows pulse once per second during long lead-ins before the first note.
            let wave_end = first_note_time_ms - 1000 - wave_idx * 1000;
            let wave_start = wave_end - 1000;
            if self.lead_in_ms < (wave_idx + 2) * 1000 {
                continue;
            }
            if timestamp < wave_start || timestamp >= wave_end {
                continue;
            }
            let progress = (timestamp - wave_start) as f32 / 1000.0;
            let alpha = (std::f32::consts::PI * progress)
                .sin()
                .abs()
                .clamp(0.0, 1.0);
            let width = (layout
                .columns
                .first()
                .map(|column| column.width)
                .unwrap_or(48) as f32
                * 0.8)
                .round()
                .max(1.0) as u32;
            let height = ((meta.h as f32) * (width as f32 / meta.w.max(1) as f32))
                .round()
                .max(1.0) as u32;
            let center_y = if layout.upside_down {
                layout.stage.top_y + height as i32 / 2 + 4
            } else {
                layout.stage.bottom_y - height as i32 / 2 - 4
            };
            let uv_rect = if layout.upside_down {
                [1.0, 1.0, 0.0, 0.0]
            } else {
                [0.0, 0.0, 1.0, 1.0]
            };
            for column in &layout.columns {
                let center_x = column.x + column.width as i32 / 2;
                let mut sprite = SpriteCommand {
                    texture_id: self.prefer_hidpi_texture_id(warning_arrow),
                    x: center_x - width as i32 / 2,
                    y: center_y - height as i32 / 2,
                    width,
                    height,
                    tint: [
                        skin.config.resolved_colour_key_warning()[0],
                        skin.config.resolved_colour_key_warning()[1],
                        skin.config.resolved_colour_key_warning()[2],
                        alpha,
                    ],
                    uv_rect,
                    z_order: z_order::WARNING_ARROW,
                    ..Default::default()
                };
                self.rotate_columns_sprite(&mut sprite, layout, transform_timestamp_ms);
                planner.add_sprite(sprite);
            }
            break;
        }
    }
    fn plan_lane_light_sprite(
        &self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        center_x: i32,
        center_y: i32,
        draw_scale: f32,
        texture_id: &str,
        tint: [f32; 4],
        z_order: i32,
        top_y: i32,
        bottom_y: i32,
        timestamp_ms: i32,
    ) {
        let Some(meta) = self.texture_meta.get(texture_id) else {
            return;
        };
        let (display_w, display_h) = legacy_texture_display_size(texture_id, meta.w, meta.h);
        let safe_scale = draw_scale.max(0.0001);
        let width = (display_w * safe_scale).round().max(1.0) as u32;
        let height = (display_h * safe_scale).round().max(1.0) as u32;
        let left = center_x - width as i32 / 2;
        let top = center_y - height as i32 / 2;
        if top + height as i32 <= top_y || top >= bottom_y {
            return;
        }
        let draw_top = top.max(top_y);
        let draw_bottom = (top + height as i32).min(bottom_y);
        let draw_h = (draw_bottom - draw_top).max(0) as u32;
        if draw_h == 0 {
            return;
        }
        // Clip lane lights vertically by adjusting V coordinates instead of changing the source texture.
        let v0 = if draw_top > top {
            (draw_top - top) as f32 / height as f32
        } else {
            0.0
        };
        let v1 = v0 + draw_h as f32 / height as f32;
        let mut sprite = SpriteCommand {
            texture_id: texture_id.to_string(),
            x: left,
            y: draw_top,
            width,
            height: draw_h,
            tint,
            uv_rect: [0.0, v0, 1.0, v1.min(1.0)],
            z_order,
            blend_mode: SpriteBlendMode::Additive,
            ..Default::default()
        };
        self.rotate_columns_sprite(&mut sprite, layout, timestamp_ms);
        planner.add_sprite(sprite);
    }
    fn plan_lane_anchored_sprite(
        &self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        x: i32,
        width: u32,
        anchor_y: i32,
        anchor_at_top: bool,
        texture_id: &str,
        tint: [f32; 4],
        z_order: i32,
        vertical_scale: f32,
        timestamp_ms: i32,
    ) {
        let Some(meta) = self.texture_meta.get(texture_id) else {
            return;
        };
        let height = ((legacy_texture_display_height(texture_id, meta.h, layout) as f32)
            * vertical_scale.clamp(0.0, 1.0))
        .round()
        .max(1.0) as u32;
        let mut sprite = SpriteCommand {
            texture_id: texture_id.to_string(),
            x,
            y: if anchor_at_top {
                anchor_y
            } else {
                anchor_y - height as i32
            },
            width,
            height,
            tint,
            z_order,
            ..Default::default()
        };
        self.rotate_columns_sprite(&mut sprite, layout, timestamp_ms);
        planner.add_sprite(sprite);
    }
    pub fn plan_playfield_cover_overlay(
        &self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        cover: Option<&PlayfieldCoverState>,
        timestamp_ms: i32,
    ) {
        let Some(cover) = cover.filter(|cover| cover.is_active()) else {
            return;
        };
        if cover.mode == super::super::PlayfieldCoverMode::Flashlight {
            // Flashlight cover is screen-wide; regular covers are constrained to the playfield columns.
            let stage_top = layout.stage.top_y as f32;
            let stage_bottom = layout.stage.bottom_y as f32;
            let top_opaque_end = (cover.window_top_y - cover.fade_height_px)
                .max(stage_top)
                .min(stage_bottom);
            let top_fade_start = cover.window_top_y.max(stage_top).min(stage_bottom);
            let bottom_fade_start = cover.window_bottom_y.max(stage_top).min(stage_bottom);
            let bottom_fade_end = (cover.window_bottom_y + cover.fade_height_px)
                .max(stage_top)
                .min(stage_bottom);
            if top_opaque_end > stage_top + 0.5 {
                planner.add_sprite(SpriteCommand {
                    texture_id: "solid_black".into(),
                    x: 0,
                    y: stage_top.floor() as i32,
                    width: self.cfg.width,
                    height: (top_opaque_end - stage_top).ceil() as u32,
                    tint: [0.0, 0.0, 0.0, 1.0],
                    z_order: z_order::PLAYFIELD_COVER,
                    ..Default::default()
                });
            }
            if top_fade_start > top_opaque_end + 0.5 {
                planner.add_sprite(SpriteCommand {
                    texture_id: "vertical_black_fade".into(),
                    x: 0,
                    y: top_opaque_end.floor() as i32,
                    width: self.cfg.width,
                    height: (top_fade_start - top_opaque_end).ceil() as u32,
                    tint: [1.0, 1.0, 1.0, 1.0],
                    z_order: z_order::PLAYFIELD_COVER,
                    ..Default::default()
                });
            }
            if bottom_fade_end > bottom_fade_start + 0.5 {
                planner.add_sprite(SpriteCommand {
                    texture_id: "vertical_black_fade".into(),
                    x: 0,
                    y: bottom_fade_start.floor() as i32,
                    width: self.cfg.width,
                    height: (bottom_fade_end - bottom_fade_start).ceil() as u32,
                    tint: [1.0, 1.0, 1.0, 1.0],
                    z_order: z_order::PLAYFIELD_COVER,
                    uv_rect: [0.0, 1.0, 1.0, 0.0],
                    ..Default::default()
                });
            }
            if stage_bottom > bottom_fade_end + 0.5 {
                planner.add_sprite(SpriteCommand {
                    texture_id: "solid_black".into(),
                    x: 0,
                    y: bottom_fade_end.floor() as i32,
                    width: self.cfg.width,
                    height: (stage_bottom - bottom_fade_end).ceil() as u32,
                    tint: [0.0, 0.0, 0.0, 1.0],
                    z_order: z_order::PLAYFIELD_COVER,
                    ..Default::default()
                });
            }
            return;
        }
        let stage_top = layout.stage.top_y as f32;
        let logical_bottom = layout.stage.hit_y as f32;
        let cover_top = cover.top_y.max(stage_top).min(logical_bottom);
        let cover_bottom = cover.bottom_y().max(stage_top).min(logical_bottom);
        let opaque_start = cover.opaque_start_y().max(stage_top).min(logical_bottom);
        let opaque_end = cover.opaque_end_y().max(stage_top).min(logical_bottom);
        let fade_start = cover.fade_start_y().max(stage_top).min(logical_bottom);
        let fade_end = cover.fade_end_y().max(stage_top).min(logical_bottom);
        if cover_bottom <= cover_top + 0.5 {
            return;
        }
        if opaque_end > opaque_start + 0.5 {
            let mut sprite = SpriteCommand {
                texture_id: "solid_black".into(),
                x: layout.stage.x,
                y: opaque_start.floor() as i32,
                width: layout.stage.width,
                height: (opaque_end - opaque_start).ceil() as u32,
                tint: [0.0, 0.0, 0.0, 1.0],
                z_order: z_order::PLAYFIELD_COVER,
                ..Default::default()
            };
            self.rotate_columns_sprite(&mut sprite, layout, timestamp_ms);
            planner.add_sprite(sprite);
        }
        if fade_end <= fade_start + 0.5 {
            return;
        }
        let mut fade_sprite = SpriteCommand {
            texture_id: "vertical_black_fade".into(),
            x: layout.stage.x,
            y: fade_start.floor() as i32,
            width: layout.stage.width,
            height: (fade_end - fade_start).ceil() as u32,
            tint: [1.0, 1.0, 1.0, 1.0],
            z_order: z_order::PLAYFIELD_COVER,
            ..Default::default()
        };
        if cover.direction == super::super::PlayfieldCoverDirection::Upwards {
            fade_sprite.uv_rect = [0.0, 1.0, 1.0, 0.0];
        }
        self.rotate_columns_sprite(&mut fade_sprite, layout, timestamp_ms);
        planner.add_sprite(fade_sprite);
    }
}
fn legacy_key_texture_display_height(
    texture_id: &str,
    texture_height: u32,
    layout: &ManiaLayoutInfo,
) -> u32 {
    legacy_texture_display_height(texture_id, texture_height, layout)
}
fn legacy_texture_display_height(
    texture_id: &str,
    texture_height: u32,
    layout: &ManiaLayoutInfo,
) -> u32 {
    // Legacy skin dimensions are logical pixels; @2x textures draw at half their bitmap height.
    let scale_adjust = if texture_id.contains("@2x") { 2.0 } else { 1.0 };
    let screen_scale = layout.stage.height.max(1) as f32 / 768.0;
    ((texture_height.max(1) as f32) / scale_adjust * screen_scale)
        .round()
        .max(1.0) as u32
}
fn legacy_texture_display_size(
    texture_id: &str,
    texture_width: u32,
    texture_height: u32,
) -> (f32, f32) {
    let scale_adjust = if texture_id.contains("@2x") { 2.0 } else { 1.0 };
    (
        texture_width.max(1) as f32 / scale_adjust,
        texture_height.max(1) as f32 / scale_adjust,
    )
}
fn legacy_lighting_draw_scale(
    skin: &SkinAssets,
    layout: &ManiaLayoutInfo,
    column: usize,
    lane_width: u32,
    hold: bool,
) -> f32 {
    let screen_scale = layout.stage.height.max(1) as f32 / 768.0;
    if !skin.config.uses_modern_mania_defaults() {
        return screen_scale;
    }
    let logical_width = if hold {
        skin.config.resolved_lighting_l_width(column)
    } else {
        skin.config.resolved_lighting_n_width(column)
    }
    .map(|value| value as f32)
    .unwrap_or_else(|| lane_width.max(1) as f32 / layout.scale_y.max(0.0001));
    (logical_width.max(1.0) / 30.0) * screen_scale
}
fn scroll_target_y(layout: &ManiaLayoutInfo, judgment_y: i32, dist_px: f32) -> i32 {
    if layout.upside_down {
        judgment_y + dist_px.round() as i32
    } else {
        judgment_y - dist_px.round() as i32
    }
}
fn mania_logical_y_to_screen(layout: &ManiaLayoutInfo, skin: &SkinAssets, logical_y: f32) -> f32 {
    let hit_position = skin.config.hit_position.unwrap_or(402) as f32;
    // Skin.ini stage positions are in the 480px mania playfield coordinate system.
    let effective_hit_position = if layout.upside_down {
        480.0 - hit_position
    } else {
        hit_position
    };
    let effective_y = if layout.upside_down {
        480.0 - logical_y
    } else {
        logical_y
    };
    layout.stage.hit_y as f32 + (effective_y - effective_hit_position) * layout.scale_y
}
fn lighting_one_shot_duration_ms(frame_count: usize) -> i32 {
    let animation_ms = frame_count as f64 * lighting_n_frame_length_ms(frame_count);
    // LightingN must last long enough for the legacy fade-in and fade-out envelope.
    animation_ms
        .max((LEGACY_HIT_EXPLOSION_FADE_IN_MS + LEGACY_HIT_EXPLOSION_FADE_OUT_MS) as f64)
        .round() as i32
}
fn lighting_n_frame_length_ms(frame_count: usize) -> f64 {
    if frame_count == 0 {
        return 1000.0 / 60.0;
    }
    (LEGACY_HIT_EXPLOSION_MIN_ANIM_MS / frame_count as f64).max(1000.0 / 60.0)
}
fn legacy_lighting_n_alpha(age_ms: i32) -> f32 {
    if age_ms < 0 {
        return 0.0;
    }
    if age_ms < LEGACY_HIT_EXPLOSION_FADE_IN_MS {
        return age_ms as f32 / LEGACY_HIT_EXPLOSION_FADE_IN_MS as f32;
    }
    let fade_out_age = age_ms - LEGACY_HIT_EXPLOSION_FADE_IN_MS;
    if fade_out_age >= LEGACY_HIT_EXPLOSION_FADE_OUT_MS {
        return 0.0;
    }
    1.0 - fade_out_age as f32 / LEGACY_HIT_EXPLOSION_FADE_OUT_MS as f32
}
