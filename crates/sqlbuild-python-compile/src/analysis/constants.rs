//! Public constants of analysis.

use crate::analysis::_helpers::getters::read_binding_catalog;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;
use sqlbuild_python_project::analysis::models::BindingCatalog;

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[GetterEntry {
    class: class_of::<BindingCatalog>,
    read: read_binding_catalog,
}];
