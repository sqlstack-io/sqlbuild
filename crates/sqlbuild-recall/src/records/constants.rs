//! Record vocabularies and limits.

/// The largest output chunk, in bytes.
pub const MAX_OUTPUT_CHUNK_BYTES: usize = 64 * 1024;
/// The output record ID infix, shared with command-output sink records.
pub const OUTPUT_RECORD_ID_INFIX: &str = "command-output";
pub const DIAGNOSTIC_RECORD_ID_INFIX: &str = "diagnostic";
/// Artifact digests are content digests with this prefix, as statement `sql_digest` without it.
pub const ARTIFACT_DIGEST_PREFIX: &str = "sha256:";
pub const SQL_ARTIFACT_KIND: &str = "sql";
pub const SQL_MEDIA_TYPE: &str = "text/x-sql";
pub const EXECUTED_SQL_ROLE: &str = "executed_sql";
/// Evidence rows of a warning or failing audit, stored as a JSON array of row objects.
pub const AUDIT_EVIDENCE_KIND: &str = "audit_evidence";
pub const AUDIT_EVIDENCE_MEDIA_TYPE: &str = "application/json";
pub const AUDIT_EVIDENCE_ROLE: &str = "audit_evidence";
/// The event whose payload may carry evidence rows.
pub const AUDIT_COMPLETED_EVENT: &str = "audit_completed";
/// A statement's start, folded into its end's row when both reach the same batch.
pub const STATEMENT_STARTED_EVENT: &str = "statement_started";
/// The events that end a statement.
pub const STATEMENT_TERMINAL_EVENTS: [&str; 2] = ["statement_completed", "statement_failed"];
/// The event field of a folded end that keeps its statement's start time.
pub const STARTED_AT_FIELD: &str = "started_at";
/// The payload field holding evidence rows, which Recall stores as an artifact instead.
pub const EVIDENCE_PAYLOAD_FIELD: &str = "evidence";
/// The payload field naming the evidence artifact's digest, without its `sha256:` prefix.
pub const SAMPLE_DIGEST_PAYLOAD_FIELD: &str = "sample_digest";
/// The most selector text an invocation's context row records; later selectors are counted but left out.
pub const MAX_RECORDED_SELECTION_BYTES: usize = 64 * 1024;
/// Diagnostic severities, least severe first.
pub const SEVERITY_ORDER: [&str; 6] = ["trace", "debug", "info", "warning", "error", "critical"];
/// Event payload fields promoted to their own `recall_events` columns.
pub const PROMOTED_TEXT_FIELDS: [&str; 11] = [
    "command",
    "resource_kind",
    "resource_name",
    "error_code",
    "error_type",
    "query_id",
    "sql_digest",
    "intent",
    "statement_kind",
    "purpose",
    "outcome",
];
pub const PROMOTED_INTEGER_FIELDS: [&str; 3] = ["exit_code", "row_count", "affected_rows"];
pub const PROMOTED_REAL_FIELDS: [&str; 1] = ["duration_ms"];
pub const CORRELATION_FIELDS: [&str; 5] = [
    "run_id",
    "resource_id",
    "resource_attempt_id",
    "operation_id",
    "statement_id",
];
pub(crate) const MICROS_PER_SECOND: i64 = 1_000_000;
pub(crate) const SECONDS_PER_DAY: i64 = 86_400;
pub(crate) const DECIMAL_RADIX: u32 = 10;
pub(crate) const FEBRUARY: i64 = 2;
/// In a year counted from March, January is month index 10.
pub(crate) const MARCH_BASED_JANUARY_INDEX: i64 = 10;
/// The output column counting chunks dropped just before this one.
pub const DROPPED_BEFORE_COLUMN: &str = "dropped_before";
/// Initial capacity of the buffer a row's content hash is computed over.
pub(crate) const CONTENT_HASH_BUFFER_BYTES: usize = 1_024;
/// The origin of an invocation an integration such as Dagster started.
pub const INTEGRATION_ORIGIN: &str = "integration";
/// The origin of an invocation a CI system ran.
pub const CI_ORIGIN: &str = "ci";
/// The origin of every other invocation.
pub const LOCAL_ORIGIN: &str = "local";
/// Environment variables a CI system sets; any non-empty value other than `false` or `0` marks a CI run.
pub const CI_ENVIRONMENT_VARIABLES: [&str; 10] = [
    "CI",
    "GITHUB_ACTIONS",
    "GITLAB_CI",
    "BUILDKITE",
    "JENKINS_URL",
    "CIRCLECI",
    "TF_BUILD",
    "TEAMCITY_VERSION",
    "BITBUCKET_BUILD_NUMBER",
    "CODEBUILD_BUILD_ID",
];
/// Values of a CI variable that do not mark a CI run, besides an empty one.
pub const CI_DISABLED_VALUES: [&str; 2] = ["false", "0"];
/// The `command_mode` of `sqb debug --no-connection`: configuration checks only.
pub const CONFIG_ONLY_MODE: &str = "config_only";
/// The `command_mode` of `sqb debug`: configuration checks and a connection test.
pub const CONFIG_AND_CONNECTION_MODE: &str = "config_and_connection";
/// The command whose mode Recall records.
pub(crate) const DEBUG_COMMAND: &str = "debug";
/// The output column holding the bytes a truncation gap replaces.
pub const OMITTED_BYTES_COLUMN: &str = "omitted_bytes";
/// The output column holding the sequences a truncation gap accounts for.
pub const OMITTED_SEQUENCES_COLUMN: &str = "omitted_sequences";
