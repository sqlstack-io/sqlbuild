//! A trusted proxy peer is served only under its own host names, so a DNS-rebinding page is refused.

use axum::body::Body;
use reqwest::StatusCode;
use serde_json::json;

use crate::api::tests::proxy_provisioning_helpers::{
    USER_HEADER, proxy_login, rows, send_from_peer,
};
use crate::api::tests::test_types::ProxyHostTestCase;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::Parameter;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const BOOTSTRAP: (&str, &str) = ("GET", "/api/control/bootstrap");
const RUN: (&str, &str) = ("POST", "/api/control/runs");

#[test]
fn given_a_trusted_proxy_peer_when_the_host_header_names_another_site_then_the_request_is_refused()
{
    let test_cases = [
        ProxyHostTestCase {
            description: "a rebinding page reads under its own host name",
            allowed_hosts: &[],
            peer: "127.0.0.1",
            request: BOOTSTRAP,
            headers: &[("host", "evil.example:8000"), ("x-sqb-csrf", "1")],
            username: "orders-rebound-reader",
            expected_status: StatusCode::FORBIDDEN,
            expected_provisioned: false,
        },
        ProxyHostTestCase {
            description: "a rebinding page writes with the CSRF header",
            allowed_hosts: &[],
            peer: "127.0.0.1",
            request: RUN,
            headers: &[("host", "evil.example:8000"), ("x-sqb-csrf", "1")],
            username: "orders-rebound-writer",
            expected_status: StatusCode::FORBIDDEN,
            expected_provisioned: false,
        },
        ProxyHostTestCase {
            description: "a forged X-Forwarded-Host naming the origin",
            allowed_hosts: &[],
            peer: "127.0.0.1",
            request: BOOTSTRAP,
            headers: &[
                ("host", "evil.example:8000"),
                ("x-forwarded-host", "control.example.com"),
                ("x-sqb-csrf", "1"),
            ],
            username: "orders-forwarded-host",
            expected_status: StatusCode::FORBIDDEN,
            expected_provisioned: false,
        },
        ProxyHostTestCase {
            description: "no Host header",
            allowed_hosts: &[],
            peer: "127.0.0.1",
            request: BOOTSTRAP,
            headers: &[("x-sqb-csrf", "1")],
            username: "orders-no-host",
            expected_status: StatusCode::FORBIDDEN,
            expected_provisioned: false,
        },
        ProxyHostTestCase {
            description: "a stock proxy that sends the backend's IPv4 loopback Host",
            allowed_hosts: &[],
            peer: "127.0.0.1",
            request: BOOTSTRAP,
            headers: &[("host", "127.0.0.1:8000"), ("x-sqb-csrf", "1")],
            username: "orders-ipv4-host",
            expected_status: StatusCode::OK,
            expected_provisioned: true,
        },
        ProxyHostTestCase {
            description: "a stock proxy that sends the backend's IPv6 loopback Host",
            allowed_hosts: &[],
            peer: "::1",
            request: BOOTSTRAP,
            headers: &[("host", "[::1]:8000"), ("x-sqb-csrf", "1")],
            username: "orders-ipv6-host",
            expected_status: StatusCode::OK,
            expected_provisioned: true,
        },
        ProxyHostTestCase {
            description: "a stock proxy that sends localhost",
            allowed_hosts: &[],
            peer: "127.0.0.1",
            request: BOOTSTRAP,
            headers: &[("host", "LocalHost:8000"), ("x-sqb-csrf", "1")],
            username: "orders-localhost-host",
            expected_status: StatusCode::OK,
            expected_provisioned: true,
        },
        ProxyHostTestCase {
            description: "a Host-preserving proxy under the origin's host",
            allowed_hosts: &[],
            peer: "127.0.0.1",
            request: BOOTSTRAP,
            headers: &[("host", "control.example.com"), ("x-sqb-csrf", "1")],
            username: "orders-origin-host",
            expected_status: StatusCode::OK,
            expected_provisioned: true,
        },
        ProxyHostTestCase {
            description: "a preserved public Host that allowed_hosts does not list",
            allowed_hosts: &[],
            peer: "127.0.0.1",
            request: BOOTSTRAP,
            headers: &[("host", "control-short:443"), ("x-sqb-csrf", "1")],
            username: "orders-unlisted-host",
            expected_status: StatusCode::FORBIDDEN,
            expected_provisioned: false,
        },
        ProxyHostTestCase {
            description: "a preserved public Host that allowed_hosts lists",
            allowed_hosts: &["Control-Short"],
            peer: "127.0.0.1",
            request: BOOTSTRAP,
            headers: &[("host", "control-short:443"), ("x-sqb-csrf", "1")],
            username: "orders-listed-host",
            expected_status: StatusCode::OK,
            expected_provisioned: true,
        },
    ];
    run(async {
        let Some(fixture) = open_test_store(Backend::Sqlite).await else {
            return;
        };
        let store = &fixture.store;
        migrate_store(store).await.expect("migrate");
        for test_case in test_cases {
            let (method, path) = test_case.request;
            let request = axum::http::Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .header(USER_HEADER, test_case.username);
            let request = test_case
                .headers
                .iter()
                .fold(request, |request, (name, value)| {
                    request.header(*name, *value)
                })
                .body(Body::from("{}"))
                .expect("request");

            let response = send_from_peer(
                store,
                proxy_login(test_case.allowed_hosts),
                (test_case.peer, request),
            )
            .await;

            assert_eq!(
                response.status(),
                test_case.expected_status,
                "{}",
                test_case.description
            );
            assert_eq!(
                rows(
                    store,
                    "SELECT COUNT(*) AS people FROM sqb_principals WHERE name = $1",
                    vec![Parameter::Text(test_case.username.into())],
                )
                .await,
                json!([{"people": i64::from(test_case.expected_provisioned)}]),
                "{}: provisioned",
                test_case.description
            );
        }
        close_test_store(fixture).await;
    });
}
