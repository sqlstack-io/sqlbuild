//! The run-on vocabulary and the text of its report sections.

pub(crate) const AUTO: &str = "auto";
pub(crate) const LOCAL: &str = "local";
pub(crate) const WAREHOUSE: &str = "warehouse";
/// The width of the run-on column: the longer of `local` and `warehouse`.
pub(crate) const COLUMN_WIDTH: usize = 9;
pub(crate) const CANNOT_RUN_LOCALLY_TITLE: &str = "Can't run locally";
pub(crate) const ON_THE_WAREHOUSE_TITLE: &str = "On the warehouse";
pub(crate) const RUNS_ON_LABEL: &str = "runs on";
/// What a local item relied on from the project: SQLQuack allowances and overrides.
pub(crate) const RELIED_ON_LABEL: &str = "relied on";
pub(crate) const TESTS_NOTE: &str = "= tests:";
pub(crate) const NOTE_LABEL: &str = "= note:";
pub(crate) const EXPANDED_LABEL: &str = "= expanded from";
/// The caret line's index in a located block: header, `-->`, gutter, source, caret.
pub(crate) const CARET_LINE: usize = 4;
