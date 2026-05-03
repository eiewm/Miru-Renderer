use super::assets::{map_column_family, ManiaFamily};
#[derive(Debug, Clone)]
pub struct SkinConfig {
    pub column_width: Vec<f32>,
    pub column_spacing: Vec<f32>,
    pub column_start: Option<f32>,
    pub column_right: Option<f32>,
    pub hit_position: f32,
    pub width_for_note_height_scale: Option<f32>,
    pub column_family: Option<Vec<ManiaFamily>>,
}
impl Default for SkinConfig {
    fn default() -> Self {
        Self {
            column_width: vec![64.0; 4],
            column_spacing: vec![0.0; 3],
            column_start: None,
            column_right: None,
            hit_position: 402.0,
            width_for_note_height_scale: None,
            column_family: None,
        }
    }
}
#[derive(Debug, Clone)]
pub struct StageLayout {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub top_y: i32,
    pub hit_y: i32,
    pub bottom_y: i32,
    pub center_x: i32,
}
#[derive(Debug, Clone)]
pub struct ColumnLayout {
    pub x: i32,
    pub width: i32,
    pub family: ManiaFamily,
    pub note_scale: f32,
}
#[derive(Debug, Clone)]
pub struct ManiaLayout {
    pub scale_x: f32,
    pub scale_y: f32,
    pub stage: StageLayout,
    pub columns: Vec<ColumnLayout>,
}
const BASE_HEIGHT: f32 = 480.0;
const BASE_WIDTH: f32 = 640.0;
// osu!mania skin layout values are authored in a 640x480 logical coordinate space.
pub fn compute_mania_layout(
    cfg: &SkinConfig,
    _canvas_w: u32,
    canvas_h: u32,
    key_count: u8,
    bottom_safe: i32,
) -> ManiaLayout {
    let scale = canvas_h as f32 / BASE_HEIGHT;
    let mut widths: Vec<f32> = cfg
        .column_width
        .iter()
        .take(key_count as usize)
        .copied()
        .collect();
    while widths.len() < key_count as usize {
        // Stable repeats the last configured width when a skin omits later columns.
        widths.push(*widths.last().unwrap_or(&64.0));
    }
    let spacing_count = (key_count as usize).saturating_sub(1);
    let mut spacings: Vec<f32> = cfg
        .column_spacing
        .iter()
        .take(spacing_count)
        .copied()
        .collect();
    while spacings.len() < spacing_count {
        spacings.push(0.0);
    }
    let logical_width: f32 = widths.iter().sum::<f32>() + spacings.iter().sum::<f32>();
    let start_logical = if let Some(cs) = cfg.column_start {
        cs
    } else if let Some(cr) = cfg.column_right {
        // ColumnRight is a right margin in logical pixels, not an absolute x coordinate.
        BASE_WIDTH - logical_width - cr
    } else {
        (BASE_WIDTH - logical_width) / 2.0
    };
    let mut columns = Vec::with_capacity(key_count as usize);
    let mut cursor = start_logical;
    let family_override = cfg.column_family.as_deref();
    for i in 0..key_count {
        let w = widths[i as usize];
        let x_px = (cursor * scale).round() as i32;
        let width_px = (w * scale).round() as i32;
        let family = map_column_family(key_count, i, family_override);
        columns.push(ColumnLayout {
            x: x_px,
            width: width_px,
            family,
            note_scale: 1.0,
        });
        cursor += w;
        if (i as usize) + 1 < key_count as usize {
            cursor += spacings[i as usize];
        }
    }
    let stage_width = (logical_width * scale).round() as i32;
    let stage_height = (BASE_HEIGHT * scale).round() as i32;
    let stage_x = (start_logical * scale).round() as i32;
    let hit_y = (cfg.hit_position * scale).round() as i32;
    let top_y = (hit_y - stage_height).max(0);
    let bottom_y = hit_y + bottom_safe;
    let ref_width = cfg
        .width_for_note_height_scale
        .unwrap_or_else(|| columns.first().map(|c| c.width as f32).unwrap_or(1.0));
    // Note scale follows the configured reference width instead of each column's raw width.
    for col in &mut columns {
        col.note_scale = col.width as f32 / ref_width.max(1.0);
    }
    ManiaLayout {
        scale_x: scale,
        scale_y: scale,
        stage: StageLayout {
            x: stage_x,
            y: top_y,
            width: stage_width,
            height: stage_height,
            top_y,
            hit_y,
            bottom_y,
            center_x: stage_x + stage_width / 2,
        },
        columns,
    }
}
