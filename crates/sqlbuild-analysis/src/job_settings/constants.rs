//! How far each job setting reaches when a selection names the job.

/// Settings that apply only to the named job's own nodes; two jobs may not disagree on one node.
pub const NODE_SETTINGS: [&str; 3] = ["run_tests", "ignore_audit_failures", "full_refresh"];
/// The setting that becomes a limit on how many of the job's nodes run at once inside the build.
pub const JOB_LIMIT_SETTING: &str = "concurrency";
/// The help every job setting conflict gives.
pub const CONFLICT_HELP: &str = "make the jobs agree, set it in only one of them, override it with \
     the command-line flag, or build with --no-job-settings";
/// The error code of job settings that cannot apply together.
pub const JOB_SETTING_CONFLICT_CODE: &str = "S027";
/// How a conflict names a job's `[retry]` table.
pub const RETRY_SETTING: &str = "[retry]";
