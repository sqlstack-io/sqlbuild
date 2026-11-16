//! A compiled model's analysis outcomes as Rust data: views read their owner, records their fields.

use pyo3::prelude::PyResult;

use sqlbuild_project::types::InferredRow;

pub(crate) const TRANSFORM_VALUES: [&str; 6] = [
    "direct",
    "cast",
    "expression",
    "aggregation",
    "star",
    "constant",
];
pub(crate) const CONFIDENCE_VALUES: [&str; 3] = ["unknown", "high", "medium"];

pub(crate) fn column_names(rows: Vec<InferredRow>) -> Vec<String> {
    rows.into_iter().map(|(name, _, _)| name).collect()
}

pub(crate) fn coded(values: &[&str], code: u8) -> PyResult<String> {
    values
        .get(usize::from(code))
        .map(|value| (*value).to_owned())
        .ok_or_else(|| pyo3::exceptions::PyIndexError::new_err("tuple index out of range"))
}
