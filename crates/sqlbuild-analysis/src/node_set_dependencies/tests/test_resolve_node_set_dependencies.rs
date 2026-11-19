use crate::node_set_dependencies::tests::helpers::{ends, missing, resolve};
use crate::node_set_dependencies::tests::test_types::{EndNodesTestCase, FailureTestCase};

/// The group running `nightly_wide` and `reporting` together.
const GROUP: &[(&str, &[&str])] = &[("daily_group", &["nightly_wide", "reporting"])];

#[test]
fn given_job_and_tag_dependencies_when_resolving_then_each_reduces_to_its_end_nodes() {
    let test_cases = [
        EndNodesTestCase {
            description: "a job whose selection branches ends in each branch",
            groups: &[],
            sets: &[("job", "nightly", None)],
            expected_ends: &[&["model 'order_items'", "model 'orders'"]],
        },
        EndNodesTestCase {
            description: "a job of one node ends in it",
            groups: &[],
            sets: &[("job", "single", None)],
            expected_ends: &[&["model 'customers'"]],
        },
        EndNodesTestCase {
            description: "a diamond ends in the node joining its branches",
            groups: &[],
            sets: &[("job", "diamond", None)],
            expected_ends: &[&["model 'finance_daily'"]],
        },
        EndNodesTestCase {
            description: "a Python node reading the job's model is its end",
            groups: &[],
            sets: &[("job", "with_task", None)],
            expected_ends: &[&["task 'export_orders'"]],
        },
        EndNodesTestCase {
            description: "exclude removes nodes before reducing to end nodes",
            groups: &[],
            sets: &[("job", "diamond", Some("finance_daily"))],
            expected_ends: &[&["model 'order_items'", "model 'orders'"]],
        },
        EndNodesTestCase {
            description: "exclude takes tag selectors",
            groups: &[],
            sets: &[("job", "diamond", Some("tag:finance finance_daily"))],
            expected_ends: &[&["model 'stg_orders'"]],
        },
        EndNodesTestCase {
            description: "a tag ends in its tagged nodes nothing tagged reads",
            groups: &[],
            sets: &[("tag", "crm", None)],
            expected_ends: &[&["model 'customers'"]],
        },
        EndNodesTestCase {
            description: "a tag excluding a Python node keeps its SQL",
            groups: &[],
            sets: &[("tag", "finance", Some("self_tagged"))],
            expected_ends: &[&["model 'order_items'", "model 'orders'"]],
        },
        EndNodesTestCase {
            description: "a job reading the dependant's job resolves when no group runs them together",
            groups: &[],
            sets: &[("job", "nightly_wide", None)],
            expected_ends: &[&["model 'customer_orders'"]],
        },
        EndNodesTestCase {
            description: "each of several sets resolves on its own",
            groups: &[],
            sets: &[("job", "single", None), ("tag", "crm", None)],
            expected_ends: &[&["model 'customers'"], &["model 'customers'"]],
        },
    ];
    for test_case in test_cases {
        let resolved = resolve(("report", "asset"), test_case.sets, test_case.groups)
            .expect(test_case.description);
        assert_eq!(
            ends(&resolved),
            test_case.expected_ends,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_unresolvable_dependencies_when_resolving_then_compile_fails_naming_the_cause() {
    let test_cases = [
        FailureTestCase {
            description: "an unknown job",
            dependant: ("report", "asset"),
            groups: &[],
            sets: &[("job", "weekly", None)],
            expected_fragments: &[
                "task 'report' depends on job 'weekly', which the project does not declare",
            ],
        },
        FailureTestCase {
            description: "an unknown tag",
            dependant: ("report", "asset"),
            groups: &[],
            sets: &[("tag", "marketing", None)],
            expected_fragments: &[
                "task 'report' depends on tag 'marketing', which no node carries",
            ],
        },
        FailureTestCase {
            description: "a set its exclude empties",
            dependant: ("report", "asset"),
            groups: &[],
            sets: &[("job", "single", Some("customers"))],
            expected_fragments: &[
                "depends on job 'single', which leaves no model",
                "after its exclude",
            ],
        },
        FailureTestCase {
            description: "another location's job",
            dependant: ("report", "asset"),
            groups: &[],
            sets: &[("job", "billing:nightly", None)],
            expected_fragments: &["a job of another code location"],
        },
        FailureTestCase {
            description: "a loader's set",
            dependant: ("sync_events", "loader"),
            groups: &[],
            sets: &[("job", "single", None)],
            expected_fragments: &[
                "loader 'sync_events' depends on job 'single'; loaders cannot depend on a job or a tag",
            ],
        },
        FailureTestCase {
            description: "a job node reading the dependant's output closes a cycle",
            dependant: ("report", "asset"),
            groups: &[],
            sets: &[("job", "loop", None)],
            expected_fragments: &[
                "task 'report' depends on job 'loop', but job 'loop''s task 'audit_export' reads task 'report''s output",
                "job(\"loop\", exclude=\"audit_export\")",
            ],
        },
        FailureTestCase {
            description: "a tag the dependant carries includes it",
            dependant: ("self_tagged", "asset"),
            groups: &[],
            sets: &[("tag", "finance", None)],
            expected_fragments: &[
                "task 'self_tagged' depends on tag 'finance', which includes task 'self_tagged' itself",
                "tag(\"finance\", exclude=\"self_tagged\")",
            ],
        },
        FailureTestCase {
            description: "a group whose jobs would wait for each other",
            dependant: ("report", "asset"),
            groups: GROUP,
            sets: &[("job", "nightly_wide", None)],
            expected_fragments: &[
                "job group 'daily_group' wait in a loop",
                "'nightly_wide' waits for 'reporting' ('nightly_wide''s model 'customer_orders' reads model 'customers')",
                "move model 'customer_orders' to another job",
            ],
        },
    ];
    for test_case in test_cases {
        let error = resolve(test_case.dependant, test_case.sets, test_case.groups)
            .expect_err(test_case.description);
        let text: String = format!("{} {}", error.message, error.help.unwrap_or_default());
        assert_eq!(
            missing(&text, test_case.expected_fragments),
            Vec::<&str>::new(),
            "{}: {text}",
            test_case.description
        );
    }
}
