//! A Python column spec's fact; its location is kept when it differs from `model_path`.

use crate::compiled_project::_helpers::schema_facts;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::compiled_project::models::SchemaColumnFact;

/// A Python column spec's fact; its location is kept when it differs from `model_path`.
#[inline]
pub fn python_column_fact(
    column: &Bound<'_, PyAny>,
    model_path: Option<&Bound<'_, PyAny>>,
) -> PyResult<SchemaColumnFact> {
    schema_facts::python_column_fact(column, model_path)
}
