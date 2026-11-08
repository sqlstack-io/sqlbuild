use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::http::HeaderMap;
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use reqwest::StatusCode;
use uuid::Uuid;

use crate::api::constants::BODY_BYTES;
use crate::api::tests::helpers::{
    gzip_bomb, gzipped_empty_object, large_snapshot, not_gzip, oversized_json, start_api, stop_api,
};
use crate::api::tests::test_types::{OlderServerTestCase, PublishEncodingTestCase};
use crate::credentials::main::tokens::issue_token::issue_token;
use crate::credentials::models::{AuditContext, CredentialScope};
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_remote_snapshot::publish_remote_snapshot;
use crate::snapshots::models::{SnapshotInput, SnapshotRecord};
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{open_test_store, run};
use crate::store::tests::store_test_types::Backend;

/// Decoding happens before the store is touched, so one backend covers every encoding case.
#[test]
fn given_encoded_snapshot_bodies_when_publishing_then_gzip_is_bounded_after_authentication() {
    let test_cases = [
        PublishEncodingTestCase {
            description: "an unauthenticated gzip bomb is refused before it is decompressed",
            forged_bearer: Some("unknown.credential"),
            encoding: "gzip",
            body: gzip_bomb,
            expected_status: StatusCode::UNAUTHORIZED,
        },
        PublishEncodingTestCase {
            description: "a gzip body that decompresses past the bound is refused",
            forged_bearer: None,
            encoding: "gzip",
            body: gzip_bomb,
            expected_status: StatusCode::PAYLOAD_TOO_LARGE,
        },
        PublishEncodingTestCase {
            description: "a plain body over the wire bound is refused",
            forged_bearer: None,
            encoding: "identity",
            body: oversized_json,
            expected_status: StatusCode::PAYLOAD_TOO_LARGE,
        },
        PublishEncodingTestCase {
            description: "an encoding other than gzip is refused",
            forged_bearer: None,
            encoding: "br",
            body: gzipped_empty_object,
            expected_status: StatusCode::UNSUPPORTED_MEDIA_TYPE,
        },
        PublishEncodingTestCase {
            description: "a body declared gzip that is not gzip is refused",
            forged_bearer: None,
            encoding: "gzip",
            body: not_gzip,
            expected_status: StatusCode::BAD_REQUEST,
        },
    ];
    run(async {
        let Some(fixture) = open_test_store(Backend::Sqlite).await else {
            return;
        };
        let store = &fixture.store;
        migrate_store(store).await.expect("migrate");
        let location_id = create_location(store, "orders", "test-admin")
            .await
            .expect("location");
        let audit = AuditContext {
            actor: "test-admin".into(),
            granting_role: "admin".into(),
            request_id: Uuid::now_v7(),
            ip_address: None,
        };
        let token = issue_token(
            store,
            &audit,
            CredentialScope::Location { location_id },
            Utc::now() + chrono::Duration::hours(1),
        )
        .await
        .expect("location credential");
        let api = start_api(store).await;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("HTTP client");
        let path = format!(
            "{}/api/locations/{location_id}/targets/development/snapshots",
            api.url
        );
        for test_case in &test_cases {
            let bearer: &str = test_case.forged_bearer.unwrap_or(&token.bearer);
            let status = client
                .post(&path)
                .header("Content-Type", "application/json")
                .header("Content-Encoding", test_case.encoding)
                .bearer_auth(bearer)
                .body((test_case.body)())
                .send()
                .await
                .expect("a response, not a reset connection")
                .status();
            assert_eq!(
                status, test_case.expected_status,
                "{}",
                test_case.description
            );
        }
        let input = large_snapshot(location_id);
        assert!(
            serde_json::to_vec(&input).expect("JSON").len() > BODY_BYTES,
            "the snapshot only fits the wire bound gzip-encoded"
        );
        let published = publish_remote_snapshot(&api.url, &token.bearer, &input)
            .await
            .expect("publish a 10k-node snapshot");
        assert_eq!(published.generation, 1);
        let current: serde_json::Value = client
            .get(&path)
            .bearer_auth(&token.bearer)
            .send()
            .await
            .expect("current snapshot")
            .json()
            .await
            .expect("snapshot JSON");
        assert_eq!(
            current["nodes"].as_object().map(serde_json::Map::len),
            Some(10_282)
        );
        stop_api(api).await;
    });
}

#[test]
fn given_a_server_that_refuses_gzip_when_publishing_then_the_snapshot_is_resent_as_plain_json() {
    let test_cases = [
        OlderServerTestCase {
            description: "a server that predates gzip rejects the encoded body as malformed JSON",
            refusal: StatusCode::BAD_REQUEST,
            expected_requests: 2,
        },
        OlderServerTestCase {
            description: "a server that refuses the encoding",
            refusal: StatusCode::UNSUPPORTED_MEDIA_TYPE,
            expected_requests: 2,
        },
    ];
    for test_case in test_cases {
        run(async {
            let requests = Arc::new(AtomicUsize::new(0));
            let seen = Arc::clone(&requests);
            let refusal = test_case.refusal;
            let router = Router::new().route(
                "/api/locations/{location_id}/targets/{target}/snapshots",
                post(move |headers: HeaderMap, body: axum::body::Bytes| {
                    let seen = Arc::clone(&seen);
                    async move {
                        seen.fetch_add(1, Ordering::SeqCst);
                        headers
                            .get("content-encoding")
                            .map_or(Ok(()), |_| Err(refusal))?;
                        let input: SnapshotInput =
                            serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
                        Ok::<_, StatusCode>(Json(SnapshotRecord {
                            snapshot_id: Uuid::nil(),
                            location_id: input.location_id,
                            target: input.target,
                            generation: input.generation,
                            manifest_digest: "older-server".into(),
                            published_at: Utc::now(),
                        }))
                    }
                }),
            );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("older server socket");
            let url = format!("http://{}", listener.local_addr().expect("address"));
            let server = tokio::spawn(async move { axum::serve(listener, router).await });
            let location_id = Uuid::now_v7();
            let published = publish_remote_snapshot(&url, "token.secret", &snapshot(location_id))
                .await
                .expect("plain JSON publish to an older server");
            assert_eq!(
                published.manifest_digest, "older-server",
                "{}",
                test_case.description
            );
            assert_eq!(
                published.location_id, location_id,
                "{}",
                test_case.description
            );
            assert_eq!(
                requests.load(Ordering::SeqCst),
                test_case.expected_requests,
                "{}",
                test_case.description
            );
            server.abort();
        });
    }
}
