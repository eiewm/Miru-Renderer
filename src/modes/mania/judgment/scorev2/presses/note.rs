use crate::modes::mania::judgment::{KeyEvent, NoteWindowView, PressTracker};
use crate::types::{HitObject, JudgmentKind, Windows};
use std::collections::HashSet;
#[derive(Debug, Clone, Copy)]
pub(crate) struct PressNoteCtx<'a> {
    pub note_pos: usize,
    pub idx: usize,
    pub ho: &'a HitObject,
    pub col_notes: &'a [(usize, &'a HitObject)],
    pub same_time_tap_count: usize,
    pub presses: &'a [i32],
    pub events: &'a [KeyEvent],
    pub windows: &'a Windows,
    pub next_note_time: Option<i32>,
    pub note_window: NoteWindowView,
    pub last_note_idx_overall: Option<usize>,
    pub extreme_ln_ends: &'a HashSet<i32>,
}
impl<'a> PressNoteCtx<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        note_pos: usize,
        idx: usize,
        ho: &'a HitObject,
        col_notes: &'a [(usize, &'a HitObject)],
        same_time_tap_count: usize,
        presses: &'a [i32],
        events: &'a [KeyEvent],
        windows: &'a Windows,
        last_note_idx_overall: Option<usize>,
        extreme_ln_ends: &'a HashSet<i32>,
    ) -> Self {
        let next_note_time = col_notes.get(note_pos + 1).map(|(_, next_ho)| next_ho.time);
        let note_window = NoteWindowView::from_note(ho, next_note_time, windows);
        Self {
            note_pos,
            idx,
            ho,
            col_notes,
            same_time_tap_count,
            presses,
            events,
            windows,
            next_note_time,
            note_window,
            last_note_idx_overall,
            extreme_ln_ends,
        }
    }
}
#[derive(Debug, Default)]
pub(crate) struct PenaltyFlags {
    pub deep_ln: bool,
    pub deep_ln_chain: bool,
    pub ln_near_deep_late: bool,
    pub short_ln_prewin_claim: bool,
    pub short_ln_prev_early: bool,
    pub short_ln_post_long: bool,
    pub sho_ln_sta_post_head: bool,
    pub ln_post_body_near: bool,
    pub ln_pos_pre_shor_inwi: bool,
    pub ln_pre_tai_pref_h100: bool,
    pub sho_ln_pre_post_head: bool,
    pub post_ln_body_late: bool,
    pub held_prev_ln_no_repr: bool,
    pub pos_pre_prwn_next_ln: bool,
    pub far_pen_pref_next_ln: bool,
    pub far_pen_yield_exact: bool,
    pub far_pen_next_chain: bool,
    pub far_exact_next_chain: bool,
    pub far_pen_h300_chain: bool,
    pub exact_prev_head_pen: bool,
    pub exact_prev_pen_chain: bool,
    pub prssls_prev_keep_pen: bool,
    pub prev_pen_keep_chain: bool,
    pub deep_tap: bool,
    pub deep_tap_chain: bool,
    pub stale_chain_prewin: bool,
    pub prev_head_noise_prwn: bool,
    pub prev_h50_noise_keep: bool,
    pub prewin_prev_near_head: bool,
    pub short_ln_prewin: bool,
    pub ln_prewin_near_head: bool,
    pub ln_prev_tap_near_head: bool,
    pub ln_pos_prev_tap_inwi: bool,
    pub prev_pen_next_ln: bool,
    pub prev_pen_next_tap: bool,
    pub pre_mis_pen_next_tap: bool,
    pub prev_miss_pen_iso: bool,
    pub post_prev_break: bool,
    pub post_prev_head_pref: bool,
    pub post_prev_head_chain: bool,
    pub post_h50_prehead_max: bool,
    pub post_h50_strong_pre: bool,
    pub post_h300_cross_fol: bool,
    pub post_h300_dense_chain: bool,
    pub post_h100_dense_fol: bool,
    pub post_prev_frag: bool,
    pub post_prev_frag_next: bool,
}
impl PenaltyFlags {
    pub(crate) fn active_rule(&self) -> Option<&'static str> {
        [
            (self.deep_ln, "deep_ln"),
            (self.deep_ln_chain, "deep_ln_chain"),
            (self.ln_near_deep_late, "ln_near_deep_late"),
            (self.short_ln_prewin_claim, "short_ln_prewin_claim"),
            (self.short_ln_prev_early, "short_ln_prev_early"),
            (self.short_ln_post_long, "short_ln_post_long"),
            (self.sho_ln_sta_post_head, "sho_ln_sta_post_head"),
            (self.ln_post_body_near, "ln_post_body_near"),
            (self.ln_pos_pre_shor_inwi, "ln_pos_pre_shor_inwi"),
            (self.ln_pre_tai_pref_h100, "ln_pre_tai_pref_h100"),
            (self.sho_ln_pre_post_head, "sho_ln_pre_post_head"),
            (self.post_ln_body_late, "post_ln_body_late"),
            (self.held_prev_ln_no_repr, "held_prev_ln_no_repr"),
            (self.pos_pre_prwn_next_ln, "pos_pre_prwn_next_ln"),
            (self.far_pen_pref_next_ln, "far_pen_pref_next_ln"),
            (self.far_pen_yield_exact, "far_pen_yield_exact"),
            (self.far_pen_next_chain, "far_pen_next_chain"),
            (self.far_exact_next_chain, "far_exact_next_chain"),
            (self.far_pen_h300_chain, "far_pen_h300_chain"),
            (self.exact_prev_head_pen, "exact_prev_head_pen"),
            (self.exact_prev_pen_chain, "exact_prev_pen_chain"),
            (self.prssls_prev_keep_pen, "prssls_prev_keep_pen"),
            (self.prev_pen_keep_chain, "prev_pen_keep_chain"),
            (self.deep_tap, "deep_tap"),
            (self.deep_tap_chain, "deep_tap_chain"),
            (self.stale_chain_prewin, "stal_chain_prwn_norm"),
            (self.prev_head_noise_prwn, "prev_head_noise_prwn"),
            (self.prev_h50_noise_keep, "prev_h50_noise_keep"),
            (self.prewin_prev_near_head, "prewin_prev_near_head"),
            (self.short_ln_prewin, "short_ln_prewin"),
            (self.ln_prewin_near_head, "ln_prewin_near_head"),
            (self.ln_prev_tap_near_head, "ln_prev_tap_near_head"),
            (self.ln_pos_prev_tap_inwi, "ln_pos_prev_tap_inwi"),
            (self.prev_pen_next_ln, "prev_pen_next_ln"),
            (self.prev_pen_next_tap, "prev_pen_next_tap"),
            (self.pre_mis_pen_next_tap, "pre_mis_pen_next_tap"),
            (self.prev_miss_pen_iso, "prev_miss_pen_iso"),
            (self.post_prev_break, "post_prev_break"),
            (self.post_prev_head_pref, "post_prev_head_pref"),
            (self.post_prev_head_chain, "post_prev_head_chain"),
            (self.post_h50_prehead_max, "post_h50_prehead_max"),
            (self.post_h50_strong_pre, "post_h50_strong_pre"),
            (self.post_h300_dense_chain, "post_h300_dense_chain"),
            (self.post_h300_cross_fol, "post_h300_cross_fol"),
            (self.post_h100_dense_fol, "post_h100_dense_fol"),
            (self.post_prev_frag, "post_prev_frag"),
            (self.post_prev_frag_next, "post_prev_frag_next"),
        ]
        .into_iter()
        .find_map(|(is_set, rule)| is_set.then_some(rule))
    }
    pub(crate) fn keeps_exact_pen(&self) -> bool {
        self.exact_prev_head_pen || self.exact_prev_pen_chain
    }
    pub(crate) fn clears_inwin_pen(&self) -> bool {
        [
            self.deep_ln,
            self.deep_ln_chain,
            self.ln_near_deep_late,
            self.short_ln_prewin_claim,
            self.short_ln_prev_early,
            self.short_ln_post_long,
            self.sho_ln_sta_post_head,
            self.ln_post_body_near,
            self.ln_pos_pre_shor_inwi,
            self.ln_pre_tai_pref_h100,
            self.sho_ln_pre_post_head,
            self.post_ln_body_late,
            self.held_prev_ln_no_repr,
            self.pos_pre_prwn_next_ln,
            self.far_pen_pref_next_ln,
            self.far_pen_yield_exact,
            self.deep_tap,
            self.deep_tap_chain,
            self.stale_chain_prewin,
            self.prev_head_noise_prwn,
            self.prewin_prev_near_head,
            self.short_ln_prewin,
            self.ln_prewin_near_head,
            self.ln_prev_tap_near_head,
            self.ln_pos_prev_tap_inwi,
            self.prev_pen_next_ln,
            self.prev_pen_next_tap,
            self.pre_mis_pen_next_tap,
            self.prev_miss_pen_iso,
            self.post_prev_break,
            self.post_h50_prehead_max,
            self.post_h300_dense_chain,
        ]
        .into_iter()
        .any(|flag| flag)
    }
}
#[derive(Debug, Default)]
pub(crate) struct HeadCandidateState {
    pub has_candidate: bool,
    pub selected_pt: i32,
    pub selected_idx: usize,
    pub steals_next_ex: bool,
    pub ln_claim_fallback: bool,
    pub tap_micro_keeps_idx: bool,
    pub prewin_follow_next_ln: bool,
    pub pre_mis_pos_hea_prom: bool,
    pub prev_miss_pen_prewin: bool,
    pub pre_ear_pen_pos_h200: bool,
    pub ghost_prehead: bool,
    pub prev_miss_clear_rule: Option<&'static str>,
    pub prev_miss_settle_rule: Option<&'static str>,
    pub late_tap_cross_tap: bool,
    pub late_tap_dense_chain: bool,
    pub late_tap_iso_head: bool,
    pub late_tap_cross_ln: bool,
    pub lat_tap_yild_next_ln: bool,
    pub prev_miss_hless300: bool,
    pub prev_miss_keeps_hless: bool,
    pub miss_body_tail_claim: bool,
    pub tail_claim_rule: Option<&'static str>,
}
#[derive(Debug)]
pub(crate) struct PressPrevState {
    pub had_prewin_pen: bool,
    pub body_break_pre_tail: bool,
    pub was_miss: bool,
    pub prev2_had_prewin_pen: bool,
    pub prev2_was_miss: bool,
    pub col_pt: Option<i32>,
    pub reserved_ln_repr: HashSet<i32>,
    pub skipped_stale: bool,
}
#[derive(Debug, Default)]
pub(crate) struct PressRuleState {
    pub early_pen: Option<i32>,
    pub pen: Option<&'static str>,
    pub stale: Option<&'static str>,
    pub tail: Option<&'static str>,
}
#[derive(Debug, Default)]
pub(crate) struct PressPickState {
    pub press: Option<i32>,
    pub tail: Option<i32>,
}
#[derive(Debug, Default)]
pub(crate) struct PressFinalState {
    pub press: Option<i32>,
    pub tail: Option<i32>,
    pub kind: Option<JudgmentKind>,
    pub delta: i32,
}
#[derive(Debug)]
pub(crate) struct PressState {
    pub press_idx: usize,
    pub prev: PressPrevState,
    pub rules: PressRuleState,
    pub pick: PressPickState,
    pub penalty_flags: PenaltyFlags,
    pub head_candidate: HeadCandidateState,
    pub final_pick: PressFinalState,
}
impl PressState {
    pub(crate) fn from_tracker(tracker: &mut PressTracker) -> Self {
        Self {
            press_idx: tracker.press_idx,
            prev: PressPrevState {
                had_prewin_pen: tracker.prev_had_prewin_pen,
                body_break_pre_tail: tracker.prev_break_pre,
                was_miss: tracker.prev_was_miss,
                prev2_had_prewin_pen: tracker.prev2_had_prewin_pen,
                prev2_was_miss: tracker.prev_prev_was_miss,
                col_pt: tracker.prev_col_pt,
                reserved_ln_repr: std::mem::take(&mut tracker.reserved_ln_repr),
                skipped_stale: false,
            },
            rules: PressRuleState::default(),
            pick: PressPickState::default(),
            penalty_flags: PenaltyFlags::default(),
            head_candidate: HeadCandidateState::default(),
            final_pick: PressFinalState::default(),
        }
    }
}
