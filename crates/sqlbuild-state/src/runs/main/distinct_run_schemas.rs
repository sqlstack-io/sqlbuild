//! Each physical schema a run writes once, however its nodes spell it.

use crate::runs::models::RunSchema;

/// `schemas`, each physical one once: no database means `default_database`; `same_name` folds names.
#[must_use]
pub fn distinct_run_schemas(
    schemas: Vec<RunSchema>,
    default_database: Option<&str>,
    same_name: &dyn Fn(&str, &str) -> bool,
) -> Vec<RunSchema> {
    let same_database = |left: &Option<String>, right: &Option<String>| match (left, right) {
        (Some(left), Some(right)) => same_name(left, right),
        (left, right) => left == right,
    };
    let mut distinct: Vec<RunSchema> = Vec::with_capacity(schemas.len());
    for schema in schemas {
        let resolved = RunSchema {
            database: schema
                .database
                .or_else(|| default_database.map(str::to_owned)),
            schema: schema.schema,
        };
        let seen: bool = distinct.iter().any(|kept| {
            same_database(&kept.database, &resolved.database)
                && same_name(&kept.schema, &resolved.schema)
        });
        if !seen {
            distinct.push(resolved);
        }
    }
    distinct
}
