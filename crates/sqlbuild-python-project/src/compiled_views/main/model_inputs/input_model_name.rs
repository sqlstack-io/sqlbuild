//! The name of `model_input`, its file stem; a native input answers from its record.

use crate::compiled_views::models::NativeModelInput;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The name of `model_input`, its file stem; a native input answers from its record.
pub fn input_model_name(model_input: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(native) = model_input.cast::<NativeModelInput>() {
        return native
            .get()
            .read(model_input.py(), |record, _| Ok(record.name.clone()));
    }
    native_field(model_input, "model_file")?
        .getattr("file_path")?
        .getattr("stem")?
        .extract()
}
