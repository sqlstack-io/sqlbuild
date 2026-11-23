//! Formatter-owned folders, selector syntax and report layout.

/// The project folders whose SQL files the formatter owns, in collection order.
pub const LINT_DIRECTORY_NAMES: [&str; 2] = [
    crate::constants::SQL_DIRECTORY,
    crate::constants::TESTS_PATH,
];
pub(crate) const PATH_SELECTOR_PREFIX: &str = "path:";
pub(crate) const PATH_SEPARATOR: char = '/';
pub(crate) const PARENT_PATH_SEGMENT: &str = "..";
pub(crate) const SQL_FILE_SUFFIX: &str = ".sql";
/// Characters that make a path selector a graph selector instead.
pub(crate) const PATH_GRAPH_MARKERS: [char; 3] = ['+', '~', ','];
/// Characters that make a model selector need canonical graph resolution.
pub(crate) const GRAPH_SELECTOR_MARKERS: [char; 6] = ['+', ':', '~', '/', '\\', ','];
pub(crate) const UNKNOWN_ROOT_CODE: &str = "S012";
pub(crate) const REPORT_LINE_WIDTH: usize = 100;
pub(crate) const HELP_PREFIX: &str = "  = help: ";
pub(crate) const HELP_CONTINUATION: &str = "          ";
pub(crate) const TAB_CHARACTER: char = '\t';
pub(crate) const SEVERITY_FAULT: &str = "fault";
pub(crate) const SEVERITY_WARNING: &str = "warning";
pub(crate) const FORMAT_PATH_CODE: &str = "C113";
