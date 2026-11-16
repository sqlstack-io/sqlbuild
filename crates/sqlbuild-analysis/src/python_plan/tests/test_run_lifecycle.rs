use std::collections::BTreeSet;

use crate::python_plan::main::run_lifecycle::run_lifecycle;
use crate::python_plan::tests::helpers::{edges, lifecycle_nodes};
use crate::python_plan::tests::test_types::LifecycleTestCase;

#[test]
fn given_selection_when_classifying_lifecycle_then_matches_python_regions() {
    let test_cases = [
        LifecycleTestCase {
            description: "a loader pulls its selected upstream tasks into ingress",
            selected: &["fetch_rates", "load_orders", "report", "check_orders"],
            nodes: &[
                ("fetch_rates", "task"),
                ("load_orders", "loader"),
                ("report", "asset"),
                ("check_orders", "check"),
            ],
            upstream: &[("load_orders", &["fetch_rates", "unselected"])],
            expected_ingress: &["fetch_rates", "load_orders"],
            expected_loaders: &["load_orders"],
            expected_read_side: &["report"],
        },
        LifecycleTestCase {
            description: "a selected loader upstream of another loader is ingress",
            selected: &["sync_orders", "load_orders"],
            nodes: &[("sync_orders", "loader"), ("load_orders", "loader")],
            upstream: &[("load_orders", &["sync_orders"])],
            expected_ingress: &["load_orders", "sync_orders"],
            expected_loaders: &["load_orders", "sync_orders"],
            expected_read_side: &[],
        },
    ];

    for test_case in test_cases {
        let selected: BTreeSet<String> = test_case
            .selected
            .iter()
            .map(|name| (*name).to_owned())
            .collect();

        let lifecycle = run_lifecycle(
            &selected,
            &lifecycle_nodes(test_case.nodes),
            &edges(test_case.upstream),
        );

        assert_eq!(
            (
                lifecycle.ingress_names,
                lifecycle.ingress_loader_names,
                lifecycle.read_side_names,
            ),
            (
                test_case
                    .expected_ingress
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect::<Vec<String>>(),
                test_case
                    .expected_loaders
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect::<Vec<String>>(),
                test_case
                    .expected_read_side
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect::<Vec<String>>(),
            ),
            "{}",
            test_case.description
        );
    }
}
