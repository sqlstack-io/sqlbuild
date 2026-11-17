//! Register the health functions and classes.

use crate::cost_telemetry::_helpers::health::CostTelemetryHealth;
use crate::cost_telemetry::_helpers::health::HEALTH_ATTRIBUTE;
use crate::cost_telemetry::_helpers::health::health;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the health functions and classes.
pub fn register_health(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<CostTelemetryHealth>()?;
    module.add(HEALTH_ATTRIBUTE, health(module.py())?)
}
