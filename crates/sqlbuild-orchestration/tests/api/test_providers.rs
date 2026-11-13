//! Published providers reach people with the jobs that use them, on both control stores.

use serde_json::json;
use uuid::Uuid;

use crate::api::tests::test_types::ProvidersTestCase;
use crate::authorization::main::bootstrap_user::bootstrap_user;
use crate::credentials::main::identities::create_principal::create_principal;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::read_person_providers::read_person_providers;
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_published_providers_when_people_read_them_then_jobs_using_their_nodes_are_listed() {
    let test_cases = [
        ProvidersTestCase {
            description: "SQLite providers",
            backend: Backend::Sqlite,
            expected_jobs: vec!["order_exports", "stock_alerts"],
        },
        ProvidersTestCase {
            description: "PostgreSQL providers",
            backend: Backend::Postgres,
            expected_jobs: vec!["order_exports", "stock_alerts"],
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
            let admin = bootstrap_user(store, &audit, "orders-admin")
                .await
                .expect("administrator");
            let read = |person: Uuid| {
                read_person_providers(
                    store,
                    (&audit, person),
                    (location_id, "development"),
                    DefaultRole::None,
                )
            };
            let before = read(admin).await.expect("read before a publish");
            assert_eq!(
                (before.snapshot_id, before.providers.len()),
                (None, 0),
                "{}: nothing is published yet",
                test_case.description
            );
            let mut input = snapshot(location_id);
            let model = input.nodes["orders"].clone();
            for (key, kind) in [
                ("model:orders", "model"),
                ("model:inventory", "model"),
                ("check:stock_levels", "check"),
            ] {
                let mut node = model.clone();
                node.kind = kind.into();
                input.nodes.insert(key.into(), node);
            }
            input.documents.insert(
                "jobs".into(),
                json!({
                    "order_exports": {"description": "Export orders", "selection": ["model:orders"]},
                    "inventory_refresh": {"description": "Refresh stock", "selection": ["model:inventory"]},
                    "stock_alerts": {"description": "Alert on stock", "selection": ["check:stock_levels"]},
                }),
            );
            input.documents.insert(
                "providers".into(),
                json!({"providers": [{
                    "name": "fulfillment_api",
                    "class": "providers.fulfillment.FulfillmentApi",
                    "description": "Client for the fulfillment service.",
                    "path": "providers/fulfillment.py",
                    "settings": [
                        {"name": "api_key", "type": "SecretStr", "required": true, "secret": true, "env": "API_KEY", "description": null},
                        {"name": "endpoint", "type": "str", "required": false, "secret": false, "env": "ENDPOINT", "description": "Base URL"},
                    ],
                    "used_by": [
                        {"kind": "hook", "name": "notify_fulfillment"},
                        {"kind": "check", "name": "stock_levels"},
                        {"kind": "sink", "name": "order_events"},
                    ],
                    "nodes": ["check:stock_levels", "model:orders"],
                }]}),
            );
            publish_snapshot(store, &input).await.expect("publish");
            let read_back = read(admin).await.expect("providers");
            assert_eq!(
                json!(read_back.providers),
                json!([{
                    "name": "fulfillment_api",
                    "class": "providers.fulfillment.FulfillmentApi",
                    "description": "Client for the fulfillment service.",
                    "path": "providers/fulfillment.py",
                    "settings": [
                        {"name": "api_key", "type": "SecretStr", "required": true, "secret": true, "env": "API_KEY", "description": null},
                        {"name": "endpoint", "type": "str", "required": false, "secret": false, "env": "ENDPOINT", "description": "Base URL"},
                    ],
                    "used_by": [
                        {"kind": "hook", "name": "notify_fulfillment"},
                        {"kind": "check", "name": "stock_levels"},
                        {"kind": "sink", "name": "order_events"},
                    ],
                    "nodes": ["check:stock_levels", "model:orders"],
                    "jobs": test_case.expected_jobs,
                }]),
                "{}: jobs selecting a node that runs the provider, including a model through its hook",
                test_case.description
            );
            let stranger = create_principal(store, &audit, "orders-stranger")
                .await
                .expect("person");
            assert!(
                matches!(read(stranger).await, Err(ControlError::Denied { .. })),
                "{}: providers need view on every node",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
