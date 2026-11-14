use sqlbuild_orchestration::runs::models::RunCommand;

/// Location actions accepted by the interpreter-side one-shot publisher.
pub(crate) const PUBLISH_ACTION: &str = "publish";

/// Workers verify the pinned nodes and environment without reevaluating declarations.
pub(crate) const EXECUTE_ACTION: &str = "execute";

/// The native implementation each frozen run command dispatches to.
pub(crate) const COMMAND_IMPLEMENTATIONS: [(RunCommand, &str); 5] = [
    (RunCommand::Build, "run_build_command"),
    (RunCommand::Test, "run_test_command"),
    (RunCommand::Seed, "run_seed_command"),
    (RunCommand::Audit, "run_audit_command"),
    (RunCommand::Freshness, "run_freshness_command"),
];

/// Snapshot node kinds a provider consumer can run inside.
pub(crate) const MODEL_NODE_KIND: &str = "model";
pub(crate) const SOURCE_NODE_KIND: &str = "source";

/// `env_prefix_target` values under which pydantic-settings prefixes alias names too.
pub(crate) const ALIAS_PREFIX_TARGET: &str = "alias";
pub(crate) const ALL_PREFIX_TARGET: &str = "all";

/// DuckDB's message when another process, such as a running build, holds the database file.
pub(crate) const DUCKDB_LOCK_CONFLICT: &str = "Conflicting lock is held";

/// The build settings a job run's `--changed` reach reads, and the reach every job run uses.
pub(crate) const CHANGED_SETTING: &str = "changed";
pub(crate) const REACH_SETTING: &str = "reach";
/// A job run reaches its whole selection: `reach` only stops builds that are not a job's run.
pub(crate) const JOB_RUN_REACH: &str = "all";
/// What a job run with `changed: True` counts when `[changes] counts` is not set: every change.
pub(crate) const JOB_RUN_COUNTS: [&str; 2] = ["definition", "source_data"];

/// A SQL unit test's published kind; a frozen run selects it back with `test:<name>`.
pub(crate) const SQL_TEST_NODE_KIND: &str = "sql_test";
