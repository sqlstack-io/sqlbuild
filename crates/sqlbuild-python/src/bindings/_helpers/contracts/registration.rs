//! Register the native contract functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_base::contracts::main::register_resource_identities::register_resource_identities;
use sqlbuild_python_base::contracts::main::register_state_literals::register_state_literals;
use sqlbuild_python_front::authored_values::main::register_sql_values::register_sql_values;
use sqlbuild_python_front::contracts::main::register_effective_config::register_effective_config;
use sqlbuild_python_front::contracts::main::register_spec_resolution::register_spec_resolution;
use sqlbuild_python_front::discovery::main::project::register_project_config::register_project_config;
use sqlbuild_python_project::contracts::main::register_contract_diagnostics::register_contract_diagnostics;
use sqlbuild_python_project::contracts::main::register_exception_types::register_exception_types;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_contract_diagnostics(module)?;
    register_effective_config(module)?;
    register_exception_types(module)?;
    register_project_config(module)?;
    register_resource_identities(module)?;
    register_spec_resolution(module)?;
    register_sql_values(module)?;
    register_state_literals(module)
}
