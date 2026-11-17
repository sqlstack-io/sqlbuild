//! Authored locations Python consumers read: YAML entry lines and project-relative paths.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyTuple, PyTupleMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `path.relative_to(project_dir)` without pathlib's per-parent comparison scan.
#[pyfunction]
#[pyo3(signature = (*, path, project_dir))]
pub(crate) fn project_relative_path<'py>(
    path: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let root_parts: Bound<'py, PyTuple> = native_field(project_dir, "parts")?.cast_into()?;
    let path_parts: Bound<'py, PyTuple> = native_field(path, "parts")?.cast_into()?;
    let same_anchor: bool = path.call_method0("is_absolute")?.is_truthy()?
        == project_dir.call_method0("is_absolute")?.is_truthy()?;
    if !same_anchor
        || path_parts.len() < root_parts.len()
        || !path_parts.get_slice(0, root_parts.len()).eq(&root_parts)?
    {
        return path.call_method1("relative_to", (project_dir,));
    }
    path.call_method1(
        "with_segments",
        path_parts.get_slice(root_parts.len(), path_parts.len()),
    )
}
