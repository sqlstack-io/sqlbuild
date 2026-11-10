use std::collections::HashMap;

/// Analysis SQL as a native parser reads it, each reference `relations` names spelled as its value.
pub fn normalize_native_renamed(
    sql: &str,
    dialect: &str,
    stub_prefix: &str,
    relations: &HashMap<String, String>,
) -> Result<String, String> {
    crate::semantic_validation::_helpers::normalization::normalize_native_sql(
        sql,
        dialect,
        stub_prefix,
        relations,
    )
}
