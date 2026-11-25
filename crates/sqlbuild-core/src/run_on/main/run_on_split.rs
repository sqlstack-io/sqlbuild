//! The summary suffix that splits a run's items between DuckDB and the warehouse.

/// `36 local, 4 warehouse`.
#[must_use]
pub fn run_on_split(local: usize, warehouse: usize) -> String {
    format!("{local} local, {warehouse} warehouse")
}
