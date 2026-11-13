//! A signed-in `sqb build` asks its configured location to refresh the warehouse copy, best-effort.

use std::path::Path;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use crate::person_login::_helpers::build_sync::{configured_location, sign_in_and_sync};
use crate::person_login::constants::BUILD_SYNC_SECONDS;

/// One stderr line about the sync, or `None` when no location is configured or nobody is signed in.
pub fn sync_after_build(project_dir: &Path, target: &str) -> Option<String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let (project_dir, target) = (project_dir.to_path_buf(), target.to_owned());
    drop(std::thread::spawn(move || {
        drop(sender.send(request_sync(&project_dir, &target)));
    }));
    match receiver.recv_timeout(Duration::from_secs(BUILD_SYNC_SECONDS)) {
        Ok(line) => line,
        Err(RecvTimeoutError::Timeout) => Some(format!(
            "Warehouse sync skipped: the request did not finish within {BUILD_SYNC_SECONDS} seconds.\n"
        )),
        Err(RecvTimeoutError::Disconnected) => None,
    }
}

fn request_sync(project_dir: &Path, target: &str) -> Option<String> {
    let outcome = match configured_location(project_dir) {
        Ok(Some(configured)) => sign_in_and_sync(configured, target),
        Ok(None) => return None,
        Err(error) => Err(error),
    };
    match outcome {
        Ok(line) => line,
        Err(error) => Some(format!("Warehouse sync skipped: {error}\n")),
    }
}
