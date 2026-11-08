use std::collections::BTreeMap;

use crate::analysis_route::tests::helpers::{fixture_relation, qualified};
use crate::analysis_route::tests::test_types::{
    FingerprintTestCase, QueryShapeTestCase, RawShapeTestCase, RelationNamesTestCase,
    StarKeysTestCase,
};
use crate::diff_selection::main::raw_query_shape::raw_query_shape;
use crate::planner_fixtures::main::star_keys::star_keys;
use crate::planner_migrations::main::migration_fingerprint::migration_fingerprint;
use crate::planner_migrations::main::query_shape::query_shape;
use crate::relation_names::main::extract_relation_names::extract_relation_names;

/// A ClickHouse query the native parser reads.
const READABLE: &str = "SELECT o.id, o.amount AS total FROM orders AS o";
/// A ClickHouse query the native parser stops at with a named gap.
const UNREADABLE: &str = "SELECT o.id FROM orders AS o ORDER BY o.id LIMIT 1 WITH TIES";
const DIALECT: &str = "clickhouse";

#[test]
fn given_clickhouse_sql_when_proving_a_column_migration_then_only_sql_read_whole_has_a_shape() {
    let test_cases = [
        QueryShapeTestCase {
            description: "SQL read whole has its projections",
            sql: READABLE,
            expected_projections: Some(&["id", "total"]),
        },
        QueryShapeTestCase {
            description: "SQL with a gap has no shape",
            sql: UNREADABLE,
            expected_projections: None,
        },
    ];
    for test_case in test_cases {
        let shape = query_shape(test_case.sql, DIALECT).expect("no internal failure");
        let projections: Option<Vec<&str>> = shape
            .as_ref()
            .map(|shape| shape.projections.iter().map(|p| p.name.as_str()).collect());
        assert_eq!(
            projections.as_deref(),
            test_case.expected_projections,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_clickhouse_sql_when_fingerprinting_a_migration_then_formatting_is_ignored_and_gaps_have_none()
 {
    let test_cases = [
        FingerprintTestCase {
            description: "the same SQL has the same fingerprint",
            sql: READABLE,
            expected_same: Some(true),
        },
        FingerprintTestCase {
            description: "layout and comments are ignored",
            sql: "SELECT o.id,\n  o.amount AS total -- note\nFROM orders AS o",
            expected_same: Some(true),
        },
        FingerprintTestCase {
            description: "a renamed column changes the fingerprint",
            sql: "SELECT o.id, o.amount AS sum FROM orders AS o",
            expected_same: Some(false),
        },
        FingerprintTestCase {
            description: "SQL with a gap has no fingerprint",
            sql: UNREADABLE,
            expected_same: None,
        },
    ];
    let identities: BTreeMap<String, String> = BTreeMap::new();
    let base = migration_fingerprint(READABLE, "", &identities, DIALECT)
        .expect("no internal failure")
        .expect("readable SQL has a fingerprint");
    for test_case in test_cases {
        let same: Option<bool> = migration_fingerprint(test_case.sql, "", &identities, DIALECT)
            .expect("no internal failure")
            .map(|fingerprint| fingerprint == base);
        assert_eq!(same, test_case.expected_same, "{}", test_case.description);
    }
}

#[test]
fn given_a_clickhouse_diff_query_when_reading_its_shape_then_unreadable_sql_is_refused() {
    let test_cases = [
        RawShapeTestCase {
            description: "a SELECT read whole is a read-only select",
            sql: READABLE,
            expected_shape: Some(("select", false)),
        },
        RawShapeTestCase {
            description: "INTO OUTFILE writes",
            sql: "SELECT o.id FROM orders AS o INTO OUTFILE 'x.csv'",
            expected_shape: Some(("select", true)),
        },
        RawShapeTestCase {
            description: "SQL with a gap is refused",
            sql: UNREADABLE,
            expected_shape: None,
        },
    ];
    for test_case in test_cases {
        let read = raw_query_shape(test_case.sql, DIALECT);
        let shape: Option<(&str, bool)> = read
            .iter()
            .map(|shape| (shape.root_key.as_str(), shape.writes_or_locks))
            .next();
        assert_eq!(shape, test_case.expected_shape, "{}", test_case.description);
    }
}

#[test]
fn given_clickhouse_sql_when_extracting_relation_names_then_they_are_polyglots() {
    let orders: &[&str] = &["analytics.orders"];
    let test_cases = [
        RelationNamesTestCase {
            description: "a CTE is not a relation",
            sql: "WITH c AS (SELECT 1 AS id) SELECT * FROM shop.orders AS o JOIN c ON o.id = c.id",
            expected_names: Some((&["shop.orders"], &[])),
        },
        RelationNamesTestCase {
            description: "SQL with a gap is not read",
            sql: UNREADABLE,
            expected_names: None,
        },
        RelationNamesTestCase {
            description: "TRUNCATE names its table",
            sql: "TRUNCATE TABLE analytics.orders",
            expected_names: Some((orders, &[])),
        },
        RelationNamesTestCase {
            description: "ALTER ... DELETE names its table",
            sql: "ALTER TABLE analytics.orders DELETE WHERE orders_id = 1",
            expected_names: Some((orders, &[])),
        },
        RelationNamesTestCase {
            description: "INTO OUTFILE names the table read",
            sql: "SELECT * FROM analytics.orders INTO OUTFILE 'x.csv' AND STDOUT",
            expected_names: Some((orders, &[])),
        },
        RelationNamesTestCase {
            description: "a temporary table is named and kept as temporary",
            sql: "CREATE TEMPORARY TABLE scratch AS SELECT * FROM analytics.orders",
            expected_names: Some((&["analytics.orders", "scratch"], &["scratch"])),
        },
        RelationNamesTestCase {
            description: "OPTIMIZE is a command naming no relation, as Polyglot reads it",
            sql: "OPTIMIZE TABLE analytics.orders FINAL",
            expected_names: Some((&[], &[])),
        },
        RelationNamesTestCase {
            description: "an ALTER with an UPDATE is a command naming no relation, as Polyglot reads it",
            sql: "ALTER TABLE analytics.orders UPDATE status = 'x' WHERE 1",
            expected_names: Some((&[], &[])),
        },
        RelationNamesTestCase {
            description: "a form the parser still gaps is not read",
            sql: "TRUNCATE TABLE analytics.orders SETTINGS a = 1",
            expected_names: None,
        },
    ];
    for test_case in test_cases {
        let found = extract_relation_names(test_case.sql, Some(DIALECT)).expect("no failure");
        let relations: Option<Vec<String>> = found
            .as_ref()
            .map(|found| found.relations.iter().map(qualified).collect());
        let names: Option<(Vec<&str>, Vec<&str>)> =
            relations
                .as_ref()
                .zip(found.as_ref())
                .map(|(relations, found)| {
                    (
                        relations.iter().map(String::as_str).collect(),
                        found.temporary.iter().map(String::as_str).collect(),
                    )
                });
        assert_eq!(
            names
                .as_ref()
                .map(|(relations, temporary)| (relations.as_slice(), temporary.as_slice())),
            test_case.expected_names,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_clickhouse_star_when_selecting_fixture_dependencies_then_unsettled_stars_take_every_relation()
 {
    let every: &[&str] = &["orders", "customers"];
    let test_cases = [
        StarKeysTestCase {
            description: "a qualified star takes its relation",
            sql: "SELECT o.* FROM orders AS o JOIN customers AS c ON o.id = c.id",
            expected_relations: &["orders"],
        },
        StarKeysTestCase {
            description: "no star takes no relation",
            sql: "SELECT count(*) AS n FROM orders AS o JOIN customers AS c ON o.id = c.id",
            expected_relations: &[],
        },
        StarKeysTestCase {
            description: "a bare star takes every relation in the query",
            sql: "SELECT * FROM orders AS o JOIN customers AS c ON o.id = c.id",
            expected_relations: every,
        },
        StarKeysTestCase {
            description: "an unknown qualifier takes every relation",
            sql: "SELECT zz.* FROM orders AS o JOIN customers AS c ON o.id = c.id",
            expected_relations: every,
        },
        StarKeysTestCase {
            description: "an alias reused across set operation branches takes both relations",
            sql: "SELECT t.* FROM orders t UNION ALL SELECT t.* FROM customers t",
            expected_relations: every,
        },
        StarKeysTestCase {
            description: "an alias reused in a subquery takes both relations",
            sql: "SELECT o.* FROM orders o WHERE o.id IN (SELECT o.id FROM customers o)",
            expected_relations: every,
        },
        StarKeysTestCase {
            description: "an alias reused across CTEs takes both relations",
            sql: "WITH a AS (SELECT x.* FROM orders x), b AS (SELECT x.* FROM customers x) \
                  SELECT a.id FROM a JOIN b ON a.id = b.id",
            expected_relations: every,
        },
        StarKeysTestCase {
            description: "SQL with a gap takes every relation",
            sql: "SELECT o.* FROM orders AS o ORDER BY o.id LIMIT 1 WITH TIES",
            expected_relations: every,
        },
    ];
    let relations = [fixture_relation("orders"), fixture_relation("customers")];
    for test_case in test_cases {
        let keys: Vec<String> = star_keys(test_case.sql, &relations, Some(DIALECT), None)
            .into_iter()
            .map(|(_, name)| name)
            .collect();
        assert_eq!(
            keys, test_case.expected_relations,
            "{}",
            test_case.description
        );
    }
}
