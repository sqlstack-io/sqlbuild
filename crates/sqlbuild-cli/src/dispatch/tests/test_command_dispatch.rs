use crate::dispatch::constants::{
    BUILD_ROUTE, CLONE_ROUTE, COMPILE_ROUTE, DBT_PLAN_ROUTE, FORMAT_ROUTE, REFACTOR_ROUTE,
    SCENARIO_CAPTURE_ROUTE,
};
use crate::dispatch::main::command_dispatch::command_dispatch;
use crate::dispatch::models::{CommandFacts, Dispatch, DispatchFailure};
use crate::dispatch::tests::helpers::{facts, failure, runs};
use crate::dispatch::tests::test_types::CommandDispatchTestCase;

#[test]
fn given_parsed_command_when_dispatching_then_selects_python_dispatcher_route() {
    let test_cases = [
        CommandDispatchTestCase {
            description: "compile runs the compile handler",
            facts: facts("compile", None),
            expected_dispatch: runs(&COMPILE_ROUTE),
        },
        CommandDispatchTestCase {
            description: "dbt plan runs the dbt plan slot",
            facts: facts("dbt", Some("plan")),
            expected_dispatch: runs(&DBT_PLAN_ROUTE),
        },
        CommandDispatchTestCase {
            description: "dbt without a subcommand fails",
            facts: facts("dbt", None),
            expected_dispatch: Dispatch::Fail(failure(
                "dbt requires a subcommand such as 'plan'",
                "C237",
            )),
        },
        CommandDispatchTestCase {
            description: "clone without --from fails",
            facts: CommandFacts {
                from_target: false,
                ..facts("clone", None)
            },
            expected_dispatch: Dispatch::Run(
                &CLONE_ROUTE,
                Some(failure("clone requires --from", "C406")),
            ),
        },
        CommandDispatchTestCase {
            description: "build --ignore-audit-failures with --no-audits fails",
            facts: CommandFacts {
                ignores_failures_of_no_audits: true,
                ..facts("build", None)
            },
            expected_dispatch: Dispatch::Run(
                &BUILD_ROUTE,
                Some(failure(
                    "--ignore-audit-failures cannot be combined with --no-audits: no audit runs, so there is no failure to ignore",
                    "C419",
                )),
            ),
        },
        CommandDispatchTestCase {
            description: "format --diff with --json fails",
            facts: CommandFacts {
                format_diff: true,
                json: true,
                ..facts("format", None)
            },
            expected_dispatch: Dispatch::Run(
                &FORMAT_ROUTE,
                Some(failure(
                    "format --diff cannot be combined with --json",
                    "C112",
                )),
            ),
        },
        CommandDispatchTestCase {
            description: "scenario capture runs the capture slot",
            facts: facts("scenario", Some("capture")),
            expected_dispatch: runs(&SCENARIO_CAPTURE_ROUTE),
        },
        CommandDispatchTestCase {
            description: "rename reads its pairs itself, so they may come only from a file",
            facts: facts("rename", None),
            expected_dispatch: runs(&REFACTOR_ROUTE),
        },
        CommandDispatchTestCase {
            description: "mv reads its pairs itself, so they may come only from a file",
            facts: facts("mv", None),
            expected_dispatch: runs(&REFACTOR_ROUTE),
        },
        CommandDispatchTestCase {
            description: "clean has no handler",
            facts: facts("clean", None),
            expected_dispatch: Dispatch::Nothing,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            command_dispatch(&test_case.facts),
            test_case.expected_dispatch,
            "{}",
            test_case.description
        );
    }
}
