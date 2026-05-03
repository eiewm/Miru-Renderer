pub use super::replay_renderer::{ColumnLayout, ManiaLayoutInfo, StageLayout};
use crate::hud::HudState;
use crate::types::HitObject;
#[derive(Debug, Clone)]
pub struct Overlay {
    pub left: i32,
    pub top: i32,
    pub opacity: f32,
    pub texture_key: usize,
}
#[derive(Debug)]
pub struct FrameData {
    pub overlays: Vec<Overlay>,
    pub width: u32,
    pub height: u32,
    pub timestamp: i32,
}
#[derive(Debug, Clone)]
pub struct InternalJudgment {
    pub index: usize,
    pub time: i32,
    pub kind: crate::types::JudgmentKind,
    pub press_time: Option<i32>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RenderLayer {
    StageBackground = 0,
    ColumnBackground = 1,
    Barlines = 2,
    KeysUnder = 3,
    Notes = 4,
    LongNotes = 5,
    StageBottom = 6,
    Lighting = 7,
    KeysOver = 8,
    Hud = 9,
}
pub struct FrameContext<'a> {
    pub timestamp: i32,
    pub width: u32,
    pub height: u32,
    pub layout: &'a ManiaLayoutInfo,
    pub hud_state: &'a HudState,
    pub judgments: &'a [InternalJudgment],
    pub hit_objects: &'a [HitObject],
    pub barlines: &'a [i32],
    pub key_mask: u8,
    pub press_times: &'a [i32],
    pub rel_times: &'a [i32],
    pub keys_under_notes: bool,
    pub enable_lighting: bool,
    pub enable_hud: bool,
    pub pps_base: f32,
    pub current_sv: f32,
}
#[derive(Debug, Default)]
pub struct ColumnLnState {
    pub has_active_ln: bool,
    pub held: bool,
}
pub struct FrameBuilder {
    layers: Vec<(RenderLayer, Overlay)>,
}
impl FrameBuilder {
    pub fn new() -> Self {
        Self {
            layers: Vec::with_capacity(256),
        }
    }
    pub fn add(&mut self, layer: RenderLayer, overlay: Overlay) {
        self.layers.push((layer, overlay));
    }
    pub fn build(mut self, width: u32, height: u32, timestamp: i32) -> FrameData {
        // RenderLayer values define painter order; sorting here keeps callers from depending on add order.
        self.layers.sort_by_key(|(layer, _)| *layer);
        let overlays = self.layers.into_iter().map(|(_, o)| o).collect();
        FrameData {
            overlays,
            width,
            height,
            timestamp,
        }
    }
    pub fn clear(&mut self) {
        self.layers.clear();
    }
}
impl Default for FrameBuilder {
    fn default() -> Self {
        Self::new()
    }
}
pub fn calc_note_y(timestamp: i32, note_time: i32, judgment_y: i32, pps: f32) -> i32 {
    let time_diff = note_time - timestamp;
    let px_offset = (time_diff as f32 / 1000.0) * pps;
    judgment_y - px_offset as i32
}
pub fn is_note_visible(
    note_y: i32,
    note_height: i32,
    top_y: i32,
    bottom_y: i32,
    margin: i32,
) -> bool {
    let note_bottom = note_y + note_height;
    note_bottom >= (top_y - margin) && note_y <= (bottom_y + margin)
}
pub fn is_consumed(judgment: Option<&InternalJudgment>, timestamp: i32, note_time: i32) -> bool {
    match judgment {
        Some(j) => {
            let is_miss_or_50 = matches!(
                j.kind,
                crate::types::JudgmentKind::Miss | crate::types::JudgmentKind::Hit50
            );
            // Misses and Hit50s stay visible long enough to show their fade/dim feedback.
            !is_miss_or_50 && j.press_time.unwrap_or(note_time) <= timestamp
        }
        None => false,
    }
}
pub fn should_dim_miss(
    judgment: Option<&InternalJudgment>,
    timestamp: i32,
    note_time: i32,
    hit50_window: i32,
) -> bool {
    match judgment {
        Some(j) if j.kind == crate::types::JudgmentKind::Miss => {
            // Do not dim a missed note until the last Hit50 timing edge has passed.
            let miss_edge = (note_time + hit50_window).max(j.time);
            timestamp >= miss_edge
        }
        _ => false,
    }
}
pub fn get_layer_order_keys_under() -> &'static [RenderLayer] {
    &[
        RenderLayer::StageBackground,
        RenderLayer::ColumnBackground,
        RenderLayer::Barlines,
        RenderLayer::KeysUnder,
        RenderLayer::Notes,
        RenderLayer::LongNotes,
        RenderLayer::StageBottom,
        RenderLayer::Lighting,
        RenderLayer::Hud,
    ]
}
pub fn get_layer_order_keys_over() -> &'static [RenderLayer] {
    &[
        RenderLayer::StageBackground,
        RenderLayer::ColumnBackground,
        RenderLayer::Barlines,
        RenderLayer::Notes,
        RenderLayer::LongNotes,
        RenderLayer::KeysOver,
        RenderLayer::StageBottom,
        RenderLayer::Lighting,
        RenderLayer::Hud,
    ]
}
