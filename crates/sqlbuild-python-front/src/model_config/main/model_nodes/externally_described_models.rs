//! The file path of each model whose layered config or model schema gives it a description.

use crate::model_config::_helpers::model_nodes::described_models;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;

/// The file path of each model whose layered config or model schema gives it a description.
#[inline]
pub fn externally_described_models<'py>(
    inputs: Bound<'py, PyAny>,
    model_schemas: Bound<'py, PyDict>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    described_models::externally_described_models(inputs, model_schemas)
}
