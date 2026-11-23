//! Planner messages keep their wording, limits and ordering.

use crate::planner_warehouse::main::freshness_unknown_message::freshness_unknown_message;
use crate::planner_warehouse::main::full_rebuild_cause::full_rebuild_cause;
use crate::planner_warehouse::main::missing_source_tables_help::missing_source_tables_help;
use crate::planner_warehouse::main::missing_source_tables_message::missing_source_tables_message;
use crate::planner_warehouse::models::FullRebuildFacts;
use crate::planner_warehouse::tests::helpers::missing;
use crate::planner_warehouse::tests::test_types::{
    FreshnessMessageTestCase, MissingSourceTestCase, RebuildCauseTestCase,
};

#[test]
fn given_missing_source_tables_when_reporting_then_lists_readers_and_remedies() {
    let test_cases = [
        MissingSourceTestCase {
            description: "one missing table",
            missing: vec![missing("orders", &["fact_orders"], None)],
            expected_message: "cannot build selected scope: 1 source table read by selected resources does not exist in the warehouse:\n  - source 'orders' (raw.orders), read by fact_orders",
            expected_help: "create or load the table, correct its database, schema, or table in the source declaration, or stop reading it from the selected models; declared 'orders' at sql/sources/orders.yml:2",
        },
        MissingSourceTestCase {
            description: "unreadable table names the role",
            missing: vec![
                missing("orders", &["a", "b"], None),
                missing("customers", &["c"], Some("ANALYST")),
            ],
            expected_message: "cannot build selected scope: 2 source tables read by selected resources do not exist in the warehouse or are not readable by the current role:\n  - source 'orders' (raw.orders), read by a, b\n  - source 'customers' (raw.customers), read by c; the warehouse reports it does not exist or is not readable by role ANALYST",
            expected_help: "create or load the table, grant SELECT on it to the role in use (ANALYST), correct its database, schema, or table in the source declaration, or stop reading it from the selected models; declared 'orders' at sql/sources/orders.yml:2, 'customers' at sql/sources/customers.yml:2",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            missing_source_tables_message(&test_case.missing),
            test_case.expected_message,
            "{}",
            test_case.description
        );
        assert_eq!(
            missing_source_tables_help(&test_case.missing),
            test_case.expected_help,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_unknown_freshness_when_warning_then_sorts_and_limits_names() {
    let many: Vec<String> = (0..12).rev().map(|index| format!("s{index:02}")).collect();
    let test_cases = [
        FreshnessMessageTestCase {
            description: "error reason with many names",
            reason: "error",
            names: many,
            expected_message: Some("source freshness unknown (error) for s00, s01, s02, s03, s04, s05, s06, s07, s08, s09 and 2 more: the freshness observation failed; run sqb freshness for each source's details".to_owned()),
        },
        FreshnessMessageTestCase {
            description: "unconfigured reason is silent",
            reason: "no_freshness_config",
            names: vec!["orders".to_owned()],
            expected_message: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            freshness_unknown_message(test_case.reason, &test_case.names),
            test_case.expected_message,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_change_facts_when_describing_rebuild_then_uses_first_matching_cause() {
    let test_cases = [
        RebuildCauseTestCase {
            description: "flag wins",
            facts: FullRebuildFacts {
                full_refresh: true,
                change_kind: "first_run".to_owned(),
                ..FullRebuildFacts::default()
            },
            expected_cause: "--full-refresh",
        },
        RebuildCauseTestCase {
            description: "changed functions",
            facts: FullRebuildFacts {
                change_kind: "changed".to_owned(),
                changed_functions: vec!["f".to_owned(), "g".to_owned()],
                ..FullRebuildFacts::default()
            },
            expected_cause: "function f, g changed with replay_on_change full",
        },
        RebuildCauseTestCase {
            description: "nothing specific",
            facts: FullRebuildFacts::default(),
            expected_cause: "full backfill",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            full_rebuild_cause(&test_case.facts),
            test_case.expected_cause,
            "{}",
            test_case.description
        );
    }
}
