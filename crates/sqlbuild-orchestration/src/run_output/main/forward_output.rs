//! Hand bytes a process stream accepted to this process's run output, under the node running on this thread.

use std::sync::PoisonError;
use std::time::Instant;

use crate::run_output::_helpers::upload::{OUTPUT_NODE, attached};
use crate::run_output::constants::UPLOAD_STALL;
use crate::run_output::models::OutputStream;

/// Nothing happens outside a worker; while the sender is behind the printing thread waits, and only a stalled sender loses output.
pub fn forward_output(stream: OutputStream, data: &[u8]) {
    let Some(uploader) = attached().clone() else {
        return;
    };
    let node: String = OUTPUT_NODE
        .with(|node| node.borrow().clone())
        .unwrap_or_default();
    let mut state = uploader.shared.locked();
    state.append((node, stream), data);
    uploader.shared.wake.notify_all();
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
