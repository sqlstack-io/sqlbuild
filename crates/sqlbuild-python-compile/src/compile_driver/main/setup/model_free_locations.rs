//! The target database and schema that Python nodes use outside any model.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::assembly::target_checks::constants::PRESERVE_TARGET_VALUE;
use sqlbuild_model_config::templates::main::holds_context_placeholder::holds_context_placeholder;

/// The database and schema of Python nodes, their results and loader fingerprints, from the
/// target's configured `(database, schema)`; a `preserve` schema reads `loader_schema`, and an
/// unset, empty, `preserve` or model-only value, such as a `${CTX:...}` schema, reads `default`
/// with the adapter method's name.
pub fn model_free_locations<'py>(
    (database, schema): (Bound<'py, PyAny>, Bound<'py, PyAny>),
    loader_schema: Option<&str>,
    default: impl Fn(&str) -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let location =
        |value: Bound<'py, PyAny>, fallback: &str| match value.extract::<Option<String>>()? {
            Some(text) if text == PRESERVE_TARGET_VALUE && fallback == SCHEMA_DEFAULT => {
                match loader_schema.filter(|schema| *schema != PRESERVE_TARGET_VALUE) {
                    Some(schema) => Ok(pyo3::types::PyString::new(value.py(), schema).into_any()),
                    None => default(fallback),
                }
            }
            Some(text)
                if !text.is_empty()
                    && text != PRESERVE_TARGET_VALUE
                    && !holds_context_placeholder(&text) =>
            {
                Ok(value)
            }
            _ => default(fallback),
        };
    Ok((
        location(database, "default_database")?,
        location(schema, SCHEMA_DEFAULT)?,
    ))
}

const SCHEMA_DEFAULT: &str = "default_schema";
