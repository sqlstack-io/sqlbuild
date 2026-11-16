//! Install this build's identity into the rules engine, first thing in every native entry point.

use sqlbuild_build_identity::identity::main::native_build_identity::native_build_identity;
use sqlbuild_rules::build_identity::errors::BuildIdentityError;
use sqlbuild_rules::build_identity::main::install_build_identity::install_build_identity;

/// Install this build's identity into the rules engine and return it, before anything reads a cache key.
pub fn install_native_build_identity() -> Result<&'static str, BuildIdentityError> {
    let identity: &'static str = native_build_identity();
    install_build_identity(identity)?;
    Ok(identity)
}
