//! The image the next reuse record keeps, taken.

use crate::compile_context::_helpers::delta;
use sqlbuild_cache::project_reuse::types::ImageSections;

/// The image the next reuse record keeps, taken.
#[inline]
pub fn take_pending_image() -> Option<ImageSections> {
    delta::take_pending_image()
}
