//! Authored SQL-test mode identifiers.

pub(crate) const MODEL_TEST_MODE: &str = "model";
/// The SQL Rules whose fixes may change a query's meaning, so `sqb format --fix` never applies them; as in SQLClean, every other SQL Rule fix is safe.
pub(crate) const UNSAFE_RULE_FIX_CODES: [&str; 1] = ["SQLCLEAN001"];
/// The note attached to a rule finding that has an available fix.
pub(crate) const RULE_FIX_AVAILABLE_NOTE: &str =
    "Fix available: `sqb format --fix` applies it after compiler verification";
pub(crate) const RULES_NATIVE_API_VERSION: i64 = 1;
/// File name of the custom-rule result cache under the project cache directory.
pub(crate) const CUSTOM_RULES_CACHE_FILE: &str = "custom-rules.json";
/// Code prefix of the SQL lint rules, which check each selected file on its own.
pub(crate) const SQL_RULE_PREFIX: &str = "SQLCLEAN";
/// Suffix of a custom host scratch directory abandoned by a cancelled run.
pub(crate) const CUSTOM_HOST_ABANDONED_SUFFIX: &str = ".abandoned";
/// Marker file whose presence tells split custom hosts the run was cancelled.
pub(crate) const CUSTOM_HOST_CANCELLED_MARKER: &str = "cancelled";
/// Rules that need proven column types and are skipped without SQL analysis.
pub(crate) const TYPE_PROOF_RULE_CODES: [&str; 1] = ["SQBRCONTRACT105"];
/// Runtime identity custom-rule hosts and their cache entries record.
pub(crate) const CUSTOM_HOST_RUNTIME_VERSION: &str = "sqlbuild-rules-custom-v3";

/// Native rules evaluations answered from the response memo.
pub(crate) const RULES_MEMO_REUSES: &str = "rules_memo_reuses";
/// The `RulesError` attribute holding how to turn off default Rules, printed only with human error text.
pub(crate) const DEFAULT_RULES_NOTICE_ATTR: &str = "default_rules_notice";
