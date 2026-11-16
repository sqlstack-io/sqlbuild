//! Build the effective project connection config without compiling resources.

use crate::compile_attachment::_helpers::effective_config::discovered_vars;
use crate::compile_attachment::_helpers::effective_config::expanded_connection;
use crate::compile_attachment::main::target_selection::target_selection;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Build the effective project connection config without compiling resources.
#[pyfunction]
#[pyo3(signature = (*, discovered_inputs, selected_target = None, cli_vars = None, catalog = false))]
pub fn build_effective_connection_config<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
    selected_target: Option<Bound<'py, PyAny>>,
    cli_vars: Option<Bound<'py, PyAny>>,
    catalog: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let (_, target_config) = target_selection(discovered_inputs, selected_target)?;
    let effective_vars: Bound<'py, PyAny> =
        discovered_vars(discovered_inputs, &target_config, cli_vars)?;
    expanded_connection(
        (
            &native_field(discovered_inputs, "project_config")?,
            &native_field(discovered_inputs, "local_config")?,
        ),
        &target_config,
        &effective_vars,
        catalog,
    )
}
