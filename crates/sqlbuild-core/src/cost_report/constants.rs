//! Cost artifact identity, report wording and table layout.

pub(crate) const ARTIFACT_VERSION: i64 = 1;
pub(crate) const OUTPUT_SCHEMA_VERSION: i64 = 1;
pub(crate) const LATEST_SELECTOR: &str = "latest";
pub(crate) const RUNNING_BUILD_STATUS: &str = "running";
pub(crate) const DEFAULT_RATE_SOURCE: &str = "default";
/// The list price for the account's region and edition: an estimate.
pub(crate) const LIST_PRICE_RATE_SOURCE: &str = "list_price";
/// The account's contract price, from Snowflake's rate sheet.
pub(crate) const RATE_SHEET_RATE_SOURCE: &str = "rate_sheet";
pub(crate) const COMPLETE_STATUS: &str = "complete";
pub(crate) const COLLECTION_FAILED_STATUS: &str = "collection_failed";
pub(crate) const WARNING_STATUSES: [&str; 2] = ["partial", "pending"];
pub(crate) const COST_STATUSES: [&str; 5] = [
    "complete",
    "partial",
    "pending",
    "unavailable",
    "collection_failed",
];
pub(crate) const UNSAFE_RUN_IDS: [&str; 2] = [".", ".."];
pub(crate) const RUN_ID_SEPARATORS: [char; 2] = ['/', '\\'];
pub(crate) const NARROW_TERMINAL_WIDTH: usize = 120;
pub(crate) const HISTORY_NUMERIC_COLUMNS: [usize; 2] = [4, 5];
pub(crate) const RESOURCE_NUMERIC_START_COLUMN: usize = 2;
pub(crate) const BYTE_SCALE: u128 = 1024;
pub(crate) const BYTE_UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
pub(crate) const NARROW_NAME_LIMIT: usize = 32;
pub(crate) const WIDE_NAME_LIMIT: usize = 36;
pub(crate) const WAREHOUSE_NAME_LIMIT: usize = 20;
pub(crate) const ELLIPSIS: &str = "...";
pub(crate) const USD_PLACES: usize = 4;
pub(crate) const CREDIT_PLACES: usize = 6;
pub(crate) const SECONDS_PLACES: usize = 2;
pub(crate) const RATE_PLACES: usize = 2;
pub(crate) const TOTAL_LABEL: &str = "TOTAL";
pub(crate) const COLUMN_GAP: &str = "  ";
pub(crate) const HISTORY_HEADINGS: [&str; 6] = [
    "Completed (UTC)",
    "Run",
    "Build",
    "Cost",
    "Attributed credits",
    "Est. USD",
];
pub(crate) const NARROW_HEADINGS: [&str; 3] = ["Model", "Attributed credits", "Est. cost"];
pub(crate) const WIDE_HEADINGS: [&str; 6] = [
    "Model",
    "Warehouse",
    "Busy share",
    "Scanned",
    "Attributed credits",
    "Est. cost",
];
pub(crate) const RATE_ADVICE: &str = "Configure cost.usd_per_credit with your Snowflake contract rate, or let the connection's role read SNOWFLAKE.ORGANIZATION_USAGE.RATE_SHEET_DAILY.";
pub(crate) const ESTIMATE_NOTE: &str = "Estimate uses fair-share visible warehouse busy time; it is not Snowflake-billed credits or invoice reconciliation.";
pub(crate) const MICROS_PER_SECOND: i128 = 1_000_000;
pub(crate) const SECONDS_PER_DAY: i128 = 86_400;
pub(crate) const SECONDS_PER_HOUR: i128 = 3_600;
pub(crate) const SECONDS_PER_MINUTE: i128 = 60;
pub(crate) const MAX_HOUR: u32 = 23;
pub(crate) const MAX_MINUTE: u32 = 59;
pub(crate) const MAX_MONTH: u32 = 12;
pub(crate) const MICROSECOND_DIGITS: usize = 6;
pub(crate) const DAYS_PER_ERA: i64 = 146_097;
pub(crate) const YEARS_PER_ERA: i64 = 400;
pub(crate) const UNIX_EPOCH_DAYS_FROM_ERA_START: i64 = 719_468;
/// January and February count as the end of the previous year in the civil-day algorithm.
pub(crate) const LAST_SHIFTED_MONTH: u32 = 2;
/// The shifted month index of January (March is 0).
pub(crate) const SHIFTED_JANUARY: i64 = 10;
