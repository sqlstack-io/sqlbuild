//! Idempotency and commit-order paging through the public store API.

use crate::helpers::recall_contract;
use crate::store_helpers::run;
use crate::store_test_types::Backend;
use crate::test_types::RecallTestCase;

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
