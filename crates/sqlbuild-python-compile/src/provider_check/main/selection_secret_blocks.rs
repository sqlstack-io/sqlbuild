//! The providers missing secrets that a selection would run, read before any warehouse connection.

use std::collections::BTreeSet;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyRef, PyResult, Python};
use pyo3::types::PyAnyMethods;
use sqlbuild_python_project::compiled_views::models::NativeProjectView;
use sqlbuild_python_project::graph::models::ProjectSelection;

use crate::provider_check::_helpers::blocks::blocks_for;
use crate::provider_check::_helpers::users::missing_providers;
use crate::provider_check::models::SecretBlock;

/// The providers missing secrets that `select`/`exclude` would run, leaving out the `not_run` node keys, with the nodes they stop.
pub fn selection_secret_blocks(
    (discovered, project): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (select, exclude): (&[String], &[String]),
    not_run: &BTreeSet<String>,
) -> PyResult<Vec<SecretBlock>> {
    let missing = missing_providers(discovered)?;
    if missing.is_empty() {
        return Ok(Vec::new());
    }
    let py: Python<'_> = project.py();
    let view: PyRef<'_, NativeProjectView> = project.extract()?;
    let graph = view.lineage_graph(py)?;
    let selection = ProjectSelection::new(project, discovered)?;
    let selected: BTreeSet<String> = selection
        .run_keys(graph.bind(py).get().graph(), select, exclude)
        .map_err(PyValueError::new_err)?
        .difference(not_run)
        .cloned()
        .collect();
    drop(view);
    blocks_for((discovered, project), (&selection, &selected), missing)
}
