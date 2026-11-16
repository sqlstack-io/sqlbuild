//! Each reference of `model_input` as `(kind, name, package, call argument count)`.

use crate::compiled_views::_helpers::model_inputs::model_input_view::reference_row_of;
use crate::compiled_views::models::NativeModelInput;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_project::models::ReferenceRow;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Each reference of `model_input` as `(kind, name, package, call argument count)`.
pub fn input_reference_rows(model_input: &Bound<'_, PyAny>) -> PyResult<Vec<ReferenceRow>> {
    let py: Python<'_> = model_input.py();
    if let Ok(native) = model_input.cast::<NativeModelInput>() {
        return native
            .get()
            .read(py, |record, _| Ok(record.references.clone()));
    }
    native_field(model_input, "references")?
        .try_iter()?
        .map(|reference| reference_row_of(&reference?))
        .collect()
}
