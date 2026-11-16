//! The destination with adapter defaults filled in; `render` renders its qualified name.

use crate::compiled_views::_helpers::compiled_models::compiled_model_view::DATABASE_KEY;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::SCHEMA_KEY;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_project::models::DestinationFact;

/// The destination with adapter defaults filled in; `render` renders its qualified name.
pub fn defaulted_destination(
    current: &DestinationFact,
    defaults: (Option<String>, Option<String>),
    render: &Bound<'_, PyAny>,
) -> PyResult<DestinationFact> {
    let (default_schema, default_database) = defaults;
    let schema: Option<String> = current.schema.clone().or(default_schema);
    let database: Option<String> = current.database.clone().or(default_database);
    let keywords: Bound<'_, PyDict> = PyDict::new(render.py());
    keywords.set_item(DATABASE_KEY, &database)?;
    keywords.set_item(SCHEMA_KEY, &schema)?;
    keywords.set_item("name", &current.name)?;
    let qualified_name: Option<String> = render.call((), Some(&keywords))?.extract()?;
    Ok(DestinationFact {
        database,
        schema,
        name: current.name.clone(),
        qualified_name,
        logical_schema: current.logical_schema.clone(),
        logical_database: current.logical_database.clone(),
    })
}
