use crate::warehouse_state::main::warehouse_state::warehouse_state;
use crate::warehouse_state::models::{RelationFilter, WarehouseStateParts};
use crate::warehouse_state::tests::helpers::{location, owned, shop_parts};
use crate::warehouse_state::tests::test_types::{
    ConsumptionTestCase, DescribedColumnsTestCase, FingerprintsTestCase, RelationsTestCase,
    RoundTripTestCase, TargetMaxTestCase,
};

#[test]
fn given_synced_relations_when_listed_then_they_match_as_the_warehouse_folds_case() {
    let test_cases = [
        RelationsTestCase {
            description: "a name in another case finds the relation in every schema, in source order",
            database: None,
            schemas: None,
            names: Some(&["orders"]),
            expected_relations: &["SHOP.ORDERS", "archive.Orders"],
        },
        RelationsTestCase {
            description: "a schema narrows the named relations",
            database: Some("warehouse"),
            schemas: Some(&["shop"]),
            names: Some(&["ORDERS"]),
            expected_relations: &["SHOP.ORDERS"],
        },
        RelationsTestCase {
            description: "a name asked for twice lists its relation once",
            database: None,
            schemas: Some(&["SHOP"]),
            names: Some(&["orders", "Orders", "customers"]),
            expected_relations: &["SHOP.ORDERS", "SHOP.CUSTOMERS"],
        },
        RelationsTestCase {
            description: "another database hides a relation that names its database",
            database: Some("reporting"),
            schemas: None,
            names: Some(&["orders"]),
            expected_relations: &["archive.Orders"],
        },
        RelationsTestCase {
            description: "an unknown name lists nothing",
            database: None,
            schemas: None,
            names: Some(&["refunds"]),
            expected_relations: &[],
        },
        RelationsTestCase {
            description: "no names lists the schema's relations but state tables other than fingerprints",
            database: None,
            schemas: Some(&["shop"]),
            names: None,
            expected_relations: &[
                "SHOP.ORDERS",
                "SHOP.CUSTOMERS",
                "SHOP._sqlbuild_fingerprints",
            ],
        },
        RelationsTestCase {
            description: "a hidden state table is not found by name either",
            database: None,
            schemas: None,
            names: Some(&["_sqlbuild_consumption"]),
            expected_relations: &[],
        },
    ];
    let state = warehouse_state(shop_parts());
    for test_case in test_cases {
        let schemas: Option<Vec<String>> = test_case.schemas.map(owned);
        let names: Option<Vec<String>> = test_case.names.map(owned);
        let listed: Vec<String> = state
            .relations(&RelationFilter {
                database: test_case.database,
                schemas: schemas.as_deref(),
                names: names.as_deref(),
            })
            .into_iter()
            .map(|relation| format!("{}.{}", relation.schema, relation.name))
            .collect();
        assert_eq!(
            listed,
            owned(test_case.expected_relations),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_recorded_fingerprints_when_a_location_is_read_then_each_node_has_its_newest() {
    let test_cases = [
        FingerprintsTestCase {
            description: "the newest timestamp wins, then the greatest run ID",
            database: Some("warehouse"),
            schema: "shop",
            expected_fingerprints: &["customers@run-b", "orders@run-2"],
        },
        FingerprintsTestCase {
            description: "a fingerprint with no database answers any database",
            database: Some("warehouse"),
            schema: "ARCHIVE",
            expected_fingerprints: &["orders@run-0"],
        },
        FingerprintsTestCase {
            description: "another database has none of the shop's fingerprints",
            database: Some("reporting"),
            schema: "shop",
            expected_fingerprints: &[],
        },
        FingerprintsTestCase {
            description: "a schema with no fingerprints has none",
            database: None,
            schema: "staging",
            expected_fingerprints: &[],
        },
    ];
    let state = warehouse_state(shop_parts());
    for test_case in test_cases {
        let read: Vec<String> = state
            .latest_fingerprints(&location(test_case.database, test_case.schema))
            .into_iter()
            .map(|fingerprint| format!("{}@{}", fingerprint.node_name, fingerprint.run_id))
            .collect();
        assert_eq!(
            read,
            owned(test_case.expected_fingerprints),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_cursor_observations_when_read_then_only_the_newest_successful_build_counts() {
    let test_cases = [
        TargetMaxTestCase {
            description: "the observation of the model's newest succeeded run",
            model: "orders",
            expected_max: Some("2026-01-02"),
        },
        TargetMaxTestCase {
            description: "an observation from a run that is not the newest succeeded one",
            model: "customers",
            expected_max: None,
        },
    ];
    let state = warehouse_state(shop_parts());
    for test_case in test_cases {
        assert_eq!(
            state.target_max(test_case.model),
            test_case.expected_max,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_described_sources_when_read_then_only_the_described_sql_answers() {
    let test_cases = [
        DescribedColumnsTestCase {
            description: "the SQL that was described",
            source: "raw_refunds",
            sql: "SELECT 1",
            expected_columns: Some(&["id", "amount"]),
        },
        DescribedColumnsTestCase {
            description: "an edited expression is not described yet",
            source: "raw_refunds",
            sql: "SELECT 2",
            expected_columns: None,
        },
        DescribedColumnsTestCase {
            description: "a source never described",
            source: "raw_orders",
            sql: "SELECT 1",
            expected_columns: None,
        },
    ];
    let state = warehouse_state(shop_parts());
    for test_case in test_cases {
        assert_eq!(
            state.described_columns(test_case.source, test_case.sql),
            Ok(test_case.expected_columns.map(owned)),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_consumption_in_two_databases_when_one_location_is_read_then_only_its_rows_return() {
    let test_cases = [
        ConsumptionTestCase {
            description: "the location's database in another case",
            database: Some("warehouse"),
            schema: "shop",
            expected_executions: &["execution-1"],
        },
        ConsumptionTestCase {
            description: "no database reads the schema in every database",
            database: None,
            schema: "SHOP",
            expected_executions: &["execution-1", "execution-2"],
        },
    ];
    let state = warehouse_state(shop_parts());
    for test_case in test_cases {
        let executions: Vec<String> = state
            .consumption_rows(&location(test_case.database, test_case.schema))
            .into_iter()
            .map(|row| row.row.execution_id)
            .collect();
        assert_eq!(
            executions,
            owned(test_case.expected_executions),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_parts_when_stored_as_json_and_read_back_then_the_state_is_unchanged() {
    let test_cases = [RoundTripTestCase {
        description: "every fact kind of the shop warehouse",
        parts: shop_parts,
        expected_unchanged: true,
    }];
    for test_case in test_cases {
        let parts: WarehouseStateParts = (test_case.parts)();
        let stored: String = serde_json::to_string(&parts).unwrap_or_default();
        let read: Option<WarehouseStateParts> = serde_json::from_str(&stored).ok();
        assert_eq!(
            read.map(|read| warehouse_state(read).parts().clone()) == Some(parts),
            test_case.expected_unchanged,
            "{}",
            test_case.description
        );
    }
}
