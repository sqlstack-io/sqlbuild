//! Register the snapshot functions and classes.

use crate::discovery::_helpers::snapshots::native_classes::NativeDirectorySnapshot;
use crate::discovery::main::snapshots::directory_snapshot_scope::directory_snapshot_scope;
use crate::discovery::models::NativeDirectorySnapshotScope;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the snapshot functions and classes.
pub fn register_snapshot(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeDirectorySnapshot>()?;
    module.add_class::<NativeDirectorySnapshotScope>()?;
    module.add_function(wrap_pyfunction!(directory_snapshot_scope, module)?)?;
    Ok(())
}
