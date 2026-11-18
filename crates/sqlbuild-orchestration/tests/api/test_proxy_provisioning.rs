//! A proxy that rewrites `Host` over plain HTTP signs people in, provisioning newcomers as viewers.

use axum::body::Body;
use reqwest::{Client, StatusCode, header};
use serde_json::{Value, json};

use crate::api::models::{HumanLogin, TrustedProxyAuthentication, UnknownProxyUsers};
use crate::api::tests::helpers::stop_api;
use crate::api::tests::proxy_provisioning_helpers::{
    LOGOUT_URL, NAME_HEADER, ORIGIN, PUBLIC_HOST, USER_HEADER, forwarded, proxy_login, rows,
    send_from_peer, settings, start,
};
use crate::api::tests::test_types::{
    ProxyPeerTrustTestCase, ProxyPeersDefaultTestCase, ProxyProvisioningTestCase,
};
use crate::credentials::main::identities::create_principal::create_principal;
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
            expected_other_host_read: StatusCode::OK,
            expected_write_statuses: [
                StatusCode::UNAUTHORIZED,
                StatusCode::UNAUTHORIZED,
                StatusCode::UNAUTHORIZED,
                StatusCode::UNPROCESSABLE_ENTITY,
                StatusCode::UNPROCESSABLE_ENTITY,
                StatusCode::UNPROCESSABLE_ENTITY,
            ],
            expected_roles: r#"[{"role_key": "viewer", "location_id": null, "target": null}]"#,
            expected_feed_headers: ["no-cache, no-transform", "no", "text/event-stream"],
        },
        ProxyProvisioningTestCase {
            description: "PostgreSQL proxy provisioning",
            backend: Backend::Postgres,
            expected_refusals: [StatusCode::UNAUTHORIZED; 5],
            expected_other_host_read: StatusCode::OK,
            expected_write_statuses: [
                StatusCode::UNAUTHORIZED,
                StatusCode::UNAUTHORIZED,
                StatusCode::UNAUTHORIZED,
                StatusCode::UNPROCESSABLE_ENTITY,
                StatusCode::UNPROCESSABLE_ENTITY,
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
            create_principal(store, &roles.context, "Orders-Lead")
                .await
                .expect("a principal named in mixed case");
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
            let case_taken_status = bootstrap(&api.url)
                .header(USER_HEADER, "orders-lead")
                .send()
                .await
                .expect("a name another principal holds in another case")
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
                    case_taken_status,
                    taken_status,
                    rejected_status
                ],
                test_case.expected_refusals,
                "{}: untrusted peer, missing username, name taken in another case, taken name, reject mode",
                test_case.description
            );
            assert_eq!(
                rows(store, "SELECT COUNT(*) AS people FROM sqb_principals WHERE name IN ('orders-viewer','orders-newcomer','orders-lead')", vec![]).await,
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
            let other_host_read = bootstrap(&api.url)
                .header("x-forwarded-host", "control-short")
                .header(USER_HEADER, "orders-viewer")
                .send()
                .await
                .expect("the UI under another host name")
                .status();
            assert_eq!(
                other_host_read, test_case.expected_other_host_read,
                "{}: a proxy serves the UI under any host name",
                test_case.description
            );
            let write = |(origin, host, proof): (Option<&str>, &str, Option<&str>)| {
                let request = client
                    .post(format!("{}/api/control/runs", api.url))
                    .header("x-forwarded-host", host)
                    .header(USER_HEADER, "orders-viewer")
                    .json(&json!({}));
                let request = origin.into_iter().fold(request, |request, origin| {
                    request.header(header::ORIGIN, origin)
                });
                proof.into_iter().fold(request, |request, proof| {
                    request.header("x-sqb-csrf", proof)
                })
            };
            let mut write_statuses = Vec::new();
            for attempt in [
                (Some("http://other.example.com"), PUBLIC_HOST, None),
                (Some("https://control.example.com"), PUBLIC_HOST, None),
                (None, PUBLIC_HOST, None),
                (Some(ORIGIN), PUBLIC_HOST, None),
                (
                    Some("http://control-short"),
                    "control-short",
                    Some("trusted-proxy"),
                ),
                (None, "control-short", Some("trusted-proxy")),
            ] {
                write_statuses.push(write(attempt).send().await.expect("write").status());
            }
            assert_eq!(
                write_statuses, test_case.expected_write_statuses,
                "{}: a cross-site form post, the https twin and an unproven write are refused; the origin or the CSRF header writes",
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

#[test]
fn given_a_trusted_proxy_without_trusted_peers_when_configured_then_both_loopback_addresses_are_trusted()
 {
    let test_cases = [ProxyPeersDefaultTestCase {
        description: "a proxy on the same host connects over either loopback address",
        configuration: json!({"origin": ORIGIN}),
        expected_peers: &["127.0.0.1", "::1"],
    }];
    for test_case in test_cases {
        let settings: TrustedProxyAuthentication =
            serde_json::from_value(test_case.configuration).expect("proxy settings");

        let peers: Vec<String> = settings
            .trusted_peers
            .iter()
            .map(ToString::to_string)
            .collect();

        assert_eq!(peers, test_case.expected_peers, "{}", test_case.description);
    }
}

#[test]
fn given_the_default_proxy_peers_when_an_identity_header_arrives_then_only_a_loopback_tcp_peer_signs_in()
 {
    let test_cases = [
        ProxyPeerTrustTestCase {
            description: "IPv4 loopback peer, the default proxy on the same host",
            login: proxy_login(&[]),
            peer: "127.0.0.1",
            forwarding_headers: &[],
            username: "orders-ipv4-loopback",
            expected_status: StatusCode::OK,
            expected_provisioned: true,
        },
        ProxyPeerTrustTestCase {
            description: "IPv6 loopback peer, the default proxy on the same host",
            login: proxy_login(&[]),
            peer: "::1",
            forwarding_headers: &[],
            username: "orders-ipv6-loopback",
            expected_status: StatusCode::OK,
            expected_provisioned: true,
        },
        ProxyPeerTrustTestCase {
            description: "IPv4-mapped loopback peer from a dual-stack listener",
            login: proxy_login(&[]),
            peer: "::ffff:127.0.0.1",
            forwarding_headers: &[],
            username: "orders-mapped-loopback",
            expected_status: StatusCode::OK,
            expected_provisioned: true,
        },
        ProxyPeerTrustTestCase {
            description: "IPv4 non-loopback peer not in trusted_peers",
            login: proxy_login(&[]),
            peer: "192.0.2.24",
            forwarding_headers: &[],
            username: "orders-ipv4-remote",
            expected_status: StatusCode::UNAUTHORIZED,
            expected_provisioned: false,
        },
        ProxyPeerTrustTestCase {
            description: "IPv6 non-loopback peer not in trusted_peers",
            login: proxy_login(&[]),
            peer: "2001:db8::24",
            forwarding_headers: &[],
            username: "orders-ipv6-remote",
            expected_status: StatusCode::UNAUTHORIZED,
            expected_provisioned: false,
        },
        ProxyPeerTrustTestCase {
            description: "IPv4-mapped non-loopback peer from a dual-stack listener",
            login: proxy_login(&[]),
            peer: "::ffff:192.0.2.24",
            forwarding_headers: &[],
            username: "orders-mapped-remote",
            expected_status: StatusCode::UNAUTHORIZED,
            expected_provisioned: false,
        },
        ProxyPeerTrustTestCase {
            description: "IPv4 non-loopback peer claiming a loopback client in forwarding headers",
            login: proxy_login(&[]),
            peer: "192.0.2.24",
            forwarding_headers: &[
                ("x-forwarded-for", "127.0.0.1"),
                ("x-real-ip", "127.0.0.1"),
                ("forwarded", "for=127.0.0.1"),
            ],
            username: "orders-ipv4-forwarded",
            expected_status: StatusCode::UNAUTHORIZED,
            expected_provisioned: false,
        },
        ProxyPeerTrustTestCase {
            description: "IPv6 non-loopback peer claiming a loopback client in forwarding headers",
            login: proxy_login(&[]),
            peer: "2001:db8::24",
            forwarding_headers: &[
                ("x-forwarded-for", "::1"),
                ("x-real-ip", "::1"),
                ("forwarded", "for=\"[::1]\""),
            ],
            username: "orders-ipv6-forwarded",
            expected_status: StatusCode::UNAUTHORIZED,
            expected_provisioned: false,
        },
        ProxyPeerTrustTestCase {
            description: "password sign-in without [trusted_proxy], header from IPv4 loopback",
            login: HumanLogin::Password(format!("https://{PUBLIC_HOST}")),
            peer: "127.0.0.1",
            forwarding_headers: &[],
            username: "orders-password-ipv4",
            expected_status: StatusCode::UNAUTHORIZED,
            expected_provisioned: false,
        },
        ProxyPeerTrustTestCase {
            description: "password sign-in without [trusted_proxy], header from IPv6 loopback",
            login: HumanLogin::Password(format!("https://{PUBLIC_HOST}")),
            peer: "::1",
            forwarding_headers: &[],
            username: "orders-password-ipv6",
            expected_status: StatusCode::UNAUTHORIZED,
            expected_provisioned: false,
        },
    ];
    run(async {
        let Some(fixture) = open_test_store(Backend::Sqlite).await else {
            return;
        };
        let store = &fixture.store;
        migrate_store(store).await.expect("migrate");
        for test_case in test_cases {
            let request = axum::http::Request::get("/api/control/bootstrap")
                .header(header::HOST, PUBLIC_HOST)
                .header("x-forwarded-host", PUBLIC_HOST)
                .header(USER_HEADER, test_case.username);
            let request = test_case
                .forwarding_headers
                .iter()
                .fold(request, |request, (name, value)| {
                    request.header(*name, *value)
                });
            let request = request.body(Body::empty()).expect("request");

            let response = send_from_peer(store, test_case.login, (test_case.peer, request)).await;

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
