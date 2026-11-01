//! Value kinds, payload keys, duration units and messages of source freshness state.

pub const TIMESTAMP_VALUE_KIND: &str = "timestamp";
pub(crate) const SOURCE_NAME_KEY: &str = "source_name";
pub(crate) const STRATEGY_KEY: &str = "strategy";
pub(crate) const VALUE_KIND_KEY: &str = "value_kind";
pub(crate) const DATA_VERSION_KEY: &str = "data_version";
pub(crate) const COMPACT_ITEM_SEPARATOR: &str = ",";
pub(crate) const COMPACT_KEY_SEPARATOR: &str = ":";
pub(crate) const UTC_SUFFIX: &str = "+00:00";
pub(crate) const SECONDS_PER_MINUTE: u128 = 60;
pub(crate) const SECONDS_PER_HOUR: u128 = 3_600;
pub(crate) const SECONDS_PER_DAY: u128 = 86_400;
pub(crate) const MICROS_PER_SECOND: u128 = 1_000_000;
/// `timedelta.max.days`.
pub(crate) const MAX_TIMEDELTA_DAYS: u128 = 999_999_999;
/// The largest C `int` a `timedelta` day count passes through.
pub(crate) const MAX_C_INT: u128 = i32::MAX as u128;
pub(crate) const C_INT_OVERFLOW: &str = "Python int too large to convert to C int";
pub const LAG_TOLERANCE_INVALID: &str =
    "source freshness lag_tolerance must be a positive duration like 15m, 2h, or 1d";
pub const AGE_POLICY_INVALID: &str =
    "source freshness age_policy value must be a positive duration like 15m, 2h, or 1d";
pub(crate) const ERA_YEARS: i128 = 400;
pub(crate) const DAYS_PER_ERA: i128 = 146_097;
pub(crate) const DAYS_PER_YEAR: i128 = 365;
pub(crate) const MARCH_BASED_MONTH_DAYS: i128 = 153;
pub(crate) const MONTH_LENGTH_DIVISOR: i128 = 5;
pub(crate) const MONTHS_FROM_MARCH: i128 = 9;
pub(crate) const FEBRUARY: u32 = 2;
/// Columns a source freshness state table must have, in Python's declaration order.
pub const REQUIRED_STATE_COLUMNS: [&str; 6] = [
    "source_name",
    "run_id",
    "strategy",
    "value_kind",
    "data_version_hash",
    "observed_at",
];
/// State value type of a text column.
pub const STRING_STATE_VALUE_TYPE: &str = "string";
/// State value type of a timestamp stored as ISO text.
pub const TEXT_TIMESTAMP_STATE_VALUE_TYPE: &str = "text_timestamp";
/// Legacy latest-reader columns; the nullable event ID is intentionally not required by reads.
pub const SOURCE_FRESHNESS_COLUMN_TYPES: [(&str, &str); 10] = [
    ("source_name", STRING_STATE_VALUE_TYPE),
    ("target_database", STRING_STATE_VALUE_TYPE),
    ("target_schema", STRING_STATE_VALUE_TYPE),
    ("target_name", STRING_STATE_VALUE_TYPE),
    ("run_id", STRING_STATE_VALUE_TYPE),
    ("strategy", STRING_STATE_VALUE_TYPE),
    ("value_kind", STRING_STATE_VALUE_TYPE),
    ("data_version", STRING_STATE_VALUE_TYPE),
    ("data_version_hash", STRING_STATE_VALUE_TYPE),
    ("observed_at", TEXT_TIMESTAMP_STATE_VALUE_TYPE),
];
pub const SOURCE_FRESHNESS_EVENT_ID: &str = "event_id";
pub const OBSERVED_AT_COLUMN: &str = "observed_at";
pub const SQLITE_ADAPTER: &str = "sqlite";
pub(crate) const SQLSERVER_ADAPTER: &str = "sqlserver";
pub(crate) const PHYSICAL_TABLE_SOURCE_FRAGMENT: &str = "requires a physical table source";
pub(crate) const INCOMPLETE_CONFIGURATION_FRAGMENT: &str = "incomplete";
