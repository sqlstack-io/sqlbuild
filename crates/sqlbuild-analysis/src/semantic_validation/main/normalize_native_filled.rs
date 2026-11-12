/// Native-normalized analysis SQL with placeholder defaults filled and each character's source.
pub fn normalize_native_filled(
    sql: &str,
    dialect: &str,
    stub_prefix: &str,
    placeholders: &[(String, String)],
) -> Result<(String, Vec<usize>), String> {
    crate::semantic_validation::_helpers::normalization::normalize_native_sql_mapped(
        sql,
        dialect,
        stub_prefix,
        placeholders,
    )
}
