//! The run-on column of a result row, after its status.

use crate::run_on::constants::COLUMN_WIDTH;

/// `local` or `warehouse`, padded so the details after it line up.
#[must_use]
pub fn run_on_column(run_on: &str) -> String {
    format!("{run_on:<COLUMN_WIDTH$}")
}
