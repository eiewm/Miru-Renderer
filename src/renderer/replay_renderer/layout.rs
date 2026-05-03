#[derive(Debug, Clone, Copy, Default)]
pub struct ColumnLayout {
    pub x: i32,
    pub width: u32,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct StageLayout {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub hit_y: i32,
    pub top_y: i32,
    pub bottom_y: i32,
}
#[derive(Debug, Clone, Default)]
pub struct ManiaLayoutInfo {
    pub stage: StageLayout,
    pub columns: Vec<ColumnLayout>,
    pub scale_y: f32,
    pub upside_down: bool,
}
impl ManiaLayoutInfo {
    pub fn num_columns(&self) -> usize {
        self.columns.len()
    }
}
