//! Build environment, vars and invocation id without compiling resources.

use crate::compile_attachment::_helpers::effective_config::discovered_vars;
use crate::compile_attachment::main::target_selection::target_selection;
use crate::model_config::main::config::run_id::run_id;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyTuple;

/// Build environment, vars and invocation id without compiling resources.
#[pyfunction]
#[pyo3(signature = (*, discovered_inputs, selected_target = None, cli_vars = None))]
pub fn build_effective_runtime_config<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
    selected_target: Option<Bound<'py, PyAny>>,
    cli_vars: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = discovered_inputs.py();
    let (target_name, target_config) = target_selection(discovered_inputs, selected_target)?;
    let effective_vars: Bound<'py, PyAny> =
        discovered_vars(discovered_inputs, &target_config, cli_vars)?;
    let run_id: Bound<'py, PyAny> = run_id(&py.None().into_bound(py))?;
    PyTuple::new(py, [target_name, effective_vars, run_id])
}
