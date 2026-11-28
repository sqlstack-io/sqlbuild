//! Hand the sessions the control database cut off to the report that records them.

use crate::health::_helpers::session_cut_offs::{SESSION_CUT_OFFS, take};
use crate::health::models::SessionCutOff;
use crate::store::models::ControlStore;

/// `store`'s cut-off sessions not yet reported, which the caller now owns.
pub(crate) fn take_session_cut_offs(store: &ControlStore) -> Vec<SessionCutOff> {
    take(&SESSION_CUT_OFFS, store)
}
