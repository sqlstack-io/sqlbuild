//! A new password is checked against the length limits, then hashed before any write transaction opens.

use crate::credentials::_helpers::passwords::hash_password;
use crate::credentials::constants::{MAX_PASSWORD_BYTES, MIN_PASSWORD_CHARACTERS};
use crate::store::errors::ControlError;

/// The hash of `password`, refused when it is shorter than the minimum or longer than the byte limit.
pub(crate) async fn hash_new_password(password: &str) -> Result<String, ControlError> {
    if password.len() > MAX_PASSWORD_BYTES || password.chars().count() < MIN_PASSWORD_CHARACTERS {
        return Err(ControlError::Invalid(format!(
            "password must contain at least {MIN_PASSWORD_CHARACTERS} characters and at most {MAX_PASSWORD_BYTES} UTF-8 bytes"
        )));
    }
    hash_password(password.into()).await
}
