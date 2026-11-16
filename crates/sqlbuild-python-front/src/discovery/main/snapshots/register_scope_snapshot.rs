//! Register the scope snapshot functions and classes.

use crate::discovery::main::snapshots::discover_scope_snapshot::discover_scope_snapshot;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the scope snapshot functions and classes.
pub fn register_scope_snapshot(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_scope_snapshot, module)?)
}
