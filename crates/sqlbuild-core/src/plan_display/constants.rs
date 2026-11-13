//! Materialization, action and cursor vocabulary the plan display reads.

pub(crate) const VIEW: &str = "view";
pub(crate) const TABLE: &str = "table";
pub(crate) const INCREMENTAL: &str = "incremental";
pub(crate) const SNAPSHOT: &str = "snapshot";
pub(crate) const CUSTOM: &str = "custom";
pub(crate) const MICROBATCH_MODE: &str = "microbatch";
pub(crate) const INCREMENTAL_ACTIONS: [&str; 3] = [
    "incremental_append",
    "incremental_delete_insert",
    "incremental_merge",
];
pub const INTEGER_CURSOR: &str = "integer";
pub const TIMESTAMP_CURSOR: &str = "timestamp";
pub const DEFAULT_CURSOR_GRAIN: &str = "second";
pub(crate) const WHOLE_DAY_CURSOR_GRAINS: [&str; 3] = ["day", "month", "year"];
pub(crate) const EFFECTIVE_BATCH_SIZE_TOKEN: &str = "effective";
pub(crate) const GRAIN_BATCH_SIZE: [(&str, &str); 6] = [
    ("second", "1s"),
    ("minute", "1m"),
    ("hour", "1h"),
    ("day", "1d"),
    ("month", "1mo"),
    ("year", "1y"),
];
pub(crate) const OLD_NAME_NONE: &str = "none";
pub(crate) const OLD_NAME_LIVE: &str = "live";
pub(crate) const OLD_NAME_VIEW_ONLY: &str = "view_only";
pub(crate) const MIGRATION_DONE: &str = "done";
pub(crate) const MIGRATION_RENAMED: &str = "renamed";
pub(crate) const MIGRATION_MANUAL: &str = "manual";
pub(crate) const MIGRATION_AUTOMATIC: &str = "automatic";
pub(crate) const MIGRATION_INCOMPATIBLE: &str = "incompatible";
pub(crate) const WATERMARK_STRATEGY: &str = "watermark";
pub(crate) const QUERY_DIFF_INDENT: &str = "      ";
pub(crate) const UNIFIED_DIFF_HEADER_PREFIXES: [&str; 3] = ["---", "+++", "@@"];
/// Names the synced-copy file a location writes for a plan preview; set, `sqb plan` never connects.
pub const PLAN_SYNCED_COPY_ENV: &str = "SQLBUILD_PLAN_SYNCED_COPY";
/// The plan JSON key describing the synced copy a preview planned from.
pub const SYNCED_COPY_KEY: &str = "synced_copy";
/// Why a preview refuses `--defer-to`: deferral reads another target's live state.
pub const SYNCED_COPY_DEFER_REFUSAL: &str =
    "plan previews from the synced copy cannot use --defer-to";
pub const SYNCED_COPY_DEFER_HELP: &str = "Run re-plans live, including deferral.";
