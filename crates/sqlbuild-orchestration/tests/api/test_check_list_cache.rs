//! The check list is held per target and tagged by its snapshot and content: a new result replaces it, an unchanged rebuild revalidates.

use std::time::Duration;

use reqwest::StatusCode;

use crate::api::tests::check_list_helpers::{
    ingest_result, read_changed_checks, read_checks, watch_changes,
};
use crate::api::tests::helpers::stop_api;
use crate::api::tests::test_types::{CheckListRebuildTestCase, CheckListRerunTestCase};
use crate::api::tests::warehouse_history_helpers::{login, seed_target, start_password_api};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const FIRST_RUN: (&str, &str, &str, &str) = ("result-1", "run-1", "2026-02-03T04:05:06Z", "pass");

#[test]
fn given_a_held_check_list_when_a_rerun_passes_again_then_the_newer_result_is_served_under_a_new_tag()
 {
    let test_cases = [
        CheckListRerunTestCase {
            description: "SQLite check list",
            backend: Backend::Sqlite,
            expected_before: ("run-1", "pass"),
            expected_after: ("run-2", "pass"),
            expected_revalidated: 304,
        },
        CheckListRerunTestCase {
            description: "PostgreSQL check list",
            backend: Backend::Postgres,
            expected_before: ("run-1", "pass"),
            expected_after: ("run-2", "pass"),
            expected_revalidated: 304,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture_store) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture_store.store;
            let (location, session_id) = seed_target(store).await;
            ingest_result(store, (location, session_id), FIRST_RUN).await;
            let (api, client) = start_password_api(store).await;
            let viewer = login(&client, &api.url, "orders-viewer").await;
            let scope = (api.url.as_str(), viewer.as_str(), location);
            // A page holds the target's change stream open, which keeps its views held across signals.
            let feed = watch_changes(&client, scope).await;

            let (status, first_tag, first) = read_checks(&client, scope, None).await;
            let first_tag = first_tag.expect("the list is tagged");
            let (unchanged, unchanged_tag, _) = read_checks(&client, scope, Some(&first_tag)).await;
            // A re-run whose check passes again: same outcome, so no node's check counts change.
            ingest_result(
                store,
                (location, session_id),
                ("result-2", "run-2", "2026-02-04T04:05:06Z", "pass"),
            )
            .await;
            let (status_after, rerun_tag, rerun) =
                read_changed_checks(&client, scope, &first_tag).await;
            let rerun_tag = rerun_tag.expect("the new list is tagged");
            let (revalidated, _, _) = read_checks(&client, scope, Some(&rerun_tag)).await;
            let expected =
                |(run_id, outcome): (&str, &str)| vec![(run_id.to_owned(), outcome.to_owned())];
            assert_eq!(
                (
                    (status, first, unchanged, unchanged_tag),
                    (status_after, rerun, rerun_tag != first_tag),
                    revalidated.as_u16()
                ),
                (
                    (
                        StatusCode::OK,
                        expected(test_case.expected_before),
                        StatusCode::NOT_MODIFIED,
                        Some(first_tag.clone())
                    ),
                    (StatusCode::OK, expected(test_case.expected_after), true),
                    test_case.expected_revalidated
                ),
                "{}: the first result and a 304 for its tag, then the re-run's result under a new tag",
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
    let test_cases = [
        CheckListRebuildTestCase {
            description: "SQLite check list",
            backend: Backend::Sqlite,
            expected_listed: ("run-1", "pass"),
            expected_revalidated: 304,
        },
        CheckListRebuildTestCase {
            description: "PostgreSQL check list",
            backend: Backend::Postgres,
            expected_listed: ("run-1", "pass"),
            expected_revalidated: 304,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture_store) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture_store.store;
            let (location, session_id) = seed_target(store).await;
            ingest_result(store, (location, session_id), FIRST_RUN).await;
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
                (rebuilt.as_u16(), late.as_u16(), status, fresh_tag, list),
                (
                    test_case.expected_revalidated,
                    test_case.expected_revalidated,
                    StatusCode::OK,
                    Some(tag.clone()),
                    vec![(
                        test_case.expected_listed.0.to_owned(),
                        test_case.expected_listed.1.to_owned()
                    )]
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
