//! Install the compile's attachment runs once, at registration.

use crate::compiled_project::_helpers::attachment_inputs::INSTALLED;
use crate::compiled_project::models::ResourceAttachment;

/// Install the compile's attachment runs once, at registration.
pub fn install(attachment: ResourceAttachment) {
    let _ = INSTALLED.get_or_init(|| attachment);
}
