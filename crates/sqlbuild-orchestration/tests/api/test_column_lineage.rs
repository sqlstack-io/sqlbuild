//! Published column lineage reaches people as one node's upstream and downstream columns, on both control stores.

use serde_json::json;
use uuid::Uuid;

use crate::api::tests::test_types::ColumnLineageTestCase;
use crate::authorization::main::bootstrap_user::bootstrap_user;
use crate::credentials::main::identities::create_principal::create_principal;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::snapshots::models::SnapshotNode;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_metadata::main::read_person_column_lineage::read_person_column_lineage;

#[test]
fn given_published_column_lineage_when_people_read_a_node_then_only_its_reachable_columns_return() {
    let test_cases = [
        ColumnLineageTestCase {
            description: "SQLite column lineage",
            backend: Backend::Sqlite,
            expected_orders_edges: vec![
                ("model:raw_orders", "model:orders"),
                ("model:orders", "model:order_totals"),
            ],
            expected_leaf_edges: 2,
        },
        ColumnLineageTestCase {
            description: "PostgreSQL column lineage",
            backend: Backend::Postgres,
            expected_orders_edges: vec![
                ("model:raw_orders", "model:orders"),
                ("model:orders", "model:order_totals"),
            ],
            expected_leaf_edges: 2,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let audit = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let location_id = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let model = |columns: serde_json::Value| SnapshotNode {
                kind: "model".into(),
                definition: json!({"materialization": "table", "columns": columns}),
                raw_sql: Some("select 1 as id".into()),
                compiled_sql: Some("select 1 as id".into()),
                python_source: None,
            };
            let mut input = snapshot(location_id);
            input.nodes = [
                (
                    "model:raw_orders",
                    model(json!([{"name": "amount", "type": "VARCHAR"}])),
                ),
                (
                    "model:orders",
                    model(json!([{"name": "amount", "type": "DECIMAL(10,2)"}])),
                ),
                (
                    "model:order_totals",
                    model(json!([{"name": "total", "type": null}])),
                ),
                (
                    "model:inventory",
                    model(json!([{"name": "quantity", "type": "INTEGER"}])),
                ),
                (
                    "model:inventory_report",
                    model(json!([{"name": "quantity", "type": "INTEGER"}])),
                ),
            ]
            .into_iter()
            .map(|(key, node)| (key.to_owned(), node))
            .collect();
            input.documents.insert(
                "lineage".into(),
                json!({"edges": [
                    ["model:raw_orders", "amount", "model:orders", "amount", "cast"],
                    ["model:orders", "amount", "model:order_totals", "total", "aggregate"],
                    ["model:inventory", "quantity", "model:inventory_report", "quantity", "direct"],
                    ["model:retired_orders", "amount", "model:orders", "amount", "direct"],
                ]}),
            );
            publish_snapshot(store, &input).await.expect("publish");
            let admin = bootstrap_user(store, &audit, "orders-admin")
                .await
                .expect("administrator");
            let read = |node_key: &'static str, person: Uuid| {
                read_person_column_lineage(
                    store,
                    (&audit, person),
                    (location_id, "development", node_key),
                    DefaultRole::None,
                )
            };
            let orders = read("model:orders", admin).await.expect("orders lineage");
            assert_eq!(
                orders
                    .edges
                    .iter()
                    .map(|e| (e.source_key.as_str(), e.target_key.as_str()))
                    .collect::<Vec<_>>(),
                test_case.expected_orders_edges,
                "{}: both directions, never another node's chain or a node outside the snapshot",
                test_case.description
            );
            assert_eq!(
                json!(orders.nodes),
                json!([
                    {"node_key": "model:order_totals", "kind": "model", "columns": [{"name": "total", "data_type": null}]},
                    {"node_key": "model:orders", "kind": "model", "columns": [{"name": "amount", "data_type": "DECIMAL(10,2)"}]},
                    {"node_key": "model:raw_orders", "kind": "model", "columns": [{"name": "amount", "data_type": "VARCHAR"}]},
                ]),
                "{}",
                test_case.description
            );
            let leaf = read("model:order_totals", admin)
                .await
                .expect("leaf lineage");
            assert_eq!(
                leaf.edges.len(),
                test_case.expected_leaf_edges,
                "{}: upstream is followed transitively",
                test_case.description
            );
            let stranger = create_principal(store, &audit, "orders-stranger")
                .await
                .expect("person");
            assert!(
                matches!(
                    read("model:orders", stranger).await,
                    Err(ControlError::Denied { .. })
                ),
                "{}: lineage needs view on every node",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
