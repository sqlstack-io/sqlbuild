use crate::project_validation::main::validate_orchestration_names::validate_orchestration_names;
use crate::project_validation::models::{
    ProjectFacts, TestBlockFact, TestFileFact, ValidationFailure,
};
use crate::project_validation::tests::helpers::{collision, declared, model, named};
use crate::project_validation::tests::test_types::ValidationTestCase;

const RENAME_DECLARATION: &str = " (a job, job group or sensor takes its name from its first \
     argument, and a calendar schedule from the function it decorates)";

#[test]
fn given_orchestration_names_when_validating_then_they_share_the_project_namespace() {
    let test_cases = [
        ValidationTestCase {
            description: "distinct names pass",
            facts: ProjectFacts {
                models: vec![model("orders")],
                orchestration: vec![
                    declared("refresh_orders", "job"),
                    declared("nightly", "job group"),
                    declared("orders_arrived", "sensor"),
                    declared("fulfillment_calendar", "calendar schedule"),
                ],
                ..ProjectFacts::default()
            },
            expected_failure: Ok(()),
        },
        ValidationTestCase {
            description: "a job named like a model",
            facts: ProjectFacts {
                models: vec![model("orders")],
                orchestration: vec![declared("orders", "job")],
                ..ProjectFacts::default()
            },
            expected_failure: collision(
                "Project resource name 'orders' is declared as both model in models/orders.sql \
                 and job in orchestration/daily.py",
                RENAME_DECLARATION,
            ),
        },
        ValidationTestCase {
            description: "a sensor named like a unit test",
            facts: ProjectFacts {
                tests: vec![TestFileFact {
                    file: named("orders_arrived", "tests/orders_arrived.sql"),
                    blocks: vec![TestBlockFact::default()],
                }],
                orchestration: vec![declared("orders_arrived", "sensor")],
                ..ProjectFacts::default()
            },
            expected_failure: collision(
                "Project resource name 'orders_arrived' is declared as both unit test in \
                 tests/orders_arrived.sql and sensor in orchestration/daily.py",
                RENAME_DECLARATION,
            ),
        },
        ValidationTestCase {
            description: "a calendar schedule named like a provider",
            facts: ProjectFacts {
                providers: vec![named("fulfillment", "providers/fulfillment.py")],
                orchestration: vec![declared("fulfillment", "calendar schedule")],
                ..ProjectFacts::default()
            },
            expected_failure: collision(
                "Project resource name 'fulfillment' is declared as both provider in \
                 providers/fulfillment.py and calendar schedule in orchestration/daily.py",
                RENAME_DECLARATION,
            ),
        },
        ValidationTestCase {
            description: "a unit test named like a model gets no orchestration advice",
            facts: ProjectFacts {
                models: vec![model("orders")],
                tests: vec![TestFileFact {
                    file: named("orders", "tests/orders.sql"),
                    blocks: vec![TestBlockFact::default()],
                }],
                ..ProjectFacts::default()
            },
            expected_failure: collision(
                "Project resource name 'orders' is declared as both model in models/orders.sql \
                 and unit test in tests/orders.sql",
                "",
            ),
        },
        ValidationTestCase {
            description: "a job group named like a job",
            facts: ProjectFacts {
                orchestration: vec![declared("daily", "job"), declared("daily", "job group")],
                ..ProjectFacts::default()
            },
            expected_failure: collision(
                "Project resource name 'daily' is declared as both job in orchestration/daily.py \
                 and job group in orchestration/daily.py",
                RENAME_DECLARATION,
            ),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            validate_orchestration_names(&test_case.facts)
                .map_err(|failure: ValidationFailure| failure.to_string()),
            test_case.expected_failure,
            "{}",
            test_case.description
        );
    }
}
