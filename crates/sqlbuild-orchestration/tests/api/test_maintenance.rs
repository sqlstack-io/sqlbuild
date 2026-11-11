//! Maintenance history through the public API: moves, their old-name view steps and cleanups.

use reqwest::StatusCode;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::tests::helpers::stop_api;
use crate::api::tests::test_types::WarehouseMetadataTestCase;
use crate::api::tests::warehouse_history_helpers::{
    batch, fixture, get, ids, login, records, seed_target, start_password_api,
};
use crate::platform::main::report_location_health::report_location_health;
use crate::platform::models::{LocationHealth, LocationHealthReport};
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_metadata::main::ingest::{
    ingest_janitor_events::ingest_janitor_events, ingest_migrations::ingest_migrations,
    ingest_old_name_views::ingest_old_name_views,
};

#[test]
fn given_moves_views_and_cleanups_when_people_read_maintenance_then_each_model_sees_its_own_history()
 {
    let test_cases = [
        WarehouseMetadataTestCase {
            description: "SQLite maintenance reads",
            backend: Backend::Sqlite,
            expected_observations: 9,
        },
        WarehouseMetadataTestCase {
            description: "PostgreSQL maintenance reads",
            backend: Backend::Postgres,
            expected_observations: 9,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture_store = open_test_store(test_case.backend).await.expect("store");
            let store = &fixture_store.store;
            let (location, session_id) = seed_target(store).await;
            let data = fixture("maintenance_history.json");
            let scope = (location, "development");
            let moves = records("migration_history.json", &data["migrations"]);
            let views = records("old_name_history.json", &data["old_name_views"]);
            let cleanups = records("janitor_event_history.json", &data["janitor_events"]);
            assert_eq!(
                moves.len() + views.len() + cleanups.len(),
                test_case.expected_observations
            );
            ingest_migrations(store, scope, &batch(session_id, "migrations", moves))
                .await
                .expect("real migration ingestion");
            ingest_old_name_views(store, scope, &batch(session_id, "old_name_views", views))
                .await
                .expect("real old-name ingestion");
            ingest_janitor_events(store, scope, &batch(session_id, "janitor_events", cleanups))
                .await
                .expect("real janitor ingestion");

            let mut preview = snapshot(location);
            preview.target = "preview".into();
            publish_snapshot(store, &preview)
                .await
                .expect("preview snapshot");
            let preview_session = Uuid::now_v7();
            report_location_health(
                store,
                location,
                &LocationHealthReport {
                    event_id: Uuid::now_v7(),
                    session_id: preview_session,
                    target: "preview".into(),
                    health: LocationHealth::Heartbeat {},
                },
            )
            .await
            .expect("preview reader");
            ingest_old_name_views(
                store,
                (location, "preview"),
                &batch(
                    preview_session,
                    "old_name_views",
                    records("old_name_history.json", &data["preview_old_name_views"]),
                ),
            )
            .await
            .expect("another target's view");

            let (api, client) = start_password_api(store).await;
            let viewer = login(&client, &api.url, "orders-viewer").await;
            let path = format!("/api/control/locations/{location}/targets/development/maintenance");

            let (status, all) = get(&client, (&api.url, &viewer), &path).await;
            assert_eq!(status, StatusCode::OK, "{}", test_case.description);
            assert_eq!(
                ids(&all, "migrations"),
                ["move-orders-2", "move-customers-1", "move-orders-1"]
            );
            assert_eq!(
                ids(&all, "old_name_views"),
                [
                    "view-orders-old-1",
                    "view-customers-1",
                    "view-orders-1",
                    "view-orders-2"
                ],
                "another target's steps never appear"
            );
            assert_eq!(ids(&all, "janitor_events"), ["cleanup-2", "cleanup-1"]);
            let grants: Vec<(&str, &Value)> = all["old_name_views"]
                .as_array()
                .expect("views")
                .iter()
                .map(|view| {
                    (
                        view["event_id"].as_str().expect("id"),
                        &view["grants_copied"],
                    )
                })
                .collect();
            assert_eq!(
                grants,
                [
                    ("view-orders-old-1", &Value::Null),
                    ("view-customers-1", &Value::Null),
                    ("view-orders-1", &Value::Null),
                    ("view-orders-2", &json!(2))
                ],
                "copied grants are counted, and unreadable stored text reads as unknown"
            );
            let body = all.to_string();
            assert!(
                !body.contains("GRANT") && !body.contains("support_analysts"),
                "no grant statement or grantee reaches people: {body}"
            );
            assert_eq!(all["migrations"][0]["origin_model"], Value::Null);
            assert_eq!(all["migrations"][0]["discovery"], "automatic");

            let (_, orders) =
                get(&client, (&api.url, &viewer), &format!("{path}?node=orders")).await;
            assert_eq!(
                ids(&orders, "migrations"),
                ["move-orders-2", "move-orders-1"]
            );
            assert_eq!(
                ids(&orders, "old_name_views"),
                ["view-orders-old-1", "view-orders-1", "view-orders-2"],
                "another model's views never appear"
            );
            assert_eq!(orders["janitor_events"], json!([]));

            let (_, newest) = get(&client, (&api.url, &viewer), &format!("{path}?limit=1")).await;
            assert_eq!(ids(&newest, "migrations"), ["move-orders-2"]);
            assert_eq!(
                ids(&newest, "old_name_views"),
                ["view-orders-1", "view-orders-2"],
                "only the listed moves' steps, not every step of their models"
            );
            assert_eq!(ids(&newest, "janitor_events"), ["cleanup-2"]);

            let (_, unknown) = get(
                &client,
                (&api.url, &viewer),
                &format!("{path}?node=unknown"),
            )
            .await;
            assert_eq!(
                unknown,
                json!({"migrations": [], "old_name_views": [], "janitor_events": []})
            );
            assert_eq!(
                get(
                    &client,
                    (&api.url, &viewer),
                    &path.replace("development", "preview")
                )
                .await
                .0,
                StatusCode::FORBIDDEN
            );
            stop_api(api).await;
            close_test_store(fixture_store).await;
        });
    }
}
