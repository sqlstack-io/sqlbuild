use std::time::Duration;

use crate::api::main::serve_api::serve_api;
use crate::api::models::ApiAuthentication;
use crate::api::tests::test_types::TestApi;
use crate::store::models::ControlStore;

pub(crate) async fn start_api(store: &ControlStore) -> TestApi {
    start_api_with_authentication(store, ApiAuthentication::default()).await
}

pub(crate) async fn start_api_with_authentication(
    store: &ControlStore,
    authentication: ApiAuthentication,
) -> TestApi {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("isolated API socket");
    let url = format!("http://{}", listener.local_addr().expect("API address"));
    let (shutdown, signal) = tokio::sync::oneshot::channel();
    let store = store.clone();
    let task = tokio::spawn(serve_api(
        store,
        listener,
        (authentication, Vec::new(), Default::default()),
        (|| {}, async {
            let _ = signal.await;
        }),
    ));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .expect("HTTP client");
    tokio::time::timeout(Duration::from_secs(5), async {
        for _ in 0..250 {
            assert!(!task.is_finished(), "API failed during startup");
            tokio::time::sleep(Duration::from_millis(20)).await;
            let Ok(response) = client
                .get(format!("{url}/api/openapi.json"))
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
            else {
                continue;
            };
            drop(response);
            return;
        }
        panic!("API exhausted readiness attempts");
    })
    .await
    .expect("API becomes ready");
    TestApi {
        url,
        shutdown,
        task,
    }
}

pub(crate) async fn stop_api(api: TestApi) {
    api.shutdown.send(()).expect("request API shutdown");
    tokio::time::timeout(Duration::from_secs(2), api.task)
        .await
        .expect("prompt API shutdown")
        .expect("join API server")
        .expect("API server stopped cleanly");
}

pub(crate) async fn read_json(
    client: &reqwest::Client,
    (url, bearer): (&str, &str),
) -> serde_json::Value {
    let response = client
        .get(url)
        .bearer_auth(bearer)
        .send()
        .await
        .expect("HTTP read");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    response.json().await.expect("typed JSON response")
}

/// Send `GET <path>` byte for byte, so no client normalises dot segments, slashes or encodings.
pub(crate) fn raw_get(address: &str, path: &str) -> (u16, String) {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(address).expect("raw socket");
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .expect("raw request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("raw response");
    let (head, body) = response.split_once("\r\n\r\n").expect("HTTP head");
    let status = head
        .split(' ')
        .nth(1)
        .expect("status")
        .parse()
        .expect("numeric status");
    (status, body.to_owned())
}

/// Gzip members that decompress to a JSON string field longer than the decompressed bound.
pub(crate) fn gzip_bomb() -> Vec<u8> {
    let filler = crate::store::main::compress::compress(&vec![b'a'; 1024 * 1024]).expect("gzip");
    let mut body = crate::store::main::compress::compress(br#"{"padding":""#).expect("gzip");
    for _ in 0..=crate::api::constants::DECOMPRESSED_BODY_BYTES / (1024 * 1024) {
        body.extend_from_slice(&filler);
    }
    body
}

/// Plain JSON one byte longer than the wire bound.
pub(crate) fn oversized_json() -> Vec<u8> {
    let mut body = br#"{"padding":""#.to_vec();
    body.resize(crate::api::constants::BODY_BYTES - 1, b'a');
    body.extend_from_slice(br#""}"#);
    body
}

pub(crate) fn gzipped_empty_object() -> Vec<u8> {
    crate::store::main::compress::compress(b"{}").expect("gzip")
}

pub(crate) fn not_gzip() -> Vec<u8> {
    b"{}".to_vec()
}

/// A DuckShop-10k-sized snapshot whose plain JSON is larger than the wire bound.
pub(crate) fn large_snapshot(location_id: uuid::Uuid) -> crate::snapshots::models::SnapshotInput {
    let mut input = crate::runs::tests::helpers::snapshot(location_id);
    input.nodes = (0..10_282)
        .map(|index| {
            let sql = format!(
                "select {index} as order_id, '{}' as note from orders_{index}",
                "x".repeat(3_400)
            );
            (
                format!("model:orders_{index}"),
                crate::snapshots::models::SnapshotNode {
                    kind: "model".into(),
                    definition: serde_json::json!({"materialization": "table"}),
                    raw_sql: Some(sql.clone()),
                    compiled_sql: Some(sql),
                    python_source: None,
                },
            )
        })
        .collect();
    input
}
