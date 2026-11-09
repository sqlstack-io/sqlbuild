//! Whitespace-, comment- and keyword-case-insensitive SQL change fingerprints.

use sqlquack_fingerprint::FingerprintDialect;

/// SQLQuack's fingerprint of `sql` under `dialect_name`; dialects SQLQuack does not tokenize read generic tokens.
pub fn query_fingerprint(sql: &str, dialect_name: &str) -> Result<String, String> {
    let dialect =
        FingerprintDialect::from_name(dialect_name).unwrap_or(FingerprintDialect::Generic);
    sqlquack_fingerprint::query_fingerprint(sql, dialect).map_err(|error| error.to_string())
}
