//! Shared HTTP transport bounds for the server and native clients.

pub(crate) const BODY_BYTES: usize = 64 * 1024 * 1024;
/// Two 1024-byte passwords can each expand sixfold through JSON escaping.
pub(crate) const PASSWORD_BODY_BYTES: usize = 16 * 1024;
pub(crate) const PASSWORD_COOKIE: &str = "__Host-sqb-session";
pub(crate) const PASSWORD_CSRF_COOKIE: &str = "__Host-sqb-csrf";
pub(crate) const PASSWORD_CSRF_HEADER: &str = "x-sqb-csrf";
pub(crate) const HTTPS_SCHEME: &str = "https";
pub(crate) const ROOT_PATH: &str = "/";
