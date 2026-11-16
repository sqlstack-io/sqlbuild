//! The attached schema of `model_input`; a native input answers without building its entry.

use crate::compiled_views::models::AttachedSchema;
use crate::compiled_views::models::NativeModelInput;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::model_config::main::headers::family_specs::family_specs;

/// The attached schema of `model_input`; a native input answers without building its entry.
pub fn attached_schema<'py>(model_input: &Bound<'py, PyAny>) -> PyResult<Option<AttachedSchema>> {
    let py: Python<'py> = model_input.py();
    if let Ok(native) = model_input.cast::<NativeModelInput>() {
        return native.get().read(py, |record, py| {
            let Some(schema) = &record.schema else {
                return Ok(None);
            };
            Ok(Some(AttachedSchema {
                name: record.name.clone(),
                dynamic_columns: schema.host.dynamic_columns.read(py)?,
                type_enforcement: schema.type_enforcement,
            }))
        });
    }
    let entry: Bound<'py, PyAny> = native_field(model_input, "schema_entry")?;
    if entry.is_none() {
        return Ok(None);
    }
    Ok(Some(AttachedSchema {
        name: native_field(&entry, "name")?.extract()?,
        dynamic_columns: family_specs(&native_field(&entry, "dynamic_columns")?)?,
        type_enforcement: native_field(&entry, "type_enforcement")?.is_truthy()?,
    }))
}
