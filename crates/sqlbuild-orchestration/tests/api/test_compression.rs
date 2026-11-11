//! People's reads are compressed; sign-in, writes, the change feed and the location API are not.

use reqwest::header;

use crate::api::models::{ApiAuthentication, HumanLogin};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::test_types::CompressionTestCase;
use crate::credentials::tests::role_helpers::role_fixture;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_signed_in_browser_when_requesting_then_only_people_reads_are_compressed() {
    let test_cases = [
        CompressionTestCase {
            description: "a person's read",
            method: reqwest::Method::GET,
            path: "/api/control/locations/{location}/targets/development/nodes",
            accept_encoding: "gzip",
            expected_status: 200,
            expected_gzip: true,
        },
        CompressionTestCase {
            description: "a client that does not accept gzip",
            method: reqwest::Method::GET,
            path: "/api/control/locations/{location}/targets/development/nodes",
            accept_encoding: "identity",
            expected_status: 200,
            expected_gzip: false,
        },
        CompressionTestCase {
            description: "the sign-in session, which carries the CSRF token",
            method: reqwest::Method::GET,
            path: "/api/auth/session",
            accept_encoding: "gzip",
            expected_status: 200,
            expected_gzip: false,
        },
        CompressionTestCase {
            description: "the change feed, whose events must not wait in an encoder",
            method: reqwest::Method::GET,
            path: "/api/control/locations/{location}/targets/development/changes",
            accept_encoding: "gzip",
            expected_status: 200,
            expected_gzip: false,
        },
        CompressionTestCase {
            description: "a write",
            method: reqwest::Method::POST,
            path: "/api/control/locations/{location}/targets/development/plans",
            accept_encoding: "gzip",
            expected_status: 401,
            expected_gzip: false,
        },
        CompressionTestCase {
            description: "the location API",
            method: reqwest::Method::GET,
            path: "/api/locations/{location}/targets/development/metadata/watermark",
            accept_encoding: "gzip",
            expected_status: 401,
            expected_gzip: false,
        },
    ];
    run(async {
        let Some(fixture) = open_test_store(Backend::Sqlite).await else {
            return;
        };
        let store = &fixture.store;
        migrate_store(store).await.expect("migrate");
        let roles = role_fixture(store).await;
        let api = start_api_with_authentication(
            store,
            ApiAuthentication {
                login: HumanLogin::Password("https://control.example.com".into()),
                ..ApiAuthentication::default()
            },
        )
        .await;
        let client = reqwest::Client::new();
        let session = format!("__Host-sqb-session={}", roles.administrator.bearer);
        for test_case in test_cases {
            let path = test_case
                .path
                .replace("{location}", &roles.location_id.to_string());
            // Only headers are read: the change feed's body stays open.
            let response = client
                .request(test_case.method, format!("{}{path}", api.url))
                .header(header::HOST, "control.example.com")
                .header(header::COOKIE, &session)
                .header(header::ACCEPT_ENCODING, test_case.accept_encoding)
                .send()
                .await
                .expect("request");
            let encoding = response.headers().get(header::CONTENT_ENCODING).cloned();
            assert_eq!(
                (
                    response.status().as_u16(),
                    encoding.as_ref().is_some_and(|value| value == "gzip")
                ),
                (test_case.expected_status, test_case.expected_gzip),
                "{}: content-encoding {encoding:?}",
                test_case.description
            );
        }
        stop_api(api).await;
        close_test_store(fixture).await;
    });
}
