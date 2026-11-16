//! The facts of every column spec in `columns`.

use crate::compiled_project::_helpers::schema_facts;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::compiled_project::models::SchemaColumnFact;

/// The facts of every column spec in `columns`.
#[inline]
pub fn python_column_facts(
    columns: &Bound<'_, PyAny>,
    model_path: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<SchemaColumnFact>> {
    schema_facts::python_column_facts(columns, model_path)
}
