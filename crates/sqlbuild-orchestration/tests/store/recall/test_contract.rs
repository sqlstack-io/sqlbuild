//! Idempotency and commit-order paging through real store transactions.

use crate::store::tests::helpers::recall_contract;
use crate::store::tests::store_helpers::run;
use crate::store::tests::store_test_types::Backend;
use crate::store::tests::test_types::RecallTestCase;

use std::time::Duration;

use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
    store_source_text::store_source_text, subscribe_changes::subscribe_changes,
};
use crate::store::models::{Change, ChangeMessage, ChangeTopic, Parameter, TransactionMode};
use crate::store::tests::store_helpers::{close_test_store, open_test_store};
use crate::store::tests::test_types::CommitCancellationTestCase;

#[test]
fn given_native_recall_facts_when_retried_and_paged_then_content_and_commit_order_are_preserved() {
    let test_cases = [
        RecallTestCase {
            description: "SQLite Recall transactions",
            backend: Backend::Sqlite,
            expected_event_ids: vec!["orders-first", "orders-later-arrival"],
        },
        RecallTestCase {
            description: "PostgreSQL Recall transactions",
            backend: Backend::Postgres,
            expected_event_ids: vec!["orders-first", "orders-later-arrival"],
        },
    ];
    for test_case in test_cases {
        let observed = run(recall_contract(test_case.backend));
        assert!(
            observed.is_none_or(|ids| ids == test_case.expected_event_ids),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_sqlite_commit_cancellation_when_blocking_work_finishes_then_persisted_changes_still_notify()
 {
    let test_cases = [CommitCancellationTestCase {
        description: "cancelled async caller cannot strand committed invalidations",
        expected_body: b"SELECT 7 AS order_id",
    }];
    for test_case in test_cases {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .expect("single blocking worker runtime")
            .block_on(async {
                let fixture = open_test_store(Backend::Sqlite)
                    .await
                    .expect("isolated SQLite");
                let store = &fixture.store;
                migrate_store(store).await.expect("migrate");
                let (sender, _) = subscribe_changes(store).await.expect("change subscription");
                let mut receiver = sender.subscribe();
                let hash = sha256_hex(test_case.expected_body);
                let mut transaction = store_source_text(
                    begin(store, TransactionMode::Write)
                        .await
                        .expect("write transaction"),
                    (hash.clone(), test_case.expected_body.to_vec()),
                )
                .await
                .expect("insert source");
                transaction
                    .changes
                    .insert(Change::NodeStatus(Uuid::now_v7(), "development".into()));

                let (started, started_signal) = tokio::sync::oneshot::channel();
                let (release, wait) = std::sync::mpsc::channel();
                let blocker = tokio::task::spawn_blocking(move || {
                    started.send(()).expect("blocking worker occupied");
                    let _ = wait.recv();
                });
                started_signal.await.expect("worker ready");
                let mut committing = Box::pin(commit(transaction));
                assert!(
                    std::future::poll_fn(|context| std::task::Poll::Ready(
                        committing.as_mut().poll(context).is_pending()
                    ))
                    .await
                );
                drop(committing);
                release.send(()).expect("release queued commit");
                blocker.await.expect("blocking worker released");
                let notice = tokio::time::timeout(Duration::from_secs(3), receiver.recv())
                    .await
                    .expect("committed write still notifies")
                    .expect("live notification channel");
                assert!(matches!(
                    notice,
                    ChangeMessage::Changed {
                        topic: ChangeTopic::NodeStatus,
                        ..
                    }
                ));
                let (transaction, rows) = query(
                    begin(store, TransactionMode::Read)
                        .await
                        .expect("read committed source"),
                    "SELECT body FROM sqb_source_texts WHERE text_hash=$1",
                    vec![Parameter::Text(hash)],
                )
                .await
                .expect("query committed bytes");
                commit(transaction).await.expect("finish read");
                let body: Vec<u8> =
                    serde_json::from_value(rows[0]["body"].clone()).expect("stored bytes");
                assert_eq!(body, test_case.expected_body, "{}", test_case.description);
                close_test_store(fixture).await;
            });
    }
}
