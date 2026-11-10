//! A person's bootstrap and change feed expose only locations and topics their roles allow.

use reqwest::{StatusCode, header};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, HumanLogin};
use crate::api::tests::changes_helpers::{
    audit_events, expect_topics, next_topic, opened, request_person_feed,
};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::test_types::PersonLiveTestCase;
use crate::credentials::tests::location_helpers::create_location;
use crate::credentials::tests::role_helpers::{SUPPORT_ROLE, orders_grant, role_fixture};
use crate::runs::main::request_job_run::request_job_run;
use crate::runs::models::JobRunRequest;
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_people_with_different_roles_when_they_bootstrap_and_subscribe_then_only_permitted_locations_and_topics_are_exposed()
 {
    let test_cases = [
        PersonLiveTestCase {
            description: "SQLite person bootstrap and change feed",
            backend: Backend::Sqlite,
            expected_denied_feed: StatusCode::FORBIDDEN,
            expected_disabled_bootstrap: StatusCode::UNAUTHORIZED,
        },
        PersonLiveTestCase {
            description: "PostgreSQL person bootstrap and change feed",
            backend: Backend::Postgres,
            expected_denied_feed: StatusCode::FORBIDDEN,
            expected_disabled_bootstrap: StatusCode::UNAUTHORIZED,
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
            let mut orders = snapshot(roles.location_id);
            orders.generation = 2;
            orders.documents.insert(
                "roles".into(),
                json!({SUPPORT_ROLE: {"description": "Answer order questions",
                    "grants": [orders_grant(json!(["view"]), &["orders"])]}}),
            );
            orders.documents.insert(
                "jobs".into(),
                json!({"orders": {"description": "Build orders", "selection": ["orders"]}}),
            );
            publish_snapshot(store, &orders).await.expect("orders jobs");
            let inventory = create_location(store, "inventory", "orders-admin")
                .await
                .expect("inventory location");
            publish_snapshot(store, &snapshot(inventory))
                .await
                .expect("inventory snapshot");
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::Password("https://control.example.com".into()),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let client = reqwest::Client::new();
            let bootstrap = |bearer: String| {
                client
                    .get(format!("{}/api/control/bootstrap", api.url))
                    .header(header::HOST, "control.example.com")
                    .header(header::COOKIE, format!("__Host-sqb-session={bearer}"))
                    .send()
            };
            let visible = |body: &Value| {
                body["access"]["locations"]
                    .as_array()
                    .expect("locations")
                    .iter()
                    .map(|location| {
                        json!([location["name"], location["target"], location["actions"]])
                    })
                    .collect::<Vec<_>>()
            };

            let audited = audit_events(store).await;
            let member: Value = bootstrap(roles.member.bearer.clone())
                .await
                .expect("member bootstrap")
                .json()
                .await
                .expect("bootstrap body");
            let administrator: Value = bootstrap(roles.administrator.bearer.clone())
                .await
                .expect("administrator bootstrap")
                .json()
                .await
                .expect("bootstrap body");
            assert_eq!(
                (visible(&member), member["access"]["actions"].clone()),
                (vec![json!(["orders", "development", ["view"]])], json!([])),
                "{}: the member sees only the location its role can view",
                test_case.description
            );
            assert_eq!(
                visible(&administrator)
                    .iter()
                    .map(|location| location[0].clone())
                    .collect::<Vec<_>>(),
                vec![json!("inventory"), json!("orders")]
            );
            assert_eq!(
                audit_events(store).await,
                audited,
                "{}: capability reporting writes no audit events",
                test_case.description
            );

            let partial = request_person_feed(
                &client,
                &api,
                (roles.location_id, "development", &roles.member.bearer),
            )
            .await
            .status();
            let unassigned = request_person_feed(
                &client,
                &api,
                (inventory, "development", &roles.member.bearer),
            )
            .await
            .status();
            assert_eq!(
                (partial, unassigned, audit_events(store).await - audited),
                (
                    test_case.expected_denied_feed,
                    test_case.expected_denied_feed,
                    2
                ),
                "{}: feeds need view of every node, and each denial is audited",
                test_case.description
            );

            let mut feed = opened(
                request_person_feed(
                    &client,
                    &api,
                    (
                        roles.location_id,
                        "development",
                        &roles.administrator.bearer,
                    ),
                )
                .await,
            )
            .await;
            let request = JobRunRequest {
                run_id: Uuid::now_v7(),
                location_id: roles.location_id,
                target: "development".into(),
                job_key: "orders".into(),
                reason: "manual verification".into(),
                priority: 0,
                window: None,
                full_refresh: false,
                select: Vec::new(),
                exclude: Vec::new(),
            };
            assert!(
                request_job_run(store, (&roles.context, None), &request)
                    .await
                    .expect("queue job")
            );
            expect_topics(
                &mut feed,
                &["runs".into(), "node_status:development".into()],
            )
            .await;
            let mut other = snapshot(inventory);
            other.generation = 2;
            publish_snapshot(store, &other)
                .await
                .expect("other location change");
            orders.generation = 3;
            publish_snapshot(store, &orders)
                .await
                .expect("orders change");
            expect_topics(
                &mut feed,
                &[
                    format!("snapshot:{}", roles.location_id),
                    "node_status:development".into(),
                ],
            )
            .await;

            let disabled = client
                .post(format!("{}/api/admin/people/{}", api.url, roles.member_id))
                .header(header::HOST, "control.example.com")
                .header(header::ORIGIN, "https://control.example.com")
                .header(
                    header::COOKIE,
                    format!("__Host-sqb-session={}", roles.administrator.bearer),
                )
                .header("x-sqb-csrf", &roles.administrator.csrf)
                .json(&json!("disable"))
                .send()
                .await
                .expect("disable member")
                .status();
            let after_disable = bootstrap(roles.member.bearer.clone())
                .await
                .expect("disabled bootstrap")
                .status();
            let people: Value = client
                .get(format!("{}/api/admin/people", api.url))
                .header(header::HOST, "control.example.com")
                .header(
                    header::COOKIE,
                    format!("__Host-sqb-session={}", roles.administrator.bearer),
                )
                .send()
                .await
                .expect("people")
                .json()
                .await
                .expect("people body");
            let member = people
                .as_array()
                .into_iter()
                .flatten()
                .find(|person| person["principal_id"] == json!(roles.member_id))
                .expect("disabled member is listed");
            assert_eq!(
                (
                    member["disabled_at"].is_string(),
                    &member["active_sessions"]
                ),
                (true, &json!(0)),
                "a disabled person's sessions no longer count as active"
            );
            let logout = client
                .post(format!("{}/api/auth/logout", api.url))
                .header(header::HOST, "control.example.com")
                .header(header::ORIGIN, "https://control.example.com")
                .header(
                    header::COOKIE,
                    format!("__Host-sqb-session={}", roles.administrator.bearer),
                )
                .header("x-sqb-csrf", &roles.administrator.csrf)
                .send()
                .await
                .expect("logout")
                .status();
            orders.generation = 4;
            publish_snapshot(store, &orders)
                .await
                .expect("change after logout");
            assert_eq!(
                (disabled, after_disable, logout, next_topic(&mut feed).await),
                (
                    StatusCode::OK,
                    test_case.expected_disabled_bootstrap,
                    StatusCode::NO_CONTENT,
                    None
                ),
                "{}: a disabled person loses bootstrap and a logged-out feed ends",
                test_case.description
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
