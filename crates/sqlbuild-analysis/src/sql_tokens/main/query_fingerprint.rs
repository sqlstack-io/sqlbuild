//! Whitespace-, comment- and keyword-case-insensitive SQL change fingerprints.

use std::str::FromStr;

use polyglot_sql::{Dialect, DialectType};
use sha2::{Digest, Sha256};

use sqlclean::sql_tokens::main::canonical_tokens::canonical_tokens;

use crate::sql_tokens::_helpers::canonical_tokens::canonical_tokens as sqlquack_canonical_tokens;
use crate::sql_tokens::constants::SQLQUACK_FINGERPRINT_DIALECTS;

/// Return the SHA-256 hex digest of the canonical token stream of `sql` under `dialect_name`.
pub fn query_fingerprint(sql: &str, dialect_name: &str) -> Result<String, String> {
    let texts: Vec<String> = match SQLQUACK_FINGERPRINT_DIALECTS
        .iter()
        .find(|dialect| dialect.name == dialect_name)
    {
        Some(dialect) => sqlquack_canonical_tokens(sql, *dialect)?,
        None => polyglot_canonical_tokens(sql, dialect_name)?,
    };
    let mut digest = Sha256::new();
    for text in texts {
        digest.update(text.len().to_string().as_bytes());
        digest.update(b":");
        digest.update(text.as_bytes());
    }
    Ok(format!("{:x}", digest.finalize()))
}

/// Dialects SQLQuack cannot tokenize yet keep SQLClean's Polyglot token stream.
fn polyglot_canonical_tokens(sql: &str, dialect_name: &str) -> Result<Vec<String>, String> {
    let dialect_type = DialectType::from_str(dialect_name)
        .map_err(|_| format!("unknown SQL fingerprint dialect '{dialect_name}'"))?;
    let dialect = Dialect::get(dialect_type);
    let tokens = dialect
        .tokenize(sql)
        .map_err(|error| format!("SQL tokenization failed: {error}"))?;
    Ok(canonical_tokens(sql, &tokens, &dialect)?
        .into_iter()
        .map(|token| token.text)
        .collect())
}
