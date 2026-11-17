//! Public constants of build progress.

use crate::build_progress::_helpers::getters::read_build_progress_callbacks;
use crate::build_progress::_helpers::getters::read_transient_line_coordinator;
use crate::build_progress::models::BuildProgressCallbacks;
use sqlbuild_python_base::build_progress::models::TransientLineCoordinator;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<BuildProgressCallbacks>,
        read: read_build_progress_callbacks,
    },
    GetterEntry {
        class: class_of::<TransientLineCoordinator>,
        read: read_transient_line_coordinator,
    },
];
