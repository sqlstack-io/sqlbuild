//! Scoped declarations the test file at `path` uses that resolve differently from `destination`.

use crate::scopes::main::index::build_scope_lookup::build_scope_lookup;
use crate::scopes::main::reports::scope_rebound_usages::scope_rebound_usages;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Scoped declarations the test file at `path` uses that resolve differently from `destination`.
pub fn unit_test_rebindings(
    project: &Bound<'_, PyAny>,
    paths: (&str, &str),
) -> PyResult<Option<Vec<String>>> {
    let py: Python<'_> = project.py();
    let index: Bound<'_, PyAny> = native_field(
        &native_field(&native_field(project, "graph")?, "project")?,
        "scope_index",
    )?;
    let lookup: Bound<'_, PyAny> = build_scope_lookup(py, &index)?.into_bound(py);
    scope_rebound_usages(py, &lookup, paths)
}
