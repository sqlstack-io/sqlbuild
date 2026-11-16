//! The running build's identity.

use crate::build_identity::_helpers::installed::INSTALLED_BUILD_IDENTITY;
use crate::build_identity::constants::MISSING_BUILD_IDENTITY;

/// The installed content hash of every native crate; panics when none was installed, as there is no fallback.
#[must_use]
pub fn native_build_identity() -> &'static str {
    match INSTALLED_BUILD_IDENTITY.get() {
        Some(identity) => identity,
        None => panic!("{MISSING_BUILD_IDENTITY}"),
    }
}
