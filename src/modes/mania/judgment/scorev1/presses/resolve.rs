use super::note::{PressNoteCtx, PressState};
use super::pick;
use super::retry;
use super::settle;
use crate::modes::mania::judgment::InternalJudgment;
pub(super) fn resolve(ctx: &PressNoteCtx<'_>, state: &mut PressState, out: &[InternalJudgment]) {
    pick::resolve_primary(ctx, state);
    retry::reselect(ctx, state, out);
    settle::finalize_candidate(ctx, state);
}
