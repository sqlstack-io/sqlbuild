//! The fact of a hand-built model's `SchemaModelEntry`, or `None` without one.

use crate::compiled_project::_helpers::schema_facts;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::compiled_project::models::ModelSchemaFact;

/// The fact of a hand-built model's `SchemaModelEntry`, or `None` without one.
#[inline]
pub fn entry_schema_fact(
    entry: &Bound<'_, PyAny>,
    model_path: &Bound<'_, PyAny>,
) -> PyResult<Option<ModelSchemaFact>> {
    schema_facts::entry_schema_fact(entry, model_path)
}
