//! Install the compile's attachment runs once, at registration.

use crate::compiled_project::_helpers::attachment_inputs;
use crate::compiled_project::models::ResourceAttachment;

/// Install the compile's attachment runs once, at registration.
#[inline]
pub fn install(attachment: ResourceAttachment) {
    attachment_inputs::install(attachment)
}
