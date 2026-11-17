use std::collections::BTreeSet;

use crate::python_plan::main::run_lifecycle::run_lifecycle;
use crate::python_plan::models::{RoundTripLoader, RunLifecycle};
use crate::python_plan::tests::helpers::{edges, lifecycle_nodes, names};
use crate::python_plan::tests::test_types::LifecycleTestCase;

#[test]
fn given_selection_when_classifying_lifecycle_then_matches_python_regions() {
    let test_cases = [
        LifecycleTestCase {
            description: "a loader pulls its selected upstream tasks into ingress",
            selected: &["fetch_rates", "load_orders", "report", "check_orders"],
            nodes: &[
                ("fetch_rates", "task", &[]),
                ("load_orders", "loader", &[]),
                ("report", "asset", &[("model", "orders")]),
                ("check_orders", "check", &[]),
            ],
            upstream: &[("load_orders", &["fetch_rates", "unselected"])],
            expected_ingress: &["fetch_rates", "load_orders"],
            expected_loaders: &["load_orders"],
            expected_read_side: &["report"],
            expected_round_trips: &[],
        },
        LifecycleTestCase {
            description: "a selected loader upstream of another loader is ingress",
            selected: &["sync_orders", "load_orders"],
            nodes: &[
                ("sync_orders", "loader", &[]),
                ("load_orders", "loader", &[]),
            ],
            upstream: &[("load_orders", &["sync_orders"])],
            expected_ingress: &["load_orders", "sync_orders"],
            expected_loaders: &["load_orders", "sync_orders"],
            expected_read_side: &[],
            expected_round_trips: &[],
        },
        LifecycleTestCase {
            description: "a loader whose selected upstream reads SQL loads after it, gated by it",
            selected: &["score_orders", "package_scores", "partner_scores"],
            nodes: &[
                ("score_orders", "task", &[("model", "orders")]),
                ("package_scores", "asset", &[]),
                ("partner_scores", "loader", &[]),
            ],
            upstream: &[
                ("partner_scores", &["package_scores"]),
                ("package_scores", &["score_orders"]),
            ],
            expected_ingress: &[],
            expected_loaders: &[],
            expected_read_side: &["package_scores", "score_orders"],
            expected_round_trips: &[(
                "partner_scores",
                &[("model", "orders")],
                &["package_scores", "score_orders"],
                &["package_scores"],
            )],
        },
        LifecycleTestCase {
            description: "only the tasks and assets a loader reads directly can let it run",
            selected: &[
                "fetch_rates",
                "score_orders",
                "package_scores",
                "partner_scores",
            ],
            nodes: &[
                ("fetch_rates", "task", &[]),
                ("score_orders", "task", &[("model", "orders")]),
                ("package_scores", "asset", &[]),
                ("partner_scores", "loader", &[]),
            ],
            upstream: &[
                ("partner_scores", &["package_scores", "fetch_rates"]),
                ("package_scores", &["score_orders"]),
            ],
            expected_ingress: &[],
            expected_loaders: &[],
            expected_read_side: &["fetch_rates", "package_scores", "score_orders"],
            expected_round_trips: &[(
                "partner_scores",
                &[("model", "orders")],
                &["fetch_rates", "package_scores", "score_orders"],
                &["fetch_rates", "package_scores"],
            )],
        },
        LifecycleTestCase {
            description: "a loader selected without its SQL-reading upstream is ordinary ingress",
            selected: &["partner_scores"],
            nodes: &[
                ("score_orders", "task", &[("model", "orders")]),
                ("partner_scores", "loader", &[]),
            ],
            upstream: &[("partner_scores", &["score_orders"])],
            expected_ingress: &["partner_scores"],
            expected_loaders: &["partner_scores"],
            expected_read_side: &[],
            expected_round_trips: &[],
        },
    ];

    for test_case in test_cases {
        let selected: BTreeSet<String> = test_case
            .selected
            .iter()
            .map(|name| (*name).to_owned())
            .collect();

        let lifecycle: RunLifecycle = run_lifecycle(
            &selected,
            &lifecycle_nodes(test_case.nodes),
            &edges(test_case.upstream),
        );

        assert_eq!(
            lifecycle,
            RunLifecycle {
                ingress_names: names(test_case.expected_ingress),
                ingress_loader_names: names(test_case.expected_loaders),
                read_side_names: names(test_case.expected_read_side),
                round_trip_loaders: test_case
                    .expected_round_trips
                    .iter()
                    .map(|(name, waits_for, gates, frontier)| RoundTripLoader {
                        name: (*name).to_owned(),
                        waits_for: waits_for
                            .iter()
                            .map(|(kind, name)| ((*kind).to_owned(), (*name).to_owned()))
                            .collect(),
                        gates: names(gates),
                        frontier: names(frontier),
                    })
                    .collect(),
            },
            "{}",
            test_case.description
        );
    }
}
