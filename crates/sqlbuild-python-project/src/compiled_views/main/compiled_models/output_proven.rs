//! Whether the model's dynamic pivot proof proved its output columns.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Whether the model's dynamic pivot proof proved its output columns.
pub fn output_proven(model: &Bound<'_, PyAny>) -> PyResult<bool> {
    if let Some(view) = native_model(model)? {
        return view.output_proven(model.py());
    }
    let proof: Bound<'_, PyAny> = native_field(model, "dynamic_column_contract")?;
    Ok(!proof.is_none() && native_field(&proof, "output_proven")?.is_truthy()?)
}
