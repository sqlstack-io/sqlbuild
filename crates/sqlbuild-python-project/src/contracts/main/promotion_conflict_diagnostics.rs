//! One K011 error per enforced-contract table model under immediate promotion.

use crate::compiled_project::models::NativeCompiledProject;
use crate::contracts::_helpers::contract_diagnostics::CONTRACT_PHASE;
use crate::contracts::_helpers::contract_diagnostics::PATHLIB_MODULE;
use crate::contracts::_helpers::contract_diagnostics::keyword_call;
use crate::contracts::_helpers::contract_diagnostics::model_resource_type;
use crate::contracts::_helpers::contract_diagnostics::native_project;
use crate::contracts::_helpers::model_contracts::{ConflictRow, project_promotion_conflicts};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyRef, PyResult, Python};
use pyo3::types::PyTuple;
use pyo3::{IntoPyObject, pyfunction};
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// One K011 error per enforced-contract table model under immediate promotion.
#[pyfunction]
#[pyo3(signature = (*, project, adapter_default, settings_file))]
pub fn promotion_conflict_diagnostics<'py>(
    py: Python<'py>,
    project: &Bound<'py, PyAny>,
    adapter_default: &Bound<'py, PyAny>,
    settings_file: String,
) -> PyResult<Bound<'py, PyTuple>> {
    let explicit_mode: Option<String> =
        native_field(&native_field(project, "settings")?, "table_promotion_mode")?.extract()?;
    let native: Bound<'py, PyAny> = native_project(project)?;
    let native: PyRef<'py, NativeCompiledProject> = native.extract()?;
    let conflicts: Vec<ConflictRow> = project_promotion_conflicts(
        py,
        (
            explicit_mode,
            adapter_default.str()?.extract()?,
            settings_file,
        ),
        &native,
    )?;
    let diagnostic_type: Bound<'py, PyAny> = record_class(py, RecordId::CompilerDiagnostic)?;
    let path_type: Bound<'py, PyAny> = py.import(PATHLIB_MODULE)?.getattr("Path")?;
    let mut diagnostics: Vec<Bound<'py, PyAny>> = Vec::with_capacity(conflicts.len());
    for ConflictRow {
        name,
        relative_path,
        code,
        message,
        help,
    } in conflicts
    {
        diagnostics.push(keyword_call(
            &diagnostic_type,
            vec![
                ("phase", CONTRACT_PHASE.into_pyobject(py)?.into_any()),
                ("severity", "error".into_pyobject(py)?.into_any()),
                ("code", code.into_pyobject(py)?.into_any()),
                ("message", message.into_pyobject(py)?.into_any()),
                ("resource_type", model_resource_type(py)?),
                ("resource_name", name.into_pyobject(py)?.into_any()),
                ("path", path_type.call1((relative_path,))?),
                ("help", help.into_pyobject(py)?.into_any()),
            ],
        )?);
    }
    PyTuple::new(py, diagnostics)
}
