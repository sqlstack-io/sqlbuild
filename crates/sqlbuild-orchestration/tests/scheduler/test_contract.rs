use crate::helpers::leadership_contract;
use crate::store_helpers::run;
use crate::store_test_types::Backend;
use crate::test_types::LeadershipTestCase;

#[test]
fn given_two_schedulers_when_leadership_expires_then_only_the_new_epoch_can_renew() {
    let test_cases = [
        LeadershipTestCase {
            description: "SQLite contenders and takeover",
            backend: Backend::Sqlite,
            expected_epoch: 2,
        },
        LeadershipTestCase {
            description: "PostgreSQL contenders and takeover",
            backend: Backend::Postgres,
            expected_epoch: 2,
        },
    ];
    for test_case in test_cases {
        let observed = run(leadership_contract(test_case.backend));
        assert!(
            observed.is_none_or(|epoch| epoch == test_case.expected_epoch),
            "{}",
            test_case.description
        );
    }
}
