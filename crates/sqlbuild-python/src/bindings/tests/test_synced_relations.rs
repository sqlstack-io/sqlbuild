use crate::bindings::_helpers::native_host::synced_relations::{RelationFilter, SyncedRelations};
use crate::bindings::tests::helpers::{catalog_record, strings};
use crate::bindings::tests::test_types::SyncedRelationsTestCase;

#[test]
fn given_a_synced_copy_when_relations_are_listed_by_name_then_they_match_as_the_warehouse_folds_case()
 {
    let test_cases = [
        SyncedRelationsTestCase {
            description: "a name in another case finds the relation in every schema, in copy order",
            database: None,
            schemas: None,
            names: Some(&["orders"]),
            expected_relations: &["SHOP.ORDERS", "archive.Orders"],
        },
        SyncedRelationsTestCase {
            description: "a schema narrows the named relations",
            database: Some("warehouse"),
            schemas: Some(&["shop"]),
            names: Some(&["ORDERS"]),
            expected_relations: &["SHOP.ORDERS"],
        },
        SyncedRelationsTestCase {
            description: "a name asked for twice lists its relation once",
            database: None,
            schemas: Some(&["SHOP"]),
            names: Some(&["orders", "Orders", "customers"]),
            expected_relations: &["SHOP.ORDERS", "SHOP.CUSTOMERS"],
        },
        SyncedRelationsTestCase {
            description: "another database hides a relation that names its database",
            database: Some("reporting"),
            schemas: None,
            names: Some(&["orders"]),
            expected_relations: &["archive.Orders"],
        },
        SyncedRelationsTestCase {
            description: "an unknown name lists nothing",
            database: None,
            schemas: None,
            names: Some(&["refunds"]),
            expected_relations: &[],
        },
        SyncedRelationsTestCase {
            description: "no names lists every relation of the schemas but state tables other than fingerprints",
            database: None,
            schemas: Some(&["shop"]),
            names: None,
            expected_relations: &[
                "SHOP.ORDERS",
                "SHOP.CUSTOMERS",
                "SHOP._sqlbuild_fingerprints",
            ],
        },
        SyncedRelationsTestCase {
            description: "a hidden state table is not found by name either",
            database: None,
            schemas: None,
            names: Some(&["_sqlbuild_consumption"]),
            expected_relations: &[],
        },
    ];
    let relations = SyncedRelations::new(vec![
        catalog_record(Some("WAREHOUSE"), "SHOP", "ORDERS"),
        catalog_record(Some("WAREHOUSE"), "SHOP", "CUSTOMERS"),
        catalog_record(None, "archive", "Orders"),
        catalog_record(Some("WAREHOUSE"), "SHOP", "_sqlbuild_fingerprints"),
        catalog_record(Some("WAREHOUSE"), "SHOP", "_sqlbuild_consumption"),
    ]);
    for test_case in test_cases {
        let schemas: Option<Vec<String>> = test_case.schemas.map(strings);
        let names: Option<Vec<String>> = test_case.names.map(strings);
        let listed: Vec<String> = relations
            .listed(&RelationFilter {
                database: test_case.database,
                schemas: schemas.as_deref(),
                names: names.as_deref(),
            })
            .into_iter()
            .map(|record| format!("{}.{}", record.state_schema, record.name))
            .collect();
        assert_eq!(
            listed,
            strings(test_case.expected_relations),
            "{}",
            test_case.description
        );
    }
}
