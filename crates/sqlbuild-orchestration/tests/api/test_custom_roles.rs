//! Custom role assignment works over HTTP with session CSRF and applies at once.

use reqwest::header;
use serde_json::{Value, json};

use crate::api::models::ApiAuthentication;
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::test_types::CustomRoleApiTestCase;
use crate::credentials::models::Action;
use crate::credentials::tests::role_helpers::{SUPPORT_ROLE, granting_role, role_fixture};
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_http_sessions_when_custom_role_is_assigned_then_it_applies_at_once() {
    let test_cases = [
        CustomRoleApiTestCase {
            description: "SQLite custom role assignment over HTTP",
            backend: Backend::Sqlite,
            expected_statuses: [400, 200],
        },
        CustomRoleApiTestCase {
            description: "PostgreSQL custom role assignment over HTTP",
            backend: Backend::Postgres,
            expected_statuses: [400, 200],
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
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    password_origin: Some("https://control.example.com".into()),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let client = reqwest::Client::new();
            let post = |path: &str, body: Value| {
                let session = &roles.administrator;
                client
                    .post(format!("{}{path}", api.url))
                    .header(header::HOST, "control.example.com")
                    .header(header::ORIGIN, "https://control.example.com")
                    .header(
                        header::COOKIE,
                        format!("__Host-sqb-session={}", session.bearer),
                    )
                    .header("x-sqb-csrf", &session.csrf)
                    .json(&body)
                    .send()
            };
            let misspelt = post(
                "/api/admin/roles",
                json!({"principal_id": roles.member_id, "role": "Operator",
                    "location_id": roles.location_id, "target": "development"}),
            )
            .await
            .expect("misspelt role")
            .status()
            .as_u16();
            let assignment = post(
                "/api/admin/roles",
                json!({"principal_id": roles.member_id, "role": SUPPORT_ROLE,
                    "location_id": roles.location_id, "target": "development"}),
            )
            .await
            .expect("assign");
            let assignment_status = assignment.status().as_u16();
            assert_eq!(
                [misspelt, assignment_status],
                test_case.expected_statuses,
                "{}",
                test_case.description
            );
            assert_eq!(
                granting_role(store, &roles, Action::View).await.as_deref(),
                Some(SUPPORT_ROLE)
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
