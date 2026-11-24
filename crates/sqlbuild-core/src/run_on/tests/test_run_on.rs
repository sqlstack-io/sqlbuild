use serde_json::{Value, json};

use crate::cli_text::models::CliStyle;
use crate::run_on::main::read_run_on_decisions::read_run_on_decisions;
use crate::run_on::main::render_local_refusals::render_local_refusals;
use crate::run_on::main::render_warehouse_census::render_warehouse_census;
use crate::run_on::main::resolve_run_on::resolve_run_on;
use crate::run_on::main::run_on_column::run_on_column;
use crate::run_on::main::run_on_inspection_line::run_on_inspection_line;
use crate::run_on::main::with_run_on_json::with_run_on_json;
use crate::run_on::models::{RunOn, RunOnLayers};
use crate::run_on::tests::helpers::{local, refused, unplaced};
use crate::run_on::tests::test_types::{RenderTestCase, ResolveTestCase};

const PLAIN: CliStyle = CliStyle { use_color: false };
const NOUNS: (&str, &str) = ("test", "tests");

fn layers(flag: Option<&str>, target: Option<&str>, settings: Option<&str>) -> RunOnLayers {
    RunOnLayers {
        flag: flag.map(str::to_owned),
        target: target.map(str::to_owned),
        settings: settings.map(str::to_owned),
    }
}

#[test]
fn given_run_on_layers_when_resolving_then_flag_beats_target_beats_settings_beats_auto() {
    let test_cases = [
        ResolveTestCase {
            description: "nothing set is auto",
            layers: layers(None, None, None),
            expected: RunOn::Auto,
        },
        ResolveTestCase {
            description: "settings apply without a target or flag",
            layers: layers(None, None, Some("warehouse")),
            expected: RunOn::Warehouse,
        },
        ResolveTestCase {
            description: "the target replaces settings",
            layers: layers(None, Some("local"), Some("warehouse")),
            expected: RunOn::Local,
        },
        ResolveTestCase {
            description: "the flag replaces the target",
            layers: layers(Some("auto"), Some("local"), Some("warehouse")),
            expected: RunOn::Auto,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            resolve_run_on(&test_case.layers),
            test_case.expected,
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
fn given_decision_when_inspecting_or_rendering_row_then_names_where_it_runs() {
    assert_eq!(run_on_inspection_line(&local("a")), "  runs on: local\n");
    assert_eq!(
        run_on_inspection_line(&refused("b", "fn:HASH", 26)),
        "  runs on: warehouse  (a known difference that isn't allowed: fn:HASH)\n"
    );
    assert_eq!(run_on_column("local"), "local    ");
    assert_eq!(run_on_column("warehouse"), "warehouse");
}

#[test]
fn given_decisions_when_adding_json_then_marks_checks_and_lists_refusals_with_tests() {
    let payload: String = json!({
        "summary": {"total_count": 2},
        "checks": [{"name": "a"}, {"name": "b"}, {"name": "c"}],
    })
    .to_string();
    let decisions = vec![
        local("a"),
        refused("b", "fn:HASH", 26),
        refused("c", "fn:HASH", 26),
    ];
    let document: Value = serde_json::from_str(
        &with_run_on_json(&payload, ("checks", &["name"]), &decisions, (1, 2)).unwrap(),
    )
    .unwrap();
    assert_eq!(
        document["checks"][0],
        json!({"name": "a", "run_on": "local", "run_on_reason": null})
    );
    assert_eq!(
        document["checks"][1]["run_on_reason"],
        json!("a known difference that isn't allowed: fn:HASH")
    );
    assert_eq!(
        document["summary"],
        json!({"total_count": 2, "local_count": 1, "warehouse_count": 2})
    );
    assert_eq!(document["refusals"].as_array().unwrap().len(), 1);
    assert_eq!(document["refusals"][0]["tests"], json!(["b", "c"]));
    assert_eq!(document["refusals"][0]["code"], json!("SQ0008"));
    assert_eq!(document["refusals"][0]["line"], json!(26));
}

#[test]
fn given_runner_json_when_reading_decisions_then_keeps_refusal_fields() {
    let decisions = read_run_on_decisions(
        r#"{"decisions": [{"name": "a", "run_on": "warehouse", "refusals": [
            {"category": "missing rule", "construct": "fn:FLATTEN", "reason": "r",
             "code": "SQ0002", "line": 3, "column": 5, "width": 7, "notes": ["n"]}]}]}"#,
    )
    .unwrap();
    assert_eq!(decisions.len(), 1);
    assert_eq!(
        decisions[0].reason().as_deref(),
        Some("missing rule: fn:FLATTEN")
    );
    assert_eq!(decisions[0].refusals[0].width, 7);
    assert!(read_run_on_decisions("{}").is_err());
}

#[test]
fn given_refusal_in_macro_output_when_rendering_then_points_at_call_and_shows_generated_sql() {
    let mut decision = refused("test_orders", "fn:HASH", 12);
    decision.refusals[0].column = Some(5);
    decision.refusals[0].snippet = Some("  status, HASH(status) AS status_hash".to_owned());
    decision.refusals[0].snippet_column = Some(11);
    decision.refusals[0].expanded_from =
        Some("@py.status_hash at sql/models/staging/stg_orders.sql:12:5".to_owned());
    let text: String = render_local_refusals(&[decision], true, NOUNS, PLAIN);
    assert!(
        text.contains(" --> sql/models/staging/stg_orders.sql:12:5\n"),
        "{text}"
    );
    assert!(
        text.contains("12 |   status, HASH(status) AS status_hash\n   |           ^^^^^^^^^^^^ "),
        "{text}"
    );
    assert!(
        text.contains(
            "  = expanded from @py.status_hash at sql/models/staging/stg_orders.sql:12:5\n"
        ),
        "{text}"
    );
}
