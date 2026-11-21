//! Service account keys: made from 256 random bits, recognised by their prefix, stored only as a hash.

use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::credentials::constants::{
    MAX_SERVICE_KEY_BYTES, MIN_IMPORTED_KEY_CHARACTERS, SERVICE_KEY_CHARACTERS, SERVICE_KEY_PREFIX,
    SERVICE_KEY_SHOWN_CHARACTERS,
};
use crate::store::errors::ControlError;

const KEY_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
/// The largest multiple of the alphabet's size in a byte; higher bytes are drawn again, so no character is likelier.
const UNBIASED_BYTES: u8 = 248;

/// A new key: the prefix and letters and digits drawn uniformly from the operating system's random source.
pub(crate) fn new_service_key() -> Result<String, ControlError> {
    let mut key = String::from(SERVICE_KEY_PREFIX);
    let mut random = [0_u8; 64];
    while key.len() < SERVICE_KEY_PREFIX.len() + SERVICE_KEY_CHARACTERS {
        getrandom::fill(&mut random)
            .map_err(|_| ControlError::Invalid("could not generate a secure key".into()))?;
        for byte in random.iter().filter(|byte| **byte < UNBIASED_BYTES) {
            if key.len() == SERVICE_KEY_PREFIX.len() + SERVICE_KEY_CHARACTERS {
                break;
            }
            key.push(char::from(
                KEY_ALPHABET[usize::from(*byte) % KEY_ALPHABET.len()],
            ));
        }
    }
    Ok(key)
}

/// A key an operator chose: the prefix, then at least the minimum of printable, non-space ASCII.
pub(crate) fn check_imported_key(key: &str) -> Result<(), ControlError> {
    let Some(secret) = key.strip_prefix(SERVICE_KEY_PREFIX) else {
        return Err(ControlError::Invalid(format!(
            "an imported key must start with {SERVICE_KEY_PREFIX}"
        )));
    };
    if secret.len() < MIN_IMPORTED_KEY_CHARACTERS {
        return Err(ControlError::Invalid(format!(
            "an imported key needs at least {MIN_IMPORTED_KEY_CHARACTERS} characters after \
             {SERVICE_KEY_PREFIX}; it has {}",
            secret.chars().count()
        )));
    }
    if key.len() > MAX_SERVICE_KEY_BYTES || !secret.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(ControlError::Invalid(format!(
            "an imported key is printable ASCII without spaces, at most {MAX_SERVICE_KEY_BYTES} \
             characters"
        )));
    }
    Ok(())
}

/// Whether a bearer credential is shaped like a service account key, as opposed to another scheme's token.
pub(crate) fn is_service_key(credential: &str) -> bool {
    credential.starts_with(SERVICE_KEY_PREFIX)
}

/// The non-secret start of a key that identifies it in lists and the audit log.
pub(crate) fn key_prefix(key: &str) -> String {
    key.chars()
        .take(SERVICE_KEY_PREFIX.len() + SERVICE_KEY_SHOWN_CHARACTERS)
        .collect()
}

/// What the store keeps of a key: its SHA-256, enough to recognise it and useless for using it.
pub(crate) fn key_hash(key: &str) -> String {
    sha256_hex(key.as_bytes())
}
