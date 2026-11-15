//! The record behaviours installed at registration: finishers, computed attributes and methods.

use std::sync::OnceLock;

use crate::records::constants::NO_RECORD_BEHAVIOURS;
use crate::records::models::RecordBehaviours;

static INSTALLED: OnceLock<&'static RecordBehaviours> = OnceLock::new();

/// Install the record behaviours; later calls keep the first behaviours.
pub(crate) fn install(behaviours: &'static RecordBehaviours) {
    let _ = INSTALLED.get_or_init(|| behaviours);
}

/// The installed record behaviours, or none before registration.
pub(crate) fn behaviours() -> &'static RecordBehaviours {
    INSTALLED.get().copied().unwrap_or(&NO_RECORD_BEHAVIOURS)
}
