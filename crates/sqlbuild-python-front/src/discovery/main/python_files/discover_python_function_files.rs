//! Discover the `@udf(...)` Python function files under `functions/python/`.

use crate::discovery::_helpers::failures::raised::stage_error;
use crate::discovery::_helpers::python_files::python_function_files::function_record;
use crate::discovery::_helpers::snapshots::native_classes::NativeProjectTree;
use crate::discovery::_helpers::snapshots::snapshot::{in_snapshot, project_root, project_tree};
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyTuple;
use sqlbuild_discovery::declaration_files::main::python_function_files::python_function_files;
use sqlbuild_discovery::models::{DiscoveredFile, ProjectRoot};
use sqlbuild_discovery::tree::models::ProjectTree;

/// Discover the `@udf(...)` Python function files under `functions/python/`.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub fn discover_python_function_files<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    in_snapshot(project_dir, || {
        let py: Python<'py> = project_dir.py();
        let tree: Bound<'py, NativeProjectTree> = project_tree(project_dir)?;
        let root: ProjectRoot = project_root(project_dir)?;
        let inner: &ProjectTree = &tree.get().inner;
        let files: Vec<DiscoveredFile<String>> = py
            .detach(|| python_function_files(&root, inner))
            .map_err(|failure| stage_error(inner, project_dir, failure))?;
        let records: Vec<Bound<'py, PyAny>> = files
            .into_iter()
            .map(|file| function_record(project_dir, inner, file))
            .collect::<PyResult<_>>()?;
        PyTuple::new(py, records)
    })
}
