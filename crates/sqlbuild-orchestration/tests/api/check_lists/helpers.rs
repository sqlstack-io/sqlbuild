//! Check results written as a location's sync writes them, and the check list and change stream read as a page reads them.

use std::time::Duration;

use reqwest::{Client, StatusCode, header};
use serde_json::Value;
use uuid::Uuid;

use crate::api::tests::warehouse_history_helpers::{ORIGIN, batch, fixture};
use crate::store::models::ControlStore;
use crate::warehouse_metadata::main::ingest::ingest_audit_results::ingest_audit_results;

/// A list read: its status, ETag and the `(run_id, outcome)` of each check.
pub(crate) type ListRead = (StatusCode, Option<String>, Vec<(String, String)>);

/// One result of the fixture check, from run `run_id` with `outcome` at `occurred_at`; panics unless it is new.
pub(crate) async fn ingest_result(
    store: &ControlStore,
    (location, session_id): (Uuid, Uuid),
    (result_id, run_id, occurred_at, outcome): (&str, &str, &str, &str),
) {
    let mut record = fixture("audit_result_history.json")["record"].clone();
    record["result_id"] = result_id.into();
    record["run_id"] = run_id.into();
    record["invocation_id"] = run_id.into();
    record["occurred_at"] = occurred_at.into();
    record["outcome"] = outcome.into();
    let applied = ingest_audit_results(
        store,
        (location, "development"),
        &batch(session_id, "audit_results", vec![record]),
    )
    .await
    .expect("ingest a check result")
    .applied;
    assert!(applied, "{result_id} is a new result");
}

/// The check list, revalidating `tag` when given.
pub(crate) async fn read_checks(
    client: &Client,
    (url, cookie, location): (&str, &str, Uuid),
    tag: Option<&str>,
) -> ListRead {
    let request = client
        .get(format!(
            "{url}/api/control/locations/{location}/targets/development/checks"
        ))
        .header(header::HOST, "control.example.com")
        .header(header::ORIGIN, ORIGIN)
        .header(header::COOKIE, cookie);
    let response = tag
        .into_iter()
        .fold(request, |request, tag| {
            request.header(header::IF_NONE_MATCH, tag)
        })
        .send()
        .await
        .expect("check list read");
    let status = response.status();
    let etag = response
        .headers()
        .get(header::ETAG)
        .map(|tag| tag.to_str().expect("ASCII ETag").to_owned());
    let list: Value = response.json().await.unwrap_or(Value::Null);
    (status, etag, runs_listed(&list))
}

/// Revalidates `tag` until the list changes, for up to five seconds; the last read either way.
pub(crate) async fn read_changed_checks(
    client: &Client,
    scope: (&str, &str, Uuid),
    tag: &str,
) -> ListRead {
    for _ in 0..50 {
        let read = read_checks(client, scope, Some(tag)).await;
        let StatusCode::NOT_MODIFIED = read.0 else {
            return read;
        };
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    read_checks(client, scope, Some(tag)).await
}

/// The target's change stream, as every open page holds one, once its first notice has arrived; the target stays watched while it is open.
pub(crate) async fn watch_changes(
    client: &Client,
    (url, cookie, location): (&str, &str, Uuid),
) -> reqwest::Response {
    let mut feed = client
        .get(format!(
            "{url}/api/control/locations/{location}/targets/development/changes"
        ))
        .header(header::HOST, "control.example.com")
        .header(header::ORIGIN, ORIGIN)
        .header(header::COOKIE, cookie)
        .header(header::ACCEPT, "text/event-stream")
        .send()
        .await
        .expect("change stream");
    assert_eq!(feed.status(), StatusCode::OK, "change stream opens");
    feed.chunk().await.expect("first notice");
    feed
}

fn runs_listed(list: &Value) -> Vec<(String, String)> {
    list.as_array()
        .map(|checks| {
            checks
                .iter()
                .map(|check| {
                    (
                        check["run_id"].as_str().unwrap_or_default().to_owned(),
                        check["outcome"].as_str().unwrap_or_default().to_owned(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}
