use std::path::PathBuf;

use crate::cli_text::models::CliStyle;
use crate::scenario_command::main::build_local_runtime_artifacts::build_local_runtime_artifacts;
use crate::scenario_command::main::build_runtime_artifacts::build_runtime_artifacts;
use crate::scenario_command::main::render_capture_result::render_capture_result;
use crate::scenario_command::main::render_namespace_line::render_namespace_line;
use crate::scenario_command::main::render_retained_section::render_retained_section;
use crate::scenario_command::main::render_snapshot_sync_result::render_snapshot_sync_result;
use crate::scenario_command::main::scenario_steps::scenario_steps;
use crate::scenario_command::models::{
    AssertionRow, CaptureRow, ExpectedRow, FixtureStatements, FunctionStatements, LoadedRelation,
    LocalRuntimeInputs, ModelArtifactEntry, NamedStatements, RetainedInputs, ScenarioNamespace,
    ScenarioResultRow, WarehouseRuntimeInputs,
};
use crate::scenario_command::tests::helpers::{
    artifact_pairs, child, coded, expectation_sql, expected_pairs, failing_result_row, relation,
    retained, statements, strings,
};
use crate::scenario_command::tests::test_types::{
    CaptureRowTestCase, ChildRowsTestCase, LocalArtifactsTestCase, NamespaceLineTestCase,
    RetainedTestCase, WarehouseArtifactsTestCase,
};
use crate::test_command::main::expectation_child_rows::expectation_child_rows;

const PLAIN: CliStyle = CliStyle { use_color: false };
const CE0: &str = "orders_paid                                                      PASS  2 relations, 3 rows\n    source   raw_orders                                          PASS  2 rows, 41 B\n    seed     countries                                           PASS  1 row, 1.5 KB\n    snapshot tests/_scenario_snapshots/orders_paid/scenario.json\n";
const SE0R: &str = "orders_paid                                                      REFRESHED  2 relations, 3 rows\n    source   raw_orders                                          PASS  2 rows, 41 B\n    seed     countries                                           PASS  1 row, 1.5 KB\n    snapshot tests/_scenario_snapshots/orders_paid/scenario.json\n";
const CF1: &str = "orders_big                                                       \u{1b}[38;5;167mFAIL\u{1b}[0m  0 relations, 0 rows\n    \u{1b}[38;5;167m\u{1b}[1merror[X512]:\u{1b}[0m too many rows\n      \u{1b}[2m= help:\u{1b}[0m narrow it\n    Rerun with --retain to inspect scenario-owned artifacts.\n    source   raw_orders                                          \u{1b}[38;5;167mFAIL\u{1b}[0m  0 rows, 0 B\n    snapshot /elsewhere/scenario.json\n";
const SF1: &str = "orders_big                                                       \u{1b}[38;5;167mFAIL\u{1b}[0m  0 relations, 0 rows\n    \u{1b}[38;5;167m\u{1b}[1merror[X512]:\u{1b}[0m too many rows\n      \u{1b}[2m= help:\u{1b}[0m narrow it\n    source   raw_orders                                          \u{1b}[38;5;167mFAIL\u{1b}[0m  0 rows, 0 B\n    snapshot /elsewhere/scenario.json\n";
const CG0: &str = "orders_kept                                                      FAIL\n    no capture\n    Retained relations:\n      source raw_orders -> main.__sqb_x__source__raw_orders\n      seed   countries\n";
const SG0: &str =
    "orders_kept                                                      FAIL\n    no capture\n";
const ASSERTION_SQL: &str = "SELECT COUNT(*) FROM (SELECT * FROM t WHERE x < 0) AS __scenario_assertion_failures;\n\nSELECT * FROM (SELECT * FROM t WHERE x < 0) AS __scenario_assertion_failures\nLIMIT 10;";

#[test]
fn given_capture_result_when_rendering_then_matches_capture_and_sync_rows() {
    let test_cases = [
        CaptureRowTestCase {
            description: "captured relations with sizes",
            row: CaptureRow {
                name: "orders_paid".to_owned(),
                passed: true,
                relations: Some(vec![
                    relation("source", "raw_orders", (true, 2, 41)),
                    relation("seed", "countries", (true, 1, 1536)),
                ]),
                snapshot_path: Some(
                    "tests/_scenario_snapshots/orders_paid/scenario.json".to_owned(),
                ),
                ..CaptureRow::default()
            },
            use_color: false,
            refresh: true,
            expected_capture_text: CE0,
            expected_sync_text: SE0R,
        },
        CaptureRowTestCase {
            description: "a coded failure with color",
            row: CaptureRow {
                name: "orders_big".to_owned(),
                error: coded(Some("X512"), "too many rows", Some("narrow it")),
                relations: Some(vec![relation("source", "raw_orders", (false, 0, 0))]),
                snapshot_path: Some("/elsewhere/scenario.json".to_owned()),
                ..CaptureRow::default()
            },
            use_color: true,
            refresh: false,
            expected_capture_text: CF1,
            expected_sync_text: SF1,
        },
        CaptureRowTestCase {
            description: "retained inputs without a snapshot",
            row: CaptureRow {
                name: "orders_kept".to_owned(),
                error: coded(None, "no capture", None),
                retained: true,
                retained_inputs: Some(RetainedInputs {
                    fixtures: vec![retained(
                        "source",
                        "raw_orders",
                        "main.__sqb_x__source__raw_orders",
                    )],
                    seeds: strings(&["countries"]),
                }),
                ..CaptureRow::default()
            },
            use_color: false,
            refresh: false,
            expected_capture_text: CG0,
            expected_sync_text: SG0,
        },
    ];

    for test_case in test_cases {
        let style: CliStyle = CliStyle {
            use_color: test_case.use_color,
        };
        assert_eq!(
            (
                render_capture_result(&test_case.row, style),
                render_snapshot_sync_result(&test_case.row, test_case.refresh, style)
            ),
            (
                test_case.expected_capture_text.to_owned(),
                test_case.expected_sync_text.to_owned()
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_warehouse_run_when_building_artifacts_then_orders_and_terminates_statements() {
    let test_cases = [WarehouseArtifactsTestCase {
        description: "every lifecycle stage, skipping empty and unplanned SQL",
        inputs: WarehouseRuntimeInputs {
            prepare_cleanup: Some(statements(&["DROP TABLE a", "DROP TABLE b;  \n"])),
            fixtures: vec![
                FixtureStatements {
                    kind: "source".to_owned(),
                    logical_name: "raw/orders".to_owned(),
                    statements: statements(&["CREATE TABLE f AS SELECT 1"]),
                },
                FixtureStatements {
                    kind: "source".to_owned(),
                    logical_name: "empty".to_owned(),
                    statements: Vec::new(),
                },
            ],
            seeds: vec![NamedStatements {
                name: "countries".to_owned(),
                statements: statements(&["INSERT INTO c VALUES (1)", "   "]),
            }],
            models: vec![
                NamedStatements {
                    name: "orders".to_owned(),
                    statements: statements(&["CREATE TABLE o AS SELECT 1"]),
                },
                NamedStatements {
                    name: "staging".to_owned(),
                    statements: statements(&["CREATE VIEW s AS SELECT 1"]),
                },
                NamedStatements {
                    name: "unplanned".to_owned(),
                    statements: statements(&["SELECT 1"]),
                },
            ],
            model_entries: vec![
                ModelArtifactEntry {
                    name: "orders".to_owned(),
                    relative_path: PathBuf::from("sql/models/marts/orders.sql"),
                },
                ModelArtifactEntry {
                    name: "staging".to_owned(),
                    relative_path: PathBuf::from("staging.sql"),
                },
            ],
            expectations: expectation_sql(),
            final_cleanup: Some(Vec::new()),
        },
        expected_artifacts: vec![
            (
                PathBuf::from("cleanup/prepare.sql"),
                "DROP TABLE a;\n\nDROP TABLE b;",
            ),
            (
                PathBuf::from("fixtures/source__raw__orders.sql"),
                "CREATE TABLE f AS SELECT 1;",
            ),
            (
                PathBuf::from("sql/seeds/countries.sql"),
                "INSERT INTO c VALUES (1);\n\n   ",
            ),
            (
                PathBuf::from("sql/models/marts/orders.sql"),
                "CREATE TABLE o AS SELECT 1;",
            ),
            (
                PathBuf::from("sql/models/staging.sql"),
                "CREATE VIEW s AS SELECT 1;",
            ),
            (
                PathBuf::from("expectations/expected__order_totals.sql"),
                "SELECT 1 EXCEPT SELECT 2",
            ),
            (
                PathBuf::from("expectations/assertion__no_negative.sql"),
                ASSERTION_SQL,
            ),
        ],
    }];

    for test_case in test_cases {
        assert_eq!(
            artifact_pairs(build_runtime_artifacts(&test_case.inputs)),
            expected_pairs(&test_case.expected_artifacts),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_local_replay_when_building_artifacts_then_writes_loads_functions_and_models() {
    let test_cases = [LocalArtifactsTestCase {
        description: "loaded snapshots, functions under their language and model SQL",
        inputs: LocalRuntimeInputs {
            loaded_relations: vec![LoadedRelation {
                kind: "source".to_owned(),
                logical_name: "raw_orders".to_owned(),
                file_path: "/p/tests/_scenario_snapshots/s/sources/raw_orders.jsonl".to_owned(),
                table_name: "__sqb_local__source__raw_orders".to_owned(),
                row_count: 2,
            }],
            functions: vec![
                FunctionStatements {
                    relative_path: PathBuf::from("sql/functions/sql/add_one.sql"),
                    language: "sql".to_owned(),
                    statements: statements(&["CREATE MACRO add_one(x) AS x + 1"]),
                },
                FunctionStatements {
                    relative_path: PathBuf::from("util/score.py"),
                    language: "python".to_owned(),
                    statements: statements(&["CREATE FUNCTION score()", "SELECT 1;"]),
                },
            ],
            models: vec![(PathBuf::from("orders.sql"), "SELECT 1\n\n".to_owned())],
            expectations: expectation_sql(),
        },
        expected_artifacts: vec![
            (
                PathBuf::from("fixtures/source__raw_orders.sql"),
                "-- loaded from /p/tests/_scenario_snapshots/s/sources/raw_orders.jsonl\n-- rows: 2\nCREATE TABLE \"__sqb_local__source__raw_orders\" AS SELECT * FROM '/p/tests/_scenario_snapshots/s/sources/raw_orders.jsonl';",
            ),
            (
                PathBuf::from("sql/functions/sql/add_one.sql"),
                "CREATE MACRO add_one(x) AS x + 1;",
            ),
            (
                PathBuf::from("sql/functions/python/util/score.sql"),
                "CREATE FUNCTION score();\n\nSELECT 1;",
            ),
            (PathBuf::from("sql/models/orders.sql"), "SELECT 1\n\n"),
            (
                PathBuf::from("expectations/expected__order_totals.sql"),
                "SELECT 1 EXCEPT SELECT 2",
            ),
            (
                PathBuf::from("expectations/assertion__no_negative.sql"),
                ASSERTION_SQL,
            ),
        ],
    }];

    for test_case in test_cases {
        assert_eq!(
            artifact_pairs(build_local_runtime_artifacts(&test_case.inputs)),
            expected_pairs(&test_case.expected_artifacts),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_scenario_result_when_listing_checks_then_each_names_its_outcome_and_rows() {
    let test_cases = [
        ChildRowsTestCase {
            description: "failed checks count what differed; passing ones say nothing",
            row: failing_result_row(),
            expected_rows: vec![
                child("expected", "fact_orders", "FAIL", "3 rows differ"),
                child("expected", "dim_customers", "PASS", ""),
                child("assertion", "no_negative", "FAIL", "1 row"),
                child("assertion", "few", "FAIL", "3 rows"),
                child("assertion", "ok", "PASS", ""),
            ],
        },
        ChildRowsTestCase {
            description: "checks that could not run are errors without counts",
            row: ScenarioResultRow {
                name: "orders_errored".to_owned(),
                status: "FAIL".to_owned(),
                expected: vec![ExpectedRow {
                    model_name: "fact_orders".to_owned(),
                    failed: true,
                    error: coded(Some("X504"), "comparison failed to run", None),
                    ..ExpectedRow::default()
                }],
                assertions: vec![AssertionRow {
                    name: "no_negative".to_owned(),
                    failed: true,
                    error: coded(Some("X507"), "assertion failed to run", None),
                    ..AssertionRow::default()
                }],
                ..ScenarioResultRow::default()
            },
            expected_rows: vec![
                child("expected", "fact_orders", "ERROR", ""),
                child("assertion", "no_negative", "ERROR", ""),
            ],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            expectation_child_rows(&scenario_steps(&test_case.row)),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_retained_scenarios_when_listing_then_shows_duckdb_and_relations_under_retain() {
    let local: ScenarioResultRow = ScenarioResultRow {
        name: "orders_local".to_owned(),
        status: "PASS".to_owned(),
        local: true,
        retained: true,
        local_duckdb_path: Some("target/run/scenarios/orders_local/local.duckdb".to_owned()),
        ..ScenarioResultRow::default()
    };
    let test_cases = [
        RetainedTestCase {
            description: "nothing retained renders nothing",
            rows: vec![ScenarioResultRow::default()],
            relations: true,
            expected_text: "",
        },
        RetainedTestCase {
            description: "--retain lists the warehouse relations",
            rows: vec![failing_result_row(), local.clone()],
            relations: true,
            expected_text: "\nRetained (2):\n  orders_paid\n    source    raw_orders -> main.__sqb_abc__source__raw_orders\n    model     fact_orders -> main.__sqb_abc__model__fact_orders\n  orders_local\n    duckdb    target/run/scenarios/orders_local/local.duckdb\n",
        },
        RetainedTestCase {
            description: "without --retain only the local DuckDB file is listed",
            rows: vec![failing_result_row(), local],
            relations: false,
            expected_text: "\nRetained (1):\n  orders_local\n    duckdb    target/run/scenarios/orders_local/local.duckdb\n",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_retained_section(&test_case.rows, test_case.relations, PLAIN),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_namespace_when_rendering_header_line_then_names_it_and_its_source() {
    let test_cases = [
        NamespaceLineTestCase {
            description: "a namespaced run names the namespace and where it came from",
            namespace: ScenarioNamespace {
                value: Some("ci_42".to_owned()),
                source: "cli".to_owned(),
            },
            expected_text: "  namespace  ci_42 (from cli)\n",
        },
        NamespaceLineTestCase {
            description: "a run without a namespace prints nothing",
            namespace: ScenarioNamespace {
                value: None,
                source: "unset".to_owned(),
            },
            expected_text: "",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            render_namespace_line(&test_case.namespace, PLAIN),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}
