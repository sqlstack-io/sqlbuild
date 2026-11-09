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
/// A key may be glued to a prefix, as in `db_password` or `PGPASSWORD`; `pwd` stands alone so `OLDPWD` and `cwd` keep their paths.
pub(crate) const SECRET_VALUE_PATTERN: &str = concat!(
    r"(?i)\b([a-z0-9_.-]*?(?:api[-_]?key|access[-_]?key|private[-_]?key|authorization|",
    r"client[-_]?secret|refresh[-_]?token|",
    r"password|passwd|secret|token|dsn|connection_string)|pwd)",
    r#"(\s*[=:]\s*)("[^"\r\n]{0,512}"|'[^'\r\n]{0,512}'|[^\s,;]{1,512})"#,
);
pub(crate) const AUTHORIZATION_PATTERN: &str = concat!(
    r"(?i)\b(authorization)(\s*[=:]\s*)(?:basic|bearer)\s+",
    r#"("[^"\r\n]{1,512}"|'[^'\r\n]{1,512}'|[^\s,;]{1,512})"#,
);
pub(crate) const BEARER_PATTERN: &str = r"(?i)\bbearer\s+[a-z0-9._~+/=-]{1,512}";
/// The password runs to the last `@` before the host, so an unencoded `@` inside it is covered.
pub(crate) const URI_CREDENTIAL_PATTERN: &str = concat!(
    r"(?i)\b([a-z][a-z0-9+.-]*://[^\s:/@]+:)([^\s]+?)",
    r#"(@[^\s@/?#,;'"]*)([\s/?#,;'"]|$)"#,
);
/// SQL that sets a password: `PASSWORD '…'` (quoted only, so prose such as "password authentication" stays) and `IDENTIFIED BY …`.
pub(crate) const DDL_PASSWORD_PATTERN: &str = concat!(
    r#"(?i)\b(password)(\s+)("[^"\r\n]{0,512}"|'[^'\r\n]{0,512}')|"#,
    r#"\b(identified\s+by)(\s+)("[^"\r\n]{0,512}"|'[^'\r\n]{0,512}'|[^\s,;]{1,512})"#,
);
pub(crate) const DDL_REPLACEMENT: &str = "${1}${2}${4}${5}[REDACTED]";
pub(crate) const PAIR_REPLACEMENT: &str = "${1}${2}[REDACTED]";
pub(crate) const BEARER_REPLACEMENT: &str = "Bearer [REDACTED]";
pub(crate) const URI_REPLACEMENT: &str = "${1}[REDACTED]${3}${4}";
/// Compiled size allowance for the bounded Unicode repetitions above.
pub(crate) const PATTERN_SIZE_LIMIT: usize = 1 << 28;
/// `scheme://name:value` with no `@` after it, as in a cut connection string; group 3 ends the authority.
pub(crate) const BARE_USERINFO_PATTERN: &str =
    r#"(?i)\b([a-z][a-z0-9+.-]*://[^\s:/@'"]+:)([^\s/?#@'"]+)([/?#]?)"#;
/// The most characters of error text kept after redaction.
pub(crate) const ERROR_TEXT_CHARS: usize = 2000;
