//! Register `_native.HOST_ENTRIES`, the host table the import test checks, and host functions.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::types::PyTuple;
use pyo3::wrap_pyfunction;
use sqlbuild_host::entries::main::host_entry_labels::host_entry_labels;

use crate::bindings::_helpers::native_host::orchestration_declarations::declaration_site;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add(
        "HOST_ENTRIES",
        PyTuple::new(module.py(), host_entry_labels())?,
    )?;
    module.add_function(wrap_pyfunction!(declaration_site, module)?)
}
