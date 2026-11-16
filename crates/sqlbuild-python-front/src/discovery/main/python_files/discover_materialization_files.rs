//! Custom materialization Python files under materializations/.

use crate::discovery::_helpers::python_files::imported_modules::{
    ProjectDir, discovered, tuple_of,
};
use crate::discovery::main::python_files::provider_by_name::provider_by_name;
use crate::discovery::main::python_files::provider_usages::provider_usages;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_host::entries::models::UserCodeHost;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// Custom materialization Python files under materializations/.
#[pyfunction]
#[pyo3(signature = (*, project_dir, providers = None))]
pub fn discover_materialization_files<'py>(
    project_dir: Bound<'py, PyAny>,
    providers: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let project = ProjectDir { path: project_dir };
    if !project
        .child("materializations")?
        .call_method0("is_dir")?
        .is_truthy()?
    {
        return tuple_of(py, Vec::new());
    }
    let providers: Bound<'py, PyDict> = match providers {
        Some(providers) => provider_by_name(&providers)?,
        None => PyDict::new(py),
    };
    let mut files: Vec<Bound<'py, PyAny>> = Vec::new();
    for file_path in project.module_files("materializations", false)? {
        let materialize: Bound<'py, PyAny> =
            project.load(UserCodeHost::LoadMaterializeFunction, &file_path)?;
        let usages: Bound<'py, PyAny> = if materialize.is_none() {
            tuple_of(py, Vec::new())?
        } else {
            provider_usages(&materialize, &providers)?
        };
        files.push(discovered(
            py,
            RecordId::DiscoveredMaterializationFile,
            &[
                ("file_path", file_path.clone()),
                ("relative_path", project.relative(&file_path)?),
                ("name", native_field(&file_path, "stem")?),
                ("provider_usages", usages),
            ],
        )?);
    }
    tuple_of(py, files)
}
