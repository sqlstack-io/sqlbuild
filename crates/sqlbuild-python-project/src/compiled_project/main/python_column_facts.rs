//! The facts of every column spec in `columns`.

use crate::compiled_project::main::python_column_fact::python_column_fact;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::compiled_project::models::SchemaColumnFact;

/// The facts of every column spec in `columns`.
pub fn python_column_facts(
    columns: &Bound<'_, PyAny>,
    model_path: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<SchemaColumnFact>> {
    columns
        .try_iter()?
        .map(|column| python_column_fact(&column?, model_path))
        .collect()
}
