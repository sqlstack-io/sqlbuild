//! Register native run-cost allocation, Snowflake cost collection and statement telemetry.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::cost_telemetry::{allocation, health, ledger, snowflake};
use sqlbuild_python_front::cost_context::main::register_cost_context::register_cost_context;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    allocation::register(module)?;
    register_cost_context(module)?;
    health::register(module)?;
    ledger::register(module)?;
    snowflake::register(module)
}
