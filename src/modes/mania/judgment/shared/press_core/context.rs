use super::super::{InternalJudgment, KeyEvent};
use crate::types::{HitObject, Windows};
use std::collections::HashSet;
#[derive(Debug, Clone)]
pub struct ColumnTimeline<'a> {
    pub notes: Vec<(usize, &'a HitObject)>,
    pub presses: &'a [i32],
    pub events: &'a [KeyEvent],
}
#[derive(Debug, Clone, Copy, Default)]
pub struct NoteNeighbors<'a> {
    pub prev: Option<(usize, &'a HitObject)>,
    pub current: Option<(usize, &'a HitObject)>,
    pub next: Option<(usize, &'a HitObject)>,
}
#[derive(Debug, Default)]
pub struct PressTracker {
    pub press_idx: usize,
    pub prev_had_prewin_pen: bool,
    pub prev_break_pre: bool,
    pub prev_was_miss: bool,
    pub prev2_had_prewin_pen: bool,
    pub prev_prev_was_miss: bool,
    pub prev_col_pt: Option<i32>,
    pub reserved_ln_repr: HashSet<i32>,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct NoteWindowView {
    pub window_start: i32,
    pub window_end_exclusive: i32,
    pub lock_end_exclusive: i32,
    pub next_window_start: Option<i32>,
    pub early_penalty_window: i32,
    pub early_penalty_start: i32,
    pub next_early_pen: Option<i32>,
}
impl NoteWindowView {
    pub fn from_note(ho: &HitObject, next_note_time: Option<i32>, windows: &Windows) -> Self {
        let window_start = ho.time - windows.hit50;
        let ln_late_end = ho.is_long_note()
            && next_note_time
                .map(|next_time| next_time <= ho.time + windows.hit50)
                .unwrap_or(false);
        let window_end_exclusive = if ho.is_long_note() {
            ho.time + windows.hit50 + if ln_late_end { 1 } else { 0 }
        } else {
            ho.time + windows.hit100
        };
        let legacy_early_win = windows.max + 4;
        let extndd_early_pen_win = windows.hit300.min(39);
        let early_penalty_window = legacy_early_win.max(extndd_early_pen_win);
        let next_window_start = next_note_time.map(|next_time| next_time - windows.hit50);
        let early_penalty_start = window_start - early_penalty_window - 1;
        let next_early_pen =
            next_window_start.map(|next_start| next_start - early_penalty_window - 1);
        Self {
            window_start,
            window_end_exclusive,
            lock_end_exclusive: window_end_exclusive,
            next_window_start,
            early_penalty_window,
            early_penalty_start,
            next_early_pen,
        }
    }
}
#[derive(Debug)]
pub struct ColumnPressContext<'a> {
    pub timeline: ColumnTimeline<'a>,
    pub tracker: PressTracker,
    pub out: Vec<InternalJudgment>,
}
