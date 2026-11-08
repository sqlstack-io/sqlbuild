/// Analysis SQL as a native parser reads it, with every stub spelled under `stub_prefix`.
pub fn normalize_native_sql(
    sql: &str,
    dialect: &str,
    stub_prefix: &str,
    line_comment_prefixes: &[&str],
) -> Result<String, String> {
    crate::semantic_validation::_helpers::normalization::normalize_native_sql(
        sql,
        dialect,
        stub_prefix,
        line_comment_prefixes,
    )
}
