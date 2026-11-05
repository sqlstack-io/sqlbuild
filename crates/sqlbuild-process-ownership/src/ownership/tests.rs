//! Real child processes stand in for workers; the launching scheduler's view of them is dropped.

#[path = "tests/helpers.rs"]
mod helpers;
#[cfg(windows)]
#[path = "tests/test_job_access.rs"]
mod test_job_access;
#[path = "tests/test_recovery.rs"]
mod test_recovery;
#[path = "tests/test_types.rs"]
mod test_types;
