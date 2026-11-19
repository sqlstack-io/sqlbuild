//! Public constants of plan output.

/// The plan JSON key of what `--changed` would run, hold back and skip.
pub const CHANGED_KEY: &str = "changed";
/// The plan JSON listing the named jobs whose own audit failures hold nothing back.
pub const IGNORE_AUDIT_FAILURES_JOBS_KEY: &str = "ignore_audit_failures_jobs";
/// The plan JSON listing the named jobs whose own models rebuild from scratch.
pub const FULL_REFRESH_JOBS_KEY: &str = "full_refresh_jobs";
/// The plan JSON mapping each named job to the warehouse its nodes run on.
pub const WAREHOUSE_JOBS_KEY: &str = "job_warehouses";
/// The plan JSON reason of a model built under a new name.
pub const RENAMED_REASON: &str = "renamed";
