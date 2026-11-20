//! The check list is held per target and tagged by its snapshot and content: a new result replaces it, an unchanged rebuild revalidates.

use std::time::Duration;

use reqwest::{Client, StatusCode, header};
use serde_json::Value;
use uuid::Uuid;

use crate::api::tests::helpers::stop_api;
use crate::api::tests::test_types::CheckListCacheTestCase;
use crate::api::tests::warehouse_history_helpers::{
    ORIGIN, batch, fixture, login, seed_target, start_password_api,
};
use crate::store::models::ControlStore;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_metadata::main::ingest::ingest_audit_results::ingest_audit_results;

const CASES: [CheckListCacheTestCase; 2] = [
    CheckListCacheTestCase {
        description: "SQLite check list",
        backend: Backend::Sqlite,
    },
    CheckListCacheTestCase {
        description: "PostgreSQL check list",
        backend: Backend::Postgres,
    },
];

/// One result of the fixture check, as a run that `outcome`d at `occurred_at`.
async fn ingest_result(
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

/// The list's status, ETag and body, revalidating `tag` when given.
async fn read_checks(
    client: &Client,
    (url, cookie, location): (&str, &str, Uuid),
    tag: Option<&str>,
) -> (StatusCode, Option<String>, Value) {
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
    (status, etag, response.json().await.unwrap_or(Value::Null))
}

/// The target's change stream, as every open page holds one, once its first notice has arrived; the target stays watched while it is open.
async fn watch_changes(
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

#[test]
fn given_a_held_check_list_when_a_rerun_passes_again_then_the_newer_result_is_served_under_a_new_tag()
 {
    for test_case in CASES {
        run(async {
            let Some(fixture_store) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture_store.store;
            let (location, session_id) = seed_target(store).await;
            ingest_result(
                store,
                (location, session_id),
                ("result-1", "run-1", "2026-02-03T04:05:06Z", "pass"),
            )
            .await;
            let (api, client) = start_password_api(store).await;
            let viewer = login(&client, &api.url, "orders-viewer").await;
            let scope = (api.url.as_str(), viewer.as_str(), location);
            let feed = watch_changes(&client, scope).await;

            let (status, first_tag, first) = read_checks(&client, scope, None).await;
            let first_tag = first_tag.expect("the list is tagged");
            let (unchanged, unchanged_tag, _) = read_checks(&client, scope, Some(&first_tag)).await;
            assert_eq!(
                (status, runs_listed(&first), unchanged, unchanged_tag),
                (
                    StatusCode::OK,
                    vec![("run-1".to_owned(), "pass".to_owned())],
                    StatusCode::NOT_MODIFIED,
                    Some(first_tag.clone())
                ),
                "{}: the first result, then a 304 for its tag",
                test_case.description
            );

            // A re-run whose check passes again: same outcome, so no node's check counts change.
            ingest_result(
                store,
                (location, session_id),
                ("result-2", "run-2", "2026-02-04T04:05:06Z", "pass"),
            )
            .await;
            let mut served = (StatusCode::NOT_MODIFIED, None, Value::Null);
            for _ in 0..50 {
                served = read_checks(&client, scope, Some(&first_tag)).await;
                if served.0 != StatusCode::NOT_MODIFIED {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let (status, rerun_tag, rerun) = served;
            assert_eq!(
                (status, runs_listed(&rerun)),
                (
                    StatusCode::OK,
                    vec![("run-2".to_owned(), "pass".to_owned())]
                ),
                "{}: the re-run's result replaces the held list",
                test_case.description
            );
            let rerun_tag = rerun_tag.expect("the new list is tagged");
            assert_ne!(rerun_tag, first_tag, "{}", test_case.description);
            let (revalidated, _, _) = read_checks(&client, scope, Some(&rerun_tag)).await;
            assert_eq!(
                revalidated,
                StatusCode::NOT_MODIFIED,
                "{}",
                test_case.description
            );
            drop(feed);
            stop_api(api).await;
            close_test_store(fixture_store).await;
        });
    }
}

#[test]
fn given_a_check_list_tag_when_the_list_is_rebuilt_unchanged_then_it_still_revalidates() {
    for test_case in CASES {
        run(async {
            let Some(fixture_store) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture_store.store;
            let (location, session_id) = seed_target(store).await;
            ingest_result(
                store,
                (location, session_id),
                ("result-1", "run-1", "2026-02-03T04:05:06Z", "pass"),
            )
            .await;
            let (api, client) = start_password_api(store).await;
            let viewer = login(&client, &api.url, "orders-viewer").await;
            let (_, tag, _) = read_checks(&client, (&api.url, &viewer, location), None).await;
            let tag = tag.expect("the list is tagged");
            stop_api(api).await;

            // Another server holds nothing, so it builds the list afresh; the same results give the same tag.
            let (api, client) = start_password_api(store).await;
            let viewer = login(&client, &api.url, "orders-viewer").await;
            let scope = (api.url.as_str(), viewer.as_str(), location);
            let feed = watch_changes(&client, scope).await;
            let (rebuilt, _, _) = read_checks(&client, scope, Some(&tag)).await;
            // An older result arriving late signals a change but leaves every check's newest result as it was.
            ingest_result(
                store,
                (location, session_id),
                ("result-0", "run-0", "2026-02-02T04:05:06Z", "error"),
            )
            .await;
            tokio::time::sleep(Duration::from_millis(500)).await;
            let (late, _, _) = read_checks(&client, scope, Some(&tag)).await;
            let (status, fresh_tag, list) = read_checks(&client, scope, None).await;
            assert_eq!(
                (rebuilt, late, status, fresh_tag, runs_listed(&list)),
                (
                    StatusCode::NOT_MODIFIED,
                    StatusCode::NOT_MODIFIED,
                    StatusCode::OK,
                    Some(tag.clone()),
                    vec![("run-1".to_owned(), "pass".to_owned())]
                ),
                "{}: a rebuild with the same results keeps the tag",
                test_case.description
            );
            drop(feed);
            stop_api(api).await;
            close_test_store(fixture_store).await;
        });
    }
}
