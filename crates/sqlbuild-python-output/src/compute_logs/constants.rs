//! Public constants of compute logs.

use crate::compute_logs::_helpers::getters::read_local_filesystem_compute_log_storage;
use crate::compute_logs::_helpers::storage::LocalFilesystemComputeLogStorage;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[GetterEntry {
    class: class_of::<LocalFilesystemComputeLogStorage>,
    read: read_local_filesystem_compute_log_storage,
}];
