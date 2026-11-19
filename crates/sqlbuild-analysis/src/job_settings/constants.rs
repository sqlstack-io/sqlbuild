//! How far each job setting reaches when a selection names the job.

/// Settings that apply only to the named job's own nodes; two jobs may not disagree on one node.
pub const NODE_SETTINGS: [&str; 4] = [
    "run_tests",
    "ignore_audit_failures",
    "full_refresh",
    WAREHOUSE_SETTING,
];
/// The warehouse the job's nodes run on.
pub const WAREHOUSE_SETTING: &str = "warehouse";
/// The node settings a Python node takes; the others, and `[retry]`, act on SQL nodes only.
pub const PYTHON_NODE_SETTINGS: [&str; 1] = [WAREHOUSE_SETTING];
/// The setting that becomes a limit on how many of the job's nodes run at once inside the build.
pub const JOB_LIMIT_SETTING: &str = "concurrency";
/// The help of two named jobs giving one node different values of a node-level setting.
pub const NODE_CONFLICT_HELP: &str = "make the jobs agree, set it in only one of them, override it \
     with the command-line flag, or build with --no-job-settings";
/// The help of named jobs disagreeing on a build-wide setting, where unset differs from set.
pub const BUILD_CONFLICT_HELP: &str =
    "make the jobs agree, override it with the command-line flag, or build with --no-job-settings";
/// The help of two named jobs giving one node different `[retry]` tables, which no flag overrides.
pub const RETRY_CONFLICT_HELP: &str =
    "make the jobs agree, set [retry] in only one of them, or build with --no-job-settings";
/// The build-wide settings a group's jobs must agree on while each member runs as its own build.
pub const GROUP_AGREED_SETTINGS: [&str; 2] = ["vars", "changed"];
/// The error code of job settings that cannot apply together.
pub const JOB_SETTING_CONFLICT_CODE: &str = "S027";
/// How a conflict names a job's `[retry]` table.
pub const RETRY_SETTING: &str = "[retry]";
