//! Credential secrets use 256 bits of entropy, represented as hexadecimal in bearer strings.

pub(crate) const TOKEN_SECRET_BYTES: usize = 32;
pub(crate) const TOKEN_SECRET_HEX_LENGTH: usize = TOKEN_SECRET_BYTES * 2;

pub(crate) const MIN_PASSWORD_CHARACTERS: usize = 12;
pub(crate) const MAX_PASSWORD_BYTES: usize = 1024;
pub(crate) const PASSWORD_ALGORITHM: &str = "argon2id";
pub(crate) const PASSWORD_KDF_SLOTS: usize = 4;
pub(crate) const PASSWORD_SESSION_SECONDS: i64 = 3600;
pub(crate) const OIDC_FLOW_SECONDS: i64 = 300;
pub(crate) const LOGIN_WINDOW_SECONDS: i64 = 60;
pub(crate) const LOGIN_USER_LIMIT: u64 = 5;
pub(crate) const LOGIN_PEER_LIMIT: u64 = 20;
pub(crate) const LOGIN_USERNAME_BYTES: usize = 256;
/// A noncredential PHC output makes missing-user checks perform the same Argon2id work.
pub(crate) const DUMMY_PASSWORD_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

pub(crate) const DEFAULT_AUDIT_RETENTION_DAYS: u16 = 400;
pub(crate) const AUDIT_RETENTION_BATCH: usize = 100;

/// Service account keys start with this, so people and secret scanners recognise them.
pub(crate) const SERVICE_KEY_PREFIX: &str = "sqbk_";
/// Random letters and digits after the prefix in a key SQLBuild creates: about 238 bits.
pub(crate) const SERVICE_KEY_CHARACTERS: usize = 40;
/// The fewest characters after the prefix that a key chosen with `--key-from-env` may have.
pub(crate) const MIN_IMPORTED_KEY_CHARACTERS: usize = 32;
pub(crate) const MAX_SERVICE_KEY_BYTES: usize = 256;
/// Characters after the prefix shown to identify a key; the rest stays secret.
pub(crate) const SERVICE_KEY_SHOWN_CHARACTERS: usize = 6;
/// The longest a key replaced by a rotation keeps working.
pub(crate) const MAX_KEY_OVERLAP_MINUTES: i64 = 1440;
/// A key's last use is written at most this often, so busy callers don't write on every request.
pub(crate) const KEY_USE_INTERVAL_SECONDS: i64 = 60;
/// A refused key's audit entry is written at most this often per key, so a misconfigured script can't flood the log.
pub(crate) const KEY_REFUSAL_INTERVAL_SECONDS: u64 = 60;
