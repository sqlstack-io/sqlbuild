//! Per-request proxy identities keep live roles and final-admin protection across replicas.

use std::collections::{BTreeMap, BTreeSet};

use reqwest::{Client, StatusCode, header};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, HumanLogin, TrustedProxyAuthentication};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::test_types::ExternalSessionTestCase;
use crate::authorization::main::bootstrap_user::bootstrap_user;
use crate::credentials::main::{
    identities::create_principal::create_principal, locations::create_location::create_location,
};
use crate::credentials::models::AuditContext;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_proxy_people_when_administrators_and_roles_change_then_live_authority_holds() {
    let test_cases = [
        ExternalSessionTestCase {
            description: "SQLite proxy administrator journey",
            backend: Backend::Sqlite,
            expected_viewer_statuses: [403, 200, 403],
        },
        ExternalSessionTestCase {
            description: "PostgreSQL proxy administrator journey",
            backend: Backend::Postgres,
            expected_viewer_statuses: [403, 200, 403],
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend).await.expect("database");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let replica = second_store(store).await;
            let context = AuditContext {
                actor: "local-control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../benchmarks/duckshop/edge_cases/orchestration/proxy_identities.json");
            let inputs: Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("DuckShop federation"))
                    .expect("inputs");
            let people = &inputs["administrator_journey"];
            let first_subject = people["bound"][0].as_str().expect("first bound person");
            let second_subject = people["bound"][1].as_str().expect("second bound person");
            let first = bootstrap_user(store, &context, first_subject)
                .await
                .expect("first admin");
            let second = create_principal(store, &context, second_subject)
                .await
                .expect("second person");
            let origin = inputs["origin"].as_str().expect("origin");
            let settings = TrustedProxyAuthentication {
                origin: origin.into(),
                trusted_peers: BTreeSet::from(["127.0.0.1".parse().expect("peer")]),
                identities: BTreeMap::from([
                    (first_subject.into(), first),
                    (second_subject.into(), second),
                ]),
            };
            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::TrustedProxy(settings.clone()),
                    ..Default::default()
                },
            )
            .await;
            let other = start_api_with_authentication(
                &replica,
                ApiAuthentication {
                    login: HumanLogin::TrustedProxy(settings),
                    ..Default::default()
                },
            )
            .await;
            let client = Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("client");
            let subjects = [first_subject, second_subject];
            let request = |index: usize, path: &str| {
                client
                    .post(format!("{}{path}", other.url))
                    .header(header::HOST, "control.example.com")
                    .header(header::ORIGIN, origin)
                    .header("x-sqb-user", subjects[index])
            };
            let read = |index: usize, path: &str| {
                client
                    .get(format!("{}{path}", other.url))
                    .header(header::HOST, "control.example.com")
                    .header("x-sqb-user", subjects[index])
            };
            let roles = |principal_id| json!({"principal_id":principal_id,"role":"admin","location_id":null,"target":null});
            let response = request(0, "/api/admin/roles")
                .json(&roles(second))
                .send()
                .await
                .expect("admin grant");
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.json::<Value>().await.expect("applied grant")["applied"],
                true
            );
            assert_eq!(
                request(0, &format!("/api/admin/people/{first}"))
                    .json(&"revoke")
                    .send()
                    .await
                    .expect("SSO admin replacement")
                    .status(),
                StatusCode::OK
            );
            assert_eq!(
                request(0, "/api/admin/roles")
                    .json(&roles(first))
                    .send()
                    .await
                    .expect("live revoked admin")
                    .status(),
                StatusCode::FORBIDDEN
            );
            for operation in ["revoke", "disable"] {
                let response = request(1, &format!("/api/admin/people/{second}"))
                    .json(&operation)
                    .send()
                    .await
                    .expect("final SSO admin");
                assert_eq!(response.status(), StatusCode::FORBIDDEN);
                let body = response.json::<Value>().await.expect("invariant denial");
                assert!(body["error"].as_str().expect("error").starts_with(
                    "cannot remove or disable the final installation administrator; decision "
                ));
            }
            let location = create_location(store, "orders", &context)
                .await
                .expect("registered location");
            let metadata = format!("/api/control/locations/{location}/targets/development/runs");
            let no_role = read(0, &metadata)
                .send()
                .await
                .expect("no ambient role")
                .status();
            let viewer = json!({"principal_id":first,"role":"viewer","location_id":location,"target":"development"});
            assert_eq!(
                request(1, "/api/admin/roles")
                    .json(&viewer)
                    .send()
                    .await
                    .expect("viewer grant")
                    .status(),
                StatusCode::OK
            );
            let granted = read(0, &metadata)
                .send()
                .await
                .expect("live viewer")
                .status();
            let mut revocation = viewer;
            revocation["revoke"] = json!(true);
            assert_eq!(
                request(1, "/api/admin/roles")
                    .json(&revocation)
                    .send()
                    .await
                    .expect("viewer revocation")
                    .status(),
                StatusCode::OK
            );
            let revoked = read(0, &metadata)
                .send()
                .await
                .expect("live viewer loss")
                .status();
            assert_eq!(
                [no_role, granted, revoked].map(|status| status.as_u16()),
                test_case.expected_viewer_statuses,
                "{}",
                test_case.description
            );
            stop_api(other).await;
            stop_api(api).await;
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}
