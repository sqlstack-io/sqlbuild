use crate::bindings::_helpers::cli::failure_cause::take_failure_cause;
use crate::bindings::tests::helpers::report_elsewhere;
use crate::bindings::tests::helpers::report_here;
use crate::bindings::tests::test_types::FailureCauseTestCase;

#[test]
fn given_a_reported_cause_when_taken_then_only_the_reporting_thread_sees_it() {
    let test_cases = [
        FailureCauseTestCase {
            description: "a cause reported by this invocation is taken once",
            report: report_here,
            expected_code: Some("C980"),
        },
        FailureCauseTestCase {
            description: "a concurrent invocation's cause is never taken",
            report: report_elsewhere,
            expected_code: None,
        },
    ];

    for test_case in test_cases {
        drop(take_failure_cause());
        (test_case.report)();

        let taken: Option<String> = take_failure_cause().map(|cause| cause.error_code);

        assert_eq!(
            (taken.as_deref(), take_failure_cause()),
            (test_case.expected_code, None),
            "{}",
            test_case.description
        );
    }
}
