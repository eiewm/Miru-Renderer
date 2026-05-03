#[derive(Debug, Clone, Copy, Default)]
pub struct PressCandidateSet {
    pub primary_press: Option<i32>,
    pub followup_press: Option<i32>,
    pub tail_only_press: Option<i32>,
    pub post_end_press: Option<i32>,
}
