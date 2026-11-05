//! Person login bounds and user-owned credential storage names.

pub(super) const SECRET_BYTES: usize = 32;
pub(super) const POLL_SECONDS: u64 = 1;
pub(super) const LOGIN_SECONDS: u64 = 300;
pub(super) const REQUEST_SECONDS: u64 = 15;
pub(super) const KEYRING_SERVICE: &str = "sqlbuild-cli";
