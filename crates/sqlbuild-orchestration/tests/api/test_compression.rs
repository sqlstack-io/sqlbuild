//! People's reads reach browsers compressed; sign-in responses, writes and the location API never are.

use reqwest::header;

use crate::api::models::{ApiAuthentication, HumanLogin};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::test_types::CompressionTestCase;
use crate::credentials::tests::role_helpers::role_fixture;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_browser_requests_when_served_then_only_people_reads_are_compressed() {
    let test_cases = [
        CompressionTestCase {
            description: "a person's read",
            method: reqwest::Method::GET,
            path: "/api/control/bootstrap",
            accept_gzip: true,
            expected_gzip: true,
        },
        CompressionTestCase {
            description: "a client that does not accept gzip",
            method: reqwest::Method::GET,
            path: "/api/control/bootstrap",
            accept_gzip: false,
            expected_gzip: false,
        },
        CompressionTestCase {
            description: "a sign-in response, which carries the CSRF token",
            method: reqwest::Method::GET,
            path: "/api/auth/session",
            accept_gzip: true,
            expected_gzip: false,
        },
        CompressionTestCase {
            description: "a write",
            method: reqwest::Method::POST,
            path: "/api/control/locations/01a1c31e-4fd5-7bb2-92a7-eabedace0dc2/targets/prod/plans",
            accept_gzip: true,
            expected_gzip: false,
        },
        CompressionTestCase {
            description: "the location API",
            method: reqwest::Method::GET,
            path: "/api/locations/01a1c31e-4fd5-7bb2-92a7-eabedace0dc2/targets/prod/metadata/watermark",
            accept_gzip: true,
            expected_gzip: false,
        },
    ];
    run(async {
        let Some(fixture) = open_test_store(Backend::Sqlite).await else {
            return;
        };
        migrate_store(&fixture.store).await.expect("migrate");
        let api = start_api_with_authentication(
            &fixture.store,
            ApiAuthentication {
                login: HumanLogin::Password("https://control.example.com".into()),
                ..ApiAuthentication::default()
            },
        )
        .await;
        let client = reqwest::Client::new();
        for test_case in test_cases {
            let mut request =
                client.request(test_case.method, format!("{}{}", api.url, test_case.path));
            if test_case.accept_gzip {
                request = request.header(header::ACCEPT_ENCODING, "gzip");
            }
            let response = request.send().await.expect("request");
            let encoding = response.headers().get(header::CONTENT_ENCODING).cloned();
            let length = response.bytes().await.expect("body").len();
            assert!(
                length > 0,
                "{}: the response has a body",
                test_case.description
            );
            assert_eq!(
                encoding.as_ref().is_some_and(|value| value == "gzip"),
                test_case.expected_gzip,
                "{}: content-encoding {encoding:?}",
                test_case.description
            );
        }
        stop_api(api).await;
        close_test_store(fixture).await;
    });
}

#[test]
fn given_a_signed_in_person_when_reading_and_following_changes_then_reads_are_compressed_and_the_feed_streams_plain()
 {
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
        let nodes = client
            .get(format!(
                "{}/api/control/locations/{}/targets/development/nodes",
                api.url, roles.location_id
            ))
            .header(header::HOST, "control.example.com")
            .header(header::COOKIE, &session)
            .header(header::ACCEPT_ENCODING, "gzip")
            .send()
            .await
            .expect("nodes");
        assert_eq!(nodes.status(), reqwest::StatusCode::OK);
        assert_eq!(nodes.headers()[header::CONTENT_ENCODING], "gzip");
        // The feed is a stream of events; compressing it would hold events back in the encoder.
        let feed = client
            .get(format!(
                "{}/api/control/locations/{}/targets/development/changes",
                api.url, roles.location_id
            ))
            .header(header::HOST, "control.example.com")
            .header(header::COOKIE, &session)
            .header(header::ACCEPT_ENCODING, "gzip")
            .send()
            .await
            .expect("feed");
        assert_eq!(feed.status(), reqwest::StatusCode::OK);
        assert_eq!(feed.headers()[header::CONTENT_TYPE], "text/event-stream");
        assert!(!feed.headers().contains_key(header::CONTENT_ENCODING));
        drop(feed);
        stop_api(api).await;
        close_test_store(fixture).await;
    });
}
