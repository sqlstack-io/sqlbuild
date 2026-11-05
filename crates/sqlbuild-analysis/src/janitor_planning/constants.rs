//! Janitor planning names, prefixes and skip reasons.

use sqlbuild_core::constants::COLUMN_MIGRATION_TABLE_NAME;
use sqlbuild_core::constants::CONSUMPTION_TABLE_NAME;
use sqlbuild_core::constants::FINGERPRINT_TABLE_NAME;
use sqlbuild_core::constants::MIGRATION_TABLE_NAME;
use sqlbuild_core::constants::OLD_NAME_VIEW_TABLE_NAME;
use sqlbuild_core::constants::SOURCE_FRESHNESS_TABLE_NAME;

/// SQLBuild state tables the janitor never acts on.
pub const BUILT_IN_EXCLUDE_PATTERNS: [&str; 10] = [
    FINGERPRINT_TABLE_NAME,
    "_sqlbuild_microbatches",
    SOURCE_FRESHNESS_TABLE_NAME,
    CONSUMPTION_TABLE_NAME,
    "_sqlbuild_node_results",
    "_sqlbuild_audit_results",
    "_sqlbuild_janitor_events",
    MIGRATION_TABLE_NAME,
    COLUMN_MIGRATION_TABLE_NAME,
    OLD_NAME_VIEW_TABLE_NAME,
];
/// Prefix of every strict janitor archive name.
pub const ARCHIVE_NAME_PREFIX: &str = "_SQB_ARCHIVE__";
/// Separator between archive name segments.
pub const ARCHIVE_NAME_SEPARATOR: &str = "__";
/// Folded prefix reserving a name for janitor archive handling.
pub const ARCHIVE_LOOKALIKE_PREFIX: &str = "_sqb_archive";
/// Label used in archive name fitting errors.
pub const ARCHIVE_ARTIFACT_LABEL: &str = "Janitor archive";
/// Reason for names that may require quoting.
pub const UNQUOTED_ADDRESSING_REASON: &str = "relation name is not a plain lowercase identifier and may require quoting; janitor does not act on it";
/// Reason for names that collide case-insensitively.
pub const CASE_COLLISION_REASON: &str =
    "relation name collides case-insensitively with another relation; janitor does not act on it";
/// Reason for archive lookalikes outside the strict grammar.
pub const MALFORMED_ARCHIVE_REASON: &str =
    "name resembles a janitor archive but does not match the strict archive grammar";
/// Reason for query-diff artifacts left to fingerprint expiry cleanup.
pub const QUERY_DIFF_ARTIFACT_REASON: &str = "query-diff artifacts use fingerprint expiry cleanup";
/// Reason for reserved query-diff names without ownership evidence.
pub const UNTRACKED_QUERY_DIFF_REASON: &str =
    "reserved query-diff name lacks matching ownership evidence";
/// Default reason for protected relations.
pub const PROTECTED_REASON: &str = "relation is protected from cleanup";
/// Reason for relations without a SQLBuild fingerprint.
pub const UNTRACKED_REASON: &str = "relation is not tracked by SQLBuild";
/// Reason when the adapter has no relation age metadata.
pub const AGE_UNSUPPORTED_REASON: &str = "adapter does not expose relation age metadata";
/// Reason when a relation's age is unknown.
pub const AGE_UNAVAILABLE_REASON: &str = "relation age is unavailable";
/// Reason protecting compatibility views at old model names.
pub const OLD_NAME_VIEW_REASON: &str = "compatibility view at an old model name";
/// Parts of a fully qualified migration origin.
pub const QUALIFIED_ORIGIN_PARTS: usize = 3;
/// Default janitor relation retention in days.
pub const DEFAULT_RETENTION_DAYS: i64 = 14;
/// Prefix of reserved query-diff artifact names.
pub const QUERY_DIFF_ARTIFACT_PREFIX: &str = "__sqlbuild_query_diff_";
/// Sides a query-diff artifact may hold.
pub const QUERY_DIFF_SIDES: [&str; 2] = ["left", "right"];
/// Hex digits ending a query-diff run id.
pub const ARTIFACT_RUN_HEX_DIGITS: usize = 12;
/// Digits of a year in a compact UTC timestamp.
pub const YEAR_DIGITS: usize = 4;
/// Digits of a month or day in a compact UTC timestamp.
pub const DATE_DIGITS: usize = 2;
/// Digits of an hour, minute or second in a compact UTC timestamp.
pub const TIME_DIGITS: usize = 2;
/// Largest month number.
pub const LAST_MONTH: u32 = 12;
/// Largest hour.
pub const LAST_HOUR: u32 = 23;
/// Largest minute.
pub const LAST_MINUTE: u32 = 59;
/// Largest second; leap seconds are rejected as Python's `datetime` rejects them.
pub const LAST_SECOND: u32 = 59;
