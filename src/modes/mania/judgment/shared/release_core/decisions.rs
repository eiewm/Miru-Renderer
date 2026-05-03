#[derive(Debug, Clone, Copy, Default)]
pub struct ReleaseCandidateSet {
    pub press_time: Option<i32>,
    pub tail_only_pt: Option<i32>,
    pub first_rel_after_press: Option<i32>,
    pub first_repr_post_rel: Option<i32>,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct ReleaseDecision {
    pub rel_time: Option<i32>,
    pub effective_press_time: Option<i32>,
    pub alt_head_press_time: Option<i32>,
}
