//! Register the snapshot functions and classes.

use crate::project_snapshot::models::NativeOrchestrationSnapshot;
use crate::project_snapshot::models::NativeOrchestrationWorker;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the snapshot functions and classes.
pub fn register_snapshot(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeOrchestrationSnapshot>()?;
    module.add_class::<NativeOrchestrationWorker>()
}
