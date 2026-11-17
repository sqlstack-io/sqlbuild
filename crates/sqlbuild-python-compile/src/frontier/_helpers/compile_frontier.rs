//! The single seam between the compiler stages and the native frontier.

use crate::frontier::main::bridged::bridged;
use crate::frontier::models::FrontierStage;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;

use crate::frontier::_helpers::stage_capture::write_stage_capture;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::types::RecordId;

/// Produce the frontier object for `until`, capturing it on request.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn compile_frontier<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "compile_frontier",
        options,
        (&["until", "python_stage"], &["native_stage"]),
    )?;
    let until: Bound<'py, PyAny> = arguments.value("until")?;
    let result: Bound<'py, PyAny> = native_frontier(
        &until,
        &arguments.value("python_stage")?,
        &arguments.value("native_stage")?,
    )?;
    write_stage_capture(&until, &result)?;
    Ok(result)
}

/// The frontier object for `until`; stages not yet native run in Python.
fn native_frontier<'py>(
    until: &Bound<'py, PyAny>,
    python_stage: &Bound<'py, PyAny>,
    native_stage: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = until.py();
    if !native_stage.is_none() {
        return native_stage.call0();
    }
    if until.is(&frontier_stage(py, FrontierStage::CompileProjectInputs)?) {
        return bridged(python_stage);
    }
    python_stage.call0()
}

/// The `CompilerStage` member of `stage`.
pub(crate) fn frontier_stage(py: Python<'_>, stage: FrontierStage) -> PyResult<Bound<'_, PyAny>> {
    let member: &str = match stage {
        FrontierStage::DiscoveredProjectInputs => "DISCOVERED_PROJECT_INPUTS",
        FrontierStage::CompileProjectInputs => "COMPILE_PROJECT_INPUTS",
        FrontierStage::CompiledProject => "COMPILED_PROJECT",
    };
    enum_member(py, RecordId::CompilerStage, member)
}
