use super::super::layout::ManiaLayoutInfo;
use super::super::model::{
    LnReleaseInfo, LnRenderInfo, NoteRenderState, ReleaseKind, RenderJudgment, Windows,
};
use super::super::render::{GameplayAnimationKind, ReplayRenderer};
use super::super::sprites::{z_order, SpriteCommand, SpritePlanner, Tint};
use super::super::state::FailAnimationState;
use crate::renderer::replay_renderer::{ColumnLayout, TextureMeta};
use crate::types::{HitObject, JudgmentKind, NoteBodyStyle, NotePart, SkinAssets};
#[derive(Debug, Clone, Copy, PartialEq)]
struct FailTransform {
    brightness: f32,
    x_offset: i32,
    rotation: f32,
}
impl ReplayRenderer {
    fn non_empty_skin_asset(name: Option<&String>) -> Option<String> {
        name.filter(|value| !value.is_empty()).cloned()
    }
    fn gameplay_asset_has_visible_pixels(&mut self, skin: &SkinAssets, texture_name: &str) -> bool {
        let anchor = self.gameplay_anchor_texture(texture_name);
        if let Some(visible) = self.gameplay_visibility_cache.get(&anchor) {
            return *visible;
        }
        // Transparent configured assets should not block fallback candidates for notes or tails.
        let visible = if self.texture_opaque_bounds.contains_key(&anchor) {
            true
        } else if let Some(data) = skin
            .find_image(&anchor)
            .or_else(|| skin.find_image(texture_name))
        {
            crate::utils::image_proc::load_rgba(data)
                .map(|img| crate::utils::image_proc::has_opaque_pixels(&img, 0))
                .unwrap_or(true)
        } else {
            self.texture_meta.contains_key(&anchor) || self.texture_meta.contains_key(texture_name)
        };
        self.gameplay_visibility_cache.insert(anchor, visible);
        visible
    }
    fn resolve_visible_gameplay_asset(
        &mut self,
        skin: &SkinAssets,
        candidates: &[Option<&String>],
    ) -> Option<String> {
        for candidate in candidates {
            let Some(name) = Self::non_empty_skin_asset(*candidate) else {
                continue;
            };
            if self.gameplay_asset_has_visible_pixels(skin, &name) {
                return Some(name);
            }
        }
        None
    }
    fn legacy_note_body_tile_height(meta: Option<&TextureMeta>, draw_width: u32) -> u32 {
        meta.map(|meta| {
            let ref_w = meta.w.max(1) as f32;
            ((meta.h as f32) * (draw_width as f32 / ref_w))
                .round()
                .max(1.0) as u32
        })
        .unwrap_or(draw_width.max(1))
    }
    fn resolve_tail_geometry_asset(&mut self, skin: &SkinAssets, column: usize) -> Option<String> {
        if let Some(name) = Self::non_empty_skin_asset(skin.config.note_image_t.get(column)) {
            return Some(name);
        }
        // Tail geometry can use head or note dimensions even if the actual tail draw asset is absent.
        self.resolve_visible_gameplay_asset(
            skin,
            &[
                skin.config.note_image_h.get(column),
                skin.config.note_image.get(column),
            ],
        )
    }
    fn resolve_tail_draw_asset(&mut self, skin: &SkinAssets, column: usize) -> Option<String> {
        if let Some(name) = Self::non_empty_skin_asset(skin.config.note_image_t.get(column)) {
            return self
                .gameplay_asset_has_visible_pixels(skin, &name)
                .then_some(name);
        }
        self.resolve_visible_gameplay_asset(
            skin,
            &[
                skin.config.note_image_h.get(column),
                skin.config.note_image.get(column),
            ],
        )
    }
    fn get_note_render_state(
        _ho: &HitObject,
        judgment: Option<RenderJudgment>,
        timestamp: i32,
        frame_eps_ms: f32,
        _windows: Option<&Windows>,
    ) -> NoteRenderState {
        let mut state = NoteRenderState::default();
        if let Some(j) = judgment {
            let press_time = j.press_time.unwrap_or(j.time);
            // 50s and misses remain visible briefly so their dimmed state can be seen.
            state.consumed = j.kind != JudgmentKind::Miss
                && j.kind != JudgmentKind::Hit50
                && press_time <= timestamp;
            if j.kind == JudgmentKind::Miss {
                if let Some(pt) = j.press_time {
                    state.miss_dim = timestamp >= (pt as f32 - frame_eps_ms * 0.75) as i32;
                }
            }
            if j.kind == JudgmentKind::Hit50 {
                state.fifty_dim = timestamp >= press_time;
            }
        }
        state
    }
    fn note_brightness_tint(state: &NoteRenderState) -> Tint {
        if state.miss_dim {
            [0.7, 0.7, 0.7, 1.0]
        } else if state.fifty_dim {
            [0.85, 0.85, 0.85, 1.0]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        }
    }
    fn ease_out_cubic(progress: f32) -> f32 {
        let clamped = progress.clamp(0.0, 1.0);
        1.0 - (1.0 - clamped).powi(3)
    }
    fn hash_unit(seed: u32) -> f32 {
        let mut value = seed;
        value ^= value >> 16;
        value = value.wrapping_mul(0x7feb_352d);
        value ^= value >> 15;
        value = value.wrapping_mul(0x846c_a68b);
        value ^= value >> 16;
        value as f32 / u32::MAX as f32
    }
    fn fail_transform(
        note_index: usize,
        piece_kind: u32,
        progress: f32,
        max_drift_px: f32,
        max_rotation_deg: f32,
        base_x: i32,
        width: u32,
        column: &ColumnLayout,
    ) -> FailTransform {
        let eased = Self::ease_out_cubic(progress);
        // Hash by note and piece kind so fail drift is deterministic but not uniform.
        let seed = (note_index as u32)
            .wrapping_mul(0x9e37_79b9)
            .wrapping_add(piece_kind.wrapping_mul(0x85eb_ca6b));
        let drift_scalar = Self::hash_unit(seed) * 2.0 - 1.0;
        let rotation_scalar = Self::hash_unit(seed ^ 0x27d4_eb2d) * 2.0 - 1.0;
        let desired_offset = drift_scalar * max_drift_px;
        let min_offset = (column.x - base_x) as f32;
        let max_offset = (column.x + column.width as i32 - (base_x + width as i32)) as f32;
        let x_offset = (desired_offset.clamp(min_offset, max_offset) * eased).round() as i32;
        FailTransform {
            brightness: 1.0 + (0.45 - 1.0) * eased,
            x_offset,
            rotation: rotation_scalar * max_rotation_deg.to_radians() * eased,
        }
    }
    fn apply_fail_tint(
        base: Tint,
        fail_state: Option<&FailAnimationState>,
        brightness: f32,
    ) -> Tint {
        let multiplier = if fail_state.is_some() {
            brightness.clamp(0.0, 1.0)
        } else {
            1.0
        };
        [
            (base[0] * multiplier).clamp(0.0, 1.0),
            (base[1] * multiplier).clamp(0.0, 1.0),
            (base[2] * multiplier).clamp(0.0, 1.0),
            base[3],
        ]
    }
    fn note_debug_label_x(layout: &ManiaLayoutInfo, note_x: i32, note_w: u32, label_w: u32) -> i32 {
        let raw_x = note_x + (note_w as i32 - label_w as i32) / 2;
        let min_x = layout.stage.x;
        let max_x = layout.stage.x + layout.stage.width as i32 - label_w as i32;
        raw_x.clamp(min_x, max_x.max(min_x))
    }
    fn note_debug_head_y(
        layout: &ManiaLayoutInfo,
        note_top_y: i32,
        note_h: u32,
        label_h: u32,
    ) -> i32 {
        let raw_y = note_top_y + (note_h as i32 - label_h as i32) / 2;
        let min_y = layout.stage.view_top_y;
        let max_y = layout.stage.view_bottom_y - label_h as i32;
        raw_y.clamp(min_y, max_y.max(min_y))
    }
    pub fn plan_normal_notes(
        &mut self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        notes: &[HitObject],
        active_indices: &[usize],
        judgments_by_idx: &[Option<RenderJudgment>],
        state_time_ms: i32,
        position_time_ms: i32,
        animation_time_ms: f64,
        pps_base: f32,
        windows: Option<&Windows>,
        fail_state: Option<&FailAnimationState>,
    ) {
        let judgment_y = layout.stage.hit_y;
        // Culling and clipping follow the drawn playfield, not the skin box: in
        // 9:16 they differ, and using the box leaves note heads floating above
        // the fill with their long-note bodies cut off at its edge.
        let top_y = layout.stage.view_top_y;
        let bottom_y = layout.stage.view_bottom_y;
        let frame_eps_ms = 1000.0 / self.cfg.fps.max(1) as f32;
        for &idx in active_indices {
            let ho = &notes[idx];
            let col = ho.column as usize;
            if col >= layout.columns.len() {
                continue;
            }
            if ho.is_long_note() {
                continue;
            }
            let column = &layout.columns[col];
            let judgment = judgments_by_idx.get(idx).copied().flatten();
            let state =
                Self::get_note_render_state(ho, judgment, state_time_ms, frame_eps_ms, windows);
            if state.consumed {
                continue;
            }
            let dist_px = self.sv_distance_px(position_time_ms, ho.time, pps_base);
            let head_bottom_y = scroll_target_y(layout, judgment_y, dist_px);
            let padding = self.get_column_padding(skin, layout, col);
            let target_w = (column.width as i32 - padding * 2).max(1) as u32;
            let note_name =
                self.resolve_visible_gameplay_asset(skin, &[skin.config.note_image.get(col)]);
            let note_anchor_name = note_name
                .as_ref()
                .map(|name| self.gameplay_anchor_texture(name))
                .or_else(|| note_name.clone());
            let note_meta = note_anchor_name
                .as_ref()
                .and_then(|name| self.texture_meta.get(name));
            let note_h = Self::legacy_note_piece_height(
                note_meta,
                target_w,
                skin.config.width_for_note_height_scale,
                layout.scale_y,
            );
            let head_top_y = scroll_piece_top_y(layout.upside_down, head_bottom_y, note_h as i32);
            let base_note_x = column.x + padding;
            if head_top_y > bottom_y || (head_top_y + note_h as i32) < top_y {
                continue;
            }
            let fail_transform = fail_state.map(|state| {
                Self::fail_transform(
                    idx,
                    1,
                    state.progress,
                    column.width as f32 * 0.12,
                    3.0,
                    base_note_x,
                    target_w,
                    column,
                )
            });
            let note_x = base_note_x
                + fail_transform
                    .map(|transform| transform.x_offset)
                    .unwrap_or(0);
            let tint = Self::apply_fail_tint(
                Self::note_brightness_tint(&state),
                fail_state,
                fail_transform
                    .map(|transform| transform.brightness)
                    .unwrap_or(1.0),
            );
            let Some(texture_id) = note_name.as_ref().map(|name| {
                self.gameplay_frame_texture(
                    name,
                    GameplayAnimationKind::Note,
                    animation_time_ms,
                    None,
                    None,
                    None,
                )
            }) else {
                continue;
            };
            let final_texture = self
                .get_or_create_scaled_sprite(skin, &texture_id, target_w, note_h)
                .unwrap_or(texture_id);
            let uv_rect = if layout.upside_down
                && skin
                    .config
                    .resolved_note_flip_when_upside_down(col, NotePart::Note)
            {
                [0.0, 1.0, 1.0, 0.0]
            } else {
                [0.0, 0.0, 1.0, 1.0]
            };
            let mut sprite = SpriteCommand {
                texture_id: final_texture,
                x: note_x,
                y: head_top_y,
                width: target_w,
                height: note_h,
                tint,
                uv_rect,
                origin: if fail_state.is_some() {
                    [target_w as f32 / 2.0, note_h as f32 / 2.0]
                } else {
                    [0.0, 0.0]
                },
                rotation: fail_transform
                    .map(|transform| transform.rotation)
                    .unwrap_or(0.0),
                z_order: z_order::NORMAL_NOTE,
                ..Default::default()
            };
            self.rotate_columns_sprite(&mut sprite, layout, state_time_ms);
            planner.add_sprite(sprite);
            if self.ln_debug {
                if let Some(j) = judgment {
                    let debug_tex_name = Self::get_debug_texture_name(idx, j.kind);
                    if self.has_texture(&debug_tex_name) {
                        if let Some(meta) = self.texture_meta.get(&debug_tex_name) {
                            let debug_x =
                                Self::note_debug_label_x(layout, note_x, target_w, meta.w);
                            let debug_y =
                                Self::note_debug_head_y(layout, head_top_y, note_h, meta.h);
                            let mut sprite = SpriteCommand {
                                texture_id: debug_tex_name,
                                x: debug_x,
                                y: debug_y,
                                width: meta.w,
                                height: meta.h,
                                tint: [1.0, 1.0, 1.0, 1.0],
                                z_order: z_order::NORMAL_NOTE + 1,
                                ..Default::default()
                            };
                            self.rotate_columns_sprite(&mut sprite, layout, state_time_ms);
                            planner.add_sprite(sprite);
                        }
                    }
                }
            }
        }
    }
    pub fn plan_long_notes(
        &mut self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        notes: &[HitObject],
        active_indices: &[usize],
        judgments_by_idx: &[Option<RenderJudgment>],
        ln_release_info: &[Option<LnReleaseInfo>],
        state_time_ms: i32,
        position_time_ms: i32,
        animation_time_ms: f64,
        key_mask: u32,
        pps_base: f32,
        windows: Option<&Windows>,
        fail_state: Option<&FailAnimationState>,
    ) {
        let judgment_y = layout.stage.hit_y;
        // Culling and clipping follow the drawn playfield, not the skin box: in
        // 9:16 they differ, and using the box leaves note heads floating above
        // the fill with their long-note bodies cut off at its edge.
        let top_y = layout.stage.view_top_y;
        let bottom_y = layout.stage.view_bottom_y;
        for &idx in active_indices {
            let ho = &notes[idx];
            let col = ho.column as usize;
            if col >= layout.columns.len() {
                continue;
            }
            if !ho.is_long_note() {
                continue;
            }
            let end_time = ho.end_time.unwrap_or(ho.time);
            let column = &layout.columns[col];
            let judgment = judgments_by_idx.get(idx).copied().flatten();
            let release = ln_release_info.get(idx).copied().flatten();
            let key_pressed = (key_mask & (1 << col)) != 0;
            let padding = self.get_column_padding(skin, layout, col);
            let target_w = (column.width as i32 - padding * 2).max(1) as u32;
            let ln_x = column.x + padding;
            let head_name = self.resolve_visible_gameplay_asset(
                skin,
                &[
                    skin.config.note_image_h.get(col),
                    skin.config.note_image.get(col),
                ],
            );
            let head_anchor_name = head_name
                .as_ref()
                .map(|name| self.gameplay_anchor_texture(name))
                .or_else(|| head_name.clone());
            let head_meta = head_anchor_name
                .as_ref()
                .and_then(|n| self.texture_meta.get(n));
            let head_h = Self::legacy_note_piece_height(
                head_meta,
                target_w,
                skin.config.width_for_note_height_scale,
                layout.scale_y,
            );
            let tail_geometry_name = self.resolve_tail_geometry_asset(skin, col);
            let tail_name = self.resolve_tail_draw_asset(skin, col);
            let tail_anchor_name = tail_geometry_name
                .as_ref()
                .map(|name| self.gameplay_anchor_texture(name))
                .or_else(|| tail_geometry_name.clone());
            let tail_meta = tail_anchor_name
                .as_ref()
                .and_then(|n| self.texture_meta.get(n));
            let tail_h = Self::legacy_note_piece_height(
                tail_meta,
                target_w,
                skin.config.width_for_note_height_scale,
                layout.scale_y,
            )
            .max(1);
            let head_dist = self.sv_distance_px(position_time_ms, ho.time, pps_base);
            let tail_dist = self.sv_distance_px(position_time_ms, end_time, pps_base);
            let release_head_bottom_y = release
                .and_then(|info| info.time)
                .filter(|&release_time| release_time > ho.time && release_time < end_time)
                .map(|release_time| {
                    let release_dist =
                        self.sv_distance_px(position_time_ms, release_time, pps_base);
                    scroll_target_y(layout, judgment_y, release_dist)
                });
            let head_bottom_y = scroll_target_y(layout, judgment_y, head_dist);
            let tail_bottom_y = scroll_target_y(layout, judgment_y, tail_dist);
            let tail_top_y =
                scroll_piece_top_y(layout.upside_down, tail_bottom_y, tail_h as i32);
            let ln_info = self.compute_ln_render_info(
                ho,
                state_time_ms,
                judgment,
                release,
                key_pressed,
                windows,
                layout.upside_down,
                judgment_y,
                head_h as i32,
                tail_h as i32,
                head_bottom_y,
                release_head_bottom_y,
                tail_top_y,
                top_y,
                bottom_y,
            );
            if !ln_info.show_ln || ln_info.finished_normally {
                continue;
            }
            let fail_transform = fail_state.map(|state| {
                Self::fail_transform(
                    idx,
                    2,
                    state.progress,
                    column.width as f32 * 0.16,
                    6.0,
                    ln_x,
                    target_w,
                    column,
                )
            });
            let shared_x = ln_x
                + fail_transform
                    .map(|transform| transform.x_offset)
                    .unwrap_or(0);
            let tint_val = ln_info.head_tint;
            let tint = Self::apply_fail_tint(
                [tint_val, tint_val, tint_val, 1.0],
                fail_state,
                fail_transform
                    .map(|transform| transform.brightness)
                    .unwrap_or(1.0),
            );
            let body_height = ln_info.body_bottom_y - ln_info.body_top_y;
            if body_height > 0 {
                // Held LNs clip at the receptor; broken or released LNs can pass
                // through. The body trails away from the receptor, so which edge
                // the receptor bounds depends on the scroll direction: the bottom
                // one going down, the top one going up.
                let receptor_edge = judgment_y + head_h as i32 / 2;
                let (clamped_top, clamped_bottom) = if ln_info.allow_pass {
                    (
                        top_y.max(ln_info.body_top_y),
                        bottom_y.min(ln_info.body_bottom_y),
                    )
                } else if layout.upside_down {
                    (
                        top_y.max(ln_info.body_top_y).max(receptor_edge),
                        bottom_y.min(ln_info.body_bottom_y),
                    )
                } else {
                    (
                        top_y.max(ln_info.body_top_y),
                        receptor_edge.min(ln_info.body_bottom_y),
                    )
                };
                if clamped_bottom > clamped_top {
                    self.plan_long_note_body(
                        planner,
                        layout,
                        skin,
                        col,
                        target_w,
                        shared_x,
                        clamped_top,
                        clamped_bottom,
                        ln_info.body_top_y,
                        ln_info.body_bottom_y,
                        animation_time_ms,
                        ho.time,
                        release.and_then(|info| info.time).or(Some(end_time)),
                        tint,
                        fail_transform
                            .map(|transform| transform.rotation)
                            .unwrap_or(0.0),
                        fail_state.is_some(),
                        state_time_ms,
                    );
                }
            }
            if ln_info.head_top_y <= bottom_y && ln_info.head_top_y + head_h as i32 >= top_y {
                let Some(head_texture) = head_name.as_ref().map(|name| {
                    self.gameplay_frame_texture(
                        name,
                        GameplayAnimationKind::Note,
                        animation_time_ms,
                        None,
                        None,
                        None,
                    )
                }) else {
                    continue;
                };
                let final_tex = self
                    .get_or_create_scaled_sprite(skin, &head_texture, target_w, head_h)
                    .unwrap_or(head_texture);
                let uv_rect = if layout.upside_down
                    && skin
                        .config
                        .resolved_note_flip_when_upside_down(col, NotePart::Head)
                {
                    [0.0, 1.0, 1.0, 0.0]
                } else {
                    [0.0, 0.0, 1.0, 1.0]
                };
                let mut sprite = SpriteCommand {
                    texture_id: final_tex,
                    x: shared_x,
                    y: ln_info.head_top_y,
                    width: target_w,
                    height: head_h,
                    tint,
                    uv_rect,
                    origin: if fail_state.is_some() {
                        [target_w as f32 / 2.0, head_h as f32 / 2.0]
                    } else {
                        [0.0, 0.0]
                    },
                    rotation: fail_transform
                        .map(|transform| transform.rotation)
                        .unwrap_or(0.0),
                    z_order: z_order::LN_HEAD,
                    ..Default::default()
                };
                self.rotate_columns_sprite(&mut sprite, layout, state_time_ms);
                planner.add_sprite(sprite);
                if self.ln_debug {
                    if let Some(j) = judgment {
                        let debug_tex_name = Self::get_debug_texture_name(idx, j.kind);
                        if self.has_texture(&debug_tex_name) {
                            if let Some(meta) = self.texture_meta.get(&debug_tex_name) {
                                let debug_x =
                                    Self::note_debug_label_x(layout, shared_x, target_w, meta.w);
                                let debug_y = Self::note_debug_head_y(
                                    layout,
                                    ln_info.head_top_y,
                                    head_h,
                                    meta.h,
                                );
                                let mut sprite = SpriteCommand {
                                    texture_id: debug_tex_name,
                                    x: debug_x,
                                    y: debug_y,
                                    width: meta.w,
                                    height: meta.h,
                                    tint: [1.0, 1.0, 1.0, 1.0],
                                    z_order: z_order::LN_HEAD + 1,
                                    ..Default::default()
                                };
                                self.rotate_columns_sprite(&mut sprite, layout, state_time_ms);
                                planner.add_sprite(sprite);
                            }
                        }
                    }
                }
            }
            let tail_end_y = ln_info.tail_top_y + tail_h as i32;
            let tail_in_bounds = ln_info.tail_top_y < bottom_y && tail_end_y > top_y;
            let tail_should_show = ln_info.show_ln && tail_in_bounds;
            if tail_should_show {
                let mut draw_tail_h = tail_h as i32;
                let mut draw_tail_y = ln_info.tail_top_y;
                let mut tail_uv = [0.0, 1.0, 1.0, 0.0];
                if ln_info.held {
                    // While holding, the tail is clipped so it never visually crosses the head anchor.
                    let head_anchor_y = ln_info.head_top_y + head_h as i32 / 2;
                    if layout.upside_down {
                        // Scrolling up the tail arrives from below, so the part to
                        // keep is the one further down and the sprite has to start
                        // at the anchor instead of at the tail top.
                        if ln_info.tail_top_y < head_anchor_y {
                            let visible_h = tail_end_y - head_anchor_y;
                            if visible_h <= 0 {
                                draw_tail_h = 0;
                            } else if (visible_h as u32) < tail_h {
                                let ratio = visible_h as f32 / tail_h as f32;
                                tail_uv[1] = ratio;
                                draw_tail_h = visible_h;
                                draw_tail_y = head_anchor_y;
                            }
                        }
                    } else if tail_end_y > head_anchor_y {
                        let visible_h = head_anchor_y - ln_info.tail_top_y;
                        if visible_h <= 0 {
                            draw_tail_h = 0;
                        } else if (visible_h as u32) < tail_h {
                            let ratio = visible_h as f32 / tail_h as f32;
                            tail_uv[3] = 1.0 - ratio;
                            draw_tail_h = visible_h;
                        }
                    }
                }
                if draw_tail_h <= 0 {
                    continue;
                }
                let Some(tail_texture) = tail_name.as_ref().map(|name| {
                    self.gameplay_frame_texture(
                        name,
                        GameplayAnimationKind::Note,
                        animation_time_ms,
                        None,
                        None,
                        None,
                    )
                }) else {
                    continue;
                };
                let tail_texture = self
                    .get_or_create_scaled_sprite(skin, &tail_texture, target_w, tail_h)
                    .unwrap_or(tail_texture);
                if layout.upside_down
                    && skin
                        .config
                        .resolved_note_flip_when_upside_down(col, NotePart::Tail)
                {
                    tail_uv = combine_uv_flip(tail_uv, true);
                }
                let mut sprite = SpriteCommand {
                    texture_id: tail_texture,
                    x: shared_x,
                    y: draw_tail_y,
                    width: target_w,
                    height: draw_tail_h as u32,
                    tint,
                    uv_rect: tail_uv,
                    origin: if fail_state.is_some() {
                        [target_w as f32 / 2.0, draw_tail_h as f32 / 2.0]
                    } else {
                        [0.0, 0.0]
                    },
                    rotation: fail_transform
                        .map(|transform| transform.rotation)
                        .unwrap_or(0.0),
                    z_order: z_order::LN_TAIL,
                    ..Default::default()
                };
                self.rotate_columns_sprite(&mut sprite, layout, state_time_ms);
                planner.add_sprite(sprite);
            }
        }
    }
    fn plan_long_note_body(
        &mut self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        skin: &SkinAssets,
        column_index: usize,
        width: u32,
        x: i32,
        clamped_top: i32,
        clamped_bottom: i32,
        full_top: i32,
        full_bottom: i32,
        animation_time_ms: f64,
        start_time_ms: i32,
        end_time_ms: Option<i32>,
        tint: Tint,
        rotation: f32,
        use_center_origin: bool,
        timestamp_ms: i32,
    ) {
        let body_name = skin
            .config
            .note_image_l
            .get(column_index)
            .or_else(|| skin.config.note_image.get(column_index))
            .filter(|name| !name.is_empty())
            .cloned();
        let Some(body_name) = body_name else {
            return;
        };
        let anchor_name = self.gameplay_anchor_texture(&body_name);
        let anchor_meta = self.texture_meta.get(&anchor_name).copied();
        let tile_h = Self::legacy_note_body_tile_height(anchor_meta.as_ref(), width);
        let texture_id = self.gameplay_frame_texture(
            &body_name,
            GameplayAnimationKind::LongNoteBody,
            animation_time_ms,
            Some(start_time_ms),
            end_time_ms,
            None,
        );
        let flip_y = layout.upside_down
            && skin
                .config
                .resolved_note_flip_when_upside_down(column_index, NotePart::Body);
        match skin.config.resolved_note_body_style(column_index) {
            // Stretch is a single scaled sprite; repeat styles preserve legacy tiled body behavior.
            NoteBodyStyle::Stretch => {
                let scaled_texture = self
                    .get_or_create_scaled_sprite(skin, &texture_id, width, tile_h)
                    .unwrap_or(texture_id);
                let height = (clamped_bottom - clamped_top).max(1) as u32;
                let mut sprite = SpriteCommand {
                    texture_id: scaled_texture,
                    x,
                    y: clamped_top,
                    width,
                    height,
                    tint,
                    uv_rect: if flip_y {
                        [0.0, 1.0, 1.0, 0.0]
                    } else {
                        [0.0, 0.0, 1.0, 1.0]
                    },
                    z_order: z_order::LN_BODY,
                    ..Default::default()
                };
                if use_center_origin {
                    sprite.origin = [width as f32 / 2.0, height as f32 / 2.0];
                    sprite.rotation = rotation;
                }
                self.rotate_columns_sprite(&mut sprite, layout, timestamp_ms);
                planner.add_sprite(sprite);
            }
            NoteBodyStyle::RepeatTop | NoteBodyStyle::RepeatBottom => {
                if let Some((cascade_texture, cascade_h)) = self
                    .get_or_create_legacy_cascade_body_texture(
                        skin,
                        &texture_id,
                        width,
                        skin.config.resolved_note_body_style(column_index),
                    )
                {
                    self.plan_legacy_cascade_body(
                        planner,
                        layout,
                        x,
                        clamped_top,
                        clamped_bottom,
                        full_top,
                        full_bottom,
                        width,
                        cascade_h,
                        &cascade_texture,
                        tint,
                        rotation,
                        use_center_origin,
                        layout.upside_down,
                        flip_y,
                        skin.config.resolved_note_body_style(column_index),
                        timestamp_ms,
                    );
                } else {
                    let scaled_texture = self
                        .get_or_create_scaled_sprite(skin, &texture_id, width, tile_h)
                        .unwrap_or(texture_id);
                    self.plan_legacy_repeating_body(
                        planner,
                        layout,
                        x,
                        clamped_top,
                        clamped_bottom,
                        full_top,
                        full_bottom,
                        width,
                        tile_h,
                        &scaled_texture,
                        tint,
                        rotation,
                        use_center_origin,
                        layout.upside_down,
                        flip_y,
                        timestamp_ms,
                    );
                }
            }
            NoteBodyStyle::RepeatTopAndBottom => {
                let scaled_texture = self
                    .get_or_create_scaled_sprite(skin, &texture_id, width, tile_h)
                    .unwrap_or(texture_id);
                self.plan_legacy_repeating_body(
                    planner,
                    layout,
                    x,
                    clamped_top,
                    clamped_bottom,
                    full_top,
                    full_bottom,
                    width,
                    tile_h,
                    &scaled_texture,
                    tint,
                    rotation,
                    use_center_origin,
                    layout.upside_down,
                    flip_y,
                    timestamp_ms,
                );
            }
        }
    }
    fn get_or_create_legacy_cascade_body_texture(
        &mut self,
        skin: &SkinAssets,
        texture_name: &str,
        width: u32,
        style: NoteBodyStyle,
    ) -> Option<(String, u32)> {
        if !matches!(
            style,
            NoteBodyStyle::RepeatTop | NoteBodyStyle::RepeatBottom
        ) {
            return None;
        }
        let max_dim = crate::utils::image_proc::MAX_TEXTURE_DIM;
        let target_w = width.max(1).min(max_dim);
        let normalized_texture_name = crate::types::SkinAssets::normalize_key(texture_name);
        let cache_key = format!(
            "legacy-cascade|{:?}|{}|{}",
            style, normalized_texture_name, target_w
        );
        if let Some(cached) = self.sprite_scaled.get(&cache_key).cloned() {
            let height = self.texture_meta.get(&cached)?.h;
            return Some((cached, height));
        }
        let data = skin.find_image(texture_name)?;
        let img = crate::utils::image_proc::load_rgba(data)?;
        // Cascade bodies trim repeated edge rows before scaling so the unique cap is not duplicated.
        let cropped = Self::trim_legacy_cascade_body_image(&img, style);
        let (orig_w, orig_h) = cropped.dimensions();
        let target_h = ((orig_h as f32) * (target_w as f32 / orig_w.max(1) as f32))
            .round()
            .max(1.0) as u32;
        let target_h = target_h.min(max_dim);
        let scaled = if self
            .linear_sampled_textures
            .contains(&normalized_texture_name)
        {
            crate::utils::image_proc::resize_exact_gameplay_note(&cropped, target_w, target_h)
        } else {
            crate::utils::image_proc::resize_exact_alpha_safe(&cropped, target_w, target_h)
        };
        let scaled_name = crate::types::SkinAssets::normalize_key(&format!(
            "{}@legacy-cascade-{:?}-{}x{}",
            normalized_texture_name, style, target_w, target_h
        ));
        if !self.load_texture_rgba(&scaled_name, scaled.as_raw(), target_w, target_h) {
            return None;
        }
        if self
            .linear_sampled_textures
            .contains(&normalized_texture_name)
        {
            self.linear_sampled_textures.insert(scaled_name.clone());
        }
        self.sprite_scaled.insert(cache_key, scaled_name.clone());
        Some((scaled_name, target_h))
    }
    fn trim_legacy_cascade_body_image(
        img: &image::RgbaImage,
        style: NoteBodyStyle,
    ) -> image::RgbaImage {
        let (_, h) = img.dimensions();
        if h <= 1 {
            return img.clone();
        }
        match style {
            NoteBodyStyle::RepeatBottom => {
                let mut repeat_start = h - 1;
                while repeat_start > 0 && Self::image_rows_equal(img, repeat_start - 1, h - 1) {
                    repeat_start -= 1;
                }
                crate::utils::image_proc::extract_vertical_range(img, 0, repeat_start + 1)
            }
            NoteBodyStyle::RepeatTop => {
                let mut repeat_end = 0;
                while repeat_end + 1 < h && Self::image_rows_equal(img, repeat_end + 1, 0) {
                    repeat_end += 1;
                }
                crate::utils::image_proc::extract_vertical_range(
                    img,
                    repeat_end,
                    h.saturating_sub(repeat_end),
                )
            }
            _ => img.clone(),
        }
    }
    fn image_rows_equal(img: &image::RgbaImage, row_a: u32, row_b: u32) -> bool {
        let stride = (img.width() * 4) as usize;
        let start_a = row_a as usize * stride;
        let start_b = row_b as usize * stride;
        let raw = img.as_raw();
        raw[start_a..start_a + stride] == raw[start_b..start_b + stride]
    }
    fn plan_legacy_cascade_body(
        &self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        x: i32,
        visible_top: i32,
        visible_bottom: i32,
        full_top: i32,
        full_bottom: i32,
        width: u32,
        cap_h: u32,
        texture_id: &str,
        tint: Tint,
        rotation: f32,
        use_center_origin: bool,
        upside_down: bool,
        flip_y: bool,
        style: NoteBodyStyle,
        timestamp_ms: i32,
    ) {
        let Some((visible_start_d, visible_end_d)) = Self::legacy_body_visible_distances(
            visible_top,
            visible_bottom,
            full_top,
            full_bottom,
            upside_down,
        ) else {
            return;
        };
        let full_len = full_bottom - full_top;
        let cap_h_i32 = cap_h.max(1) as i32;
        match style {
            NoteBodyStyle::RepeatBottom => {
                let textured_len = full_len.min(cap_h_i32);
                self.plan_legacy_body_texture_segment(
                    planner,
                    layout,
                    x,
                    full_top,
                    full_bottom,
                    width,
                    texture_id,
                    tint,
                    rotation,
                    use_center_origin,
                    upside_down,
                    flip_y,
                    visible_start_d,
                    visible_end_d,
                    0,
                    textured_len,
                    0,
                    cap_h,
                    timestamp_ms,
                );
                self.plan_legacy_body_row_segment(
                    planner,
                    layout,
                    x,
                    full_top,
                    full_bottom,
                    width,
                    texture_id,
                    tint,
                    rotation,
                    use_center_origin,
                    upside_down,
                    flip_y,
                    visible_start_d,
                    visible_end_d,
                    cap_h_i32,
                    full_len,
                    cap_h.saturating_sub(1) as f32 + 0.5,
                    cap_h,
                    timestamp_ms,
                );
            }
            NoteBodyStyle::RepeatTop => {
                let textured_len = full_len.min(cap_h_i32);
                let repeated_len = full_len.saturating_sub(cap_h_i32);
                let texture_start_d = cap_h_i32.saturating_sub(textured_len);
                self.plan_legacy_body_row_segment(
                    planner,
                    layout,
                    x,
                    full_top,
                    full_bottom,
                    width,
                    texture_id,
                    tint,
                    rotation,
                    use_center_origin,
                    upside_down,
                    flip_y,
                    visible_start_d,
                    visible_end_d,
                    0,
                    repeated_len,
                    0.5,
                    cap_h,
                    timestamp_ms,
                );
                self.plan_legacy_body_texture_segment(
                    planner,
                    layout,
                    x,
                    full_top,
                    full_bottom,
                    width,
                    texture_id,
                    tint,
                    rotation,
                    use_center_origin,
                    upside_down,
                    flip_y,
                    visible_start_d,
                    visible_end_d,
                    repeated_len,
                    full_len,
                    texture_start_d,
                    cap_h,
                    timestamp_ms,
                );
            }
            _ => {}
        }
    }
    fn legacy_body_visible_distances(
        visible_top: i32,
        visible_bottom: i32,
        full_top: i32,
        full_bottom: i32,
        upside_down: bool,
    ) -> Option<(i32, i32)> {
        if visible_bottom <= visible_top || full_bottom <= full_top {
            return None;
        }
        // Use distance along the LN body so upside-down and normal scroll share the same segment math.
        let (visible_start_d, visible_end_d) = if upside_down {
            (full_bottom - visible_bottom, full_bottom - visible_top)
        } else {
            (visible_top - full_top, visible_bottom - full_top)
        };
        (visible_end_d > visible_start_d).then_some((visible_start_d, visible_end_d))
    }
    fn legacy_body_distance_to_screen(
        full_top: i32,
        full_bottom: i32,
        upside_down: bool,
        start_d: i32,
        end_d: i32,
    ) -> (i32, i32) {
        if upside_down {
            (full_bottom - end_d, full_bottom - start_d)
        } else {
            (full_top + start_d, full_top + end_d)
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn plan_legacy_body_texture_segment(
        &self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        x: i32,
        full_top: i32,
        full_bottom: i32,
        width: u32,
        texture_id: &str,
        tint: Tint,
        rotation: f32,
        use_center_origin: bool,
        upside_down: bool,
        flip_y: bool,
        visible_start_d: i32,
        visible_end_d: i32,
        body_start_d: i32,
        body_end_d: i32,
        texture_start_d: i32,
        texture_h: u32,
        timestamp_ms: i32,
    ) {
        let draw_start_d = visible_start_d.max(body_start_d);
        let draw_end_d = visible_end_d.min(body_end_d);
        if draw_end_d <= draw_start_d {
            return;
        }
        let (draw_top, draw_bottom) = Self::legacy_body_distance_to_screen(
            full_top,
            full_bottom,
            upside_down,
            draw_start_d,
            draw_end_d,
        );
        let uv_top =
            (texture_start_d + (draw_start_d - body_start_d)) as f32 / texture_h.max(1) as f32;
        let uv_bottom =
            (texture_start_d + (draw_end_d - body_start_d)) as f32 / texture_h.max(1) as f32;
        let mut uv = [0.0, uv_top, 1.0, uv_bottom];
        if flip_y {
            uv = combine_uv_flip(uv, true);
        }
        self.add_legacy_body_sprite(
            planner,
            layout,
            x,
            width,
            texture_id,
            draw_top,
            draw_bottom,
            tint,
            uv,
            rotation,
            use_center_origin,
            timestamp_ms,
        );
    }
    #[allow(clippy::too_many_arguments)]
    fn plan_legacy_body_row_segment(
        &self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        x: i32,
        full_top: i32,
        full_bottom: i32,
        width: u32,
        texture_id: &str,
        tint: Tint,
        rotation: f32,
        use_center_origin: bool,
        upside_down: bool,
        flip_y: bool,
        visible_start_d: i32,
        visible_end_d: i32,
        body_start_d: i32,
        body_end_d: i32,
        row_center_d: f32,
        texture_h: u32,
        timestamp_ms: i32,
    ) {
        let draw_start_d = visible_start_d.max(body_start_d);
        let draw_end_d = visible_end_d.min(body_end_d);
        if draw_end_d <= draw_start_d {
            return;
        }
        let (draw_top, draw_bottom) = Self::legacy_body_distance_to_screen(
            full_top,
            full_bottom,
            upside_down,
            draw_start_d,
            draw_end_d,
        );
        let row_center = row_center_d / texture_h.max(1) as f32;
        let mut uv = [0.0, row_center, 1.0, row_center];
        if flip_y {
            uv = combine_uv_flip(uv, true);
        }
        self.add_legacy_body_sprite(
            planner,
            layout,
            x,
            width,
            texture_id,
            draw_top,
            draw_bottom,
            tint,
            uv,
            rotation,
            use_center_origin,
            timestamp_ms,
        );
    }
    fn add_legacy_body_sprite(
        &self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        x: i32,
        width: u32,
        texture_id: &str,
        draw_top: i32,
        draw_bottom: i32,
        tint: Tint,
        uv_rect: [f32; 4],
        rotation: f32,
        use_center_origin: bool,
        timestamp_ms: i32,
    ) {
        let draw_h = (draw_bottom - draw_top).max(1) as u32;
        let mut sprite = SpriteCommand {
            texture_id: texture_id.to_string(),
            x,
            y: draw_top,
            width,
            height: draw_h,
            tint,
            uv_rect,
            z_order: z_order::LN_BODY,
            ..Default::default()
        };
        if use_center_origin {
            sprite.origin = [width as f32 / 2.0, draw_h as f32 / 2.0];
            sprite.rotation = rotation;
        }
        self.rotate_columns_sprite(&mut sprite, layout, timestamp_ms);
        planner.add_sprite(sprite);
    }
    fn plan_legacy_repeating_body(
        &self,
        planner: &mut SpritePlanner,
        layout: &ManiaLayoutInfo,
        x: i32,
        visible_top: i32,
        visible_bottom: i32,
        full_top: i32,
        full_bottom: i32,
        width: u32,
        tile_h: u32,
        texture_id: &str,
        tint: Tint,
        rotation: f32,
        use_center_origin: bool,
        upside_down: bool,
        flip_y: bool,
        timestamp_ms: i32,
    ) {
        if visible_bottom <= visible_top || full_bottom <= full_top {
            return;
        }
        let tile_h_i32 = tile_h.max(1) as i32;
        let (visible_start_d, visible_end_d) = if upside_down {
            (full_bottom - visible_bottom, full_bottom - visible_top)
        } else {
            (visible_top - full_top, visible_bottom - full_top)
        };
        if visible_end_d <= visible_start_d {
            return;
        }
        // Tile indices are computed from the full body, not the clipped viewport, to keep phase stable.
        let first_tile = visible_start_d.div_euclid(tile_h_i32);
        let last_tile = (visible_end_d - 1).div_euclid(tile_h_i32);
        for tile_idx in first_tile..=last_tile {
            let tile_start_d = tile_idx * tile_h_i32;
            let tile_end_d = tile_start_d + tile_h_i32;
            let draw_start_d = visible_start_d.max(tile_start_d);
            let draw_end_d = visible_end_d.min(tile_end_d);
            if draw_end_d <= draw_start_d {
                continue;
            }
            let (draw_top, draw_bottom) = if upside_down {
                (full_bottom - draw_end_d, full_bottom - draw_start_d)
            } else {
                (full_top + draw_start_d, full_top + draw_end_d)
            };
            let draw_h = (draw_bottom - draw_top).max(1) as u32;
            let uv_top = (draw_start_d - tile_start_d) as f32 / tile_h.max(1) as f32;
            let uv_bottom = (draw_end_d - tile_start_d) as f32 / tile_h.max(1) as f32;
            let mut uv = [0.0, uv_top, 1.0, uv_bottom];
            if flip_y {
                uv = combine_uv_flip(uv, true);
            }
            let mut sprite = SpriteCommand {
                texture_id: texture_id.to_string(),
                x,
                y: draw_top,
                width,
                height: draw_h,
                tint,
                uv_rect: uv,
                z_order: z_order::LN_BODY,
                ..Default::default()
            };
            if use_center_origin {
                sprite.origin = [width as f32 / 2.0, draw_h as f32 / 2.0];
                sprite.rotation = rotation;
            }
            self.rotate_columns_sprite(&mut sprite, layout, timestamp_ms);
            planner.add_sprite(sprite);
        }
    }
    fn compute_ln_render_info(
        &self,
        ho: &HitObject,
        timestamp: i32,
        judgment: Option<RenderJudgment>,
        release: Option<LnReleaseInfo>,
        key_pressed: bool,
        _windows: Option<&Windows>,
        upside_down: bool,
        judgment_y: i32,
        head_h: i32,
        tail_h: i32,
        head_bottom_y: i32,
        release_head_bottom_y: Option<i32>,
        tail_top_y: i32,
        top_y: i32,
        bottom_y: i32,
    ) -> LnRenderInfo {
        let end_time = ho.end_time.unwrap_or(ho.time);
        let mut info = LnRenderInfo::default();
        info.head_top_y = scroll_piece_top_y(upside_down, head_bottom_y, head_h);
        info.tail_top_y = tail_top_y;
        let head_center_y = info.head_top_y + head_h / 2;
        let tail_center_y = info.tail_top_y + tail_h / 2;
        info.body_top_y = head_center_y.min(tail_center_y);
        info.body_bottom_y = head_center_y.max(tail_center_y);
        if end_time > ho.time && info.body_bottom_y <= info.body_top_y {
            info.body_bottom_y = info.body_top_y + 1;
        }
        let j_kind = judgment.map(|j| j.kind);
        let rel_kind = release.map(|r| r.kind).unwrap_or(ReleaseKind::None);
        let rel_time = release.and_then(|r| r.time);
        let is_double_tap = release.map(|r| r.double_tap).unwrap_or(false);
        let rel_is_miss = rel_kind == ReleaseKind::Miss;
        let rel_is_50 = rel_kind == ReleaseKind::Hit50;
        let rel_is_good = rel_kind != ReleaseKind::None && !rel_is_miss;
        let frame_eps = 1000.0 / self.cfg.fps.max(1) as f32;
        let press_time = judgment.and_then(|j| j.press_time);
        let has_press = press_time.is_some();
        let press_after_end = press_time.map(|pt| pt > end_time).unwrap_or(false);
        let press_dim_started = press_time
            .map(|pt| timestamp >= (pt as f32 - frame_eps * 0.75) as i32)
            .unwrap_or(false);
        let press_hold_started = press_time.map(|pt| timestamp >= pt).unwrap_or(false);
        let press_is_late = press_time.map(|pt| pt > ho.time).unwrap_or(false);
        let late_fifty = j_kind == Some(JudgmentKind::Hit50) && press_is_late;
        let head_good = j_kind != Some(JudgmentKind::Miss) && j_kind != Some(JudgmentKind::Hit50);
        // Late 50s still draw as a held LN until release logic decides how the body should end.
        let head_good_for_visuals = head_good || late_fifty;
        let mut head_miss_dim = j_kind == Some(JudgmentKind::Miss) && press_dim_started;
        let mut head_fifty_dim = j_kind == Some(JudgmentKind::Hit50) && press_dim_started;
        if late_fifty {
            head_miss_dim = false;
            head_fifty_dim = false;
        }
        let release_active = rel_time
            .map(|rt| timestamp >= (rt as f32 - frame_eps * 0.75) as i32)
            .unwrap_or(false);
        let mut miss_dim = head_miss_dim;
        let mut fifty_dim = head_fifty_dim;
        if release_active {
            if rel_is_miss {
                miss_dim = true;
                fifty_dim = false;
            } else if rel_is_50 && !miss_dim {
                fifty_dim = true;
            }
        }
        const LATE_HOLD_TIMEOUT_MS: i32 = 200;
        let mut effective_end_time = end_time;
        if let Some(rt) = rel_time {
            if rt > end_time {
                // Late releases get a short visual grace period, but cannot extend the LN indefinitely.
                effective_end_time = rt.min(end_time + LATE_HOLD_TIMEOUT_MS);
            }
        }
        let good_release_finished = !is_double_tap
            && !rel_is_50
            && rel_is_good
            && rel_time.is_some_and(|rt| timestamp >= rt);
        // Notes leave the playfield past the edge opposite the one they entered
        // from. Testing only the bottom made every not-yet-reached LN look like it
        // had already left when scrolling up, which retired it before it was drawn.
        let tail_off_screen = if upside_down {
            tail_top_y + tail_h < top_y - 50
        } else {
            tail_top_y > bottom_y + 50
        };
        // Bad releases keep the LN visible until the tail clears the playfield.
        let pass_through_finished = (is_double_tap || rel_is_50 || rel_is_miss) && tail_off_screen;
        let consumed =
            press_time.map(|pt| pt <= timestamp).unwrap_or(false) && timestamp >= ho.time;
        let hold_window_active = has_press
            && press_hold_started
            && timestamp >= ho.time
            && timestamp <= effective_end_time
            && rel_time.is_none_or(|rt| timestamp < rt);
        let hold_input_active = if rel_time.is_some() {
            // Replay releases are authoritative; key state may already be frozen by fail logic.
            true
        } else {
            key_pressed
        };
        let held =
            head_good_for_visuals && !is_double_tap && hold_window_active && hold_input_active;
        let normal_end = timestamp >= (effective_end_time as f32 - frame_eps * 0.75) as i32
            && !rel_is_miss
            && !miss_dim
            && !fifty_dim
            && !is_double_tap
            && !press_after_end
            && (!held || timestamp >= effective_end_time);
        let finished = good_release_finished || pass_through_finished || normal_end;
        info.finished_normally = finished;
        if finished {
            return info;
        }
        let is_broken = rel_kind == ReleaseKind::Miss || j_kind == Some(JudgmentKind::Miss);
        let has_bad_judgment = miss_dim
            || fifty_dim
            || (release_active && rel_is_miss)
            || (release_active && rel_is_50);
        let show_ln = held
            || !consumed
            || release_active
            || has_bad_judgment
            || is_broken
            || miss_dim
            || fifty_dim
            || is_double_tap;
        info.is_broken = is_broken;
        info.held = held;
        info.show_ln = show_ln;
        if !info.show_ln {
            return info;
        }
        info.head_tint = if miss_dim {
            0.7
        } else if fifty_dim {
            0.85
        } else {
            1.0
        };
        info.allow_pass = !info.held;
        if info.held {
            let clamped_head_bottom = if upside_down {
                judgment_y.max(head_bottom_y)
            } else {
                judgment_y.min(head_bottom_y)
            };
            info.head_top_y = scroll_piece_top_y(upside_down, clamped_head_bottom, head_h);
            let head_center_y = info.head_top_y + head_h / 2;
            // The tail reaches the receptor from below when scrolling up.
            let tail_has_reached_receptor = if upside_down {
                info.tail_top_y <= judgment_y
            } else {
                info.tail_top_y + tail_h >= judgment_y
            };
            if tail_has_reached_receptor {
                info.tail_top_y = scroll_piece_top_y(upside_down, judgment_y, tail_h);
                info.body_top_y = head_center_y;
                info.body_bottom_y = head_center_y;
            } else {
                let tail_center_y = info.tail_top_y + tail_h / 2;
                info.body_top_y = tail_center_y.min(head_center_y);
                info.body_bottom_y = tail_center_y.max(head_center_y);
            }
        } else if release_active && (rel_is_miss || rel_is_50 || is_double_tap) {
            if let Some(release_head_bottom_y) = release_head_bottom_y {
                let release_head_top_y =
                    scroll_piece_top_y(upside_down, release_head_bottom_y, head_h);
                let release_body_anchor_y = release_head_top_y + head_h / 2;
                let tail_center_y = info.tail_top_y + tail_h / 2;
                info.head_top_y = release_head_top_y;
                info.body_top_y = release_body_anchor_y.min(tail_center_y);
                info.body_bottom_y = release_body_anchor_y.max(tail_center_y);
                if end_time > ho.time && info.body_bottom_y <= info.body_top_y {
                    info.body_bottom_y = info.body_top_y + 1;
                }
            }
        }
        info
    }
    fn legacy_note_piece_height(
        meta: Option<&TextureMeta>,
        draw_width: u32,
        width_for_note_height_scale: Option<u32>,
        scale_y: f32,
    ) -> u32 {
        let Some(meta) = meta else {
            return draw_width.max(1);
        };
        let texture_width = meta.w.max(1) as f32;
        let texture_height = meta.h.max(1) as f32;
        let note_height = width_for_note_height_scale
            .map(|value| (value as f32 * scale_y).max(1.0))
            .unwrap_or(draw_width.max(1) as f32);
        // widthForNoteHeightScale decouples note height from the current column width.
        (texture_height * (note_height / texture_width))
            .round()
            .max(1.0) as u32
    }
}
fn scroll_target_y(layout: &ManiaLayoutInfo, judgment_y: i32, dist_px: f32) -> i32 {
    if layout.upside_down {
        judgment_y + dist_px.round() as i32
    } else {
        judgment_y - dist_px.round() as i32
    }
}
/// Top edge of a note piece whose leading edge sits at `lead_y`. The leading edge
/// is the sprite's bottom when scrolling down and its top when scrolling up, so a
/// piece always sits on the side of the judgement line it is travelling from.
fn scroll_piece_top_y(upside_down: bool, lead_y: i32, height: i32) -> i32 {
    if upside_down {
        lead_y
    } else {
        lead_y - height
    }
}
fn combine_uv_flip(mut uv: [f32; 4], flip_y: bool) -> [f32; 4] {
    if flip_y {
        uv = [uv[0], uv[3], uv[2], uv[1]];
    }
    uv
}
