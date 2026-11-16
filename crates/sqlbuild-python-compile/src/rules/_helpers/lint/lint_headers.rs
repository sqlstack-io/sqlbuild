//! The path-derived role of one lint input.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyBool;
use sqlbuild_rules::lint_text::main::named_declaration_role_kind::named_declaration_role_kind;

use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const HOOKS_FOLDER: &str = "hooks";

/// Classify a lint path, retaining the caller's supplied relative path when available.
#[pyfunction]
#[pyo3(signature = (*, file_path, project_dir, relative_path=None))]
pub(crate) fn lint_file_role<'py>(
    file_path: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
    relative_path: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = file_path.py();
    let relative: Bound<'py, PyAny> = match relative_path {
        Some(relative) => relative,
        None => {
            if !file_path
                .call_method1("is_relative_to", (project_dir,))?
                .is_truthy()?
            {
                return new_record(py, RecordId::LintFileRole, (), None);
            }
            file_path.call_method1("relative_to", (project_dir,))?
        }
    };
    let parts: Vec<String> = native_field(&relative, "parts")?.extract()?;
    let kind: Bound<'py, PyAny> = match named_declaration_role_kind(&parts) {
        Some(kind) => enum_from_value(py, RecordId::DeclarationKind, kind)?,
        None => py.None().into_bound(py),
    };
    new_record_with(
        py,
        RecordId::LintFileRole,
        &[
            ("declaration_kind", kind),
            ("in_project", PyBool::new(py, true).to_owned().into_any()),
            (
                "in_hook_directory",
                PyBool::new(py, parts.first().is_some_and(|part| part == HOOKS_FOLDER))
                    .to_owned()
                    .into_any(),
            ),
        ],
    )
}
