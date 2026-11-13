//! Python's `compute_query_hash`: a query fingerprint under the project's dialect alias.

use sqlbuild_core::fingerprint_identity::main::fingerprint_dialect::fingerprint_dialect;

use crate::sql_tokens::main::query_fingerprint::query_fingerprint;

/// The fingerprint of `sql` under `dialect` (aliased, or generic when empty or absent).
pub fn compiler_query_fingerprint(sql: &str, dialect: Option<&str>) -> Result<String, String> {
    query_fingerprint(sql, fingerprint_dialect(dialect))
}
