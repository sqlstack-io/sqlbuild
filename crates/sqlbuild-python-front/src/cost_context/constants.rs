//! Constants of cost context.

/// The methods an adapter with warehouse cost telemetry implements.
pub(crate) const COST_AWARE_METHODS: [&str; 2] = ["cost_capability", "collect_run_cost"];

pub(crate) const DECIMAL_MODULE: &str = "decimal";
