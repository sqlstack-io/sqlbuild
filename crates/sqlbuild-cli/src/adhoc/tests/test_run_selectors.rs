use std::collections::BTreeSet;

use crate::adhoc::tests::helpers::{audited_orders_publication, past_runs};
use crate::adhoc::tests::test_types::{RunSelectorIdsTestCase, RunSelectorPlanTestCase};

#[test]
fn given_past_runs_and_audits_when_planned_then_run_and_audit_selectors_freeze_exact_keys() {
    let test_cases = [
        RunSelectorPlanTestCase {
            description: "failed: freezes the failed model and the failed end-of-build audit",
            command: &["build", "failed:01a1d3b9"],
            expected_selection: Ok(&["audit:orders_match_payments", "model:stg_orders"]),
        },
        RunSelectorPlanTestCase {
            description: "failed:+ adds what reads the failed model",
            command: &[
                "build",
                "--select",
                "failed:01a1d3b9+",
                "--exclude",
                "kind:asset",
            ],
            expected_selection: Ok(&[
                "audit:orders_match_payments",
                "model:orders",
                "model:stg_orders",
            ]),
        },
        RunSelectorPlanTestCase {
            description: "an attached audit freezes under its target",
            command: &["build", "audit:not_null_*"],
            expected_selection: Ok(&["audit:not_null_orders_id:model:orders:order_id"]),
        },
        RunSelectorPlanTestCase {
            description: "run: less what it skipped",
            command: &[
                "build",
                "--select",
                "run:01a1d3b9",
                "--exclude",
                "skipped:01a1d3b9",
            ],
            expected_selection: Ok(&["model:customers", "model:stg_orders"]),
        },
        RunSelectorPlanTestCase {
            description: "an unknown audit is refused",
            command: &["build", "audit:orders_match_payment"],
            expected_selection: Err("unknown audit name 'orders_match_payment'"),
        },
    ];
    let publication = audited_orders_publication();
    for test_case in test_cases {
        let command: Vec<String> = test_case
            .command
            .iter()
            .map(|token| (*token).to_owned())
            .collect();
        let actual = publication
            .plan(&command, &past_runs())
            .map(|plan| plan.selection.into_iter().collect::<Vec<String>>())
            .map_err(|error| {
                test_case
                    .expected_selection
                    .err()
                    .filter(|fragment| error.contains(fragment))
                    .unwrap_or(error.as_str())
                    .to_owned()
            });
        assert_eq!(
            actual,
            test_case
                .expected_selection
                .map(|keys| keys.iter().map(|key| (*key).to_owned()).collect())
                .map_err(str::to_owned),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_command_texts_when_scanned_then_the_run_ids_they_name_are_listed() {
    let test_cases = [
        RunSelectorIdsTestCase {
            description: "every run a command names, in expansions and intersections, flags ignored",
            texts: &[
                "build",
                "failed:01a1d3b9+",
                "--exclude",
                "skipped:01a1d3c0,tag:x",
            ],
            expected_ids: &["01a1d3b9", "01a1d3c0"],
        },
        RunSelectorIdsTestCase {
            description: "audits and jobs name no run",
            texts: &["build", "audit:orders_match_payments", "job:carrier_sync"],
            expected_ids: &[],
        },
    ];
    let publication = audited_orders_publication();
    for test_case in test_cases {
        let texts: Vec<String> = test_case
            .texts
            .iter()
            .map(|text| (*text).to_owned())
            .collect();
        assert_eq!(
            publication.run_selector_ids(&texts),
            test_case
                .expected_ids
                .iter()
                .map(|id| (*id).to_owned())
                .collect::<BTreeSet<String>>(),
            "{}",
            test_case.description
        );
    }
}
