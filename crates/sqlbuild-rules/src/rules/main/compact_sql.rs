use sqlbuild_sqltext::sql_scan::models::QuotePolicy;

/// `sql` lowercased without whitespace or comments outside quoted text.
pub(crate) fn compact_sql(sql: &str, policy: QuotePolicy) -> String {
    crate::rules::_helpers::sql_checks::numeric_decisions::compact_sql(sql, policy)
}
