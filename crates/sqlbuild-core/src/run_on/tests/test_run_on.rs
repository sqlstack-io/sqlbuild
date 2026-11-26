use serde_json::json;

use crate::cli_text::models::CliStyle;
use crate::run_on::_helpers::explain_rows::refusal_explain_rows;
use crate::run_on::main::apply_local_refusals::apply_local_refusals;
use crate::run_on::main::read_local_refusal::read_local_refusal;
use crate::run_on::main::refusal_causes::refusal_causes;
use crate::run_on::main::render_refusal_report::render_refusal_report;
use crate::run_on::main::resolve_run_on::resolve_run_on;
use crate::run_on::main::run_on_header::run_on_header;
use crate::run_on::main::run_on_inspection_line::run_on_inspection_line;
use crate::run_on::models::{LocalRefusal, RunOn, RunOnDecision};
use crate::run_on::tests::helpers::{
    layers, local, read_digest, refusal_digest, refused, refused_in_macro_output,
    refused_in_macro_source, refused_with_code, run_on_document, unplaced,
};
use crate::run_on::tests::test_types::{
    CausesTestCase, HeaderTestCase, InspectionTestCase, JsonTestCase, LocalRefusalTestCase,
    ReadTestCase, RenderTestCase, ResolveTestCase,
};

use crate::run_output::models::{ExplainRow, HeaderCause};

const PLAIN: CliStyle = CliStyle { use_color: false };
const NOUNS: (&str, &str) = ("test", "tests");

#[test]
fn given_run_on_layers_when_resolving_then_flag_beats_target_beats_settings_beats_auto() {
    let test_cases = [
        ResolveTestCase {
            description: "nothing set is auto",
            layers: layers(None, None, None),
            expected_run_on: RunOn::Auto,
        },
        ResolveTestCase {
            description: "settings apply without a target or flag",
            layers: layers(None, None, Some("warehouse")),
            expected_run_on: RunOn::Warehouse,
        },
        ResolveTestCase {
            description: "the target replaces settings",
            layers: layers(None, Some("local"), Some("warehouse")),
            expected_run_on: RunOn::Local,
        },
        ResolveTestCase {
            description: "the flag replaces the target",
            layers: layers(Some("auto"), Some("local"), Some("warehouse")),
            expected_run_on: RunOn::Auto,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            resolve_run_on(&test_case.layers),
            test_case.expected_run_on,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_decisions_when_counting_causes_then_groups_by_code_and_construct_most_first() {
    let test_cases = [
        CausesTestCase {
            description: "every test local has no causes",
            decisions: vec![local("a")],
            expected_causes: Vec::new(),
            expected_rows: Vec::new(),
        },
        CausesTestCase {
            description: "refusals are counted per cause and listed per place",
            decisions: vec![
                local("a"),
                refused("b", "fn:HASH", 26),
                refused("c", "fn:HASH", 26),
                refused_in_macro_source("d"),
                unplaced("e"),
            ],
            expected_causes: vec![
                HeaderCause {
                    label: "fn:HASH".to_owned(),
                    count: 3,
                    detail: "SQ0008 \u{b7} 1 model".to_owned(),
                },
                HeaderCause {
                    label: "missing rule".to_owned(),
                    count: 1,
                    detail: String::new(),
                },
            ],
            expected_rows: vec![
                ExplainRow {
                    count: 2,
                    reason: "fn:HASH".to_owned(),
                    place: "sql/models/staging/stg_orders.sql:26".to_owned(),
                },
                ExplainRow {
                    count: 1,
                    reason: "fn:HASH".to_owned(),
                    place: "@sql.status_hash  sql/macros/status_hash.sql:3".to_owned(),
                },
                ExplainRow {
                    count: 1,
                    reason: "missing rule".to_owned(),
                    place: "node:method_call".to_owned(),
                },
            ],
        },
    ];
    for test_case in test_cases {
        let causes = refusal_causes(&test_case.decisions);
        let rows = refusal_explain_rows(&test_case.decisions);
        assert_eq!(
            causes, test_case.expected_causes,
            "{}",
            test_case.description
        );
        assert_eq!(rows, test_case.expected_rows, "{}", test_case.description);
    }
}

#[test]
fn given_refusals_when_rendering_report_then_groups_by_cause_and_lists_origins() {
    let test_cases = [
        RenderTestCase {
            description: "every test local prints nothing",
            decisions: vec![local("a")],
            errors: true,
            verbose: false,
            columns: 80,
            expected_text: "",
        },
        RenderTestCase {
            description: "--run-on local: one error per cause at 80 columns",
            decisions: vec![
                refused("test_stg_orders", "fn:HASH", 26),
                refused("test_daily_revenue_chain", "fn:HASH", 26),
                refused_with_code("test_customers", "fn:IS_DOUBLE", "SQ0002"),
                unplaced("test_fact_orders"),
                local("calculates_line_total_cents"),
            ],
            errors: true,
            verbose: false,
            columns: 80,
            expected_text: "\nCan't run locally (4 tests):\n\nerror[SQ0008]: HASH gives a different value in DuckDB          2 tests · 1 model\n   = note: Equal inputs still get equal hashes within one run.\n   = help: allow it for local tests in sqlbuild_project.toml with\n           `[testing.local] allow = { hash = true }`; `sqlquack explain hash` says when it is safe\n     --> sql/models/staging/stg_orders.sql:26:3\n      |\n   26 |   HASH(status) AS status_hash\n      |   ^^^^^^^^^^^^ DuckDB computes its own hash, never Snowflake's\n\nerror[SQ0002]: fn:IS_DOUBLE has no DuckDB equivalent            1 test · 1 model\n   = help: run these tests on the warehouse with --run-on auto\n     --> sql/models/staging/stg_orders.sql:40:3\n      |\n   40 |   IS_DOUBLE(payload) AS is_number\n      |   ^^^^^^^^^^^^^^^^^ no rule converts this function\n\nerror: missing rule                                                       1 test\n   └── node:method_call  1 test   test_fact_orders\n   = note: no rule converts METHOD CALL\n   = help: run these tests on the warehouse with --run-on auto\n",
        },
        RenderTestCase {
            description: "--run-on local at 160 columns",
            decisions: vec![
                refused("test_stg_orders", "fn:HASH", 26),
                refused("test_daily_revenue_chain", "fn:HASH", 26),
                refused_with_code("test_customers", "fn:IS_DOUBLE", "SQ0002"),
                unplaced("test_fact_orders"),
            ],
            errors: true,
            verbose: false,
            columns: 160,
            expected_text: "\nCan't run locally (4 tests):\n\nerror[SQ0008]: HASH gives a different value in DuckDB                                                                                          2 tests · 1 model\n   = note: Equal inputs still get equal hashes within one run.\n   = help: allow it for local tests in sqlbuild_project.toml with\n           `[testing.local] allow = { hash = true }`; `sqlquack explain hash` says when it is safe\n     --> sql/models/staging/stg_orders.sql:26:3\n      |\n   26 |   HASH(status) AS status_hash\n      |   ^^^^^^^^^^^^ DuckDB computes its own hash, never Snowflake's\n\nerror[SQ0002]: fn:IS_DOUBLE has no DuckDB equivalent                                                                                            1 test · 1 model\n   = help: run these tests on the warehouse with --run-on auto\n     --> sql/models/staging/stg_orders.sql:40:3\n      |\n   40 |   IS_DOUBLE(payload) AS is_number\n      |   ^^^^^^^^^^^^^^^^^ no rule converts this function\n\nerror: missing rule                                                                                                                                       1 test\n   └── node:method_call  1 test   test_fact_orders\n   = note: no rule converts METHOD CALL\n   = help: run these tests on the warehouse with --run-on auto\n",
        },
        RenderTestCase {
            description: "auto --verbose: the tests run on the warehouse, so each cause is a note",
            decisions: vec![
                refused("test_stg_orders", "fn:HASH", 26),
                unplaced("test_fact_orders"),
            ],
            errors: false,
            verbose: true,
            columns: 80,
            expected_text: "\nOn the warehouse (2 tests):\n\nnote[SQ0008]: HASH gives a different value in DuckDB            1 test · 1 model\n   = note: Equal inputs still get equal hashes within one run.\n   = help: allow it for local tests in sqlbuild_project.toml with\n           `[testing.local] allow = { hash = true }`; `sqlquack explain hash` says when it is safe\n     --> sql/models/staging/stg_orders.sql:26:3\n      |\n   26 |   HASH(status) AS status_hash\n      |   ^^^^^^^^^^^^ DuckDB computes its own hash, never Snowflake's\n\nnote: missing rule                                                        1 test\n   └── node:method_call  1 test   test_fact_orders\n   = note: no rule converts METHOD CALL\n",
        },
        RenderTestCase {
            description: "two places of one cause are one group with both frames",
            decisions: vec![
                refused("test_stg_orders", "fn:HASH", 26),
                refused("test_customers", "fn:HASH", 31),
            ],
            errors: true,
            verbose: false,
            columns: 80,
            expected_text: "\nCan't run locally (2 tests):\n\nerror[SQ0008]: HASH gives a different value in DuckDB\n   2 tests · 1 model · 2 places\n   = note: Equal inputs still get equal hashes within one run.\n   = help: allow it for local tests in sqlbuild_project.toml with\n           `[testing.local] allow = { hash = true }`; `sqlquack explain hash` says when it is safe\n\n   1. stg_orders  1 test\n        --> sql/models/staging/stg_orders.sql:26:3\n         |\n      26 |   HASH(status) AS status_hash\n         |   ^^^^^^^^^^^^ DuckDB computes its own hash, never Snowflake's\n\n   2. stg_orders  1 test\n        --> sql/models/staging/stg_orders.sql:31:3\n         |\n      31 |   HASH(status) AS status_hash\n         |   ^^^^^^^^^^^^ DuckDB computes its own hash, never Snowflake's\n",
        },
    ];
    for test_case in test_cases {
        let text = render_refusal_report(
            &test_case.decisions,
            (test_case.errors, test_case.verbose),
            NOUNS,
            (test_case.columns, PLAIN),
        );
        assert_eq!(text, test_case.expected_text, "{}", test_case.description);
    }
}

#[test]
fn given_decision_when_inspecting_then_names_where_it_runs_and_why() {
    let test_cases = [
        InspectionTestCase {
            description: "a local test",
            decision: local("a"),
            expected_line: "  runs on: local\n",
        },
        InspectionTestCase {
            description: "a refused test names the refusal",
            decision: refused("b", "fn:HASH", 26),
            expected_line: "  runs on: warehouse  (a known difference that isn't allowed: fn:HASH)\n",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            run_on_inspection_line(&test_case.decision),
            test_case.expected_line,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_decisions_when_adding_json_then_marks_checks_and_lists_refusals_with_tests() {
    let test_cases = [JsonTestCase {
        description: "checks name where they ran; refusals are deduplicated with their tests",
        decisions: vec![
            local("a"),
            refused("b", "fn:HASH", 26),
            refused("c", "fn:HASH", 26),
        ],
        expected_checks: json!([
            {"name": "a", "run_on": "local", "run_on_reason": null},
            {"name": "b", "run_on": "warehouse", "run_on_reason": "a known difference that isn't allowed: fn:HASH"},
            {"name": "c", "run_on": "warehouse", "run_on_reason": "a known difference that isn't allowed: fn:HASH"},
        ]),
        expected_summary: json!({"total_count": 2, "local_count": 1, "warehouse_count": 2}),
        expected_refusals: json!([{"tests": ["b", "c"], "code": "SQ0008", "line": 26}]),
    }];
    for test_case in test_cases {
        let document = run_on_document(&test_case.decisions);
        assert_eq!(
            document["checks"], test_case.expected_checks,
            "{}",
            test_case.description
        );
        assert_eq!(
            document["summary"], test_case.expected_summary,
            "{}",
            test_case.description
        );
        assert_eq!(
            refusal_digest(&document),
            test_case.expected_refusals,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_runner_json_when_reading_decisions_then_keeps_refusal_fields() {
    let test_cases = [
        ReadTestCase {
            description: "a refusal keeps its label and width",
            runner_json: r#"{"decisions": [{"name": "a", "run_on": "warehouse", "refusals": [
                {"category": "missing rule", "construct": "fn:FLATTEN", "reason": "r",
                 "code": "SQ0002", "line": 3, "column": 5, "width": 7, "notes": ["n"]}]}]}"#,
            expected_decisions: Some(vec![(Some("missing rule: fn:FLATTEN".to_owned()), 7)]),
        },
        ReadTestCase {
            description: "JSON without decisions is rejected",
            runner_json: "{}",
            expected_decisions: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            read_digest(test_case.runner_json),
            test_case.expected_decisions,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_refusal_in_macro_when_rendering_then_frames_the_macro_source_or_the_call_site() {
    let test_cases = [
        RenderTestCase {
            description: "the macro's own file holds the construct, so the frame is there",
            decisions: vec![refused_in_macro_source("test_orders")],
            errors: true,
            verbose: false,
            columns: 80,
            expected_text: "\nCan't run locally (1 test):\n\nerror[SQ0008]: HASH gives a different value in DuckDB           1 test · 1 model\n   = note: Equal inputs still get equal hashes within one run.\n   = help: allow it for local tests in sqlbuild_project.toml with\n           `[testing.local] allow = { hash = true }`; `sqlquack explain hash` says when it is safe\n    --> sql/macros/status_hash.sql:3:5\n     |\n   3 |     HASH(status)\n     |     ^^^^^^^^^^^^ DuckDB computes its own hash, never Snowflake's\n    = used by 1 model:\n      stg_orders (1)\n",
        },
        RenderTestCase {
            description: "the construct isn't in the macro's file, so the call site is framed",
            decisions: vec![refused_in_macro_output("test_orders")],
            errors: true,
            verbose: false,
            columns: 80,
            expected_text: "\nCan't run locally (1 test):\n\nerror[SQ0008]: HASH gives a different value in DuckDB           1 test · 1 model\n   = note: Equal inputs still get equal hashes within one run.\n   = help: allow it for local tests in sqlbuild_project.toml with\n           `[testing.local] allow = { hash = true }`; `sqlquack explain hash` says when it is safe\n     --> sql/models/staging/stg_orders.sql:12:5\n      |\n   12 |   status, HASH(status) AS status_hash\n      |           ^^^^^^^^^^^^ DuckDB computes its own hash, never Snowflake's\n      = expanded from @py.status_hash at sql/models/staging/stg_orders.sql:12:5\n",
        },
    ];
    for test_case in test_cases {
        let text = render_refusal_report(
            &test_case.decisions,
            (test_case.errors, test_case.verbose),
            NOUNS,
            (test_case.columns, PLAIN),
        );
        assert_eq!(text, test_case.expected_text, "{}", test_case.description);
    }
}

#[test]
fn given_a_local_test_refused_while_running_when_applied_then_auto_moves_it_and_local_keeps_it() {
    let guard = LocalRefusal {
        category: "SQLQuack guard".to_owned(),
        construct: "array_agg_order".to_owned(),
        reason: "ARRAY_AGG of several values in no defined order".to_owned(),
        width: 1,
        ..LocalRefusal::default()
    };
    let test_cases = [
        LocalRefusalTestCase {
            description: "auto runs the test on the warehouse, giving the guard as the reason",
            mode: RunOn::Auto,
            expected_decision: RunOnDecision {
                name: "test_orders".to_owned(),
                run_on: "warehouse".to_owned(),
                refusals: vec![guard.clone()],
            },
        },
        LocalRefusalTestCase {
            description: "local keeps the test local and lists the guard it can't run past",
            mode: RunOn::Local,
            expected_decision: RunOnDecision {
                name: "test_orders".to_owned(),
                run_on: "local".to_owned(),
                refusals: vec![guard.clone()],
            },
        },
    ];
    for test_case in test_cases {
        let mut decisions = vec![local("test_orders"), local("test_customers")];
        let fields = [
            ("test", "test_orders"),
            ("category", "SQLQuack guard"),
            ("construct", "array_agg_order"),
            ("reason", "ARRAY_AGG of several values in no defined order"),
        ];

        apply_local_refusals(
            &mut decisions,
            vec![read_local_refusal(
                fields
                    .into_iter()
                    .map(|(key, value)| (key.to_owned(), value.to_owned()))
                    .collect(),
            )],
            test_case.mode,
        );

        assert_eq!(
            decisions,
            vec![test_case.expected_decision, local("test_customers")],
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_decisions_when_building_header_then_counts_where_tests_run_by_mode() {
    let decisions: Vec<RunOnDecision> = vec![
        local("a"),
        local("b"),
        refused("c", "fn:HASH", 26),
        unplaced("d"),
    ];
    let local_refused: Vec<RunOnDecision> = decisions
        .iter()
        .cloned()
        .map(|decision| RunOnDecision {
            run_on: "local".to_owned(),
            ..decision
        })
        .collect();
    let test_cases = [
        HeaderTestCase {
            description: "auto sends refused tests to the warehouse and explains them",
            decisions: decisions.clone(),
            mode: RunOn::Auto,
            explain: true,
            expected_counts: (2, 2, 0),
            expected_causes: 2,
            expected_explain_rows: 2,
        },
        HeaderTestCase {
            description: "--run-on local runs nothing refused and lists the causes",
            decisions: local_refused,
            mode: RunOn::Local,
            explain: true,
            expected_counts: (2, 0, 2),
            expected_causes: 2,
            expected_explain_rows: 0,
        },
        HeaderTestCase {
            description: "--run-on warehouse names no causes",
            decisions: decisions
                .iter()
                .cloned()
                .map(|decision| RunOnDecision {
                    run_on: "warehouse".to_owned(),
                    refusals: Vec::new(),
                    ..decision
                })
                .collect(),
            mode: RunOn::Warehouse,
            explain: true,
            expected_counts: (0, 4, 0),
            expected_causes: 0,
            expected_explain_rows: 0,
        },
    ];
    for test_case in test_cases {
        let header = run_on_header(
            &test_case.decisions,
            (test_case.mode, test_case.explain),
            ("Tests", NOUNS),
        );
        assert_eq!(
            (
                (header.local, header.warehouse, header.not_run),
                header.causes.len(),
                header.explain.len()
            ),
            (
                test_case.expected_counts,
                test_case.expected_causes,
                test_case.expected_explain_rows
            ),
            "{}",
            test_case.description
        );
    }
}
