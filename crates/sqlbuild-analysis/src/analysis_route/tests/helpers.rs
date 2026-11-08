use crate::planner_fixtures::models::{FixtureColumn, FixtureStarRelation};
use crate::relation_names::models::RelationName;

/// A relation written `schema.name`, or `name` without a schema.
pub(super) fn qualified(relation: &RelationName) -> String {
    let parts: Vec<&str> = relation
        .schema
        .iter()
        .map(String::as_str)
        .chain([relation.name.as_str()])
        .collect();
    parts.join(".")
}

/// A fixture relation `name` with one typed column.
pub(super) fn fixture_relation(name: &str) -> FixtureStarRelation {
    FixtureStarRelation {
        key: ("model".to_owned(), name.to_owned()),
        columns: vec![FixtureColumn {
            name: "id".to_owned(),
            folded_name: "id".to_owned(),
            sql_type: Some("Int32".to_owned()),
            nullable: Some(false),
        }],
        authoritative: true,
    }
}
