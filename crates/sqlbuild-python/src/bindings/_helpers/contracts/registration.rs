//! Register the native contract functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::authored_values::sql_values;
use crate::bindings::_helpers::contracts::{
    contract_diagnostics, effective_config, exception_types, spec_resolution,
};
use crate::bindings::_helpers::discovery::project_config;
use sqlbuild_python_base::contracts::main::register_resource_identities::register_resource_identities;
use sqlbuild_python_base::contracts::main::register_state_literals::register_state_literals;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    contract_diagnostics::register(module)?;
    effective_config::register(module)?;
    exception_types::register(module)?;
    project_config::register(module)?;
    register_resource_identities(module)?;
    spec_resolution::register(module)?;
    sql_values::register(module)?;
    register_state_literals(module)
}
