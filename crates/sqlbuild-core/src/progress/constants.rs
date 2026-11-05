//! Lifecycle event types, operation labels and layout of CLI progress lines.

/// Operations with visible progress lines, and the label each renders with.
pub(crate) const VISIBLE_OPERATION_LABELS: [(&str, &str); 32] = [
    ("clone_finalization", "Clone finalization"),
    ("discovery_declaration_parse", "Declaration parsing"),
    ("discovery_filesystem_walk", "Filesystem discovery"),
    ("discovery_project_assembly", "Project assembly"),
    ("discovery_python_import", "Python discovery"),
    ("external_manifest_discovery", "External manifest discovery"),
    ("project_discovery", "Project discovery"),
    ("project_compile", "Project compile"),
    ("sql_test_planning", "SQL test planning"),
    ("dbt_command", "dbt command"),
    ("external_source_load", "loader callable"),
    ("ingestr_command", "ingestr subprocess"),
    ("adbc_driver_install", "DuckDB driver install"),
    ("managed_source_load", "loader callable"),
    ("python_asset", "Python asset attempt"),
    ("python_check", "Python check attempt"),
    ("python_hook", "Python hook"),
    ("python_task", "Python task attempt"),
    ("sql_hook", "SQL hook"),
    ("audit_evaluation", "Audit evaluation"),
    ("sql_test_setup", "SQL test setup"),
    ("sql_test_assertion", "SQL test assertion"),
    (
        "source_freshness_metadata_observation",
        "Freshness metadata",
    ),
    ("source_freshness_query_observation", "Freshness query"),
    ("source_freshness_publication", "Publish freshness state"),
    ("retention_inspection", "Inspect retention"),
    ("retention_application", "Apply retention"),
    ("table_type_inspection", "Inspect table type"),
    ("table_type_conversion", "Convert table type"),
    ("schema_synchronization", "Synchronize schema"),
    ("staging_creation", "Create staging relation"),
    ("relation_promotion", "Promote relation"),
];
/// Operations a live transient owner shows on a terminal when no resource attempt owns them.
pub(crate) const TTY_TRANSIENT_OWNER_OPERATIONS: [&str; 19] = [
    "adbc_driver_install",
    "dbt_command",
    "discovery_declaration_parse",
    "discovery_filesystem_walk",
    "discovery_project_assembly",
    "discovery_python_import",
    "external_manifest_discovery",
    "external_source_load",
    "ingestr_command",
    "managed_source_load",
    "project_compile",
    "project_discovery",
    "sql_test_planning",
    "python_asset",
    "python_check",
    "python_hook",
    "python_task",
    "sql_hook",
    "sql_test_setup",
];
/// Resource-scoped operations a live result owner shows on a terminal.
pub(crate) const TTY_RESOURCE_OWNER_OPERATIONS: [&str; 2] =
    ["audit_evaluation", "sql_test_assertion"];
pub(crate) const FRESHNESS_METADATA_OPERATION: &str = "source_freshness_metadata_observation";
pub(crate) const FRESHNESS_QUERY_OPERATION: &str = "source_freshness_query_observation";
pub(crate) const RESOURCE_START: &str = "resource_attempt_started";
pub(crate) const SCENARIO_RESOURCE_PREFIX: &str = "scenario:";
pub(crate) const OPERATION_START: &str = "operation_started";
pub(crate) const RETRY_SCHEDULED: &str = "retry_scheduled";
pub(crate) const STATEMENT_START: &str = "statement_started";
pub(crate) const STATEMENT_SUBMITTED: &str = "statement_submitted";
pub(crate) const STATEMENT_HEARTBEAT: &str = "statement_heartbeat";
pub(crate) const STATEMENT_COMPLETED: &str = "statement_completed";
pub(crate) const STATEMENT_FAILED: &str = "statement_failed";
pub(crate) const FAILED_SUFFIX: &str = "failed";
pub(crate) const SKIPPED_SUFFIX: &str = "skipped";
/// Width of the resource kind column of resource rows.
pub(crate) const KIND_WIDTH: usize = 10;
/// Indent of rows that belong to a resource attempt.
pub(crate) const NESTED_PREFIX: &str = "    ";
pub(crate) const UNKNOWN_RESOURCE_NAME: &str = "warehouse";
/// Messages that complete a planning phase rather than update it.
pub(crate) const PLANNING_COMPLETION_PREFIXES: [&str; 15] = [
    "Built ",
    "Checked ",
    "Compiled ",
    "Evaluated ",
    "Applied ",
    "Finalized ",
    "Generated ",
    "Inspected ",
    "Loaded ",
    "Planned ",
    "Prepared ",
    "Recorded ",
    "Refreshed ",
    "Resolved ",
    "Cloned ",
];
/// Messages that end a planning phase with a failure.
pub(crate) const PLANNING_FAILURE_PREFIXES: [&str; 1] = ["Failed "];
pub(crate) const ASSERTION_PREFIX: &str = "assertion ";
pub(crate) const PASS_OUTCOME: &str = "pass";
pub(crate) const ERROR_OUTCOME: &str = "error";
pub(crate) const TIMEOUT_OUTCOME: &str = "timeout";
/// Inspection SQL longer than this many characters is cut.
pub(crate) const SQL_PREVIEW_LENGTH: usize = 160;
pub(crate) const EXECUTION_TITLE: &str = "Execution";
pub(crate) const LINEAGE_COMMAND: &str = "lineage";
pub(crate) const QUERY_COMMAND: &str = "query";
pub(crate) const LOCATION_COMMAND: &str = "location";
pub(crate) const DBT_COMMAND: &str = "dbt";
pub(crate) const DBT_JSON_OPTION: &str = "--json";
pub(crate) const JSON_FORMAT: &str = "json";
/// What replaces the cut tail of a transient status message.
pub(crate) const STATUS_ELLIPSIS: &str = "...";
/// The resource kind of audits and the prefix of their resource identifiers.
pub(crate) const AUDIT_RESOURCE_KIND: &str = "audit";
pub(crate) const AUDIT_RESOURCE_PREFIX: &str = "audit:";
/// Audit outcomes and the status words their rows show.
pub(crate) const AUDIT_OUTCOME_STATUSES: [(&str, &str); 5] = [
    ("pass", "PASS"),
    ("warn", "WARN"),
    ("error", "FAIL"),
    ("insufficient", "INSUFFICIENT"),
    ("skipped", "SKIP"),
];
