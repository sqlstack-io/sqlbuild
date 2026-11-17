//! Register `_native.HOST_ENTRIES`, the host table the import test checks, and host functions.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::types::PyTuple;
use sqlbuild_host::entries::main::host_entry_labels::host_entry_labels;

use crate::bindings::_helpers::native_host::live_reads;
use crate::bindings::_helpers::planner_warehouse::synced_reads::WarehouseAccessRefusedError;
use sqlbuild_python_base::native_host::models::Dialect;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add(
        "HOST_ENTRIES",
        PyTuple::new(module.py(), host_entry_labels())?,
    )?;
    module.add(
        "WarehouseAccessRefusedError",
        module.py().get_type::<WarehouseAccessRefusedError>(),
    )?;
    module.add_class::<Dialect>()?;
    live_reads::register(module)?;
    sqlbuild_python_base::orchestration_reads::main::register_orchestration_declarations::register_orchestration_declarations(module)
}
