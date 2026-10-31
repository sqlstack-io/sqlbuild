//! Credential secrets use 256 bits of entropy, represented as hexadecimal in bearer strings.

pub(crate) const TOKEN_SECRET_BYTES: usize = 32;
pub(crate) const TOKEN_SECRET_HEX_LENGTH: usize = TOKEN_SECRET_BYTES * 2;
