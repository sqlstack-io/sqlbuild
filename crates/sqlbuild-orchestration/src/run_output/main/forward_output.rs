//! Hand bytes a process stream accepted to this process's run output, under the node running on this thread.

use crate::run_output::_helpers::upload::{OUTPUT_NODE, attached};
use crate::run_output::models::OutputStream;

/// Nothing happens outside a worker. Never waits: call it under the lock that orders the output,
/// then `wait_for_output_room` once that lock and the GIL are released.
pub fn forward_output(stream: OutputStream, data: &[u8]) {
    let Some(uploader) = attached().clone() else {
        return;
    };
    let node: String = OUTPUT_NODE
        .with(|node| node.borrow().clone())
        .unwrap_or_default();
    uploader.shared.locked().append((node, stream), data);
    uploader.shared.wake.notify_all();
}
