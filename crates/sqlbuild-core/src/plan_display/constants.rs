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
/// The plan JSON key describing the synced copy a preview planned from.
pub const SYNCED_COPY_KEY: &str = "synced_copy";
/// What a preview prints where a plan would connect to the warehouse: it reads the synced copy.
pub const SYNCED_COPY_READING: &str = "Reading the synced warehouse state...";
/// Why a preview refuses `--defer-to`: deferral reads another target's live state.
pub const SYNCED_COPY_DEFER_REFUSAL: &str =
    "plan previews from the synced copy cannot use --defer-to";
pub const SYNCED_COPY_DEFER_HELP: &str = "Run re-plans live, including deferral.";
/// The planner's action for a model the run leaves alone.
pub(crate) const SKIP_ACTION: &str = "skip";
/// The reason of a function or seed without a change.
pub(crate) const NO_CHANGE_REASON: &str = "no_change";
/// The reason of a model or seed without a recorded build.
pub(crate) const FIRST_RUN_REASON: &str = "first_run";
/// The plan document field of per-model metadata, which a summary leaves out.
pub(crate) const METADATA_FIELD: &str = "metadata";
/// The plan document's row lists, which a summary empties and counts by section instead.
pub(crate) const PLAN_ROW_LISTS: [&str; 10] = [
    "models",
    "seeds",
    "source_loads",
    "functions",
    "python_nodes",
    "warnings",
    "migrations",
    "column_migrations",
    "table_type_conversions",
    "retention",
];
/// The `--changed` row lists, which a summary empties and counts by section instead.
pub(crate) const CHANGED_ROW_LISTS: [&str; 3] = ["runs", "held_back", "unchanged"];
/// The most distinct notes a summary counts for one section, the most frequent first.
pub const MAX_SECTION_NOTES: usize = 20;
/// The unified diff header lines, which are neither added nor removed lines.
pub(crate) const DIFF_HEADER_PREFIXES: [&str; 3] = ["---", "+++", "@@"];
/// The model fields that hold diff text, which pages leave out and serve one model at a time.
pub const MODEL_DIFF_FIELDS: [&str; 2] = ["query_diff", "config_diff"];
/// The model field holding the counts of its diffs, kept when the diff text is not.
pub const DIFF_STATS_FIELD: &str = "diff_stats";
/// The model field of a short config change in words, such as `full_refresh true → false`.
pub const CONFIG_CHANGE_FIELD: &str = "config_change";
/// The most top-level config keys a config change reads in one line.
pub(crate) const CONFIG_CHANGE_KEYS: usize = 2;
/// The indent of a top-level key in a config diff, which prints the config as JSON with two-space indents.
pub(crate) const CONFIG_TOP_LEVEL_INDENT: &str = "  ";
/// A config key's value on the side of a change where it is not set.
pub(crate) const UNSET_CONFIG_VALUE: &str = "(unset)";
/// What a warning template says in place of a quoted or generated name.
pub(crate) const ELIDED_NAME: &str = "…";
/// The quotes a warning puts around a name.
pub(crate) const QUOTE_CHARACTERS: [char; 3] = ['\'', '`', '"'];
/// The summary field listing warning templates with their counts.
pub(crate) const WARNING_GROUPS_FIELD: &str = "warning_groups";
/// The most warning groups a summary lists, the largest first; warnings past them page as the rest.
pub(crate) const MAX_WARNING_GROUPS: usize = 50;
