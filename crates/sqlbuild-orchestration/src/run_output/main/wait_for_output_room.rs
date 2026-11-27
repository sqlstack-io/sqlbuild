//! Hold a printing thread while the sender is behind, so output is not lost to a slow server.

use std::sync::PoisonError;
use std::time::Instant;

use crate::run_output::_helpers::worker::upload::attached;
use crate::run_output::constants::UPLOAD_STALL;

/// Waits while too many batches are queued (only a sender stalled past `UPLOAD_STALL` loses output); call it holding no lock and no GIL.
pub fn wait_for_output_room() {
    let Some(uploader) = attached().clone() else {
        return;
    };
    let mut state = uploader.shared.locked();
    let deadline: Instant = Instant::now() + UPLOAD_STALL;
    while state.backlogged() {
        let Some(left) = deadline.checked_duration_since(Instant::now()) else {
            state.shed();
            break;
        };
        state = uploader
            .shared
            .wake
            .wait_timeout(state, left)
            .unwrap_or_else(PoisonError::into_inner)
            .0;
    }
}
