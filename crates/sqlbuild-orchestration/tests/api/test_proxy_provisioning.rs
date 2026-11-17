//! A proxy that rewrites `Host` over plain HTTP signs people in, provisioning newcomers as viewers.

use reqwest::{Client, StatusCode, header};
use serde_json::{Value, json};

use crate::api::models::UnknownProxyUsers;
use crate::api::tests::helpers::stop_api;
use crate::api::tests::proxy_provisioning_helpers::{
    LOGOUT_URL, NAME_HEADER, ORIGIN, USER_HEADER, forwarded, rows, settings, start,
};
use crate::api::tests::test_types::ProxyProvisioningTestCase;
use crate::credentials::tests::role_helpers::role_fixture;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::Parameter;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_host_rewriting_http_proxy_when_people_sign_in_then_newcomers_become_viewers_and_spoofing_is_refused()
 {
    let test_cases = [
        ProxyProvisioningTestCase {
            description: "SQLite proxy provisioning",
            backend: Backend::Sqlite,
            expected_refusals: [StatusCode::UNAUTHORIZED; 5],
            expected_write_statuses: [
                StatusCode::UNAUTHORIZED,
                StatusCode::UNAUTHORIZED,
                StatusCode::UNAUTHORIZED,
                StatusCode::UNPROCESSABLE_ENTITY,
            ],
            expected_roles: r#"[{"role_key": "viewer", "location_id": null, "target": null}]"#,
            expected_feed_headers: ["no-cache, no-transform", "no", "text/event-stream"],
        },
        ProxyProvisioningTestCase {
            description: "PostgreSQL proxy provisioning",
            backend: Backend::Postgres,
            expected_refusals: [StatusCode::UNAUTHORIZED; 5],
            expected_write_statuses: [
                StatusCode::UNAUTHORIZED,
                StatusCode::UNAUTHORIZED,
                StatusCode::UNAUTHORIZED,
                StatusCode::UNPROCESSABLE_ENTITY,
            ],
            expected_roles: r#"[{"role_key": "viewer", "location_id": null, "target": null}]"#,
            expected_feed_headers: ["no-cache, no-transform", "no", "text/event-stream"],
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
            let api = start(
                store,
                settings("127.0.0.1", UnknownProxyUsers::ProvisionViewer),
            )
            .await;
            let untrusted = start(
                store,
                settings("192.0.2.24", UnknownProxyUsers::ProvisionViewer),
            )
            .await;
            let rejecting = start(store, settings("127.0.0.1", UnknownProxyUsers::Reject)).await;
            let client = Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("HTTP client");
            let bootstrap = |url: &str| {
                forwarded(
                    &client,
                    reqwest::Method::GET,
                    format!("{url}/api/control/bootstrap"),
                )
            };
            let untrusted_status = bootstrap(&untrusted.url)
                .header(USER_HEADER, "orders-viewer")
                .send()
                .await
                .expect("untrusted peer")
                .status();
            let missing_status = bootstrap(&api.url)
                .send()
                .await
                .expect("no username")
                .status();
            let wrong_host_status = bootstrap(&api.url)
                .header("x-forwarded-host", "other.example.com")
                .header(USER_HEADER, "orders-viewer")
                .send()
                .await
                .expect("another host")
                .status();
            let taken_status = bootstrap(&api.url)
                .header(USER_HEADER, "support-person")
                .send()
                .await
                .expect("a name another principal holds")
                .status();
            let rejected_status = bootstrap(&rejecting.url)
                .header(USER_HEADER, "orders-newcomer")
                .send()
                .await
                .expect("rejected newcomer")
                .status();
            assert_eq!(
                [
                    untrusted_status,
                    missing_status,
                    wrong_host_status,
                    taken_status,
                    rejected_status
                ],
                test_case.expected_refusals,
                "{}: untrusted peer, missing username, other host, taken name, reject mode",
                test_case.description
            );
            assert_eq!(
                rows(store, "SELECT COUNT(*) AS people FROM sqb_principals WHERE name IN ('orders-viewer','orders-newcomer')", vec![]).await,
                json!([{"people": 0}]),
                "{}: refused requests provision nobody",
                test_case.description
            );
            let first = bootstrap(&api.url)
                .header(USER_HEADER, " Orders-Viewer ")
                .header(NAME_HEADER, "Orders Viewer")
                .send()
                .await
                .expect("first sign-in");
            assert_eq!(first.status(), StatusCode::OK, "{}", test_case.description);
            let first: Value = first.json().await.expect("bootstrap");
            let second: Value = bootstrap(&api.url)
                .header(USER_HEADER, "orders-viewer")
                .send()
                .await
                .expect("second sign-in")
                .json()
                .await
                .expect("bootstrap");
            assert_eq!(
                (
                    &first["login_mode"],
                    &first["access"]["name"],
                    &first["display_name"],
                    &first["logout_url"],
                    &second["access"]["principal_id"],
                    second.get("display_name"),
                ),
                (
                    &json!("trusted_proxy"),
                    &json!("orders-viewer"),
                    &json!("Orders Viewer"),
                    &json!(LOGOUT_URL),
                    &first["access"]["principal_id"],
                    None,
                ),
                "{}: one person, named per request",
                test_case.description
            );
            let principal: uuid::Uuid =
                serde_json::from_value(first["access"]["principal_id"].clone()).expect("person");
            assert_eq!(
                rows(store, "SELECT role_key, location_id, target FROM sqb_role_assignments WHERE principal_id=$1", vec![Parameter::Id(principal)]).await,
                serde_json::from_str::<Value>(test_case.expected_roles).expect("expected roles"),
                "{}: exactly the installation-wide viewer role",
                test_case.description
            );
            let write = |origin: Option<&str>| {
                let request = forwarded(
                    &client,
                    reqwest::Method::POST,
                    format!("{}/api/control/runs", api.url),
                )
                .header(USER_HEADER, "orders-viewer")
                .json(&json!({}));
                origin.into_iter().fold(request, |request, origin| {
                    request.header(header::ORIGIN, origin)
                })
            };
            let mut write_statuses = Vec::new();
            for origin in [
                Some("http://other.example.com"),
                Some("https://control.example.com"),
                None,
                Some(ORIGIN),
            ] {
                write_statuses.push(write(origin).send().await.expect("write").status());
            }
            assert_eq!(
                write_statuses, test_case.expected_write_statuses,
                "{}: only the configured http origin may write",
                test_case.description
            );
            let feed = forwarded(
                &client,
                reqwest::Method::GET,
                format!(
                    "{}/api/control/locations/{}/targets/development/changes",
                    api.url, roles.location_id
                ),
            )
            .header(USER_HEADER, "orders-viewer")
            .header(header::ACCEPT, "text/event-stream")
            .send()
            .await
            .expect("change feed");
            let header_text = |name: &str| {
                feed.headers()
                    .get(name)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned)
            };
            assert_eq!(
                (
                    feed.status(),
                    ["cache-control", "x-accel-buffering", "content-type"].map(header_text),
                ),
                (
                    StatusCode::OK,
                    test_case
                        .expected_feed_headers
                        .map(|value| Some(value.to_owned())),
                ),
                "{}: a proxy neither caches, compresses nor buffers the feed",
                test_case.description
            );
            drop(feed);
            stop_api(rejecting).await;
            stop_api(untrusted).await;
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
