use super::*;
impl ManiaVideoConverter {
    pub(crate) fn build_layout(
        &self,
        key_count: u8,
        skin: &SkinAssets,
        hud_config: Option<&HudConfig>,
    ) -> ManiaLayoutInfo {
        let canvas_h = self.settings.height;
        let key_count = key_count as usize;
        let base_height = 480.0f32;
        let base_width = 640.0f32;
        // osu!mania skin coordinates are authored in a 640x480 space; scale from height.
        let scale_y = canvas_h as f32 / base_height;
        let scale_from_height = scale_y;
        let mut width_list: Vec<f32> = skin
            .config
            .column_widths
            .as_ref()
            .map(|w| w.iter().take(key_count).map(|&x| x as f32).collect())
            .unwrap_or_else(|| vec![64.0; key_count]);
        while width_list.len() < key_count {
            width_list.push(width_list.last().copied().unwrap_or(64.0));
        }
        let spacing_count = key_count.saturating_sub(1);
        let mut spacing_list: Vec<f32> = skin
            .config
            .column_spacing
            .as_ref()
            .map(|s| s.iter().take(spacing_count).map(|&x| x as f32).collect())
            .unwrap_or_default();
        while spacing_list.len() < spacing_count {
            spacing_list.push(0.0);
        }
        let logical_width: f32 = width_list.iter().sum::<f32>() + spacing_list.iter().sum::<f32>();
        let start_logical: f32 = if let Some(cs) = skin.config.column_start {
            cs as f32
        } else if let Some(cr) = skin.config.column_right {
            base_width - logical_width - cr as f32
        } else {
            (base_width - logical_width) / 2.0
        };
        let mut columns: Vec<ColumnLayout> = Vec::with_capacity(key_count);
        let mut cursor = start_logical;
        for i in 0..key_count {
            let w = width_list[i];
            let x_px = (cursor * scale_from_height).round() as i32;
            let width_px = (w * scale_from_height).round() as u32;
            columns.push(ColumnLayout {
                x: x_px,
                width: width_px,
            });
            cursor += w;
            if i + 1 < key_count {
                cursor += spacing_list[i];
            }
        }
        let mut stage_width = (logical_width * scale_from_height).round() as u32;
        let stage_height = (base_height * scale_from_height).round() as u32;
        let mut stage_x = (start_logical * scale_from_height).round() as i32;
        let mut stage_top_override: Option<i32> = None;
        // HUD column overrides apply after skin geometry so editor layouts can move the playfield.
        if let Some(columns_cfg) = hud_config.and_then(|cfg| cfg.elements.columns.as_ref()) {
            stage_top_override = columns_cfg
                .base
                .y
                .filter(|v| v.is_finite())
                .map(|v| v.round() as i32);
            let mut next_columns = columns.clone();
            let mut applied_items = 0usize;
            let base_x = columns_cfg
                .base
                .x
                .filter(|v| v.is_finite())
                .map(|v| v.round() as i32);
            let base_width = columns_cfg
                .base
                .width
                .filter(|v| v.is_finite() && *v > 0.0)
                .map(|v| v.round().max(1.0) as u32);
            let global_gap = columns_cfg.gap.filter(|v| v.is_finite() && *v >= 0.0);
            let has_gap_override = global_gap.is_some()
                || columns_cfg.items.iter().any(|item| {
                    item.gap_after
                        .filter(|v| v.is_finite() && *v >= 0.0)
                        .is_some()
                });
            if has_gap_override && !next_columns.is_empty() {
                let mut cursor = base_x.unwrap_or(next_columns[0].x);
                for idx in 0..next_columns.len() {
                    next_columns[idx].x = cursor;
                    let gap = if idx + 1 < next_columns.len() {
                        columns_cfg
                            .items
                            .iter()
                            .find(|item| item.index == Some(idx))
                            .and_then(|item| item.gap_after)
                            .filter(|v| v.is_finite() && *v >= 0.0)
                            .or(global_gap)
                            .unwrap_or_else(|| {
                                next_columns
                                    .get(idx + 1)
                                    .map(|right| {
                                        (right.x
                                            - (next_columns[idx].x
                                                + next_columns[idx].width as i32))
                                            .max(0) as f32
                                    })
                                    .unwrap_or(0.0)
                            })
                    } else {
                        0.0
                    };
                    cursor += next_columns[idx].width as i32 + gap.round() as i32;
                }
                applied_items = key_count;
            }
            for item in &columns_cfg.items {
                let idx = match item.index {
                    Some(v) if v < next_columns.len() => v,
                    _ => continue,
                };
                let base = next_columns[idx];
                let next_x = item
                    .x
                    .filter(|v| v.is_finite())
                    .map(|v| v.round() as i32)
                    .unwrap_or(base.x);
                let next_width = item
                    .width
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .map(|v| v.round().max(1.0) as u32)
                    .unwrap_or(base.width.max(1));
                next_columns[idx] = ColumnLayout {
                    x: next_x,
                    width: next_width,
                };
                applied_items += 1;
            }
            if applied_items == 0 && (base_x.is_some() || base_width.is_some()) {
                let current_left = columns.iter().map(|c| c.x).min().unwrap_or(stage_x);
                let current_right = columns
                    .iter()
                    .map(|c| c.x + c.width as i32)
                    .max()
                    .unwrap_or(stage_x + stage_width as i32);
                let current_width = (current_right - current_left).max(1) as f32;
                let target_left = base_x.unwrap_or(current_left);
                let target_width = base_width.unwrap_or(current_width.round() as u32);
                let scale = target_width as f32 / current_width;
                next_columns = columns
                    .iter()
                    .map(|column| {
                        let rel_x = (column.x - current_left) as f32;
                        let x = target_left + (rel_x * scale).round() as i32;
                        let width = (column.width as f32 * scale).round().max(1.0) as u32;
                        ColumnLayout { x, width }
                    })
                    .collect();
                applied_items = key_count;
            }
            if applied_items > 0 {
                if base_x.is_some() || base_width.is_some() {
                    let current_left = next_columns.iter().map(|c| c.x).min().unwrap_or(stage_x);
                    let current_right = next_columns
                        .iter()
                        .map(|c| c.x + c.width as i32)
                        .max()
                        .unwrap_or(stage_x + stage_width as i32);
                    let current_width = (current_right - current_left).max(1) as f32;
                    let target_left = base_x.unwrap_or(current_left);
                    let target_width = base_width.unwrap_or(current_width.round() as u32);
                    let scale = target_width as f32 / current_width;
                    next_columns = next_columns
                        .iter()
                        .map(|column| {
                            let rel_x = (column.x - current_left) as f32;
                            let x = target_left + (rel_x * scale).round() as i32;
                            let width = (column.width as f32 * scale).round().max(1.0) as u32;
                            ColumnLayout { x, width }
                        })
                        .collect();
                }
                columns = next_columns;
                if let (Some(bx), Some(bw)) = (base_x, base_width) {
                    stage_x = bx;
                    stage_width = bw;
                } else {
                    let left = columns.iter().map(|c| c.x).min().unwrap_or(stage_x);
                    let right = columns
                        .iter()
                        .map(|c| c.x + c.width as i32)
                        .max()
                        .unwrap_or(stage_x + stage_width as i32);
                    stage_x = left;
                    stage_width = (right - left).max(1) as u32;
                }
            }
        }
        let hit_position = skin.config.hit_position.unwrap_or(402) as f32;
        let logical_hit_position = if skin.config.resolved_upside_down() {
            base_height - hit_position
        } else {
            hit_position
        };
        let default_hit_y = (logical_hit_position * scale_from_height).round() as i32;
        let default_top_y = (default_hit_y - stage_height as i32).max(0);
        let top_y = stage_top_override.unwrap_or(default_top_y);
        let hit_y = if stage_top_override.is_some() {
            // Moving the stage top also moves the hit line so receptor distance stays unchanged.
            default_hit_y + (top_y - default_top_y)
        } else {
            default_hit_y
        };
        let bottom_y = top_y + stage_height as i32;
        println!(
            "   [layout] columnStart={:?}, columnWidths={:?}, hitPos={}",
            skin.config.column_start, skin.config.column_widths, hit_position
        );
        println!(
            "   [layout] stageX={}, stageW={}, topY={}, hitY={}, scale={:.3}",
            stage_x, stage_width, top_y, hit_y, scale_from_height
        );
        ManiaLayoutInfo {
            stage: StageLayout {
                x: stage_x,
                y: top_y,
                width: stage_width,
                height: stage_height,
                hit_y,
                top_y,
                bottom_y,
            },
            columns,
            scale_y: scale_from_height,
            upside_down: skin.config.resolved_upside_down(),
        }
    }
    pub(crate) fn compute_barlines(
        &self,
        beatmap: &Beatmap,
        start_ms: i32,
        end_ms: i32,
    ) -> Vec<i32> {
        let mut barlines = Vec::new();
        let base_tp = beatmap
            .timing_points
            .iter()
            .find(|tp| tp.uninherited)
            .cloned()
            .unwrap_or_default();
        if base_tp.beat_length <= 0.0 {
            return barlines;
        }
        let bar_duration = base_tp.beat_length * base_tp.meter as f64;
        let mut t = base_tp.time;
        while t > start_ms as f64 {
            t -= bar_duration;
        }
        while t <= end_ms as f64 {
            if t >= start_ms as f64 {
                barlines.push(t as i32);
            }
            t += bar_duration;
        }
        barlines
    }
    pub(crate) fn build_key_mask_timeline(
        &self,
        actions: &[crate::types::replay::KeyAction],
    ) -> Vec<KeyMaskEvent> {
        // Store only mask transitions; per-frame lookup can then reuse the last stable state.
        let mut events = Vec::with_capacity(actions.len());
        let mut current_mask = 0u32;
        for action in actions {
            let mask = action.keys_mask;
            if mask != current_mask {
                events.push(KeyMaskEvent {
                    time: action.time,
                    mask,
                });
                current_mask = mask;
            }
        }
        events
    }
    pub(crate) fn get_key_mask_at(&self, time: i32, timeline: &[KeyMaskEvent]) -> u32 {
        let mut mask = 0u32;
        for event in timeline {
            if event.time > time {
                break;
            }
            mask = event.mask;
        }
        mask
    }
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct KeyMaskEvent {
    pub(crate) time: i32,
    pub(crate) mask: u32,
}
