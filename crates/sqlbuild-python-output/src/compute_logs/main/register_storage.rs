//! Register the local filesystem compute log storage class.

use crate::compute_logs::_helpers::storage::LocalFilesystemComputeLogStorage;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the local filesystem compute log storage class.
pub fn register_storage(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<LocalFilesystemComputeLogStorage>()
}
