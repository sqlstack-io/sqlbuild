//! Public constants of model config.

use crate::model_config::_helpers::config::getters::read_native_config_error;
use crate::model_config::errors::NativeConfigError;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[GetterEntry {
    class: class_of::<NativeConfigError>,
    read: read_native_config_error,
}];
