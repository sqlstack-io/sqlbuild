use serde_json::json;

use crate::cli_text::models::CliStyle;
use crate::run_on::main::apply_local_refusals::apply_local_refusals;
use crate::run_on::main::read_local_refusal::read_local_refusal;
use crate::run_on::main::render_local_refusals::render_local_refusals;
use crate::run_on::main::render_warehouse_census::render_warehouse_census;
use crate::run_on::main::resolve_run_on::resolve_run_on;
use crate::run_on::main::run_on_column::run_on_column;
use crate::run_on::main::run_on_inspection_line::run_on_inspection_line;
use crate::run_on::models::{LocalRefusal, RunOn, RunOnDecision};
use crate::run_on::tests::helpers::{
    layers, local, read_digest, refusal_digest, refused, refused_in_macro_output, run_on_document,
    unplaced,
};
use crate::run_on::tests::test_types::{
    ColumnTestCase, FragmentTestCase, InspectionTestCase, JsonTestCase, LocalRefusalTestCase,
    ReadTestCase, RenderTestCase, ResolveTestCase,
};

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
fn given_decisions_when_rendering_census_then_counts_tests_per_refusal_most_first() {
    let test_cases = [
        RenderTestCase {
            description: "every test local prints nothing",
            decisions: vec![local("a")],
            expected_text: "",
        },
        RenderTestCase {
            description: "refusals are counted per test, most tests first",
            decisions: vec![
                local("a"),
                refused("b", "fn:HASH", 26),
                refused("c", "fn:HASH", 26),
                unplaced("d"),
            ],
            expected_text: "\nOn the warehouse (3 tests)\n  a known difference that isn't allowed: fn:HASH  2 tests\n  missing rule: node:method_call                  1 test\n",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            render_warehouse_census(&test_case.decisions, NOUNS, PLAIN),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_refusals_when_rendering_diagnostics_then_dedupes_by_place_and_lists_blocked_tests() {
    let test_cases = [RenderTestCase {
        description: "one diagnostic per construct and place, with every blocked test",
        decisions: vec![
            refused("test_stg_orders", "fn:HASH", 26),
            refused("test_daily_revenue_chain", "fn:HASH", 26),
            unplaced("test_fact_orders"),
            local("calculates_line_total_cents"),
        ],
        expected_text: "\nCan't run locally (3 tests)\n\nerror[SQ0008]: HASH gives a different value in DuckDB\n --> sql/models/staging/stg_orders.sql:26:3\n   |\n26 |   HASH(status) AS status_hash\n   |   ^^^^^^^^^^^^ DuckDB computes its own hash, never Snowflake's\n   |\n  = note: Equal inputs still get equal hashes within one run.\n  = help: allow it with hash = true under [allow] in a sqlquack.toml\n  = tests: test_stg_orders, test_daily_revenue_chain\n\nerror: missing rule: node:method_call\n  = note: no rule converts METHOD CALL\n  = tests: test_fact_orders\n",
    }];
    for test_case in test_cases {
        assert_eq!(
            render_local_refusals(&test_case.decisions, true, NOUNS, PLAIN),
            test_case.expected_text,
            "{}",
            test_case.description
        );
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
fn given_run_on_when_rendering_row_column_then_pads_to_one_width() {
    let test_cases = [
        ColumnTestCase {
            description: "local is padded to warehouse's width",
            run_on: "local",
            expected_column: "local    ",
        },
        ColumnTestCase {
            description: "warehouse fills the column",
            run_on: "warehouse",
            expected_column: "warehouse",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            run_on_column(test_case.run_on),
            test_case.expected_column,
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
fn given_refusal_in_macro_output_when_rendering_then_points_at_call_and_shows_generated_sql() {
    let test_cases = [FragmentTestCase {
        description: "the call site, the generated SQL and the macro that produced it",
        decisions: vec![refused_in_macro_output("test_orders")],
        expected_fragments: &[
            " --> sql/models/staging/stg_orders.sql:12:5\n",
            "12 |   status, HASH(status) AS status_hash\n   |           ^^^^^^^^^^^^ ",
            "  = expanded from @py.status_hash at sql/models/staging/stg_orders.sql:12:5\n",
        ],
    }];
    for test_case in test_cases {
        let text: String = render_local_refusals(&test_case.decisions, true, NOUNS, PLAIN);
        assert!(
            test_case
                .expected_fragments
                .iter()
                .all(|fragment| text.contains(fragment)),
            "{}: {text}",
            test_case.description
        );
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
