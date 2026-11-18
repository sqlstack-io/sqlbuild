use crate::runs::models::RunSchema;

/// Names compared as an upper-case-folding warehouse resolves unquoted names.
pub(super) fn folded(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

pub(super) fn run_schemas(pairs: &[(Option<&str>, &str)]) -> Vec<RunSchema> {
    pairs
        .iter()
        .map(|(database, schema)| RunSchema {
            database: database.map(str::to_owned),
            schema: (*schema).to_owned(),
        })
        .collect()
}
