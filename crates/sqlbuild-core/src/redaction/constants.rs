//! Secret field names and the replacement diagnostic redaction writes.

pub(crate) const SECRET_FIELD_PARTS: [&str; 11] = [
    "access_key",
    "api_key",
    "authorization",
    "client_secret",
    "credential",
    "dsn",
    "password",
    "private_key",
    "refresh_token",
    "secret",
    "token",
];
pub(crate) const SECRET_VALUE_PATTERN: &str = concat!(
    r"(?i)\b(api[-_]?key|access[-_]?key|private[-_]?key|authorization|",
    r"client[-_]?secret|refresh[-_]?token|",
    r"password|passwd|pwd|secret|token|dsn|connection_string)",
    r#"(\s*[=:]\s*)("[^"\r\n]{0,512}"|'[^'\r\n]{0,512}'|[^\s,;]{1,512})"#,
);
pub(crate) const AUTHORIZATION_PATTERN: &str = concat!(
    r"(?i)\b(authorization)(\s*[=:]\s*)(?:basic|bearer)\s+",
    r#"("[^"\r\n]{1,512}"|'[^'\r\n]{1,512}'|[^\s,;]{1,512})"#,
);
pub(crate) const BEARER_PATTERN: &str = r"(?i)\bbearer\s+[a-z0-9._~+/=-]{1,512}";
pub(crate) const URI_CREDENTIAL_PATTERN: &str =
    r"(?i)\b([a-z][a-z0-9+.-]*://[^\s:/@]+:)([^\s@]+)(@)";
pub(crate) const PAIR_REPLACEMENT: &str = "${1}${2}[REDACTED]";
pub(crate) const BEARER_REPLACEMENT: &str = "Bearer [REDACTED]";
pub(crate) const URI_REPLACEMENT: &str = "${1}[REDACTED]${3}";
/// Compiled size allowance for the bounded Unicode repetitions above.
pub(crate) const PATTERN_SIZE_LIMIT: usize = 1 << 28;
/// `scheme://user:secret` with no `@` after it, as when a warehouse cuts a quoted connection string short.
pub(crate) const BARE_USERINFO_PATTERN: &str =
    r#"(?i)\b([a-z][a-z0-9+.-]*://[^\s:/@'"]+:)([^\s/@'"]+)"#;
/// The most characters of error text kept after redaction.
pub(crate) const ERROR_TEXT_CHARS: usize = 2000;
