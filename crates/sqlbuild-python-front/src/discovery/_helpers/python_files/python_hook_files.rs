//! The Python hook files of every hook role, listed natively for the Python module loader.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_discovery::declaration_files::main::python_hook_files::python_hook_files;
use sqlbuild_discovery::declaration_files::models::{
    CollectionFailure, DiscoverySession, FileScope,
};
use sqlbuild_discovery::tree::models::ProjectTree;

use crate::discovery::_helpers::failures::raised::{failure_error, stage_error};
use crate::discovery::_helpers::project::project_files::path_object;
use crate::discovery::_helpers::records::declaration_records::{
    add_scope_fields, discovery_session, path_class,
};
use crate::discovery::_helpers::snapshots::native_classes::{
    NativeDiscoverySession, NativeProjectTree,
};
use crate::discovery::_helpers::snapshots::snapshot::{in_snapshot, project_tree};

/// Each public Python hook file as `file_path`, `relative_path` and its role's scope fields.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub(crate) fn discover_python_hook_files<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    in_snapshot(project_dir, || {
        let py: Python<'py> = project_dir.py();
        let tree: Bound<'py, NativeProjectTree> = project_tree(project_dir)?;
        let session: Bound<'py, NativeDiscoverySession> = discovery_session(project_dir)?;
        let inner: &ProjectTree = &tree.get().inner;
        let discovery: &DiscoverySession = &session.get().inner;
        let listed: Vec<(String, FileScope)> =
            match py.detach(|| python_hook_files(discovery, inner)) {
                Ok(listed) => listed,
                Err(CollectionFailure::Stage(failure)) => {
                    return Err(stage_error(inner, project_dir, failure));
                }
                Err(CollectionFailure::Layout(failure)) => return Err(failure_error(py, failure)),
            };
        let mut files: Vec<Bound<'py, PyDict>> = Vec::with_capacity(listed.len());
        for (relative_path, scope) in listed {
            let relative: Bound<'py, PyAny> =
                path_class(py)?.call1((path_object(py, inner, relative_path)?,))?;
            let fields: Bound<'py, PyDict> = PyDict::new(py);
            fields.set_item("file_path", project_dir.div(&relative)?)?;
            fields.set_item("relative_path", relative)?;
            add_scope_fields(inner, &scope, &fields)?;
            files.push(fields);
        }
        PyTuple::new(py, files)
    })
}
