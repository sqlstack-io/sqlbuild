//! The project Python module files Python imports, listed natively in Python's sorted order.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyTuple;
use sqlbuild_discovery::declaration_files::main::python_module_files::python_module_files;
use sqlbuild_discovery::tree::models::ProjectTree;

use crate::discovery::_helpers::failures::raised::stage_error;
use crate::discovery::_helpers::project::project_files::path_object;
use crate::discovery::_helpers::snapshots::native_classes::NativeProjectTree;
use crate::discovery::_helpers::snapshots::snapshot::project_tree;

/// `sorted((project_dir / root).rglob("*.py"))` without `__init__` modules, or `_` parts too.
#[pyfunction]
#[pyo3(signature = (*, project_dir, root, public_only))]
pub(crate) fn discover_python_module_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    root: &str,
    public_only: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = project_dir.py();
    let tree: Bound<'py, NativeProjectTree> = project_tree(project_dir)?;
    let inner: &ProjectTree = &tree.get().inner;
    let files: Vec<String> = py
        .detach(|| python_module_files(inner, root, public_only))
        .map_err(|failure| stage_error(inner, project_dir, failure))?;
    let paths: Vec<Bound<'py, PyAny>> = files
        .into_iter()
        .map(|relative_path| project_dir.div(path_object(py, inner, relative_path)?))
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, paths)
}
