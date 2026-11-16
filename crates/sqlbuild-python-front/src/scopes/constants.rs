//! Public constants of scopes.

use crate::scopes::_helpers::index::getters::read_native_scope_index;
use crate::scopes::models::NativeScopeIndex;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The folder a scope path is relative to when its owner names none.
pub const CURRENT_FOLDER: &str = ".";

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[GetterEntry {
    class: class_of::<NativeScopeIndex>,
    read: read_native_scope_index,
}];
