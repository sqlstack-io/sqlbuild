//! Install the running build's identity.

use crate::build_identity::_helpers::installed::INSTALLED_BUILD_IDENTITY;
use crate::build_identity::errors::BuildIdentityError;

/// Install the running build's non-empty identity once per process; installing the same one again succeeds.
pub fn install_build_identity(identity: &str) -> Result<(), BuildIdentityError> {
    if identity.is_empty() {
        return Err(BuildIdentityError::Empty);
    }
    let installed: &String = INSTALLED_BUILD_IDENTITY.get_or_init(|| identity.to_owned());
    if installed != identity {
        return Err(BuildIdentityError::Conflicting {
            installed: installed.clone(),
            offered: identity.to_owned(),
        });
    }
    Ok(())
}
