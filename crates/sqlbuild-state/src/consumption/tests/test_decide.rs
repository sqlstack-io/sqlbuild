use crate::consumption::constants::RECENT_EXECUTIONS;
use crate::consumption::models::ChangeKind::{Definition, SourceData};
use crate::consumption::models::{ChangedOutcome, ForcedRun, RunReason};
use crate::consumption::tests::helpers::{
    JOB_A, JOB_B, after, reads_held, refreshed_by, runs, streaming, two_jobs,
};
use crate::consumption::tests::test_types::{DecideTestCase, Scenario};

#[test]
fn given_recorded_builds_when_deciding_changed_then_counts_and_reach_pick_what_runs() {
    let mut new_data_rebuilt = streaming();
    new_data_rebuilt.new_data("src_events", "d2");
    new_data_rebuilt.build(&["stg_events"]);
    let mut redefined_rebuilt = streaming();
    redefined_rebuilt.redefine("stg_events", "v2");
    redefined_rebuilt.build(&["stg_events"]);
    let mut new_data = streaming();
    new_data.new_data("src_events", "d2");
    let mut pruned = streaming();
    pruned.new_data("src_events", "d2");
    pruned.build(&["stg_events"]);
    pruned.prune("stg_events");
    let mut converging = Scenario::new(&[
        ("helper", &[]),
        ("clean_orders", &["helper"]),
        ("fct_revenue", &["helper"]),
        ("dim_dates", &["src_dates"]),
        ("summary", &["clean_orders", "fct_revenue", "dim_dates"]),
    ]);
    converging.build(&[
        "helper",
        "clean_orders",
        "fct_revenue",
        "dim_dates",
        "summary",
    ]);
    converging.redefine("helper", "v2");
    let mut propagated = two_jobs();
    propagated.build(&["stg_orders", "int_orders"]);
    let two_jobs = two_jobs();
    let mut both_redefined = Scenario::new(&[
        ("stg_a", &["src_a"]),
        ("stg_b", &["src_b"]),
        ("joined", &["stg_b", "stg_a"]),
    ]);
    both_redefined.build(&["stg_a", "stg_b", "joined"]);
    both_redefined.redefine("stg_a", "v2");
    both_redefined.redefine("stg_b", "v2");
    both_redefined.build(&["stg_a", "stg_b"]);
    let mut renamed = streaming();
    renamed.force("fct_events", ForcedRun::PlannerChange("renamed".to_owned()));
    let mut deep = Scenario::new(&[
        ("raw_copy", &["src_events"]),
        ("stg_events", &["raw_copy"]),
        ("daily", &["stg_events"]),
    ]);
    deep.build(&["raw_copy", "stg_events", "daily"]);
    for batch in 0..=RECENT_EXECUTIONS {
        deep.new_data("src_events", &format!("d{batch}"));
        deep.build(&["raw_copy", "stg_events"]);
    }
    let test_cases = [
        DecideTestCase {
            description: "an upstream rebuilt with only new data stays out of --changed",
            outcomes: new_data_rebuilt.decide(&[Definition], None),
            expected_outcomes: vec![
                ("stg_events", ChangedOutcome::Unchanged),
                ("fct_events", ChangedOutcome::Unchanged),
            ],
        },
        DecideTestCase {
            description: "counting source data, the same rebuilt upstream pulls its downstream in",
            outcomes: new_data_rebuilt.decide(&[Definition, SourceData], None),
            expected_outcomes: vec![
                ("stg_events", ChangedOutcome::Unchanged),
                (
                    "fct_events",
                    runs(RunReason::InputChanged("model:stg_events".to_owned())),
                ),
            ],
        },
        DecideTestCase {
            description: "an upstream rebuilt with a definition change pulls its downstream in",
            outcomes: redefined_rebuilt.decide(&[Definition], None),
            expected_outcomes: vec![
                ("stg_events", ChangedOutcome::Unchanged),
                (
                    "fct_events",
                    runs(RunReason::UpstreamDefinitionChanged(
                        "model:stg_events".to_owned(),
                    )),
                ),
            ],
        },
        DecideTestCase {
            description: "new source data is not a change by default",
            outcomes: new_data.decide(&[Definition], None),
            expected_outcomes: vec![
                ("stg_events", ChangedOutcome::Unchanged),
                ("fct_events", ChangedOutcome::Unchanged),
            ],
        },
        DecideTestCase {
            description: "counting only source data, new data runs the source's reader and downstream",
            outcomes: new_data.decide(&[SourceData], None),
            expected_outcomes: vec![
                (
                    "stg_events",
                    runs(RunReason::InputChanged("src_events".to_owned())),
                ),
                ("fct_events", after("stg_events")),
            ],
        },
        DecideTestCase {
            description: "an upstream rebuild whose earlier execution was pruned counts as a change",
            outcomes: pruned.decide(&[Definition], None),
            expected_outcomes: vec![
                ("stg_events", ChangedOutcome::Unchanged),
                (
                    "fct_events",
                    runs(RunReason::InputChanged("model:stg_events".to_owned())),
                ),
            ],
        },
        DecideTestCase {
            description: "a change stops at another triggered job; a node in no job runs",
            outcomes: two_jobs.decide(&[Definition], Some(&[JOB_A, JOB_B])),
            expected_outcomes: vec![
                ("stg_orders", runs(RunReason::DefinitionChanged)),
                ("int_orders", after("stg_orders")),
                ("fct_revenue", refreshed_by("job_b")),
                ("rpt_pnl", refreshed_by("job_b")),
                ("orders_export", after("stg_orders")),
            ],
        },
        DecideTestCase {
            description: "a job without a trigger counts as no job, so its nodes run",
            outcomes: two_jobs.decide(&[Definition], Some(&[JOB_A])),
            expected_outcomes: vec![
                ("stg_orders", runs(RunReason::DefinitionChanged)),
                ("int_orders", after("stg_orders")),
                ("fct_revenue", after("int_orders")),
                ("rpt_pnl", after("fct_revenue")),
                ("orders_export", after("stg_orders")),
            ],
        },
        DecideTestCase {
            description: "with no jobs at all, everything downstream runs",
            outcomes: two_jobs.decide(&[Definition], Some(&[])),
            expected_outcomes: vec![
                ("stg_orders", runs(RunReason::DefinitionChanged)),
                ("int_orders", after("stg_orders")),
                ("fct_revenue", after("int_orders")),
                ("rpt_pnl", after("fct_revenue")),
                ("orders_export", after("stg_orders")),
            ],
        },
        DecideTestCase {
            description: "--reach all ignores job edges",
            outcomes: two_jobs.decide(&[Definition], None),
            expected_outcomes: vec![
                ("stg_orders", runs(RunReason::DefinitionChanged)),
                ("int_orders", after("stg_orders")),
                ("fct_revenue", after("int_orders")),
                ("rpt_pnl", after("fct_revenue")),
                ("orders_export", after("stg_orders")),
            ],
        },
        DecideTestCase {
            description: "a model in two jobs runs when one of them holds the change",
            outcomes: two_jobs.decide(
                &[Definition],
                Some(&[
                    ("job_a", &["stg_orders", "int_orders", "fct_revenue"]),
                    JOB_B,
                ]),
            ),
            expected_outcomes: vec![
                ("stg_orders", runs(RunReason::DefinitionChanged)),
                ("int_orders", after("stg_orders")),
                ("fct_revenue", after("int_orders")),
                ("rpt_pnl", refreshed_by("job_b")),
                ("orders_export", after("stg_orders")),
            ],
        },
        DecideTestCase {
            description: "once the change reached a model in both jobs, its downstream still waits for the job without it",
            outcomes: propagated.decide(
                &[Definition],
                Some(&[JOB_A, ("job_b", &["int_orders", "fct_revenue", "rpt_pnl"])]),
            ),
            expected_outcomes: vec![
                ("stg_orders", ChangedOutcome::Unchanged),
                ("int_orders", ChangedOutcome::Unchanged),
                ("fct_revenue", refreshed_by("job_b")),
                ("rpt_pnl", refreshed_by("job_b")),
                (
                    "orders_export",
                    runs(RunReason::UpstreamDefinitionChanged(
                        "model:stg_orders".to_owned(),
                    )),
                ),
            ],
        },
        DecideTestCase {
            description: "branches meeting behind a held-back node are held back; untouched parents never block",
            outcomes: converging.decide(&[Definition], Some(&[("job_b", &["fct_revenue"])])),
            expected_outcomes: vec![
                ("helper", runs(RunReason::DefinitionChanged)),
                ("clean_orders", after("helper")),
                ("fct_revenue", refreshed_by("job_b")),
                ("dim_dates", ChangedOutcome::Unchanged),
                ("summary", reads_held("fct_revenue")),
            ],
        },
        DecideTestCase {
            description: "a node runs when any job holding one of its changes contains it, whatever the input order",
            outcomes: both_redefined.decide(
                &[Definition],
                Some(&[("job_a", &["stg_a", "joined"]), ("job_b", &["stg_b"])]),
            ),
            expected_outcomes: vec![
                ("stg_a", ChangedOutcome::Unchanged),
                ("stg_b", ChangedOutcome::Unchanged),
                (
                    "joined",
                    runs(RunReason::UpstreamDefinitionChanged(
                        "model:stg_b".to_owned(),
                    )),
                ),
            ],
        },
        DecideTestCase {
            description: "a planner change that is not a definition edit runs even when only source data counts",
            outcomes: renamed.decide(&[SourceData], None),
            expected_outcomes: vec![
                ("stg_events", ChangedOutcome::Unchanged),
                (
                    "fct_events",
                    runs(RunReason::Forced(ForcedRun::PlannerChange(
                        "renamed".to_owned(),
                    ))),
                ),
            ],
        },
        DecideTestCase {
            description: "new data through a chain rebuilt more often than the recent window is still not a change",
            outcomes: deep.decide(&[Definition], None),
            expected_outcomes: vec![
                ("raw_copy", ChangedOutcome::Unchanged),
                ("stg_events", ChangedOutcome::Unchanged),
                ("daily", ChangedOutcome::Unchanged),
            ],
        },
    ];
    for test_case in test_cases {
        let outcomes: Vec<(&str, ChangedOutcome)> = test_case
            .outcomes
            .iter()
            .map(|(name, outcome)| (name.as_str(), outcome.clone()))
            .collect();
        assert_eq!(
            outcomes, test_case.expected_outcomes,
            "{}",
            test_case.description
        );
    }
}
