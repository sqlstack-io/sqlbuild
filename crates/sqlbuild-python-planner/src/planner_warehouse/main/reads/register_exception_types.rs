//! Register the warehouse read exception types.

use crate::planner_warehouse::_helpers::synced::exception_types::WarehouseAccessRefusedError;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the warehouse read exception types.
pub fn register_exception_types(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add(
        "WarehouseAccessRefusedError",
        module.py().get_type::<WarehouseAccessRefusedError>(),
    )
}
