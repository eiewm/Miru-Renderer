use crate::types::HitObject;
use std::collections::HashSet;
#[derive(Debug, Default)]
pub struct ReleaseTracker {
    pub reclaimed_pairs: HashSet<(usize, i32)>,
    pub rescued_pairs: HashSet<(usize, i32)>,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct NoteReleaseContext<'a> {
    pub prev_same_col: Option<(usize, &'a HitObject)>,
    pub current: Option<(usize, &'a HitObject)>,
    pub next_same_col: Option<(usize, &'a HitObject)>,
}
