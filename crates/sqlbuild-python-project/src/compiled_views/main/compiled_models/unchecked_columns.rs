//! Output columns whose types semantic recovery forgot, in any order.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Output columns whose types semantic recovery forgot, in any order.
pub fn unchecked_columns(model: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if let Some(view) = native_model(model)? {
        return Ok(view.unchecked_columns().to_vec());
    }
    native_field(model, "unchecked_output_columns")?
        .try_iter()?
        .map(|column| column?.extract())
        .collect()
}
