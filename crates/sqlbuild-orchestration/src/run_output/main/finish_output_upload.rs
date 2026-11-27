//! Send this process's last run output within a budget, stop its sender and report what was lost.

use std::time::{Duration, Instant};

use crate::run_output::_helpers::worker::upload::attached;
use crate::run_output::models::UploadSummary;

/// `None` when this process sent no run output.
pub fn finish_output_upload(budget: Duration) -> Option<UploadSummary> {
    let uploader = attached().take()?;
    {
        let mut state = uploader.shared.locked();
        state.end_lines();
        state.closing = Some(Instant::now() + budget);
    }
    uploader.shared.wake.notify_all();
    let sender = uploader
        .sender
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    if let Some(sender) = sender {
        drop(sender.join());
    }
    let summary = uploader.shared.locked().summary.clone();
    Some(summary)
}
