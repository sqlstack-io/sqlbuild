//! Shared reads for imported project Python discovery: paths, shim calls and records.

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyTuple, PyTupleMethods};
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;

use sqlbuild_host::entries::models::UserCodeHost;

use crate::discovery::_helpers::project::locations::project_relative_path;
use crate::discovery::_helpers::python_files::python_module_files::discover_python_module_files;
use pyo3::IntoPyObjectExt;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::event_exporting::main::current_scope::current_scope;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

/// One project directory under discovery.
pub(crate) struct ProjectDir<'py> {
    pub(crate) path: Bound<'py, PyAny>,
}

impl<'py> ProjectDir<'py> {
    pub(crate) fn py(&self) -> Python<'py> {
        self.path.py()
    }

    /// `project_dir / name`.
    pub(crate) fn child(&self, name: &str) -> PyResult<Bound<'py, PyAny>> {
        self.path.div(name)
    }

    /// `project_relative_path(path=file_path, project_dir=project_dir)`.
    pub(crate) fn relative(&self, file_path: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        project_relative_path(&file_path.clone(), &self.path.clone())
            .and_then(|value| value.into_bound_py_any(self.py()))
    }

    /// `str(file_path.relative_to(project_dir))`.
    pub(crate) fn relative_to(&self, file_path: &Bound<'py, PyAny>) -> PyResult<String> {
        file_path
            .call_method1("relative_to", (&self.path,))?
            .str()?
            .extract()
    }

    /// Python files under `root`, in discovery order.
    pub(crate) fn module_files(
        &self,
        root: &str,
        public_only: bool,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        Ok(discover_python_module_files(&self.path, root, public_only)?
            .iter()
            .collect())
    }

    /// `shim.<loader>(file_path=file_path, project_dir=project_dir)`.
    pub(crate) fn load(
        &self,
        loader: UserCodeHost,
        file_path: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        call_with(
            &shim(self.py(), loader)?,
            &[
                ("file_path", file_path.clone()),
                ("project_dir", self.path.clone()),
            ],
        )
    }

    /// The cached providers, lifecycle sinks and output sinks of a warm command, if any.
    pub(crate) fn cached_extensions(&self) -> PyResult<Option<Bound<'py, PyAny>>> {
        let scope: Bound<'py, PyAny> = current_scope(self.py())?;
        if scope.is_none() {
            return Ok(None);
        }
        let cached: Bound<'py, PyAny> = call_with(
            &native_field(&scope, "cached_extensions")?,
            &[("project_dir", self.path.clone())],
        )?;
        Ok((!cached.is_none()).then_some(cached))
    }
}

/// `project_modules.<name>`.
pub(crate) fn shim<'py>(py: Python<'py>, name: UserCodeHost) -> PyResult<Bound<'py, PyAny>> {
    host_entry(py, name)
}

/// Functions defined in one loaded module.
pub(crate) fn module_functions<'py>(
    module: &Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    shim(module.py(), UserCodeHost::ModuleFunctions)?
        .call1((module,))?
        .try_iter()?
        .collect()
}

/// `sqlbuild._native.<class_name>(**pairs)`.
pub(crate) fn discovered<'py>(
    py: Python<'py>,
    class_name: RecordId,
    pairs: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(py, class_name, pairs)
}

/// `sqlbuild._native.<class_name>(message)`.
pub(crate) fn discovery_error(py: Python<'_>, class_name: ErrorId, message: String) -> PyErr {
    new_plain_error(py, class_name, &message)
}

/// `discovery_error` raised from `cause`.
pub(crate) fn discovery_error_from(
    py: Python<'_>,
    (class_name, message): (ErrorId, String),
    cause: PyErr,
) -> PyErr {
    let error: PyErr = discovery_error(py, class_name, message);
    error.set_cause(py, Some(cause));
    error
}

/// Whether `error` is an `Exception`, which discovery wraps; other exceptions propagate.
pub(crate) fn is_exception(py: Python<'_>, error: &PyErr) -> bool {
    error.is_instance_of::<PyException>(py)
}

/// A tuple of the given items.
pub(crate) fn tuple_of<'py>(
    py: Python<'py>,
    items: Vec<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyTuple::new(py, items)?.into_any())
}

/// `name` of the read definition, or `None` when the function carries no such definition.
pub(crate) fn definition<'py>(
    function: &Bound<'py, PyAny>,
    reader: UserCodeHost,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let read: Bound<'py, PyAny> = host_entry(function.py(), reader)?.call1((function,))?;
    Ok((!read.is_none()).then_some(read))
}
