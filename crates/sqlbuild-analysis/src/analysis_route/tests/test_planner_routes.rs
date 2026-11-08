use std::collections::BTreeMap;

use crate::diff_selection::main::raw_query_shape::raw_query_shape;
use crate::planner_fixtures::main::star_keys::star_keys;
use crate::planner_fixtures::models::{FixtureColumn, FixtureStarRelation};
use crate::planner_migrations::main::migration_fingerprint::migration_fingerprint;
use crate::planner_migrations::main::query_shape::query_shape;
use crate::relation_names::main::extract_relation_names::extract_relation_names;
use crate::relation_names::models::RelationName;

/// A ClickHouse query the native parser reads, and one it stops at (a named gap).
const READABLE: &str = "SELECT o.id, o.amount AS total FROM orders AS o";
const UNREADABLE: &str = "SELECT o.id FROM orders AS o INTO OUTFILE 'x.csv'";

#[test]
fn given_clickhouse_sql_when_proving_a_column_migration_then_only_sql_read_whole_has_a_shape() {
    let shape = query_shape(READABLE, "clickhouse")
        .expect("no internal failure")
        .expect("readable SQL has a shape");
    let names: Vec<&str> = shape.projections.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["id", "total"]);
    assert!(
        query_shape(UNREADABLE, "clickhouse")
            .expect("no internal failure")
            .is_none()
    );
}

#[test]
fn given_clickhouse_sql_when_fingerprinting_a_migration_then_formatting_is_ignored_and_gaps_have_none()
 {
    let identities: BTreeMap<String, String> = BTreeMap::new();
    let fingerprint = |sql: &str| migration_fingerprint(sql, "", &identities, "clickhouse");
    let base = fingerprint(READABLE).expect("no internal failure");
    assert!(base.is_some());
    assert_eq!(
        fingerprint("SELECT o.id,\n  o.amount AS total -- note\nFROM orders AS o").expect("ok"),
        base
    );
    assert_ne!(
        fingerprint("SELECT o.id, o.amount AS sum FROM orders AS o").expect("ok"),
        base
    );
    assert_eq!(fingerprint(UNREADABLE).expect("ok"), None);
}

#[test]
fn given_a_clickhouse_diff_query_when_reading_its_shape_then_unreadable_sql_is_refused() {
    let shape = raw_query_shape(READABLE, "clickhouse").expect("readable");
    assert_eq!(shape.root_key, "select");
    assert!(!shape.writes_or_locks);
    assert!(raw_query_shape(UNREADABLE, "clickhouse").is_err());
}

#[test]
fn given_clickhouse_sql_when_extracting_relation_names_then_ctes_are_skipped_and_gaps_have_none() {
    let extracted = extract_relation_names(
        "WITH c AS (SELECT 1 AS id) SELECT * FROM shop.orders AS o JOIN c ON o.id = c.id",
        Some("clickhouse"),
    )
    .expect("no failure")
    .expect("readable");
    assert_eq!(
        extracted.relations,
        [RelationName {
            name: "orders".to_owned(),
            schema: Some("shop".to_owned()),
            database: None,
        }]
    );
    assert_eq!(
        extract_relation_names(UNREADABLE, Some("clickhouse")).expect("no failure"),
        None
    );
}

#[test]
fn given_a_clickhouse_star_when_selecting_fixture_dependencies_then_unsettled_stars_take_every_relation()
 {
    let relation = |name: &str| FixtureStarRelation {
        key: ("model".to_owned(), name.to_owned()),
        columns: vec![FixtureColumn {
            name: "id".to_owned(),
            folded_name: "id".to_owned(),
            sql_type: Some("Int32".to_owned()),
            nullable: Some(false),
        }],
        authoritative: true,
    };
    let relations = [relation("orders"), relation("customers")];
    let keys = |sql: &str| star_keys(sql, &relations, Some("clickhouse"), None);
    let orders = vec![("model".to_owned(), "orders".to_owned())];
    let every: Vec<(String, String)> = relations.iter().map(|r| r.key.clone()).collect();
    let joined = "FROM orders AS o JOIN customers AS c ON o.id = c.id";
    assert_eq!(keys(&format!("SELECT o.* {joined}")), orders);
    assert_eq!(keys(&format!("SELECT count(*) AS n {joined}")), Vec::new());
    assert_eq!(keys(&format!("SELECT * {joined}")), every);
    assert_eq!(keys(&format!("SELECT zz.* {joined}")), every);
    assert_eq!(
        keys(&format!("SELECT o.* {joined} INTO OUTFILE 'x.csv'")),
        every
    );
}
