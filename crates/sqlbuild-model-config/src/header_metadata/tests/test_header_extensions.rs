use crate::header_metadata::tests::helpers::{dynamic_summary, factory_summary, family, map};
use crate::header_metadata::tests::test_types::{
    DynamicColumnsTestCase, FactoryReferencesTestCase,
};
use crate::tests::test_types::Value;

#[test]
fn given_dynamic_columns_when_parsing_then_families_or_first_error_are_returned() {
    let test_cases = [
        DynamicColumnsTestCase {
            description: "one family without a pattern",
            raw: map(&[("revenue", family(None))]),
            expected_families: Ok(&["revenue:SUM:-"]),
        },
        DynamicColumnsTestCase {
            description: "empty block",
            raw: map(&[]),
            expected_families: Err(
                "model 'orders' dynamic_columns must be a non-empty named block in sql/models/orders.sql",
            ),
        },
        DynamicColumnsTestCase {
            description: "unsupported keys are sorted",
            raw: map(&[(
                "revenue",
                map(&[("zone", Value::Null), ("alias", Value::Null)]),
            )]),
            expected_families: Err(
                "model 'orders' dynamic column family 'revenue' has unsupported keys: alias, zone",
            ),
        },
        DynamicColumnsTestCase {
            description: "blank required key",
            raw: map(&[("revenue", map(&[("pivot_column", Value::Str("  "))]))]),
            expected_families: Err(
                "model 'orders' dynamic column family 'revenue' requires a non-empty pivot_column",
            ),
        },
        DynamicColumnsTestCase {
            description: "invalid pattern",
            raw: map(&[("revenue", family(Some("(")))]),
            expected_families: Err(
                "model 'orders' dynamic column family 'revenue' has invalid name_pattern: \
                 unclosed group",
            ),
        },
        DynamicColumnsTestCase {
            description: "two families need patterns",
            raw: map(&[("revenue", family(Some("rev_.*"))), ("units", family(None))]),
            expected_families: Err(
                "model 'orders' declares multiple dynamic column families; each family requires \
                 name_pattern so runtime columns can be classified unambiguously",
            ),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            dynamic_summary(&test_case.raw),
            test_case
                .expected_families
                .map(|families| families.iter().map(|family| (*family).to_owned()).collect())
                .map_err(str::to_owned),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_audit_factories_when_resolving_then_references_or_first_error_are_returned() {
    let test_cases = [
        FactoryReferencesTestCase {
            description: "known factories in order",
            raw: Value::List(vec![Value::Str("freshness"), Value::Str("daily_checks")]),
            expected_references: Ok(&["freshness", "daily_checks"]),
        },
        FactoryReferencesTestCase {
            description: "a tuple is not a list",
            raw: Value::Tuple(vec![Value::Str("freshness")]),
            expected_references: Err("sql/models/orders.sql model audit_factories must be a list"),
        },
        FactoryReferencesTestCase {
            description: "dotted entry",
            raw: Value::List(vec![Value::Str("pkg.freshness")]),
            expected_references: Err(
                "sql/models/orders.sql model audit_factories entry 'pkg.freshness' must be a bare identifier",
            ),
        },
        FactoryReferencesTestCase {
            description: "repeated entry",
            raw: Value::List(vec![Value::Str("freshness"), Value::Str("freshness")]),
            expected_references: Err(
                "model 'orders' in sql/models/orders.sql references audit factory 'freshness' more than once",
            ),
        },
        FactoryReferencesTestCase {
            description: "unknown entry lists sorted factories",
            raw: Value::List(vec![Value::Str("volume")]),
            expected_references: Err(
                "model 'orders' in sql/models/orders.sql references unknown audit factory 'volume'. \
                 Available audit factories: daily_checks, freshness",
            ),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            factory_summary(&test_case.raw),
            test_case
                .expected_references
                .map(|references| references.iter().map(|name| (*name).to_owned()).collect())
                .map_err(str::to_owned),
            "{}",
            test_case.description
        );
    }
}
