//! Public constants of analysis session.

use crate::analysis_session::_helpers::getters::read_native_model_analysis_session;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;
use sqlbuild_python_project::analysis_session::models::NativeModelAnalysisSession;

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[GetterEntry {
    class: class_of::<NativeModelAnalysisSession>,
    read: read_native_model_analysis_session,
}];
