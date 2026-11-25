use crate::planner_fixtures::models::FixtureColumn;
use crate::planner_fixtures::models::{FixtureReadKey, FixtureReadReference};
use crate::planner_fixtures::models::{FixtureTypeModel, FixtureTypeProjection, FixtureTypeRead};

pub(super) fn type_model() -> FixtureTypeModel {
    FixtureTypeModel {
        key: ("model".into(), "combined".into()),
        projections: vec![FixtureTypeProjection {
            output_column: "id".into(),
            upstream: vec![
                FixtureTypeRead {
                    key: ("source".into(), "orders".into()),
                    column: "order_id".into(),
                },
                FixtureTypeRead {
                    key: ("source".into(), "customers".into()),
                    column: "customer_id".into(),
                },
            ],
        }],
    }
}

pub(super) fn column(name: &str, sql_type: &str, nullable: bool) -> FixtureColumn {
    FixtureColumn {
        name: name.into(),
        folded_name: name.into(),
        sql_type: Some(sql_type.into()),
        nullable: Some(nullable),
    }
}

/// A read cache key over one `orders` source reference.
pub(super) fn source_read_key(sql: &str) -> FixtureReadKey {
    FixtureReadKey {
        sql: sql.to_owned(),
        references: vec![FixtureReadReference {
            kind: "source".into(),
            name: "orders".into(),
            package: None,
            argument_count: None,
        }],
        dialect: Some("duckdb".into()),
    }
}
