use crate::types::JudgmentKind;
#[derive(Debug, Clone, Copy, Default)]
pub struct HeadDecision {
    pub press_time: Option<i32>,
    pub kind: Option<JudgmentKind>,
    pub early_pen_pt: Option<i32>,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct TailDecision {
    pub tail_only_pt: Option<i32>,
    pub final_press_time: Option<i32>,
}
