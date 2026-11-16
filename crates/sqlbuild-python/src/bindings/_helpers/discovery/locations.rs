//! Authored locations Python consumers read: YAML entry lines and project-relative paths.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::types::{PyTuple, PyTupleMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_discovery::yaml_files::main::yaml_entry_lines::yaml_entry_lines;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The 1-based line of the outermost `- name: <name>` entry, if present.
#[pyfunction]
#[pyo3(signature = (*, contents, name))]
pub(crate) fn yaml_entry_line(contents: &str, name: &str) -> Option<usize> {
    yaml_entry_lines(contents)
        .into_iter()
        .find_map(|(entry, line)| (entry == name).then_some(line))
}

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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(yaml_entry_line, module)?)?;
    module.add_function(wrap_pyfunction!(project_relative_path, module)?)?;
    Ok(())
}
