//! Person login bounds and user-owned credential storage names.

pub(super) const SECRET_BYTES: usize = 32;
pub(super) const POLL_SECONDS: u64 = 1;
pub(super) const LOGIN_SECONDS: u64 = 300;
pub(super) const REQUEST_SECONDS: u64 = 15;
pub(super) const KEYRING_SERVICE: &str = "sqlbuild-cli";
/// Overrides `[control] server` and `[control] location`; an extra trusted CA for the server.
pub(super) const CONTROL_SERVER_ENV: &str = "SQLBUILD_CONTROL_SERVER";
pub(super) const CONTROL_LOCATION_ENV: &str = "SQLBUILD_CONTROL_LOCATION";
pub(super) const CONTROL_CA_CERT_ENV: &str = "SQLBUILD_CONTROL_CA_CERT";
/// A post-build sync request gives up after this long so the build's exit is never held.
pub(super) const BUILD_SYNC_SECONDS: u64 = 5;
