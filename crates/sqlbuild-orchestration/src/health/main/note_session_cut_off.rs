//! Keep a session the control database cut off, for the health probe to report.

use crate::health::_helpers::session_cut_offs::{SESSION_CUT_OFFS, note};
use crate::store::models::ControlStore;

/// Keep `error` for `store`'s health probe when PostgreSQL cut the session off that met it.
pub(crate) fn note_session_cut_off(store: &ControlStore, error: &sqlx_core::Error) {
    note(&SESSION_CUT_OFFS, store, error);
}
