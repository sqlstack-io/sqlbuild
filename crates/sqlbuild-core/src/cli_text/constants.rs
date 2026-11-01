//! The ANSI styles, glyphs and limits of human CLI output.

pub(crate) const BOLD: &str = "\x1b[1m";
pub(crate) const DIM: &str = "\x1b[2m";
pub(crate) const RED: &str = "\x1b[38;5;167m";
pub(crate) const GREEN: &str = "\x1b[32m";
pub(crate) const YELLOW: &str = "\x1b[33m";
pub(crate) const BLUE: &str = "\x1b[34m";
pub(crate) const ORANGE: &str = "\x1b[38;5;208m";
pub(crate) const RESET: &str = "\x1b[0m";
pub(crate) const SUCCESS_STATUS_LABELS: [&str; 3] = ["OK", "PASS", "SUCCESS"];
pub(crate) const WARNING_STATUS_LABELS: [&str; 2] = ["WARN", "WARNING"];
pub(crate) const ERROR_STATUS_LABELS: [&str; 4] = ["ERROR", "FAIL", "FAILED", "TIMEOUT"];
pub(crate) const SKIPPED_STATUS_LABELS: [&str; 2] = ["SKIP", "SKIPPED"];
pub(crate) const PHASE_OK_GLYPH: &str = "\u{2713}";
pub(crate) const PHASE_FAIL_GLYPH: &str = "\u{2717}";
pub(crate) const TREE_BRANCH_GLYPH: &str = "\u{251c}\u{2500}\u{2500}";
pub(crate) const TREE_LAST_GLYPH: &str = "\u{2514}\u{2500}\u{2500}";
/// The bar a non-last entry's detail rows carry under its connector.
pub(crate) const TREE_CONTINUATION_GLYPH: &str = "\u{2502}   ";
/// The start of a terminal style escape and the character that ends it.
pub(crate) const STYLE_ESCAPE_START: char = '\u{1b}';
pub(crate) const STYLE_ESCAPE_END: char = 'm';
/// The human report lists at most this many models.
pub(crate) const HUMAN_MODEL_LIMIT: usize = 100;
pub(crate) const MIN_MODEL_NAME_WIDTH: usize = 24;
pub(crate) const MAX_MODEL_NAME_WIDTH: usize = 48;
pub(crate) const STATUS_WIDTH: usize = 4;
/// The semantic notice shows at most this many reasons, and this many models per reason.
pub(crate) const NOTICE_DISPLAY_LIMIT: usize = 10;
/// What `fit` puts in place of the cut tail of a long name.
pub(crate) const ELLIPSIS: &str = "...";
pub(crate) const OPEN_SOURCE: &str = "open source: ";
pub(crate) const OPEN_INPUT: &str = "open input: ";
pub(crate) const UNKNOWN_FUNCTION: &str = "unknown function: ";
/// The last line of an enforced strict report.
pub(crate) const STRICT_ENFORCED_FOOTER: &str = "  Strict (settings.strict = true or --strict): each \
    unproven check fails the compile; declare, override or catalog what it needs.";
/// The last line of a report-only strict report.
pub(crate) const STRICT_REPORT_FOOTER: &str = "  Report only (settings.strict = \"report\"): the exit \
    code is unchanged; set strict = true or pass --strict to fail on these.";
pub(crate) const INVALID_UPSTREAM: &str = "depends on invalid model ";
pub(crate) const DEFAULT_ACTION: &str =
    "declare the input shape, then run `sqb compile --no-cache`";
/// Names narrower than this still get a column this wide.
pub(crate) const MIN_NAME_COLUMN_WIDTH: usize = 20;
/// Indent of a section's entry rows before tree conversion.
pub(crate) const ENTRY_ROW_PREFIX: &str = "  ";
/// Indent of an entry's detail rows before tree conversion.
pub(crate) const LEAF_ROW_PREFIX: &str = "    ";
/// Indent of rows nested below a detail row, which keep their text.
pub(crate) const NESTED_ROW_PREFIX: &str = "      ";
pub(crate) const LEAF_KEY_SEPARATOR: &str = ": ";
/// A row whose text starts with one of these after its indent is not a tree row.
pub(crate) const TREE_EXEMPT_CHARS: [char; 2] = [' ', '.'];
pub(crate) const STALE_INPUT_WARNING_TITLE: &str = "Stale inputs detected";
pub(crate) const SOURCE_FRESHNESS_UNKNOWN_WARNING_TITLE: &str = "source freshness unknown";
pub(crate) const DIFF_HEADER_MARKER: &str = "# ";
pub(crate) const EFFECTIVE_BATCH_SIZE_TOKEN: &str = "effective";
pub(crate) const REASON_GROUPS: [(&str, &str); 7] = [
    ("full_refresh", "Full refresh"),
    ("query_changed", "Query changed"),
    ("function_changed", "Function changed"),
    ("config_changed", "Config changed"),
    ("schema_changed", "Schema changed"),
    ("first_run", "First run"),
    ("renamed", "Renamed"),
];
pub(crate) const SKIP_ACTION: &str = "skip";
pub(crate) const CREATE_VIEW_ACTION: &str = "create_view";
pub(crate) const CUSTOM_ACTION: &str = "custom";
pub(crate) const FULL_REFRESH_REASON: &str = "full_refresh";
pub(crate) const FIRST_RUN_REASON: &str = "first_run";
pub(crate) const QUERY_CHANGED_REASON: &str = "query_changed";
pub(crate) const SCHEMA_CHANGED_REASON: &str = "schema_changed";
pub(crate) const CONFIG_CHANGED_REASON: &str = "config_changed";
pub(crate) const NO_CHANGE_REASON: &str = "no_change";
pub(crate) const NORMAL_INCREMENTAL_REASON: &str = "normal_incremental";
pub(crate) const BOUNDED_BACKFILL: &str = "bounded";
pub(crate) const FULL_BACKFILL: &str = "full";
pub(crate) const FORWARD_BACKFILL: &str = "forward";
pub(crate) const COLUMN_ADDED: &str = "column_added";
pub(crate) const COLUMN_REMOVED: &str = "column_removed";
pub(crate) const COLUMN_TYPE_CHANGED: &str = "column_type_changed";
pub(crate) const ERROR_SEVERITY_VALUE: &str = "error";
pub(crate) const INFO_SEVERITY_VALUE: &str = "info";
pub(crate) const CHANGED_IDENTITY: &str = "changed";
pub(crate) const PRE_SQL_INGRESS_PHASE: &str = "pre_sql_ingress";
pub(crate) const READ_SIDE_PHASE: &str = "read_side";
pub(crate) const PYTHON_INGRESS_TITLE: &str = "Python ingress";
pub(crate) const PYTHON_READ_SIDE_TITLE: &str = "Python read-side";
pub(crate) const PLAN_READY_TITLE: &str = "Plan ready";
pub(crate) const FOOTER_SUCCESS_LABELS: [&str; 5] =
    ["PASS", "OK", "SYNC_PASS", "REFRESH_PASS", "UNCHANGED"];
pub(crate) const FOOTER_WARNING_LABELS: [&str; 3] = ["WARN", "TOLERATED", "UNKNOWN"];
pub(crate) const FOOTER_ERROR_LABELS: [&str; 4] = ["FAIL", "ERROR", "SYNC_FAIL", "REFRESH_FAIL"];
pub(crate) const FOOTER_SKIP_LABELS: [&str; 1] = ["SKIP"];
pub(crate) const COUNT_HEADER_OPENING: &str = " (";
pub(crate) const COUNT_HEADER_CLOSING: &str = ")";
pub(crate) const DBT_NO_MATCH_WARNING: &str = "No dbt or SQLBuild resources matched the selection.";
pub(crate) const DBT_RESOURCE_FALLBACK_TYPE: &str = "resource";
/// An inline error wraps to at most this many lines before it is truncated.
pub(crate) const MAX_INLINE_ERROR_LINES: usize = 2;
