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
                ("source:shop.raw_orders", "model:raw_orders"),
                ("seed:currencies", "model:orders"),
            ],
            expected_leaf_edges: 4,
        },
        ColumnLineageTestCase {
            description: "PostgreSQL column lineage",
            backend: Backend::Postgres,
            expected_orders_edges: vec![
                ("model:raw_orders", "model:orders"),
                ("model:orders", "model:order_totals"),
                ("source:shop.raw_orders", "model:raw_orders"),
                ("seed:currencies", "model:orders"),
            ],
            expected_leaf_edges: 4,
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
            let node = |kind: &str, columns: serde_json::Value| SnapshotNode {
                kind: kind.into(),
                definition: json!({"materialization": "table", "columns": columns}),
                raw_sql: Some("select 1 as id".into()),
                compiled_sql: Some("select 1 as id".into()),
                python_source: None,
            };
            let model = |columns: serde_json::Value| node("model", columns);
            let mut input = snapshot(location_id);
            input.nodes = [
                (
                    "source:shop.raw_orders",
                    node("source", json!([{"name": "amount", "type": "VARCHAR"}])),
                ),
                (
                    "seed:currencies",
                    node("seed", json!([{"name": "code", "type": "VARCHAR"}])),
                ),
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
            // A chain longer than one definition batch, so the read loads definitions in several queries.
            let chain: Vec<String> = (0..1200).map(|i| format!("model:chain_{i:04}")).collect();
            for key in &chain {
                input.nodes.insert(
                    key.clone(),
                    model(json!([{"name": "id", "type": "INTEGER"}])),
                );
            }
            let mut edges: Vec<serde_json::Value> = vec![
                json!([
                    "model:raw_orders",
                    "amount",
                    "model:orders",
                    "amount",
                    "cast"
                ]),
                json!([
                    "model:orders",
                    "amount",
                    "model:order_totals",
                    "total",
                    "aggregate"
                ]),
                json!([
                    "model:inventory",
                    "quantity",
                    "model:inventory_report",
                    "quantity",
                    "direct"
                ]),
                json!([
                    "model:retired_orders",
                    "amount",
                    "model:orders",
                    "amount",
                    "direct"
                ]),
                json!([
                    "source:shop.raw_orders",
                    "amount",
                    "model:raw_orders",
                    "amount",
                    "direct"
                ]),
                json!(["seed:currencies", "code", "model:orders", "amount", "join"]),
            ];
            edges.extend(
                chain
                    .windows(2)
                    .map(|pair| json!([pair[0], "id", pair[1], "id", "direct"])),
            );
            input
                .documents
                .insert("lineage".into(), json!({ "edges": edges }));
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
                    {"node_key": "seed:currencies", "kind": "seed", "columns": [{"name": "code", "data_type": "VARCHAR"}]},
                    {"node_key": "source:shop.raw_orders", "kind": "source", "columns": [{"name": "amount", "data_type": "VARCHAR"}]},
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
            let middle = read("model:chain_0600", admin)
                .await
                .expect("chain lineage");
            assert_eq!(
                (
                    middle.edges.len(),
                    middle
                        .nodes
                        .iter()
                        .filter(|node| node.columns.len() == 1)
                        .count()
                ),
                (1199, 1200),
                "{}: every touched node's columns load across definition batches",
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
