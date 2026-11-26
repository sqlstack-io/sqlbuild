//! Labels, widths and limits of test and scenario run output.

pub(crate) const MIDDLE_ELLIPSIS: &str = "...";
pub(crate) const PASS_LABEL: &str = "PASS";
pub(crate) const FAIL_LABEL: &str = "FAIL";
pub(crate) const TIMEOUT_LABEL: &str = "TIMEOUT";
pub(crate) const NOT_RUN_LABEL: &str = "NOT_RUN";
pub(crate) const TOTAL_LABEL: &str = "TOTAL";
/// The width of the label column a failure's fields start with.
pub(crate) const FIELD_LABEL_WIDTH: usize = 10;
/// Where a failure field's text starts: four spaces, then the label column.
pub(crate) const FIELD_INDENT: usize = 4 + FIELD_LABEL_WIDTH;
/// Failure text wraps at no less than this, however narrow the terminal.
pub(crate) const MIN_WRAP_WIDTH: usize = 30;
/// The short list of failed items stops after this many, without `--verbose`.
pub(crate) const FAILED_LIST_CAP: usize = 20;
pub(crate) const VERBOSE_FLAG: &str = "--verbose";
pub(crate) const FAIL_GLYPH: &str = "\u{2717}";
pub(crate) const LAST_GLYPH: &str = "\u{2514}\u{2500}\u{2500}";
pub(crate) const BRANCH_GLYPH: &str = "\u{251c}\u{2500}\u{2500}";
pub(crate) const MIDDLE_DOT: &str = "\u{00b7}";
pub(crate) const SUCCESS_LABEL: &str = "Completed successfully";
pub(crate) const ERRORS_LABEL: &str = "Completed with errors";
/// The live footer is left one column short of the terminal, so a line never wraps.
pub(crate) const FOOTER_MARGIN: usize = 1;
pub(crate) const RUN_ON_TITLE: &str = "Run on";
pub(crate) const ON_THE_WAREHOUSE: &str = "On the warehouse";
pub(crate) const EXPLAIN_HINT: &str = "(--explain for where)";
pub(crate) const SHORT_REASON_WIDTH: usize = 48;
pub(crate) const ENGINE_ERROR_MARK: &str = " Error:";
/// The header and the rollup line name at most this many causes.
pub(crate) const HEADER_CAUSE_LIMIT: usize = 5;
/// What the header says local items run in.
pub(crate) const LOCAL_NOTE: &str = "DuckDB";
pub(crate) const NOTE_PREFIX: &str = "  note: ";
/// The width of the label column of the `Run on` tree.
pub(crate) const HEADER_LABEL_WIDTH: usize = 22;
