//! The declared key facts of a model input or compiled model.

use crate::compiled_views::_helpers::model_inputs::model_input_view::model_relation_key_row;
use crate::compiled_views::models::NativeCompiledModel;
use crate::compiled_views::models::NativeModelInput;
use crate::compiled_views::models::RelationKeyRow;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The declared key facts of a model input or compiled model.
pub fn model_key_row(model: &Bound<'_, PyAny>) -> PyResult<RelationKeyRow> {
    let py: Python<'_> = model.py();
    if let Ok(input) = model.cast::<NativeModelInput>() {
        return input.get().read_with_owner(py, |owner, record| {
            model_relation_key_row(owner, record, py)
        });
    }
    if let Ok(compiled) = model.cast::<NativeCompiledModel>() {
        return compiled.get().relation_key_row(py);
    }
    native_field(model, "relation_key_row")?.extract()
}
