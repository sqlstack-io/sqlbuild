//! The fields discovery reports for SQL hook entries.

/// The fields a SQL hook entry reports, in output order.
pub(crate) const SQL_HOOK_OUTPUT_FIELDS: [&str; 6] = [
    "statement",
    "name",
    "relative_path",
    "definition_sql",
    "kwargs",
    "description",
];

/// The flag of the static commands that lets them compile without provider secrets.
pub(crate) const ALLOW_MISSING_SECRETS_FLAG: &str = "--allow-missing-secrets";
/// The environment variable that sets `--allow-missing-secrets`.
pub(crate) const ALLOW_MISSING_SECRETS_ENV: &str = "SQLBUILD_ALLOW_MISSING_SECRETS";
/// The `[settings]` key of `sqlbuild_local.toml` that sets `--allow-missing-secrets`.
pub(crate) const ALLOW_MISSING_SECRETS_SETTING: &str = "allow_missing_secrets";

/// Environment values that switch a boolean `SQLBUILD_*` variable on.
pub(crate) const TRUE_ENV_VALUES: [&str; 4] = ["1", "on", "true", "yes"];
