//! After a direct build, ask the configured location to sync the target the build used.

use std::path::Path;

/// One stderr line about the sync, or `None` when no location is configured or nobody is signed in.
/// Never fails: every problem becomes a skipped line, and the request is bounded to a few seconds.
pub fn sync_after_build(project_dir: &Path, target: &str) -> Option<String> {
    crate::person_login::main::sync_after_build::sync_after_build(project_dir, target)
}
