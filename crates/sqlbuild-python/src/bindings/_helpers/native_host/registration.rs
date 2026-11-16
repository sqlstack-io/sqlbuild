//! Register `_native.HOST_ENTRIES`, the host table the import test checks, and host functions.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::types::PyTuple;
use sqlbuild_host::entries::main::host_entry_labels::host_entry_labels;

use sqlbuild_python_base::warehouse_access::main::register_synced_copy::register_synced_copy;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add(
        "HOST_ENTRIES",
        PyTuple::new(module.py(), host_entry_labels())?,
    )?;
    register_synced_copy(module)?;
    sqlbuild_python_base::orchestration_reads::main::register_orchestration_declarations::register_orchestration_declarations(module)
}
