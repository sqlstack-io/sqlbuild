//! Register native run-cost allocation, Snowflake cost collection and statement telemetry.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_front::cost_context::main::register_cost_context::register_cost_context;
use sqlbuild_python_output::cost_telemetry::main::register_allocation::register_allocation;
use sqlbuild_python_output::cost_telemetry::main::register_health::register_health;
use sqlbuild_python_output::cost_telemetry::main::register_ledger::register_ledger;
use sqlbuild_python_output::cost_telemetry::main::register_snowflake::register_snowflake;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_allocation(module)?;
    register_cost_context(module)?;
    register_health(module)?;
    register_ledger(module)?;
    register_snowflake(module)
}
