//! Crash points must not separate a tick from its queued work and durable cursor.

use crate::scheduler::tests::contracts::ticks::helpers::transaction_contract;
use crate::scheduler::tests::contracts::ticks::test_types::TickTransactionTestCase;
use crate::store::tests::store_helpers::run;
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_two_schedulers_and_faults_between_writes_when_restarting_then_ticks_runs_recall_and_cursor_commit_together()
 {
    let test_cases = [
        TickTransactionTestCase {
            description: "SQLite fenced tick transaction",
            backend: Backend::Sqlite,
            expected_ticks: 3,
            expected_runs: 1,
        },
        TickTransactionTestCase {
            description: "PostgreSQL fenced tick transaction",
            backend: Backend::Postgres,
            expected_ticks: 3,
            expected_runs: 1,
        },
    ];
    for test_case in test_cases {
        let observed = run(transaction_contract(test_case.backend));
        assert!(
            observed
                .is_none_or(|counts| counts == (test_case.expected_ticks, test_case.expected_runs)),
            "{}",
            test_case.description
        );
    }
}
