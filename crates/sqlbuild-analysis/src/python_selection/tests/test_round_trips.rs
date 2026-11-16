use crate::python_selection::tests::helpers::{expected_round_trip, round_trip_run};
use crate::python_selection::tests::test_types::{RoundTripNodeSpec, RoundTripTestCase};

const SCORE_ORDERS: RoundTripNodeSpec = ("score_orders", "task", &[], &[("model", "orders")]);
const PARTNER_SCORES: RoundTripNodeSpec = ("partner_scores", "loader", &["score_orders"], &[]);
const HELP: &str = "if this is intended, set `allow_warehouse_round_trips = true` under [settings] \
                    in sqlbuild_project.toml";
const CYCLE_HELP: &str = "each arrow points to what a node depends on; a task or asset upstream \
                          of a loader cannot read anything built from the source that loader fills";

#[test]
fn given_a_loader_depending_on_python_that_reads_sql_when_selecting_then_the_setting_decides() {
    let test_cases = [
        RoundTripTestCase {
            description: "a direct SQL-reading task is refused by default",
            nodes: &[SCORE_ORDERS, PARTNER_SCORES],
            allow_warehouse_round_trips: false,
            select: &[],
            expected_selection: Err((
                "S000",
                "loader 'partner_scores' depends on task 'score_orders', which reads SQL\n  \
                 This is a warehouse round trip: data leaves the warehouse and comes back through \
                 this loader.\n  Lineage and column lineage restart at the source \
                 'partner_scores', and builds and job\n  groups run the task before the loader.",
                HELP,
            )),
        },
        RoundTripTestCase {
            description: "a transitive SQL-reading asset is refused by default and named with its path",
            nodes: &[
                ("score_orders", "asset", &[], &[("model", "orders")]),
                ("package_scores", "task", &["score_orders"], &[]),
                ("partner_scores", "loader", &["package_scores"], &[]),
            ],
            allow_warehouse_round_trips: false,
            select: &[],
            expected_selection: Err((
                "S000",
                "loader 'partner_scores' depends on asset 'score_orders' (through task \
                 'package_scores'), which reads SQL\n  This is a warehouse round trip: data \
                 leaves the warehouse and comes back through this loader.\n  Lineage and column \
                 lineage restart at the source 'partner_scores', and builds and job\n  groups run \
                 the asset before the loader.",
                HELP,
            )),
        },
        RoundTripTestCase {
            description: "the setting allows the round trip, which upstream selection follows",
            nodes: &[SCORE_ORDERS, PARTNER_SCORES],
            allow_warehouse_round_trips: true,
            select: &["+scored_orders"],
            expected_selection: Ok((
                &[
                    ("model", "orders"),
                    ("model", "scored_orders"),
                    ("source", "partner_scores"),
                ],
                &["partner_scores", "score_orders"],
            )),
        },
        RoundTripTestCase {
            description: "a loader depending on a task that reads no SQL is always allowed",
            nodes: &[
                ("fetch_scores", "task", &[], &[]),
                ("partner_scores", "loader", &["fetch_scores"], &[]),
            ],
            allow_warehouse_round_trips: false,
            select: &["+scored_orders"],
            expected_selection: Ok((
                &[
                    ("model", "orders"),
                    ("model", "scored_orders"),
                    ("source", "partner_scores"),
                ],
                &["fetch_scores", "partner_scores"],
            )),
        },
        RoundTripTestCase {
            description: "a task reading what its downstream loader's source builds is a cycle",
            nodes: &[
                ("score_orders", "task", &[], &[("model", "scored_orders")]),
                PARTNER_SCORES,
            ],
            allow_warehouse_round_trips: true,
            select: &[],
            expected_selection: Err((
                "S000",
                "loader 'partner_scores' is in a dependency cycle: loader 'partner_scores' -> \
                 task 'score_orders' -> model 'scored_orders' -> source 'partner_scores' -> \
                 loader 'partner_scores'",
                CYCLE_HELP,
            )),
        },
        RoundTripTestCase {
            description: "a cycle is reported before the setting is consulted",
            nodes: &[
                ("score_orders", "task", &[], &[("model", "scored_orders")]),
                PARTNER_SCORES,
            ],
            allow_warehouse_round_trips: false,
            select: &[],
            expected_selection: Err((
                "S000",
                "loader 'partner_scores' is in a dependency cycle: loader 'partner_scores' -> \
                 task 'score_orders' -> model 'scored_orders' -> source 'partner_scores' -> \
                 loader 'partner_scores'",
                CYCLE_HELP,
            )),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            round_trip_run(
                test_case.nodes,
                test_case.allow_warehouse_round_trips,
                test_case.select
            ),
            expected_round_trip(test_case.expected_selection),
            "{}",
            test_case.description
        );
    }
}
