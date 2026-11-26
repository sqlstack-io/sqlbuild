//! A location asks each idle interpreter what its sinks delivered once a minute and reports each interval's change.

use std::time::Duration;

use sqlbuild_orchestration::platform::models::LocationHealth;
use uuid::Uuid;

use crate::native_commands::_helpers::location::health::{report_compiler_sinks, sink_check};
use crate::native_commands::tests::helpers::{
    answering_server, kafka, location_options, scripted_compiler,
};
use crate::native_commands::tests::test_types::{SinkCheckTestCase, SinkReportTestCase};

const TOTALS: &str = "read line; echo '{\"sinks\":[{\"name\":\"kafka\",\"accepted\":4,\"delivered\":4,\"dropped\":0,\"failed\":0}]}'; \
    read line; echo '{\"sinks\":[{\"name\":\"kafka\",\"accepted\":120,\"delivered\":4,\"dropped\":0,\"failed\":116}]}'; exec sleep 600";

#[test]
fn given_an_interpreters_sink_totals_when_checked_then_each_due_check_reports_the_change_since_the_last()
 {
    let test_cases = [SinkCheckTestCase {
        description: "a clean minute, a minute that failed every event, then a check that isn't due",
        script: TOTALS,
        minutes_passed: vec![1, 1, 0],
        expected_reports: vec![Some(kafka((4, 4, 0))), Some(kafka((116, 0, 116))), None],
    }];
    for test_case in test_cases {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let mut compiler = runtime.block_on(async { scripted_compiler(test_case.script) });
        let mut reports: Vec<Option<LocationHealth>> = Vec::new();
        for minutes in &test_case.minutes_passed {
            compiler.sinks.checked_at = compiler
                .sinks
                .checked_at
                .map(|checked| checked - Duration::from_secs(61 * minutes));
            let (checked, report) = runtime.block_on(sink_check(compiler));
            compiler = checked;
            reports.push(report.expect("sink check"));
        }
        assert_eq!(
            reports, test_case.expected_reports,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_an_idle_interpreter_when_its_sinks_are_due_then_the_location_reports_them_or_restarts_a_silent_one()
 {
    let test_cases = [
        SinkReportTestCase {
            description: "an interpreter that answers is kept and its interval reported",
            script: TOTALS,
            expected_outcome: (true, 1),
        },
        SinkReportTestCase {
            description: "an interpreter that answers nonsense is stopped and nothing reported",
            script: "read line; echo 'not json'; exec sleep 600",
            expected_outcome: (false, 0),
        },
    ];
    for test_case in test_cases {
        let directory = tempfile::tempdir().expect("temporary directory");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let (url, answered) = runtime.block_on(answering_server(&[]));
        let options = crate::native_commands::models::LocationServe {
            server_url: url,
            ..location_options(directory.path())
        };
        let compiler = runtime.block_on(async { scripted_compiler(test_case.script) });
        let kept = runtime
            .block_on(report_compiler_sinks(
                Some(compiler),
                (&options, ("token", Uuid::nil())),
            ))
            .expect("no control error");
        assert_eq!(
            (
                kept.is_some(),
                answered.load(std::sync::atomic::Ordering::SeqCst),
            ),
            test_case.expected_outcome,
            "{}",
            test_case.description
        );
    }
}
