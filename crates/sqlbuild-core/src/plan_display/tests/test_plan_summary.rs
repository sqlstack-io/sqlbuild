//! A plan's summary counts what it runs and leaves out zero counts; with nothing at all it is `0 selected`.

use crate::plan_display::main::plan_summary_parts::plan_summary_parts;
use crate::plan_display::models::PlanSummaryCounts;
use crate::plan_display::tests::test_types::PlanSummaryTestCase;

#[test]
fn given_plan_counts_when_summarised_then_only_what_runs_is_named() {
    let test_cases = [
        PlanSummaryTestCase {
            description: "a job of one Python asset",
            counts: PlanSummaryCounts {
                python: 1,
                ..PlanSummaryCounts::default()
            },
            expected_parts: &["1 Python node"],
        },
        PlanSummaryTestCase {
            description: "models, a reloaded source and Python nodes",
            counts: PlanSummaryCounts {
                selected: 3,
                sources: 1,
                reloads_sources: true,
                python: 2,
                ..PlanSummaryCounts::default()
            },
            expected_parts: &["3 selected", "1 source to reload", "2 Python nodes"],
        },
        PlanSummaryTestCase {
            description: "a plan with nothing at all",
            counts: PlanSummaryCounts::default(),
            expected_parts: &["0 selected"],
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            plan_summary_parts(&test_case.counts),
            test_case.expected_parts,
            "{}",
            test_case.description
        );
    }
}
