use serde_json::{Value, json};

use crate::plan_display::main::plan_section_page::plan_section_page;
use crate::plan_display::main::plan_summary::plan_summary;
use crate::plan_display::models::PlanPageSection;
use crate::plan_display::tests::helpers::{changed_shop_plan, shop_plan};
use crate::plan_display::tests::test_types::{PageTestCase, SummaryTestCase};

fn names(rows: &Value) -> Vec<String> {
    rows.as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| row.get("name").or(Some(row)).and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

#[test]
fn given_a_plan_when_summarised_then_changed_models_stay_whole_and_large_sections_are_counted() {
    let test_cases = [
        SummaryTestCase {
            description: "without --changed every unskipped model is built",
            document: shop_plan,
            expected_models: &["orders"],
            expected_built: &["orders", "customers", "payments", "events"],
            expected_pages: json!({"routine_models": 3}),
            expected_runs: Value::Null,
        },
        SummaryTestCase {
            description: "--changed builds only the models it runs and keeps the runs of what the summary carries",
            document: changed_shop_plan,
            expected_models: &["orders"],
            expected_built: &["orders", "customers"],
            expected_pages: json!({"routine_models": 1, "held_back": 1, "unchanged": 2}),
            expected_runs: json!(["model:orders", "seed:countries"]),
        },
    ];
    for test_case in test_cases {
        let summary: Value = plan_summary(&(test_case.document)());
        let runs: Value = summary["changed"]["runs"]
            .as_array()
            .map_or(Value::Null, |runs| {
                runs.iter().map(|run| run["node"].clone()).collect()
            });
        assert_eq!(
            (
                names(&summary["models"]),
                names(&summary["built_model_names"]),
                summary["pages"].clone(),
                runs,
                summary.get("metadata").is_none(),
                summary["models"][0]["query_diff"].clone(),
            ),
            (
                test_case
                    .expected_models
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect(),
                test_case
                    .expected_built
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect(),
                test_case.expected_pages,
                test_case.expected_runs,
                true,
                json!(["+SELECT 1"]),
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_failure_report_when_summarised_then_it_is_unchanged() {
    let test_cases = [json!({"error": {"message": "the plan is too large to show"}})];
    for report in test_cases {
        assert_eq!(
            plan_summary(&report),
            report,
            "a failure report has no sections"
        );
    }
}

#[test]
fn given_a_plan_when_a_section_page_is_read_then_it_is_that_window_in_plan_order() {
    let test_cases = [
        PageTestCase {
            description: "routine models skip the changed and skipped ones",
            document: shop_plan,
            section: PlanPageSection::RoutineModels,
            window: (1, 5),
            expected_rows: json!(["payments", "events"]),
            expected_total: 3,
            expected_runs: json!([]),
        },
        PageTestCase {
            description: "a routine model's page carries its --changed run",
            document: changed_shop_plan,
            section: PlanPageSection::RoutineModels,
            window: (0, 10),
            expected_rows: json!(["customers"]),
            expected_total: 1,
            expected_runs: json!(["model:customers"]),
        },
        PageTestCase {
            description: "unchanged nodes page by offset",
            document: changed_shop_plan,
            section: PlanPageSection::Unchanged,
            window: (1, 1),
            expected_rows: json!(["model:events"]),
            expected_total: 2,
            expected_runs: json!([]),
        },
        PageTestCase {
            description: "a plan without --changed holds nothing back",
            document: shop_plan,
            section: PlanPageSection::HeldBack,
            window: (0, 10),
            expected_rows: json!([]),
            expected_total: 0,
            expected_runs: json!([]),
        },
    ];
    for test_case in test_cases {
        let page = plan_section_page(&(test_case.document)(), test_case.section, test_case.window);
        let rows: Value = page
            .rows
            .iter()
            .map(|row| row.get("name").cloned().unwrap_or_else(|| row.clone()))
            .collect();
        let runs: Value = page.runs.iter().map(|run| run["node"].clone()).collect();
        assert_eq!(
            (rows, page.total, runs),
            (
                test_case.expected_rows,
                test_case.expected_total,
                test_case.expected_runs
            ),
            "{}",
            test_case.description
        );
    }
}
