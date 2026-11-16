//! Config keys and vocabularies the Python model validators accept.

/// Built-in materializations that never name a custom materialization.
pub const BUILTIN_MATERIALIZATIONS: [&str; 4] = ["view", "table", "incremental", "snapshot"];
/// Materializations that own a physical managed table.
pub const TABLE_BACKED_MATERIALIZATIONS: [&str; 3] = ["table", "incremental", "snapshot"];
/// Materializations that keep history across runs.
pub const HISTORY_MATERIALIZATIONS: [&str; 2] = ["incremental", "snapshot"];
/// The view materialization.
pub const VIEW_MATERIALIZATION: &str = "view";
/// The table materialization.
pub const TABLE_MATERIALIZATION: &str = "table";
/// The incremental materialization.
pub const INCREMENTAL_MATERIALIZATION: &str = "incremental";
/// The snapshot materialization.
pub const SNAPSHOT_MATERIALIZATION: &str = "snapshot";

/// `IncrementalStrategy` values.
pub const INCREMENTAL_STRATEGIES: [&str; 3] = ["append", "delete_insert", "merge"];
/// The append incremental strategy.
pub const APPEND_STRATEGY: &str = "append";
/// The delete-insert incremental strategy.
pub const DELETE_INSERT_STRATEGY: &str = "delete_insert";
/// The merge incremental strategy.
pub const MERGE_STRATEGY: &str = "merge";
/// `IncrementalMode` values.
pub const INCREMENTAL_MODES: [&str; 2] = ["full", "microbatch"];
/// The microbatch incremental mode.
pub const MICROBATCH_MODE: &str = "microbatch";
/// `MicrobatchStrategy` values.
pub const MICROBATCH_STRATEGIES: [&str; 2] = ["rolling_window", "watermark"];
/// The rolling-window microbatch strategy.
pub const ROLLING_WINDOW_STRATEGY: &str = "rolling_window";
/// The watermark microbatch strategy.
pub const WATERMARK_STRATEGY: &str = "watermark";
/// `CursorWatermarkMode` values.
pub const WATERMARK_MODES: [&str; 2] = ["all", "any"];
/// `CursorInputRole` values.
pub const CURSOR_INPUT_ROLES: [&str; 2] = ["filter", "watermark"];
/// The filter cursor input role.
pub const FILTER_ROLE: &str = "filter";
/// The watermark cursor input role.
pub const WATERMARK_ROLE: &str = "watermark";
/// `CursorType` values.
pub const CURSOR_TYPES: [&str; 2] = ["timestamp", "integer"];
/// The timestamp cursor type.
pub const TIMESTAMP_CURSOR: &str = "timestamp";
/// The integer cursor type.
pub const INTEGER_CURSOR: &str = "integer";
/// `CursorGrain` values with their `GRAIN_BATCH_SIZE` durations.
pub const CURSOR_GRAIN_BATCH_SIZES: [(&str, &str); 6] = [
    ("second", "1s"),
    ("minute", "1m"),
    ("hour", "1h"),
    ("day", "1d"),
    ("month", "1mo"),
    ("year", "1y"),
];
/// `FutureCursorAction` values.
pub const FUTURE_CURSOR_ACTIONS: [&str; 2] = ["cap", "error"];
/// `MicrobatchLimitAction` values.
pub const MICROBATCH_LIMIT_ACTIONS: [&str; 4] = ["cap_from_end", "cap_from_start", "error", "warn"];
/// The cap-from-start microbatch limit action.
pub const CAP_FROM_START_ACTION: &str = "cap_from_start";
/// Accepted `unaccounted_partition_policy` values.
pub const UNACCOUNTED_PARTITION_POLICIES: [&str; 3] =
    ["synthesize", "recover_empty", "recover_all"];
/// `ContractPolicy` values.
pub const CONTRACT_POLICIES: [&str; 2] = ["none", "enforced"];
/// The enforced contract policy.
pub const ENFORCED_CONTRACT: &str = "enforced";
/// `OnSchemaChange` values.
pub const ON_SCHEMA_CHANGE_POLICIES: [&str; 4] =
    ["ignore", "fail", "append_new_columns", "sync_all_columns"];
/// Unbounded `replay_on_change` values.
pub const REPLAY_ON_CHANGE_ACTIONS: [&str; 2] = ["forward", "full"];
/// The prefix of a bounded `replay_on_change` value.
pub const BOUNDED_REPLAY_PREFIX: &str = "bounded-";
/// `SnapshotStrategy` values.
pub const SNAPSHOT_STRATEGIES: [&str; 2] = ["timestamp", "check"];
/// The timestamp snapshot strategy.
pub const TIMESTAMP_SNAPSHOT_STRATEGY: &str = "timestamp";
/// The check snapshot strategy.
pub const CHECK_SNAPSHOT_STRATEGY: &str = "check";
/// `HistoricalInput` values.
pub const HISTORICAL_INPUTS: [&str; 2] = ["snapshot", "changes"];
/// The changes historical input.
pub const CHANGES_HISTORICAL_INPUT: &str = "changes";
/// `InitialValidFrom` values.
pub const INITIAL_VALID_FROM_VALUES: [&str; 3] = ["updated_at", "observed_at", "execution_time"];
/// `SnapshotFullRefreshPolicy` values.
pub const SNAPSHOT_FULL_REFRESH_POLICIES: [&str; 3] = ["deny", "require_confirmation", "allow"];
/// `SnapshotSchemaChangePolicy` values.
pub const SNAPSHOT_SCHEMA_CHANGE_POLICIES: [&str; 3] =
    ["deny", "require_confirmation", "append_new_columns"];
/// The snapshot schema-change policy that conflicts with an enforced contract.
pub const APPEND_NEW_COLUMNS_POLICY: &str = "append_new_columns";
/// The `on_schema_change` value the change-policy help shows.
pub const ON_SCHEMA_CHANGE_EXAMPLE: &str = "append_new_columns";
/// The `replay_on_change` value the change-policy help shows.
pub const REPLAY_ON_CHANGE_EXAMPLE: &str = "bounded-14d";
/// The `replay_on_change` values the change-policy message lists, in its order.
pub const REPLAY_ON_CHANGE_VALID_VALUES: [&str; 3] = ["forward", "full", "bounded-<duration>"];
/// The code of the snapshot schema-change conflict with an enforced contract.
pub const CONTRACT_SCHEMA_CHANGE_CODE: &str = "K012";
/// The note naming the project setting that keeps microbatches sequential.
pub const MICROBATCH_CONCURRENCY_NOTE: &str = "the current value is [settings] \
microbatch_concurrency = false (from sqlbuild_project.toml or its default)";
/// The help showing the project setting that allows concurrent microbatches.
pub const MICROBATCH_CONCURRENCY_HELP: &str = "to run microbatches concurrently, set this in \
sqlbuild_project.toml:\n            [settings]\n            microbatch_concurrency = true";
/// The indent of the setting snippet lines in help text.
pub const SETTING_SNIPPET_INDENT: &str = "            ";
/// The duration-disabled cursor policy value.
pub const CURSOR_POLICY_DISABLED: &str = "disabled";
/// The `batch_size` token that follows the cursor grain.
pub const EFFECTIVE_BATCH_SIZE: &str = "effective";
/// The SQL wildcard column.
pub const SQL_WILDCARD: &str = "*";
/// The `@@@name` placeholder sigil.
pub const PLACEHOLDER_SIGIL: &str = "@@@";

/// Keys that only incremental models may set.
pub const INCREMENTAL_ONLY_KEYS: [&str; 6] = [
    "on_schema_change",
    "replay_on_change",
    "append_cursor_inclusive",
    "merge_exclude_columns",
    "full_refresh",
    "replace_key",
];
/// Cursor input keys that require a cursor-based incremental model.
pub const CURSOR_INPUT_KEYS: [&str; 3] = [
    "cursor_inputs",
    "cursor_filter_inputs",
    "cursor_watermark_inputs",
];
/// Cursor input keys that have been removed in favour of `cursor_inputs`.
pub const REMOVED_CURSOR_INPUT_KEYS: [&str; 2] =
    ["cursor_filter_inputs", "cursor_watermark_inputs"];
/// Keys a snapshot model may not set.
pub const SNAPSHOT_DISALLOWED_KEYS: [&str; 16] = [
    "incremental_strategy",
    "incremental_mode",
    "append_cursor_inclusive",
    "batch_size",
    "cursor",
    "cursor_type",
    "cursor_grain",
    "cursor_inputs",
    "cursor_filter_inputs",
    "cursor_watermark_inputs",
    "cursor_end",
    "microbatch_strategy",
    "cursor_watermark_mode",
    "max_microbatches",
    "microbatch_limit",
    "lookback",
];
/// Keys a custom materialization may not set.
pub const CUSTOM_MATERIALIZATION_DISALLOWED_KEYS: [&str; 17] = [
    "on_schema_change",
    "incremental_strategy",
    "incremental_mode",
    "append_cursor_inclusive",
    "batch_size",
    "cursor",
    "cursor_type",
    "cursor_grain",
    "cursor_inputs",
    "cursor_filter_inputs",
    "cursor_watermark_inputs",
    "cursor_end",
    "microbatch_strategy",
    "cursor_watermark_mode",
    "max_microbatches",
    "microbatch_limit",
    "lookback",
];
/// The `microbatch_limit` keys, in no particular order.
pub const MICROBATCH_LIMIT_KEYS: [&str; 2] = ["max_batches", "action"];
/// The watermark `cursor_inputs` block keys, in no particular order.
pub const WATERMARK_BLOCK_KEYS: [&str; 2] = ["column", "roles"];
/// The config key naming the old-name compatibility view.
pub const OLD_NAME_VIEW_KEY: &str = "old_name_view";

/// `SqlReferenceKind` values a model may reference.
pub const REF_KIND: &str = "ref";
/// A seed reference.
pub const SEED_KIND: &str = "seed";
/// A source reference.
pub const SOURCE_KIND: &str = "source";
/// A scalar SQL function reference.
pub const UDF_KIND: &str = "udf";
/// A table function reference.
pub const TABLE_FUNCTION_KIND: &str = "table_fn";
/// The `__dbt_ref` reference kind.
pub const DBT_REF_KIND: &str = "dbt_ref";

/// The `inherit` storage policy value.
pub const INHERIT_POLICY: &str = "inherit";
/// The `disabled` retention value.
pub const DISABLED_RETENTION: &str = "disabled";
/// The permanent table type.
pub const PERMANENT_TABLE_TYPE: &str = "permanent";
/// The transient table type.
pub const TRANSIENT_TABLE_TYPE: &str = "transient";

/// The position of days among duration units.
pub const DAY_UNIT_INDEX: usize = 2;

/// The first and last years `datetime` accepts.
pub(crate) const MIN_YEAR: i64 = 1;
pub(crate) const MAX_YEAR: i64 = 9999;
pub(crate) const MICROS_PER_SECOND: i64 = 1_000_000;
pub(crate) const SECONDS_PER_MINUTE: i64 = 60;
pub(crate) const SECONDS_PER_HOUR: i64 = 3_600;
pub(crate) const HOURS_PER_DAY: i64 = 24;
pub(crate) const MICROS_PER_DAY: i64 = 86_400 * MICROS_PER_SECOND;
/// The largest valid hour, minute and second fields.
pub(crate) const LAST_HOUR: i64 = HOURS_PER_DAY - 1;
pub(crate) const LAST_MINUTE: i64 = SECONDS_PER_MINUTE - 1;
pub(crate) const LAST_SECOND: i64 = SECONDS_PER_MINUTE - 1;
pub(crate) const MONTHS_PER_YEAR: i64 = 12;
pub(crate) const FEBRUARY: i64 = 2;
pub(crate) const DECEMBER_INDEX: usize = 12;
pub(crate) const DAYS_PER_WEEK: i64 = 7;
pub(crate) const DAYS_PER_YEAR: i64 = 365;
pub(crate) const DAYS_PER_4_YEARS: i64 = 1_461;
pub(crate) const DAYS_PER_CENTURY: i64 = 36_524;
pub(crate) const DAYS_PER_400_YEARS: i64 = 146_097;
/// The Gregorian leap-year cycles in years.
pub(crate) const LEAP_CYCLE: i64 = 4;
pub(crate) const CENTURY: i64 = 100;
pub(crate) const GREGORIAN_CYCLE: i64 = 400;
/// `ord_to_ymd`'s month estimate `(days + 50) >> 5`.
pub(crate) const ORDINAL_MONTH_BIAS: i64 = 50;
pub(crate) const ORDINAL_MONTH_SHIFT: i64 = 5;
/// Monday-based weekday numbers of the first day of a year with 53 ISO weeks.
pub(crate) const WEDNESDAY: i64 = 2;
pub(crate) const THURSDAY: i64 = 3;
pub(crate) const LONG_YEAR_WEEKS: i64 = 53;
/// Days before each month in a common year, indexed by month.
pub(crate) const DAYS_BEFORE_MONTH: [i64; 13] =
    [0, 0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
/// Days in each month of a common year, indexed by month.
pub(crate) const DAYS_IN_MONTH: [i64; 13] = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
/// Multipliers that scale a fraction of 1-5 digits to microseconds.
pub(crate) const FRACTION_CORRECTION: [i64; 5] = [100_000, 10_000, 1_000, 100, 10];
pub(crate) const FRACTION_DIGITS: usize = 6;
/// Hour, minute and second; the fraction follows them.
pub(crate) const CLOCK_COMPONENTS: usize = 3;
/// ISO date text lengths `_find_isoformat_datetime_separator` tests.
pub(crate) const YEAR_DIGITS: usize = 4;
pub(crate) const WEEK_DATE_MARK_INDEX: usize = 5;
pub(crate) const BASIC_WEEK_DATE_LENGTH: usize = 7;
pub(crate) const EXTENDED_WEEK_DATE_LENGTH: usize = 8;
pub(crate) const EXTENDED_DATE_LENGTH: usize = 10;
pub(crate) const EXTENDED_WEEK_DAY_LENGTH: usize = 10;
/// CPython minor releases whose `fromisoformat` behaviour changed.
pub(crate) const PYTHON_313: u8 = 13;
pub(crate) const PYTHON_314: u8 = 14;
/// The largest duration amount SQLBuild accepts: a signed 64-bit integer.
pub(crate) const MAX_DURATION_AMOUNT: u128 = i64::MAX as u128;
/// The MODEL header key of the retention override.
pub(crate) const RETENTION_KEY: &str = "time_travel_retention";

/// Materializations whose columns can be renamed when the model also migrates.
pub(crate) const MIGRATABLE_MATERIALIZATIONS: [&str; 4] =
    ["table", "view", "incremental", "snapshot"];
pub(crate) const QUALIFIED_NAME_SEPARATOR: char = '.';
pub(crate) const MATERIALIZATION_HELP: &str =
    "remove migrate_from from the column, or declare the model's own migrate_from";
pub(crate) const ORIGIN_HELP: &str =
    "write the old column name, for example revenue (migrate_from amount)";
pub(crate) const CLAIMED_HELP: &str =
    "keep migrate_from on the column that should receive the existing data";
pub(crate) const CHAINED_HELP: &str = "a chain or swap cannot be renamed in place safely; give the new column \
                            a name no other renamed column uses, or rebuild the model with \
                            --full-refresh";

/// The table engine key.
pub const ENGINE_KEY: &str = "engine";
/// The sorting key.
pub const ORDER_BY_KEY: &str = "order_by";
/// The partitioning key.
pub const PARTITION_BY_KEY: &str = "partition_by";
/// The primary key, a prefix of the sorting key.
pub const PRIMARY_KEY_KEY: &str = "primary_key";
/// The table settings key.
pub const TABLE_SETTINGS_KEY: &str = "table_settings";
/// Physical table layout keys, in the order their errors are checked.
pub const PHYSICAL_STORAGE_KEYS: [&str; 5] = [
    ENGINE_KEY,
    ORDER_BY_KEY,
    PARTITION_BY_KEY,
    PRIMARY_KEY_KEY,
    TABLE_SETTINGS_KEY,
];
/// Materializations whose tables SQLBuild creates with the model's physical layout.
pub const PHYSICAL_STORAGE_MATERIALIZATIONS: [&str; 2] = ["table", "incremental"];
/// The name suffix every supported table engine shares.
pub const ENGINE_FAMILY_SUFFIX: &str = "MergeTree";
/// Engine name prefixes of replicated engines, which need a cluster.
pub const UNSUPPORTED_ENGINE_PREFIXES: [&str; 1] = ["Replicated"];
