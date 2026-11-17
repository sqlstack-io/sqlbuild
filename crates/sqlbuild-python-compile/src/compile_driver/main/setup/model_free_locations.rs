//! The target database and schema that Python nodes use outside any model.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_model_config::templates::main::holds_context_placeholder::holds_context_placeholder;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The compiled project's target database and schema, each from `default` with the adapter method's name when it is unset, empty, or resolves only inside a model, such as a `${CTX:...}` schema.
pub fn model_free_locations<'py>(
    project: &Bound<'py, PyAny>,
    default: impl Fn(&str) -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let location = |field: &str, fallback: &str| {
        let value = native_field(project, field)?;
        match value.extract::<Option<String>>()? {
            Some(text) if !text.is_empty() && !holds_context_placeholder(&text) => Ok(value),
            _ => default(fallback),
        }
    };
    Ok((
        location("effective_target_database", "default_database")?,
        location("effective_target_schema", "default_schema")?,
    ))
}
