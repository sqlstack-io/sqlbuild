//! The input relations of a dynamic pivot proof that proved the model's outputs, else `None`.

use crate::compiled_views::main::compiled_models::native_model::native_model;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The input relations of a dynamic pivot proof that proved the model's outputs, else `None`.
pub fn proven_inputs(model: &Bound<'_, PyAny>) -> PyResult<Option<Vec<String>>> {
    if let Some(view) = native_model(model)? {
        return view.proven_inputs(model.py());
    }
    let proof: Bound<'_, PyAny> = native_field(model, "dynamic_column_contract")?;
    if proof.is_none() || !native_field(&proof, "output_proven")?.is_truthy()? {
        return Ok(None);
    }
    native_field(&proof, "input_relations")?.extract().map(Some)
}
