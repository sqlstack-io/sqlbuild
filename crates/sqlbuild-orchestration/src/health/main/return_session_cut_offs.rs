//! Put back the cut-off sessions a report could not record.

use crate::health::_helpers::session_cut_offs::{SESSION_CUT_OFFS, restore};
use crate::health::models::SessionCutOff;
use crate::store::models::ControlStore;

/// Keep `unreported` for `store`'s next report, ahead of any cut off since.
pub(crate) fn return_session_cut_offs(store: &ControlStore, unreported: Vec<SessionCutOff>) {
    restore(&SESSION_CUT_OFFS, store, unreported);
}
