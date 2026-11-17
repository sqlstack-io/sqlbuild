//! The target database and schema that Python nodes use outside any model.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyString;
use sqlbuild_analysis::assembly::target_checks::constants::PRESERVE_TARGET_VALUE;
use sqlbuild_model_config::templates::main::holds_context_placeholder::holds_context_placeholder;
use sqlbuild_python_base::warehouse_access::main::adapter_default_location::adapter_default_location;

/// Python nodes' `(database, schema)`: a `preserve` schema reads `loader_schema`; unusable values the adapter's.
pub(crate) fn model_free_locations<'py>(
    (database, schema): (Bound<'py, PyAny>, Bound<'py, PyAny>),
    loader_schema: Option<&str>,
    adapter: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let kept_schema: Option<&str> =
        loader_schema.filter(|name| !name.is_empty() && *name != PRESERVE_TARGET_VALUE);
    let location = |value: Bound<'py, PyAny>, method: &str, preserved: Option<&str>| match (
        value.extract::<Option<String>>()?.as_deref(),
        preserved,
    ) {
        (Some(PRESERVE_TARGET_VALUE), Some(name)) => Ok(PyString::new(value.py(), name).into_any()),
        (Some(text), _)
            if !text.is_empty()
                && text != PRESERVE_TARGET_VALUE
                && !holds_context_placeholder(text) =>
        {
            Ok(value)
        }
        _ => adapter_default_location(adapter, method),
    };
    Ok((
        location(database, "default_database", None)?,
        location(schema, "default_schema", kept_schema)?,
    ))
}
