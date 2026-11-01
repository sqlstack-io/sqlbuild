//! Credential secrets use 256 bits of entropy, represented as hexadecimal in bearer strings.

pub(crate) const TOKEN_SECRET_BYTES: usize = 32;
pub(crate) const TOKEN_SECRET_HEX_LENGTH: usize = TOKEN_SECRET_BYTES * 2;

pub(crate) const MIN_PASSWORD_CHARACTERS: usize = 12;
pub(crate) const MAX_PASSWORD_BYTES: usize = 1024;
pub(crate) const PASSWORD_ALGORITHM: &str = "argon2id";
pub(crate) const PASSWORD_KDF_SLOTS: usize = 4;
pub(crate) const PASSWORD_SESSION_SECONDS: i64 = 3600;
pub(crate) const LOGIN_WINDOW_SECONDS: i64 = 60;
pub(crate) const LOGIN_USER_LIMIT: u64 = 5;
pub(crate) const LOGIN_PEER_LIMIT: u64 = 20;
pub(crate) const LOGIN_USERNAME_BYTES: usize = 256;
/// A noncredential PHC output makes missing-user checks perform the same Argon2id work.
pub(crate) const DUMMY_PASSWORD_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
