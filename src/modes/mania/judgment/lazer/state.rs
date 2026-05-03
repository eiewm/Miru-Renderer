use crate::modes::mania::judgment::ReleaseKind;
use crate::types::JudgmentKind;
#[derive(Debug, Clone, Default)]
pub(super) struct TapState {
    pub kind: Option<JudgmentKind>,
    pub press_time: Option<i32>,
}
impl TapState {
    #[inline]
    pub fn is_resolved(&self) -> bool {
        self.kind.is_some()
    }
}
#[derive(Debug, Clone, Default)]
pub(super) struct HoldState {
    pub head_kind: Option<JudgmentKind>,
    pub head_press_time: Option<i32>,
    pub late_hold_start: Option<i32>,
    pub holding: bool,
    pub body_broken: bool,
    pub first_early_rel: Option<i32>,
    pub firs_repr_post_break: Option<i32>,
    pub last_repr_time: Option<i32>,
    pub rel_post_first_repr: Option<i32>,
    pub tail_kind: Option<ReleaseKind>,
    pub tail_time: Option<i32>,
}
impl HoldState {
    #[inline]
    pub fn head_resolved(&self) -> bool {
        self.head_kind.is_some()
    }
    #[inline]
    pub fn is_resolved(&self) -> bool {
        self.tail_kind.is_some()
    }
    #[inline]
    pub fn effective_press_time(&self) -> Option<i32> {
        self.head_press_time.or(self.late_hold_start)
    }
    pub fn mark_repress(&mut self, time: i32) {
        if self.body_broken && self.firs_repr_post_break.is_none() {
            self.firs_repr_post_break = Some(time);
        }
        if self.body_broken {
            self.last_repr_time = Some(time);
        }
    }
}
#[derive(Debug, Clone)]
pub(super) enum LaneObjectState {
    Tap(TapState),
    Hold(HoldState),
}
impl LaneObjectState {
    pub fn new(is_long_note: bool) -> Self {
        if is_long_note {
            Self::Hold(HoldState::default())
        } else {
            Self::Tap(TapState::default())
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PressDispatch {
    Ignored,
    ConsumedMiss,
    ConsumedHit,
}
impl PressDispatch {
    #[inline]
    pub fn consumed(self) -> bool {
        !matches!(self, Self::Ignored)
    }
    #[inline]
    pub fn is_hit(self) -> bool {
        matches!(self, Self::ConsumedHit)
    }
}
