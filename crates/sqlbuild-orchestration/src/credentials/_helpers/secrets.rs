//! One encoding and constant-time check for high-entropy bearer secrets.

use std::fmt::Write;

use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::credentials::constants::{TOKEN_SECRET_BYTES, TOKEN_SECRET_HEX_LENGTH};
use crate::store::errors::ControlError;

pub(crate) fn new_bearer(id: Uuid) -> Result<String, ControlError> {
    let mut random = [0_u8; TOKEN_SECRET_BYTES];
    getrandom::fill(&mut random)
        .map_err(|_| ControlError::Invalid("could not generate a secure token".into()))?;
    let mut bearer = format!("{id}.");
    for byte in random {
        write!(bearer, "{byte:02x}")
            .map_err(|_| ControlError::Invalid("could not format a secure token".into()))?;
    }
    Ok(bearer)
}

pub(crate) fn bearer_id(bearer: &str) -> Option<Uuid> {
    let (id, secret) = bearer.split_once('.')?;
    if secret.len() != TOKEN_SECRET_HEX_LENGTH
        || !secret.bytes().all(|value| value.is_ascii_hexdigit())
    {
        return None;
    }
    let Ok(id) = Uuid::parse_str(id) else {
        return None;
    };
    Some(id)
}

pub(crate) fn secret_matches(secret: &str, expected_hash: &str) -> bool {
    bool::from(
        sha256_hex(secret.as_bytes())
            .as_bytes()
            .ct_eq(expected_hash.as_bytes()),
    )
}
