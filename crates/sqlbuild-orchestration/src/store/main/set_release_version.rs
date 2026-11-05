//! The hosting CLI names its release once, before any control event is recorded.

use crate::store::_helpers::release_version::RELEASE_VERSION;

/// Later calls keep the first version; one process hosts one release.
pub fn set_release_version(version: &'static str) {
    RELEASE_VERSION.get_or_init(|| version);
}
