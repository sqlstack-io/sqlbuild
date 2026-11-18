//! Register `_native.HOST_ENTRIES`, the host table the import test checks, and host functions.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::types::PyTuple;
use sqlbuild_host::entries::main::host_entry_labels::host_entry_labels;

use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_planner::live_reads::main::register_live_reads::register_live_reads;
use sqlbuild_python_planner::planner_warehouse::main::reads::register_exception_types::register_exception_types;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add(
        "HOST_ENTRIES",
        PyTuple::new(module.py(), host_entry_labels())?,
    )?;
    register_exception_types(module)?;
    module.add_class::<Dialect>()?;
    register_live_reads(module)?;
    sqlbuild_python_base::orchestration_reads::main::register_orchestration_declarations::register_orchestration_declarations(module)
}
