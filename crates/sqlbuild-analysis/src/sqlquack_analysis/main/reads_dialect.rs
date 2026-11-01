//! `reads_dialect`: whether SQLQuack analyses a dialect's models.

use crate::sqlquack_analysis::_helpers::schema::sqlquack_dialect;

/// Whether SQLQuack reads models in the analysis dialect `dialect`.
pub fn reads_dialect(dialect: &str) -> bool {
    sqlquack_dialect(dialect, false).is_some()
}
