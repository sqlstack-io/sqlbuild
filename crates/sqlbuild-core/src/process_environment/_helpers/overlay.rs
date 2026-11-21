//! The one overlay of this process.

use std::sync::{PoisonError, RwLock};

use crate::process_environment::models::EnvironmentOverlay;

static OVERLAY: RwLock<EnvironmentOverlay> = RwLock::new(EnvironmentOverlay {
    set: Vec::new(),
    remove: Vec::new(),
});

pub(crate) fn current() -> EnvironmentOverlay {
    OVERLAY
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

pub(crate) fn replace(overlay: EnvironmentOverlay) {
    *OVERLAY.write().unwrap_or_else(PoisonError::into_inner) = overlay;
}
