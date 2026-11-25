use serde_json::{Value, json};

use crate::plan_display::_helpers::diff_lines::diff_stats;
use crate::plan_display::main::detach_model_diffs::detach_model_diffs;
use crate::plan_display::main::plan_section_page::plan_section_page;
use crate::plan_display::main::plan_summary::plan_summary;
use crate::plan_display::models::{DiffStats, PlanPageSection};
use crate::plan_display::tests::helpers::{first_build, full_plan, labels, owned};
use crate::plan_display::tests::test_types::{
    ConfigChangeTestCase, DetachTestCase, DiffStatsTestCase, ModelPageTestCase, NoteTestCase,
    PageTestCase, SectionTestCase, SummaryFieldTestCase, SummarySizeTestCase,
};

#[test]
fn given_a_plan_when_paging_each_section_then_every_section_pages_its_rows_in_plan_order() {
    let test_cases = [
        SectionTestCase {
            description: "models that can't be planned, though --changed doesn't run them",
            section: PlanPageSection::PlanningError,
            expected_rows: &["unplannable"],
        },
        SectionTestCase {
            description: "models an explicit window skips, though skipped and not run by --changed",
            section: PlanPageSection::WindowBeforeCursorStart,
            expected_rows: &["late_feed"],
        },
        SectionTestCase {
            description: "full refresh",
            section: PlanPageSection::FullRefresh,
            expected_rows: &["archive"],
        },
        SectionTestCase {
            description: "query changed",
            section: PlanPageSection::QueryChanged,
            expected_rows: &["orders", "payments"],
        },
        SectionTestCase {
            description: "function changed",
            section: PlanPageSection::FunctionChanged,
            expected_rows: &["refunds"],
        },
        SectionTestCase {
            description: "config changed",
            section: PlanPageSection::ConfigChanged,
            expected_rows: &["totals"],
        },
        SectionTestCase {
            description: "schema changed",
            section: PlanPageSection::SchemaChanged,
            expected_rows: &["ledger"],
        },
        SectionTestCase {
            description: "first run",
            section: PlanPageSection::FirstRun,
            expected_rows: &["daily_orders", "daily_payments"],
        },
        SectionTestCase {
            description: "renamed",
            section: PlanPageSection::Renamed,
            expected_rows: &["old_orders"],
        },
        SectionTestCase {
            description: "routine models that --changed runs",
            section: PlanPageSection::RoutineModels,
            expected_rows: &["customers", "Customer_Notes", "events"],
        },
        SectionTestCase {
            description: "migrations",
            section: PlanPageSection::Migrations,
            expected_rows: &["old_orders", "totals"],
        },
        SectionTestCase {
            description: "column migrations",
            section: PlanPageSection::ColumnMigrations,
            expected_rows: &["ledger"],
        },
        SectionTestCase {
            description: "table type changes",
            section: PlanPageSection::TableTypeConversions,
            expected_rows: &["totals"],
        },
        SectionTestCase {
            description: "retention",
            section: PlanPageSection::Retention,
            expected_rows: &["main.events"],
        },
        SectionTestCase {
            description: "loader reloads",
            section: PlanPageSection::LoaderReloads,
            expected_rows: &["raw_orders"],
        },
        SectionTestCase {
            description: "loader loads",
            section: PlanPageSection::LoaderLoads,
            expected_rows: &["raw_payments"],
        },
        SectionTestCase {
            description: "source reloads",
            section: PlanPageSection::SourceReloads,
            expected_rows: &["shop_db"],
        },
        SectionTestCase {
            description: "source loads",
            section: PlanPageSection::SourceLoads,
            expected_rows: &["partner_feed"],
        },
        SectionTestCase {
            description: "changed functions",
            section: PlanPageSection::ChangedFunctions,
            expected_rows: &["to_cents"],
        },
        SectionTestCase {
            description: "functions",
            section: PlanPageSection::Functions,
            expected_rows: &["clean_name"],
        },
        SectionTestCase {
            description: "first-run seeds --changed runs",
            section: PlanPageSection::FirstRunSeeds,
            expected_rows: &["countries"],
        },
        SectionTestCase {
            description: "changed seeds --changed runs",
            section: PlanPageSection::ChangedSeeds,
            expected_rows: &["currency_rates"],
        },
        SectionTestCase {
            description: "unchanged seeds --changed runs",
            section: PlanPageSection::UnchangedSeeds,
            expected_rows: &["regions"],
        },
        SectionTestCase {
            description: "Python nodes",
            section: PlanPageSection::PythonNodes,
            expected_rows: &["score_orders"],
        },
        SectionTestCase {
            description: "held back",
            section: PlanPageSection::HeldBack,
            expected_rows: &["daily_revenue", "revenue_report"],
        },
        SectionTestCase {
            description: "skipped by --changed",
            section: PlanPageSection::Unchanged,
            expected_rows: &["not_run", "idle_seed"],
        },
        SectionTestCase {
            description: "warnings, largest group first and each group in plan order",
            section: PlanPageSection::Warnings,
            expected_rows: &[
                "scenario 'checkout' step 'pay' is stale",
                "scenario 'checkout' step 'ship' is stale",
                "scenario 'returns' step `refund` is stale",
                "model stg__orders_2 has no tests.",
                "model stg__customers_3 has no tests.",
                "orders: already-built dates keep their old results",
            ],
        },
    ];
    for test_case in test_cases {
        let document = full_plan();

        let whole = plan_section_page(&document, test_case.section, ((0, 50), None));
        let second = plan_section_page(&document, test_case.section, ((1, 1), None));

        assert_eq!(
            (labels(&whole.rows), whole.total, labels(&second.rows)),
            (
                owned(test_case.expected_rows),
                test_case.expected_rows.len(),
                owned(&test_case.expected_rows[1.min(test_case.expected_rows.len())..])
                    .into_iter()
                    .take(1)
                    .collect::<Vec<_>>(),
            ),
            "{}: every row once, and a later page starts where the first left off",
            test_case.description
        );
    }
}

#[test]
fn given_a_name_filter_when_paging_then_matching_rows_page_with_their_count() {
    let test_cases = [
        PageTestCase {
            description: "no filter pages the section",
            section: PlanPageSection::RoutineModels,
            window: (0, 2),
            name: None,
            expected_rows: &["customers", "Customer_Notes"],
            expected_total: 3,
            expected_section_total: 3,
        },
        PageTestCase {
            description: "a substring matches regardless of case",
            section: PlanPageSection::RoutineModels,
            window: (0, 50),
            name: Some("CUST"),
            expected_rows: &["customers", "Customer_Notes"],
            expected_total: 2,
            expected_section_total: 3,
        },
        PageTestCase {
            description: "matches page like rows",
            section: PlanPageSection::RoutineModels,
            window: (1, 1),
            name: Some("cust"),
            expected_rows: &["Customer_Notes"],
            expected_total: 2,
            expected_section_total: 3,
        },
        PageTestCase {
            description: "node keys match by name",
            section: PlanPageSection::Unchanged,
            window: (0, 50),
            name: Some("idle"),
            expected_rows: &["idle_seed"],
            expected_total: 1,
            expected_section_total: 2,
        },
        PageTestCase {
            description: "warnings match by message",
            section: PlanPageSection::Warnings,
            window: (0, 50),
            name: Some("checkout"),
            expected_rows: &[
                "scenario 'checkout' step 'pay' is stale",
                "scenario 'checkout' step 'ship' is stale",
            ],
            expected_total: 2,
            expected_section_total: 6,
        },
        PageTestCase {
            description: "a retention change matches by its models",
            section: PlanPageSection::Retention,
            window: (0, 50),
            name: Some("EVENTS"),
            expected_rows: &["main.events"],
            expected_total: 1,
            expected_section_total: 1,
        },
        PageTestCase {
            description: "a renamed model matches by its old name",
            section: PlanPageSection::Renamed,
            window: (0, 50),
            name: Some("orders_v1"),
            expected_rows: &["old_orders"],
            expected_total: 1,
            expected_section_total: 1,
        },
        PageTestCase {
            description: "nothing matching is an empty page",
            section: PlanPageSection::FirstRun,
            window: (0, 50),
            name: Some("nowhere"),
            expected_rows: &[],
            expected_total: 0,
            expected_section_total: 2,
        },
    ];
    for test_case in test_cases {
        let page = plan_section_page(
            &full_plan(),
            test_case.section,
            (test_case.window, test_case.name),
        );

        assert_eq!(
            (labels(&page.rows), page.total, page.section_total),
            (
                owned(test_case.expected_rows),
                test_case.expected_total,
                test_case.expected_section_total
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_model_pages_when_read_then_rows_carry_diff_counts_runs_warnings_and_window_starts() {
    let test_cases = [
        ModelPageTestCase {
            description: "changed models count their diffs, without the text, and bring their runs and warnings",
            section: PlanPageSection::QueryChanged,
            expected_stats: vec![
                json!({"query": {"added": 1, "removed": 1, "whitespace_only": true}}),
                json!({
                    "query": {"added": 1, "removed": 1, "whitespace_only": false},
                    "config": {"added": 1, "removed": 1, "whitespace_only": false},
                }),
            ],
            expected_runs: &["model:orders", "model:payments"],
            expected_warnings: &["orders: already-built dates keep their old results"],
            expected_starts: json!({}),
        },
        ModelPageTestCase {
            description: "routine models bring their window starts",
            section: PlanPageSection::RoutineModels,
            expected_stats: vec![Value::Null, Value::Null, Value::Null],
            expected_runs: &["model:customers", "model:Customer_Notes", "model:events"],
            expected_warnings: &[],
            expected_starts: json!({"events": "2026-11-01"}),
        },
    ];
    for test_case in test_cases {
        let page = plan_section_page(&full_plan(), test_case.section, ((0, 50), None));

        assert_eq!(
            (
                page.rows
                    .iter()
                    .map(|row| row.get("diff_stats").cloned().unwrap_or_default())
                    .collect::<Vec<_>>(),
                page.rows.iter().any(|row| row.get("query_diff").is_some()),
                page.runs
                    .iter()
                    .map(|run| run["node"].clone())
                    .collect::<Vec<_>>(),
                page.warnings
                    .iter()
                    .map(|warning| warning["message"].clone())
                    .collect::<Vec<_>>(),
                Value::Object(page.cursor_window_starts),
            ),
            (
                test_case.expected_stats,
                false,
                test_case
                    .expected_runs
                    .iter()
                    .map(|node| json!(node))
                    .collect(),
                test_case
                    .expected_warnings
                    .iter()
                    .map(|message| json!(message))
                    .collect(),
                test_case.expected_starts,
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_plan_when_summarised_then_sections_are_counts_and_no_row_list_is_left() {
    let test_cases = [
        SummaryFieldTestCase {
            description: "models that can't be planned first",
            pointer: "/sections/0/section",
            expected_value: json!("planning_error"),
        },
        SummaryFieldTestCase {
            description: "then models the window skips",
            pointer: "/sections/1/section",
            expected_value: json!("window_before_cursor_start"),
        },
        SummaryFieldTestCase {
            description: "then full refresh",
            pointer: "/sections/2/section",
            expected_value: json!("full_refresh"),
        },
        SummaryFieldTestCase {
            description: "routine models after the changes",
            pointer: "/sections/9/section",
            expected_value: json!("routine_models"),
        },
        SummaryFieldTestCase {
            description: "warnings last",
            pointer: "/sections/26/section",
            expected_value: json!("warnings"),
        },
        SummaryFieldTestCase {
            description: "no section beyond the plan's",
            pointer: "/sections/27",
            expected_value: Value::Null,
        },
        SummaryFieldTestCase {
            description: "a section's size",
            pointer: "/sections/7/total",
            expected_value: json!(2),
        },
        SummaryFieldTestCase {
            description: "pages count each section",
            pointer: "/pages/routine_models",
            expected_value: json!(3),
        },
        SummaryFieldTestCase {
            description: "models are a row list",
            pointer: "/models",
            expected_value: json!([]),
        },
        SummaryFieldTestCase {
            description: "warnings are a row list",
            pointer: "/warnings",
            expected_value: json!([]),
        },
        SummaryFieldTestCase {
            description: "source loads are a row list",
            pointer: "/source_loads",
            expected_value: json!([]),
        },
        SummaryFieldTestCase {
            description: "migrations are a row list",
            pointer: "/migrations",
            expected_value: json!([]),
        },
        SummaryFieldTestCase {
            description: "--changed runs are a row list",
            pointer: "/changed/runs",
            expected_value: json!([]),
        },
        SummaryFieldTestCase {
            description: "--changed unchanged nodes are a row list",
            pointer: "/changed/unchanged",
            expected_value: json!([]),
        },
        SummaryFieldTestCase {
            description: "seeds count by reason: first run",
            pointer: "/pages/first_run_seeds",
            expected_value: json!(1),
        },
        SummaryFieldTestCase {
            description: "seeds count by reason: changed",
            pointer: "/pages/changed_seeds",
            expected_value: json!(1),
        },
        SummaryFieldTestCase {
            description: "seeds count by reason: unchanged",
            pointer: "/pages/unchanged_seeds",
            expected_value: json!(1),
        },
        SummaryFieldTestCase {
            description: "warnings grouped by template, largest first, with how many things the group names",
            pointer: "/warning_groups",
            expected_value: json!([
                {"template": "scenario … step … is stale", "count": 3, "subject": {"noun": "scenario", "count": 2}},
                {"template": "model … has no tests.", "count": 2, "subject": null},
                {"template": "already-built dates keep their old results", "count": 1, "subject": null},
            ]),
        },
        SummaryFieldTestCase {
            description: "migration steps leave out the renamed model's own migration",
            pointer: "/migration_steps",
            expected_value: json!(4),
        },
        SummaryFieldTestCase {
            description: "--changed facts stay",
            pointer: "/changed/reach",
            expected_value: json!("all"),
        },
        SummaryFieldTestCase {
            description: "window starts are per model",
            pointer: "/synced_copy/cursor_window_starts",
            expected_value: json!({}),
        },
        SummaryFieldTestCase {
            description: "metadata is left out",
            pointer: "/metadata",
            expected_value: Value::Null,
        },
    ];
    for test_case in test_cases {
        let summary: Value = plan_summary(&full_plan());

        assert_eq!(
            summary
                .pointer(test_case.pointer)
                .cloned()
                .unwrap_or_default(),
            test_case.expected_value,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_plan_when_summarised_then_each_section_counts_the_notes_its_rows_share() {
    let test_cases = [
        NoteTestCase {
            description: "a note every first-run row shares counts all of them",
            section: "first_run",
            expected_note: json!({"kind": "run_reason", "value": "no completed record", "count": 2}),
        },
        NoteTestCase {
            description: "a note some routine rows share counts those",
            section: "routine_models",
            expected_note: json!({"kind": "run_reason", "value": "no completed record", "count": 2}),
        },
        NoteTestCase {
            description: "a routine row run for what it reads",
            section: "routine_models",
            expected_note: json!({"kind": "reads", "value": "orders", "count": 1}),
        },
        NoteTestCase {
            description: "a whitespace-only query diff",
            section: "query_changed",
            expected_note: json!({"kind": "query_whitespace_only", "value": null, "count": 1}),
        },
        NoteTestCase {
            description: "what refreshes a held-back node",
            section: "held_back",
            expected_note: json!({"kind": "refreshed_by", "value": "finance", "count": 1}),
        },
    ];
    for test_case in test_cases {
        let summary: Value = plan_summary(&full_plan());

        let notes: Vec<Value> = summary["sections"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|entry| entry["section"] == test_case.section)
            .flat_map(|entry| entry["notes"].as_array().cloned().unwrap_or_default())
            .collect();
        assert!(
            notes.contains(&test_case.expected_note),
            "{}: {notes:?}",
            test_case.description
        );
    }
}

#[test]
fn given_diffs_when_counted_then_whitespace_only_needs_identical_text_without_whitespace() {
    let test_cases = [
        DiffStatsTestCase {
            description: "spacing changed only",
            diff: &["-select  id,\tname", "+select id, name"],
            expected_stats: DiffStats {
                added: 1,
                removed: 1,
                whitespace_only: true,
            },
        },
        DiffStatsTestCase {
            description: "a line rewrapped",
            diff: &["-select id, name", "+select id,", "+  name"],
            expected_stats: DiffStats {
                added: 2,
                removed: 1,
                whitespace_only: true,
            },
        },
        DiffStatsTestCase {
            description: "a keyword's case changed is not whitespace only",
            diff: &["-select id", "+SELECT id"],
            expected_stats: DiffStats {
                added: 1,
                removed: 1,
                whitespace_only: false,
            },
        },
        DiffStatsTestCase {
            description: "headers and context are not changed lines",
            diff: &[
                "--- a/orders.sql",
                "+++ b/orders.sql",
                "@@ -1,2 +1,2 @@",
                " from orders",
                "-where 1",
                "+where 2",
            ],
            expected_stats: DiffStats {
                added: 1,
                removed: 1,
                whitespace_only: false,
            },
        },
        DiffStatsTestCase {
            description: "a removed SQL comment inside a hunk is a changed line, not a header",
            diff: &[
                "--- a/orders.sql",
                "+++ b/orders.sql",
                "@@ -1,2 +1 @@",
                "--- legacy",
                "-where x = 1",
                "+where  x = 1",
            ],
            expected_stats: DiffStats {
                added: 1,
                removed: 2,
                whitespace_only: false,
            },
        },
        DiffStatsTestCase {
            description: "no changed lines",
            diff: &[" select id"],
            expected_stats: DiffStats {
                added: 0,
                removed: 0,
                whitespace_only: false,
            },
        },
    ];
    for test_case in test_cases {
        let stats = diff_stats(&owned(test_case.diff));

        assert_eq!(stats, test_case.expected_stats, "{}", test_case.description);
    }
}

#[test]
fn given_a_document_whose_diffs_were_detached_when_paged_then_counts_remain_and_text_is_kept_apart()
{
    let test_cases = [
        DetachTestCase {
            description: "a query diff",
            model: "orders",
            expected_detached: Some(&[
                "--- a",
                "+++ b",
                "@@ -1 +1 @@",
                "-select  id",
                "+select id",
            ]),
        },
        DetachTestCase {
            description: "a model without a diff",
            model: "customers",
            expected_detached: None,
        },
    ];
    for test_case in test_cases {
        let original = full_plan();

        let (detached, diffs) = detach_model_diffs(original.clone());

        let page = |document: &Value| {
            plan_section_page(document, PlanPageSection::QueryChanged, ((0, 50), None)).rows
        };
        let model_diff = |document: &Value| {
            document["models"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|row| row["name"] == test_case.model)
                .and_then(|row| row.get("query_diff").cloned())
        };
        assert_eq!(
            (
                page(&detached),
                diffs
                    .get(test_case.model)
                    .and_then(|diff| diff.get("query_diff").cloned()),
                model_diff(&detached),
                model_diff(&original),
            ),
            (
                page(&original),
                test_case.expected_detached.map(|lines| json!(lines)),
                None,
                test_case.expected_detached.map(|lines| json!(lines)),
            ),
            "{}: pages read the same counts, and the text leaves the document whole",
            test_case.description
        );
    }
}

#[test]
fn given_a_large_first_build_when_summarised_then_the_summary_stays_small() {
    let test_cases = [
        SummarySizeTestCase {
            description: "10,000 first-run models",
            models: 10_000,
            changed_with_warnings: false,
            expected_max_bytes: 1_024,
        },
        SummarySizeTestCase {
            description: "10,000 models under --changed, each with a warning of its own",
            models: 10_000,
            changed_with_warnings: true,
            expected_max_bytes: 16_384,
        },
    ];
    for test_case in test_cases {
        let document = first_build(test_case.models, test_case.changed_with_warnings);

        let summary = serde_json::to_vec(&plan_summary(&document)).unwrap_or_default();

        assert!(
            summary.len() < test_case.expected_max_bytes,
            "{}: summary {} bytes of a {}-byte document",
            test_case.description,
            summary.len(),
            serde_json::to_vec(&document).unwrap_or_default().len()
        );
    }
}

#[test]
fn given_config_diffs_when_paged_whole_or_detached_then_short_ones_read_as_one_config_change() {
    let test_cases = [
        ConfigChangeTestCase {
            description: "one plain key changes",
            section: PlanPageSection::ConfigChanged,
            model: "totals",
            expected_change: Some("full_refresh true → false"),
        },
        ConfigChangeTestCase {
            description: "a key set from nothing to a list, beside a query change",
            section: PlanPageSection::QueryChanged,
            model: "payments",
            expected_change: Some("tags [] → [\"finance\"]"),
        },
        ConfigChangeTestCase {
            description: "three keys are too many for one line",
            section: PlanPageSection::SchemaChanged,
            model: "ledger",
            expected_change: None,
        },
        ConfigChangeTestCase {
            description: "a key nested in an object is not a top-level change",
            section: PlanPageSection::FirstRun,
            model: "daily_orders",
            expected_change: None,
        },
        ConfigChangeTestCase {
            description: "a comma moved by a new last key is not a change of the key before",
            section: PlanPageSection::FirstRun,
            model: "daily_payments",
            expected_change: Some("tags (unset) → none"),
        },
        ConfigChangeTestCase {
            description: "no config diff",
            section: PlanPageSection::QueryChanged,
            model: "orders",
            expected_change: None,
        },
    ];
    for test_case in test_cases {
        let whole = full_plan();
        let (detached, _) = detach_model_diffs(whole.clone());

        let change = |document: &Value| {
            plan_section_page(document, test_case.section, ((0, 50), None))
                .rows
                .into_iter()
                .find(|row| row["name"] == test_case.model)
                .and_then(|row| row.get("config_change").cloned())
        };
        assert_eq!(
            (change(&whole), change(&detached)),
            (
                test_case.expected_change.map(|change| json!(change)),
                test_case.expected_change.map(|change| json!(change)),
            ),
            "{}: the same with the diff text in the document or kept apart",
            test_case.description
        );
    }
}

#[test]
fn given_renames_when_paged_or_summarised_then_rows_keep_their_marks_and_steps_leave_renames_out() {
    let test_cases = [
        SummaryFieldTestCase {
            description: "a renamed row names the model it was renamed from",
            pointer: "/renamed/origin_model",
            expected_value: json!("orders_v1"),
        },
        SummaryFieldTestCase {
            description: "a renamed row says what stays at its old name",
            pointer: "/renamed/old_name/action",
            expected_value: json!("view_only"),
        },
        SummaryFieldTestCase {
            description: "a rename's own migration is marked",
            pointer: "/migrations/0/is_rename",
            expected_value: json!(true),
        },
        SummaryFieldTestCase {
            description: "another migration is not",
            pointer: "/migrations/1/is_rename",
            expected_value: json!(false),
        },
    ];
    let plan = full_plan();
    let observed = json!({
        "renamed": plan_section_page(&plan, PlanPageSection::Renamed, ((0, 1), None)).rows[0],
        "migrations": plan_section_page(&plan, PlanPageSection::Migrations, ((0, 50), None)).rows,
    });
    for test_case in test_cases {
        assert_eq!(
            observed
                .pointer(test_case.pointer)
                .cloned()
                .unwrap_or_default(),
            test_case.expected_value,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_schema_retention_change_without_models_when_filtered_then_its_schema_name_matches() {
    let test_cases = [
        PageTestCase {
            description: "a schema's retention change matches by the schema's name",
            section: PlanPageSection::Retention,
            window: (0, 50),
            name: Some("ARCHIVE"),
            expected_rows: &["analytics_archive"],
            expected_total: 1,
            expected_section_total: 2,
        },
        PageTestCase {
            description: "a table's retention change still matches by its models",
            section: PlanPageSection::Retention,
            window: (0, 50),
            name: Some("events"),
            expected_rows: &["main.events"],
            expected_total: 1,
            expected_section_total: 2,
        },
    ];
    let mut plan = full_plan();
    plan["retention"]
        .as_array_mut()
        .expect("retention rows")
        .push(json!({"scope": "schema", "name": "analytics_archive", "models": [], "desired_days": 30, "actual_days": 7, "direction": "increase"}));
    for test_case in test_cases {
        let page = plan_section_page(&plan, test_case.section, (test_case.window, test_case.name));

        assert_eq!(
            (labels(&page.rows), page.total, page.section_total),
            (
                owned(test_case.expected_rows),
                test_case.expected_total,
                test_case.expected_section_total
            ),
            "{}",
            test_case.description
        );
    }
}
