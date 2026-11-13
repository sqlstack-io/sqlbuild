//! Shared HTTP transport bounds for the server and native clients.

pub const BODY_BYTES: usize = 64 * 1024 * 1024;
/// A gzip body may decompress to this many bytes; a parsed snapshot holds about 7.6 times its JSON size.
pub(crate) const DECOMPRESSED_BODY_BYTES: usize = 320 * 1024 * 1024;
pub(crate) const GZIP_ENCODING: &str = "gzip";
pub(crate) const IDENTITY_ENCODING: &str = "identity";
pub(crate) const JSON_MEDIA_TYPE: &str = "application/json";
/// Two 1024-byte passwords can each expand sixfold through JSON escaping.
pub(crate) const PASSWORD_BODY_BYTES: usize = 16 * 1024;
pub(crate) const PASSWORD_COOKIE: &str = "__Host-sqb-session";
pub(crate) const PASSWORD_CSRF_COOKIE: &str = "__Host-sqb-csrf";
pub(crate) const PASSWORD_CSRF_HEADER: &str = "x-sqb-csrf";
pub(crate) const PROXY_USER_HEADER: &str = "x-sqb-user";
pub(crate) const OIDC_FLOW_COOKIE: &str = "__Host-sqb-oidc";
pub(crate) const OIDC_RESPONSE_BYTES: usize = 1024 * 1024;
#[cfg(unix)]
pub(crate) const DOCKER_IMAGE_HEX_LENGTH: usize = 64;
pub(crate) const HTTPS_SCHEME: &str = "https";
pub(crate) const HTTP_SCHEME: &str = "http";
pub(crate) const LOOPBACK_HOST: &str = "localhost";
pub(crate) const ROOT_PATH: &str = "/";
pub(crate) const UI_INDEX_PATH: &str = "/index.html";
pub(crate) const UI_IMMUTABLE_PREFIX: &str = "/_app/immutable/";
/// The page's own meta policy carries every other directive; a meta tag cannot set frame-ancestors.
pub(crate) const UI_FRAME_POLICY: &str = "frame-ancestors 'none'";
pub(crate) const API_PATH: &str = "/api";
pub(crate) const API_PREFIX: &str = "/api/";
pub(crate) const AUTH_PREFIX: &str = "/auth/";
/// Percent-encoded dot, slash and backslash: a UI path carrying one is refused, never decoded.
pub(crate) const UI_REFUSED_ENCODINGS: [&str; 3] = ["%2e", "%2f", "%5c"];
pub(crate) const UI_DOT_SEGMENTS: [&str; 2] = [".", ".."];
pub(crate) const UI_DOUBLE_SLASH: &str = "//";
