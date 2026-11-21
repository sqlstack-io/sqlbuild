//! The integration-result record contract: identity, limits and allowed values.

pub(crate) const SCHEMA_VERSION: i64 = 1;
pub(crate) const RECORD_KIND: &str = "integration_result";
pub(crate) const RESOURCE_FAILED_EVENT: &str = "resource_attempt_failed";
pub(crate) const RESOURCE_SKIPPED_EVENT: &str = "resource_attempt_skipped";
pub(crate) const RESOURCE_TERMINALS: [&str; 3] = [
    "resource_attempt_completed",
    RESOURCE_FAILED_EVENT,
    RESOURCE_SKIPPED_EVENT,
];
pub(crate) const RESOURCE_SKIP_CODES: [&str; 4] = ["dependency", "explicit", "fan_in", "scheduler"];
pub(crate) const FAILED_STATUS: &str = "failed";
pub(crate) const SKIPPED_STATUS: &str = "skipped";
pub(crate) const LOADER_KIND: &str = "loader";
pub(crate) const AUDIT_KIND: &str = "audit";
pub(crate) const ASSET_OUTPUT: &str = "asset";
pub(crate) const CHECK_OUTPUT: &str = "check";
pub(crate) const MAX_RECORD_BYTES: usize = 65_536;
pub(crate) const MAX_METADATA_BYTES: usize = 16_384;
pub(crate) const MAX_STRING_CHARS: usize = 2_048;
pub(crate) const MAX_IDENTIFIER_CHARS: usize = 512;
pub(crate) const MAX_COLLECTION_ITEMS: usize = 128;
pub(crate) const MAX_NESTING_DEPTH: usize = 6;
pub(crate) const ASSET_KINDS: [&str; 11] = [
    "asset", "custom", "loader", "model", "seed", "snapshot", "source", "table", "table_fn", "udf",
    "view",
];
pub(crate) const CHECK_KINDS: [&str; 3] = [AUDIT_KIND, "python_check", "sql_test"];
pub(crate) const EXTRA_RESOURCE_KINDS: [&str; 3] = ["audit", "check", "test"];
pub(crate) const MODEL_RESOURCE_KINDS: [&str; 5] = ["custom", "model", "snapshot", "table", "view"];
pub(crate) const ASSET_STATUSES: [&str; 4] = ["failed", "skipped", "success", "warning"];
pub(crate) const FAILED_PHASES: [&str; 12] = [
    "audit",
    "contract",
    "custom_materialization",
    "dml",
    "fingerprint",
    "microbatch_state",
    "post_hook",
    "pre_hook",
    "promotion",
    "schema_change",
    "staging",
    "type_enforcement",
];
pub(crate) const CHECK_STATUSES: [&str; 6] =
    ["error", "fail", "insufficient", "pass", "skipped", "warn"];
pub(crate) const CHECK_NON_FAIL_STATUSES: [&str; 2] = ["insufficient", "pass"];
/// `AuditSeverity` values, in declaration order.
pub(crate) const AUDIT_SEVERITIES: [&str; 2] = ["warn", "error"];
/// Every audit attachment kind: model, source and seed attachments, and end-of-run audits.
pub(crate) const CHECK_ATTACHMENT_KINDS: [&str; 4] = ["end", "model", "seed", "source"];
pub(crate) const CHECK_RUN_SCOPE_PHASES: [&str; 2] = ["delta_and_final", "final"];
pub(crate) const EVALUATION_MODES: [&str; 2] = ["violations", "measurement"];
pub(crate) const BOUND_OPERATORS: [&str; 2] = ["below", "above"];
pub(crate) const RANGE_OPERATOR: &str = "outside";
pub(crate) const CLONE_ACTIONS: [&str; 7] = [
    "cloned",
    "copied",
    "recreated_view",
    "recreated_function",
    "warning_missing_source",
    "skipped_missing_dependency",
    "failed",
];
pub(crate) const SKIP_MODES: [&str; 2] = ["hard", "soft"];
pub(crate) const FUTURE_CURSOR_KEYS: [&str; 16] = [
    "action",
    "applied_bounds",
    "cursor_column",
    "determining_input",
    "discovered_bounds",
    "end",
    "future_end_detected",
    "future_start_detected",
    "inputs",
    "invocation_time",
    "max_distance",
    "maximum",
    "maximum_allowed_bounds",
    "minimum",
    "relation",
    "start",
];
pub(crate) const MAXIMUM_START_KEYS: [&str; 10] = [
    "action",
    "cursor_column",
    "effective_start",
    "highest_eligible_target_max",
    "input",
    "invocation_time",
    "max_ahead",
    "maximum_allowed_start",
    "physical_target_max",
    "relation",
];
pub(crate) const MAXIMUM_START_ACTIONS: [&str; 2] = ["cap", "error"];
pub(crate) const MAXIMUM_START_INPUT_KEYS: [&str; 2] = ["relation", "cursor_column"];
pub(crate) const MAXIMUM_START_REQUIRED_KEYS: [&str; 8] = [
    "action",
    "max_ahead",
    "invocation_time",
    "physical_target_max",
    "highest_eligible_target_max",
    "effective_start",
    "maximum_allowed_start",
    "input",
];
pub(crate) const MICROBATCH_KEYS: [&str; 22] = [
    "action",
    "batch_concurrency",
    "batch_count",
    "batch_size",
    "concurrent_enabled",
    "contiguous_frontier",
    "count",
    "global_concurrency",
    "known_gap_count",
    "limit",
    "physical_generation_id",
    "recovery_batch_count",
    "replay_requirement_id",
    "replay_requirement_state",
    "required_model_version_hash",
    "run_type",
    "strategy",
    "reason",
    "synthetic_completion_count",
    "unaccounted_interval_count",
    "unaccounted_partition_policy",
    "unknown_fingerprint_count",
];
pub(crate) const MICROBATCH_COUNT_KEYS: [&str; 10] = [
    "batch_concurrency",
    "batch_count",
    "count",
    "global_concurrency",
    "known_gap_count",
    "limit",
    "recovery_batch_count",
    "synthetic_completion_count",
    "unaccounted_interval_count",
    "unknown_fingerprint_count",
];
pub(crate) const MICROBATCH_RUN_TYPES: [&str; 3] = ["backfill", "normal", "replay_on_change"];
pub(crate) const MICROBATCH_LIMIT_ACTIONS: [&str; 4] =
    ["cap_from_end", "cap_from_start", "error", "warn"];
pub(crate) const MICROBATCH_PARTITION_POLICIES: [&str; 3] =
    ["recover_all", "recover_empty", "synthesize"];
pub(crate) const MICROBATCH_REPLAY_STATES: [&str; 4] = [
    "complete_with_unknown_fingerprints",
    "incomplete",
    "superseded",
    "verified_complete",
];
/// Asset fields in record order, with whether each is required.
pub(crate) const ASSET_FIELDS: [(&str, bool); 14] = [
    ("kind", true),
    ("name", true),
    ("status", true),
    ("action", false),
    ("target", false),
    ("origin_relation", false),
    ("staging_relation", false),
    ("failed_phase", false),
    ("loader", false),
    ("rows_loaded", false),
    ("materialized", false),
    ("future_cursor_safety", false),
    ("maximum_start_safety", false),
    ("microbatch", false),
];
/// Asset fields whose default is an empty object rather than `null`.
pub(crate) const ASSET_MAPPING_FIELDS: [&str; 3] =
    ["future_cursor_safety", "maximum_start_safety", "microbatch"];
pub(crate) const CHECK_FIELDS: [(&str, bool); 24] = [
    ("kind", true),
    ("name", true),
    ("check_id", true),
    ("passed", true),
    ("status", true),
    ("dag_check_id", false),
    ("severity", false),
    ("asset_name", false),
    ("attachment_kind", false),
    ("attached_column_name", false),
    ("attached_target_name", false),
    ("run_scope_phase", false),
    ("row_count", false),
    ("reused", false),
    ("evaluation_mode", false),
    ("measured_value", false),
    ("sample_count", false),
    ("sample_unit", false),
    ("minimum_samples", false),
    ("thresholds", false),
    ("evidence_count", false),
    ("evidence_truncated", false),
    ("audit_definition_name", false),
    ("audit_description", false),
];
/// Envelope fields in record order, before `projection_degraded`, `asset` and `checks`.
pub(crate) const ENVELOPE_FIELDS: [(&str, bool); 22] = [
    ("schema_version", true),
    ("record_kind", true),
    ("event_id", true),
    ("event_sequence", true),
    ("event_type", true),
    ("occurred_at", true),
    ("invocation_id", true),
    ("run_id", true),
    ("resource_id", true),
    ("resource_attempt_id", true),
    ("operation_id", true),
    ("statement_id", true),
    ("resource_kind", true),
    ("resource_name", true),
    ("attempt_number", true),
    ("duration_ms", true),
    ("output_kind", true),
    ("command", true),
    ("error_code", false),
    ("error_type", false),
    ("skip_code", false),
    ("skip_mode", false),
];
/// Asset enrichment a reader may drop to accept a newer writer's record.
pub(crate) const OPTIONAL_ASSET_ENRICHMENT: [&str; 4] = [
    "action",
    "failed_phase",
    "future_cursor_safety",
    "maximum_start_safety",
];
pub(crate) const OPTIONAL_MICROBATCH_ENUMS: [&str; 4] = [
    "action",
    "replay_requirement_state",
    "run_type",
    "unaccounted_partition_policy",
];
pub(crate) const OPTIONAL_CHECK_ENRICHMENT: [&str; 3] =
    ["attachment_kind", "run_scope_phase", "severity"];
pub(crate) const PROJECTION_DEGRADED_FIELD: &str = "projection_degraded";
pub(crate) const THRESHOLD_LEVELS: [&str; 2] = ["warn", "error"];
/// Maximum-start fields whose values are not plain timestamps.
pub(crate) const MAXIMUM_START_STRUCTURED_KEYS: [&str; 2] = ["action", "input"];
pub(crate) const MAXIMUM_START_NULLABLE_KEY: &str = "highest_eligible_target_max";
/// Identity fields that are not identifiers: the terminal type and its timestamp.
pub(crate) const NON_IDENTIFIER_IDENTITY_KEYS: [&str; 2] = ["event_type", "occurred_at"];
pub(crate) const MAX_HOUR: u32 = 23;
pub(crate) const MAX_MINUTE_OR_SECOND: u32 = 59;
pub(crate) const MAX_MONTH: u32 = 12;
pub(crate) const MAX_DAY: u32 = 31;
pub(crate) const MIN_DURATION_MS: f64 = 0.0;
