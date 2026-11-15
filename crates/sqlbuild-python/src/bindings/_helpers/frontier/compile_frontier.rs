//! The single seam between the compiler stages and the native frontier.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::frontier::stage_capture::write_stage_capture;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::types::RecordId;

/// Produce the frontier object for `until`, capturing it on request.
#[pyfunction]
#[pyo3(signature = (**options))]
fn compile_frontier<'py>(
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

/// The compiler stages native callers produce through the frontier.
#[derive(Clone, Copy)]
pub(crate) enum FrontierStage {
    DiscoveredProjectInputs,
    CompileProjectInputs,
    CompiledProject,
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

/// `stage()` run inside the macro bridge user macros render through.
pub(crate) fn bridged<'py>(stage: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &host_entry(stage.py(), UserCodeHost::RunWithMacroBridge)?,
        &[("stage", stage.clone())],
    )
}

/// The frontier object `produced` for `stage`, captured on request.
pub(crate) fn produced<'py>(
    stage: FrontierStage,
    produced: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    write_stage_capture(&frontier_stage(produced.py(), stage)?, &produced)?;
    Ok(produced)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compile_frontier, module)?)
}
