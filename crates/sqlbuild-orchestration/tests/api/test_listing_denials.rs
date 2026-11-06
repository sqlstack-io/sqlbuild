//! Refused listings name no node and read the same for an unassigned and an unknown location.

use reqwest::header;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, HumanLogin};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::listing_denials_helpers::{audited_scope, masked};
use crate::api::tests::test_types::ListingDenialTestCase;
use crate::credentials::tests::location_helpers::create_location;
use crate::credentials::tests::role_helpers::role_fixture;
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const LISTINGS: [&str; 6] = [
    "runs",
    "catalog?schema=orders",
    "statistics?schema=orders&name=orders",
    "checks",
    "checks/orders_not_null",
    "maintenance",
];

const NODES: [&str; 4] = ["customers", "inventory", "orders", "products"];

#[test]
fn given_refused_listings_when_people_read_them_then_no_node_is_named_and_unknown_locations_look_unassigned()
 {
    let test_cases = [
        ListingDenialTestCase {
            description: "SQLite listing denials",
            backend: Backend::Sqlite,
            expected_status: 403,
            expected_error: "missing view on one or more nodes in this location; decision <decision>",
        },
        ListingDenialTestCase {
            description: "PostgreSQL listing denials",
            backend: Backend::Postgres,
            expected_status: 403,
            expected_error: "missing view on one or more nodes in this location; decision <decision>",
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let roles = role_fixture(store).await;
            let inventory = create_location(store, "inventory", "orders-admin")
                .await
                .expect("unassigned location");
            publish_snapshot(store, &snapshot(inventory))
                .await
                .expect("unassigned snapshot");
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::Password("https://control.example.com".into()),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let client = reqwest::Client::new();
            let read = |location: Uuid, listing: &str| {
                client
                    .get(format!(
                        "{}/api/control/locations/{location}/targets/development/{listing}",
                        api.url
                    ))
                    .header(header::HOST, "control.example.com")
                    .header(
                        header::COOKIE,
                        format!("__Host-sqb-session={}", roles.member.bearer),
                    )
                    .send()
            };
            for listing in LISTINGS {
                let (partial, partial_decision) =
                    masked(read(roles.location_id, listing).await.expect("partial")).await;
                let (unassigned, unassigned_decision) =
                    masked(read(inventory, listing).await.expect("unassigned")).await;
                let (unknown, unknown_decision) =
                    masked(read(Uuid::now_v7(), listing).await.expect("unknown")).await;
                let body: Value =
                    serde_json::from_str(partial["body"].as_str().expect("body")).expect("JSON");
                assert_eq!(
                    (
                        partial["status"].clone(),
                        body["error"].clone(),
                        unassigned == unknown,
                        partial == unassigned,
                    ),
                    (
                        json!(test_case.expected_status),
                        json!(test_case.expected_error),
                        true,
                        true
                    ),
                    "{}: {listing} names no node and hides whether the location exists",
                    test_case.description
                );
                assert!(
                    NODES
                        .iter()
                        .all(|node| !partial["body"].as_str().unwrap_or_default().contains(node)),
                    "{}: {listing} body names a node",
                    test_case.description
                );
                assert_eq!(
                    (
                        audited_scope(store, unassigned_decision).await[0].clone(),
                        audited_scope(store, unknown_decision).await[0].clone(),
                        audited_scope(store, partial_decision).await[1]
                            .as_str()
                            .is_some_and(|reason| reason.contains("inventory")),
                    ),
                    (json!(NODES), json!([]), true),
                    "{}: {listing} audits every refusal with the full detail",
                    test_case.description
                );
            }
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
