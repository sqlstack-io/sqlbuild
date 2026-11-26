//! Column widths of grouped command progress rows.

pub(crate) const LABEL_WIDTH: usize = 10;
pub(crate) const AUDIT_NAME_WIDTH: usize = 50;
pub(crate) const UNATTACHED_GROUP: &str = "(unattached)";
pub(crate) const CLEAR_LINE: &str = "\r\x1b[K";
pub(crate) const PASS_OUTCOME: &str = "pass";
pub(crate) const WARN_OUTCOME: &str = "warn";
pub(crate) const ERROR_OUTCOME: &str = "error";
pub(crate) const STATUS_WIDTH: usize = 6;
pub(crate) const RUN_ON_WIDTH: usize = 9;
/// `  5.45s`: two spaces, then the seconds right-aligned in seven.
pub(crate) const SECONDS_WIDTH: usize = 9;
/// The `N real models` detail shows from this terminal width.
pub(crate) const DETAIL_MIN_COLUMNS: usize = 120;
pub(crate) const MIN_ROW_NAME_WIDTH: usize = 20;
pub(crate) const PASS_STATUS: &str = "PASS";
/// Where a child row's glyph starts, past the counter.
pub(crate) const CHILD_GLYPH_INDENT: usize = 2;
/// From the child glyph to the child name: glyph, space and a ten-wide label.
pub(crate) const CHILD_NAME_OFFSET: usize = 14;
