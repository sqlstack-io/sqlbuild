//! Public constants of presentation text.

use crate::presentation_text::_helpers::getters::read_python_cli_style;
use crate::presentation_text::models::PythonCliStyle;
use crate::records::main::registry::class_of::class_of;
use crate::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[GetterEntry {
    class: class_of::<PythonCliStyle>,
    read: read_python_cli_style,
}];
