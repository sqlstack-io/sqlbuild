//! Hand the next reuse record the image to keep beside it.

use crate::compile_context::_helpers::delta;
use sqlbuild_cache::project_reuse::types::ImageSections;

/// Hand the next reuse record the image to keep beside it.
#[inline]
pub fn set_pending_image(image: Option<ImageSections>) {
    delta::set_pending_image(image)
}
