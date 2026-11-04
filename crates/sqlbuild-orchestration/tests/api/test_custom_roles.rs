//! Custom role assignment, activation and approval work over HTTP with session CSRF.

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
fn given_http_sessions_when_custom_role_is_assigned_requested_and_approved_then_only_another_administrator_activates_it()
 {
    let test_cases = [
        CustomRoleApiTestCase {
            description: "SQLite custom role approval over HTTP",
            backend: Backend::Sqlite,
            expected_statuses: [400, 200, 200, 403, 200],
        },
        CustomRoleApiTestCase {
            description: "PostgreSQL custom role approval over HTTP",
            backend: Backend::Postgres,
            expected_statuses: [400, 200, 200, 403, 200],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let roles = role_fixture(store, 2).await;
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    password_origin: Some("https://control.example.com".into()),
                    ..ApiAuthentication::default()
                },
            )
            .await;
            let client = reqwest::Client::new();
            let post = |administrator: usize, path: &str, body: Value| {
                let session = &roles.administrators[administrator];
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
                0,
                "/api/admin/roles",
                json!({"principal_id": roles.member_id, "role": "Operator",
                    "location_id": roles.location_id, "target": "development"}),
            )
            .await
            .expect("misspelt role")
            .status()
            .as_u16();
            let assignment = post(
                0,
                "/api/admin/roles",
                json!({"principal_id": roles.member_id, "role": SUPPORT_ROLE,
                    "location_id": roles.location_id, "target": "development"}),
            )
            .await
            .expect("assign");
            let assignment_status = assignment.status().as_u16();
            let activation = post(
                0,
                "/api/admin/role-activations",
                json!({"location_id": roles.location_id, "target": "development",
                    "role_key": SUPPORT_ROLE, "reason": "Support answers order questions"}),
            )
            .await
            .expect("request");
            let activation_status = activation.status().as_u16();
            let activation: Value = activation.json().await.expect("activation body");
            let approval = |confirm: bool| {
                json!({"request_id": activation["request_id"], "decision": "approved",
                    "reason": "Reviewed", "confirm_self_approval": confirm})
            };
            let refused = post(0, "/api/admin/role-approvals", approval(true))
                .await
                .expect("self approval")
                .status()
                .as_u16();
            let approved = post(1, "/api/admin/role-approvals", approval(false))
                .await
                .expect("approval")
                .status()
                .as_u16();
            assert_eq!(
                [
                    misspelt,
                    assignment_status,
                    activation_status,
                    refused,
                    approved
                ],
                test_case.expected_statuses,
                "{}",
                test_case.description
            );
            assert_eq!(
                activation["definition_digest"].as_str().map(str::len),
                Some(64)
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
