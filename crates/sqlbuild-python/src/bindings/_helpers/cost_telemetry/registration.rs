//! Register native run-cost allocation, Snowflake cost collection and statement telemetry.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::cost_telemetry::allocation;
use crate::bindings::_helpers::cost_telemetry::context;
use crate::bindings::_helpers::cost_telemetry::health;
use crate::bindings::_helpers::cost_telemetry::ledger;
use crate::bindings::_helpers::cost_telemetry::snowflake;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    allocation::register(module)?;
    context::register(module)?;
    health::register(module)?;
    ledger::register(module)?;
    snowflake::register(module)
}
