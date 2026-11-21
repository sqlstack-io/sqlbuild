//! A newer server in front of the real API: it adds a field to a worker's execution and passes its run events through.

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::Method;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{any, get};
use serde_json::Value;

use crate::api::tests::test_types::{NewerServer as Newer, TestApi};
use crate::store::errors::ControlError;

/// Serve a newer server over `real` that adds `added_by_newer_server` at `added`.
pub(crate) async fn newer_server(real: &str, added: &'static [&'static str]) -> TestApi {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("newer server socket");
    let url = format!(
        "http://{}",
        listener.local_addr().expect("newer server address")
    );
    let application = Router::new()
        .route("/api/runs/{run_id}/execution", get(execution))
        .route("/api/runs/{run_id}/{endpoint}", any(passthrough))
        .with_state(Newer {
            real: real.trim_end_matches('/').to_owned(),
            added,
        });
    let (shutdown, signal) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        axum::serve(listener, application)
            .with_graceful_shutdown(async {
                let _ = signal.await;
            })
            .await
            .map_err(ControlError::from)
    });
    TestApi {
        url,
        shutdown,
        task,
    }
}

/// The real execution, with a field added at the newer server's path.
async fn execution(
    State(newer): State<Newer>,
    Path(run_id): Path<String>,
    headers: HeaderMap,
) -> axum::Json<Value> {
    let mut body: Value = forward(
        &newer,
        reqwest::Method::GET,
        (&run_id, "execution"),
        (headers, Bytes::new()),
    )
    .await
    .1
    .json()
    .await
    .expect("execution JSON");
    let added: Option<Value> = newer.added.split_last().and_then(|(field, parents)| {
        let parent: String = parents.iter().map(|part| format!("/{part}")).collect();
        body.pointer_mut(&parent)?
            .as_object_mut()?
            .insert((*field).to_owned(), Value::Bool(true))
    });
    drop(added);
    axum::Json(body)
}

/// Any other run request, such as an event or a heartbeat, passed to the real API unchanged.
async fn passthrough(
    State(newer): State<Newer>,
    Path((run_id, endpoint)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> (StatusCode, Bytes) {
    let method = reqwest::Method::from_bytes(method.as_str().as_bytes()).expect("method");
    let (status, response) = forward(&newer, method, (&run_id, &endpoint), (headers, body)).await;
    (status, response.bytes().await.expect("real API response"))
}

/// Send the worker's request to the real API with its credential and content type.
async fn forward(
    newer: &Newer,
    method: reqwest::Method,
    (run_id, endpoint): (&str, &str),
    (mut headers, body): (HeaderMap, Bytes),
) -> (StatusCode, reqwest::Response) {
    headers.remove(axum::http::header::HOST);
    headers.remove(axum::http::header::CONTENT_LENGTH);
    let request = reqwest::Client::new()
        .request(
            method,
            format!("{}/api/runs/{run_id}/{endpoint}", newer.real),
        )
        .headers(headers)
        .body(body);
    let response: reqwest::Response = request.send().await.expect("real API");
    (
        StatusCode::from_u16(response.status().as_u16()).expect("status"),
        response,
    )
}

/// The reason a worker gives for a request with `field` it can't read.
pub(crate) fn refusal(field: &str) -> Option<String> {
    Some(format!(
        "this worker runs SQLBuild {} and can't read a run request from a newer server (unknown field `{field}`); upgrade the location",
        env!("CARGO_PKG_VERSION")
    ))
}
