//! Whitespace-, comment- and keyword-case-insensitive SQL change fingerprints.

use std::str::FromStr;

use polyglot_sql::{Dialect, DialectType};
use sha2::{Digest, Sha256};

use sqlclean::sql_tokens::main::canonical_tokens::canonical_tokens;

/// Return the SHA-256 hex digest of the canonical token stream of `sql` under `dialect_name`.
pub fn query_fingerprint(sql: &str, dialect_name: &str) -> Result<String, String> {
    let dialect_type = DialectType::from_str(dialect_name)
        .map_err(|_| format!("unknown SQL fingerprint dialect '{dialect_name}'"))?;
    let dialect = Dialect::get(dialect_type);
    let mut digest = Sha256::new();
    let tokens = dialect
        .tokenize(sql)
        .map_err(|error| format!("SQL tokenization failed: {error}"))?;
    for token in canonical_tokens(sql, &tokens, &dialect)? {
        digest.update(token.text.len().to_string().as_bytes());
        digest.update(b":");
        digest.update(token.text.as_bytes());
    }
    Ok(format!("{:x}", digest.finalize()))
}
