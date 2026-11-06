//! Authored MODEL header metadata keys the native parser recognises.

/// Keys one MODEL column mapping may declare.
pub const MODEL_COLUMN_KEYS: [&str; 5] =
    ["type", "nullable", "description", "audits", "migrate_from"];
/// The column key a MODEL header may declare and a SCHEMA declaration may not.
pub const MIGRATE_FROM_KEY: &str = "migrate_from";
/// Audit options that are not passed to the audit as arguments.
pub const AUDIT_OPTION_KEYS: [&str; 9] = [
    "name",
    "description",
    "severity",
    "run_scope",
    "always_run",
    "thresholds",
    "minimum_samples",
    "evidence_limit",
    "meta",
];
/// The `thresholds` keys of the warning and error bounds.
pub const THRESHOLD_WARN_KEY: &str = "warn";
pub const THRESHOLD_ERROR_KEY: &str = "error";
/// Threshold operators, as `ThresholdOperator` values.
pub const THRESHOLD_OPERATORS: [&str; 3] = [BELOW_OPERATOR, ABOVE_OPERATOR, "outside"];
pub const BELOW_OPERATOR: &str = "below";
pub const ABOVE_OPERATOR: &str = "above";
/// Audit severities a header may name.
pub const AUDIT_SEVERITIES: [&str; 2] = ["warn", "error"];
/// The audit that contradicts `nullable = true`.
pub const NOT_NULL_AUDIT_NAME: &str = "not_null";
/// The enum membership audit and its ordered values argument.
pub const ACCEPTED_VALUES_AUDIT: &str = "accepted_values";
pub const ACCEPTED_VALUES_ARGUMENT: &str = "values";
/// The audit collection on a schema column or model.
pub const AUDITS_KEY: &str = "audits";
/// The help every `ResourceIdentityError` shows.
pub const IDENTITY_HELP: &str = "Rename the authored identity and update its references, selectors, \
and integration keys. SQLBuild does not silently normalize resource identities. Double \
underscores remain valid.";
/// The identity suggested when nothing of the authored name remains.
pub const FALLBACK_IDENTITY: &str = "resource_name";
/// The label MODEL header messages name.
pub const MODEL_LABEL: &str = "model";
/// The code of the nullable column that also declares `not_null`.
pub const NULLABLE_NOT_NULL_CODE: &str = "P002";
/// Keys one `dynamic_columns` family may declare.
pub const DYNAMIC_FAMILY_KEYS: [&str; 5] = [
    "pivot_column",
    "value_column",
    "aggregate",
    "type",
    "name_pattern",
];
/// The family keys that must hold non-blank text, in the order they are checked.
pub const DYNAMIC_REQUIRED_KEYS: [&str; 4] = ["pivot_column", "value_column", "aggregate", "type"];
/// The optional regular expression that classifies a family's runtime columns.
pub const DYNAMIC_NAME_PATTERN_KEY: &str = "name_pattern";
