use std::path::Path;

use crate::scenario_command::main::format_captured_at::format_captured_at;
use crate::scenario_command::main::render_namespace_header::render_namespace_header;
use crate::scenario_command::main::resolve_scenario_namespace::resolve_scenario_namespace;
use crate::scenario_command::main::resolve_snapshot_limits::resolve_snapshot_limits;
use crate::scenario_command::main::scenario_activity_message::scenario_activity_message;
use crate::scenario_command::main::select_scenarios::select_scenarios;
use crate::scenario_command::main::snapshot_capture_warning::snapshot_capture_warning;
use crate::scenario_command::main::sql_analysis_required_error::sql_analysis_required_error;
use crate::scenario_command::main::validate_scenario_test_flags::validate_scenario_test_flags;
use crate::scenario_command::models::{
    ScenarioNamespace, ScenarioSelection, ScenarioTestFlags, SnapshotLimitConfig,
    SnapshotLimitInputs, SnapshotLimits, SqlAnalysisCommand,
};
use crate::scenario_command::tests::helpers::{
    PROJECT_DIR, candidates, invalid_namespace, namespace, scenario_files, selected_names, strings,
};
use crate::scenario_command::tests::test_types::{
    CapturedAtTestCase, FlagsTestCase, LimitsTestCase, NamespaceTestCase, NamespaceTextTestCase,
    SelectionTestCase, SqlAnalysisTestCase, WarningTestCase,
};

#[test]
fn given_namespace_sources_when_resolving_then_highest_precedence_wins() {
    let long: String = "x".repeat(128);
    let too_long: String = "x".repeat(129);
    let test_cases = [
        NamespaceTestCase {
            description: "CLI wins",
            candidates: candidates([Some("job-1"), Some("job-2"), Some("local"), Some("project")]),
            expected_namespace: namespace("job-1", "cli"),
        },
        NamespaceTestCase {
            description: "environment wins",
            candidates: candidates([None, Some("job-2"), Some("local"), Some("project")]),
            expected_namespace: namespace("job-2", "env"),
        },
        NamespaceTestCase {
            description: "local wins",
            candidates: candidates([None, None, Some("local"), Some("project")]),
            expected_namespace: namespace("local", "local config"),
        },
        NamespaceTestCase {
            description: "project fallback",
            candidates: candidates([None, None, None, Some("project")]),
            expected_namespace: namespace("project", "project config"),
        },
        NamespaceTestCase {
            description: "unset preserves default",
            candidates: candidates([None; 4]),
            expected_namespace: Ok(ScenarioNamespace {
                value: None,
                source: "unset".to_owned(),
            }),
        },
        NamespaceTestCase {
            description: "CLI overrides invalid environment",
            candidates: candidates([Some("job-1"), Some(""), None, None]),
            expected_namespace: namespace("job-1", "cli"),
        },
        NamespaceTestCase {
            description: "empty environment does not fall back",
            candidates: candidates([None, Some(""), None, Some("project")]),
            expected_namespace: invalid_namespace(),
        },
        NamespaceTestCase {
            description: "all allowed characters",
            candidates: candidates([Some("AZaz09-_."), None, None, None]),
            expected_namespace: namespace("AZaz09-_.", "cli"),
        },
        NamespaceTestCase {
            description: "maximum length",
            candidates: candidates([Some(&long), None, None, None]),
            expected_namespace: namespace(&long, "cli"),
        },
        NamespaceTestCase {
            description: "too long",
            candidates: candidates([Some(&too_long), None, None, None]),
            expected_namespace: invalid_namespace(),
        },
        NamespaceTestCase {
            description: "whitespace",
            candidates: candidates([Some(" "), None, None, None]),
            expected_namespace: invalid_namespace(),
        },
        NamespaceTestCase {
            description: "slash",
            candidates: candidates([Some("job/1"), None, None, None]),
            expected_namespace: invalid_namespace(),
        },
        NamespaceTestCase {
            description: "colon",
            candidates: candidates([Some("job:1"), None, None, None]),
            expected_namespace: invalid_namespace(),
        },
        NamespaceTestCase {
            description: "trailing newline",
            candidates: candidates([Some("job\n"), None, None, None]),
            expected_namespace: invalid_namespace(),
        },
        NamespaceTestCase {
            description: "non ASCII",
            candidates: candidates([Some("é"), None, None, None]),
            expected_namespace: invalid_namespace(),
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            resolve_scenario_namespace(test_case.candidates),
            test_case.expected_namespace,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_namespace_when_rendering_progress_then_names_value_and_source() {
    let test_cases = [
        NamespaceTextTestCase {
            description: "a set namespace",
            namespace: ScenarioNamespace {
                value: Some("job-1".to_owned()),
                source: "cli".to_owned(),
            },
            expected_header: "Scenario namespace: job-1 (source: cli)\n",
            expected_activity: "Running scenarios... (namespace: job-1; source: cli)",
        },
        NamespaceTextTestCase {
            description: "no namespace",
            namespace: ScenarioNamespace {
                value: None,
                source: "unset".to_owned(),
            },
            expected_header: "Scenario namespace: (unset) (source: unset)\n",
            expected_activity: "Running scenarios... (namespace: (unset); source: unset)",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            (
                render_namespace_header(&test_case.namespace),
                scenario_activity_message("Running scenarios...", &test_case.namespace)
            ),
            (
                test_case.expected_header.to_owned(),
                test_case.expected_activity.to_owned()
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_scenario_selectors_when_selecting_then_returns_matching_scenarios() {
    let absolute_file: String = format!("{PROJECT_DIR}/tests/scenarios/orders/paid.sql");
    let absolute_folder: String = format!("{PROJECT_DIR}/tests/scenarios/orders");
    let test_cases = [
        SelectionTestCase {
            scenario_files: scenario_files(),
            description: "no selectors select every scenario",
            selectors: Vec::new(),
            exclude: Vec::new(),
            expected_names: Ok(vec!["orders_paid", "orders_refund", "customers_new"]),
        },
        SelectionTestCase {
            scenario_files: scenario_files(),
            description: "excludes scenarios from explicit selection",
            selectors: strings(&["tests/scenarios"]),
            exclude: strings(&["orders_refund"]),
            expected_names: Ok(vec!["orders_paid", "customers_new"]),
        },
        SelectionTestCase {
            scenario_files: scenario_files(),
            description: "selector order wins and duplicates are dropped",
            selectors: strings(&["customers_new", "orders_paid", "customers_new"]),
            exclude: Vec::new(),
            expected_names: Ok(vec!["customers_new", "orders_paid"]),
        },
        SelectionTestCase {
            scenario_files: scenario_files(),
            description: "a folder relative to the scenario root",
            selectors: strings(&["orders/"]),
            exclude: Vec::new(),
            expected_names: Ok(vec!["orders_paid", "orders_refund"]),
        },
        SelectionTestCase {
            scenario_files: scenario_files(),
            description: "an absolute folder",
            selectors: vec![absolute_folder],
            exclude: Vec::new(),
            expected_names: Ok(vec!["orders_paid", "orders_refund"]),
        },
        SelectionTestCase {
            scenario_files: scenario_files(),
            description: "a file relative to the project",
            selectors: strings(&["./tests/scenarios/orders/refund.sql"]),
            exclude: Vec::new(),
            expected_names: Ok(vec!["orders_refund"]),
        },
        SelectionTestCase {
            scenario_files: scenario_files(),
            description: "a file relative to the scenario root",
            selectors: strings(&["customers_new.sql"]),
            exclude: Vec::new(),
            expected_names: Ok(vec!["customers_new"]),
        },
        SelectionTestCase {
            scenario_files: scenario_files(),
            description: "an absolute file selects only that scenario",
            selectors: vec![absolute_file],
            exclude: Vec::new(),
            expected_names: Ok(vec!["orders_paid"]),
        },
        SelectionTestCase {
            scenario_files: scenario_files(),
            description: "excluding a folder",
            selectors: Vec::new(),
            exclude: strings(&["orders"]),
            expected_names: Ok(vec!["customers_new"]),
        },
        SelectionTestCase {
            scenario_files: scenario_files(),
            description: "the project folder selects every scenario",
            selectors: strings(&["."]),
            exclude: Vec::new(),
            expected_names: Ok(vec!["orders_paid", "orders_refund", "customers_new"]),
        },
        SelectionTestCase {
            scenario_files: scenario_files(),
            description: "unknown selector",
            selectors: strings(&["missing"]),
            exclude: Vec::new(),
            expected_names: Err(("C453", "Unknown scenario selector 'missing'")),
        },
        SelectionTestCase {
            scenario_files: Vec::new(),
            description: "no discovered scenarios",
            selectors: Vec::new(),
            exclude: Vec::new(),
            expected_names: Err((
                "C451",
                "No SQL scenarios were discovered under tests/scenarios",
            )),
        },
        SelectionTestCase {
            scenario_files: scenario_files(),
            description: "unknown exclusion",
            selectors: Vec::new(),
            exclude: strings(&["missing.sql"]),
            expected_names: Err(("C453", "Unknown scenario selector 'missing.sql'")),
        },
        SelectionTestCase {
            scenario_files: scenario_files(),
            description: "rejects upstream graph selector",
            selectors: strings(&["+orders_paid"]),
            exclude: Vec::new(),
            expected_names: Err((
                "C457",
                "Scenario selector '+orders_paid' uses graph operators, which are not supported",
            )),
        },
        SelectionTestCase {
            scenario_files: scenario_files(),
            description: "rejects path graph selector",
            selectors: strings(&["orders_paid~orders_refund"]),
            exclude: Vec::new(),
            expected_names: Err((
                "C457",
                "Scenario selector 'orders_paid~orders_refund' uses graph operators, which are \
                 not supported",
            )),
        },
    ];
    for test_case in test_cases {
        let selected = select_scenarios(
            &test_case.scenario_files,
            ScenarioSelection {
                selectors: &test_case.selectors,
                exclude: &test_case.exclude,
                project_dir: Path::new(PROJECT_DIR),
            },
        );
        assert_eq!(
            selected_names(&test_case.scenario_files, selected),
            test_case
                .expected_names
                .map(|names| names.into_iter().map(str::to_owned).collect())
                .map_err(|(code, message)| (code, message.to_owned())),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_scenario_test_flags_when_validating_then_rejects_unsupported_combinations() {
    let test_cases = [
        FlagsTestCase {
            description: "local with retain",
            flags: ScenarioTestFlags {
                local: true,
                retain: true,
                ..ScenarioTestFlags::default()
            },
            expected_code: Some("C452"),
        },
        FlagsTestCase {
            description: "sync on the warehouse",
            flags: ScenarioTestFlags {
                warehouse: true,
                sync_snapshots: true,
                ..ScenarioTestFlags::default()
            },
            expected_code: Some("C454"),
        },
        FlagsTestCase {
            description: "refresh on the warehouse",
            flags: ScenarioTestFlags {
                warehouse: true,
                refresh: true,
                ..ScenarioTestFlags::default()
            },
            expected_code: Some("C454"),
        },
        FlagsTestCase {
            description: "local refresh",
            flags: ScenarioTestFlags {
                local: true,
                refresh: true,
                ..ScenarioTestFlags::default()
            },
            expected_code: None,
        },
        FlagsTestCase {
            description: "auto sync and retain",
            flags: ScenarioTestFlags {
                retain: true,
                sync_snapshots: true,
                ..ScenarioTestFlags::default()
            },
            expected_code: None,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            validate_scenario_test_flags(test_case.flags)
                .err()
                .map(|error| error.code),
            test_case.expected_code,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_sql_analysis_off_when_checking_scenario_command_then_explains_how_to_turn_it_on() {
    let test_cases = [
        SqlAnalysisTestCase {
            description: "local replay turned off by flag",
            command: SqlAnalysisCommand::LocalTest,
            disabled_by_flag: true,
            guidance: None,
            expected_error: Some((
                "scenario test --run-on local requires SQL analysis, but `--no-sql-analysis` turns it off",
                "run the command without `--no-sql-analysis`",
            )),
        },
        SqlAnalysisTestCase {
            description: "capture turned off by flag",
            command: SqlAnalysisCommand::Capture,
            disabled_by_flag: true,
            guidance: None,
            expected_error: Some((
                "scenario capture requires SQL analysis, but `--no-sql-analysis` turns it off",
                "run the capture without `--no-sql-analysis`",
            )),
        },
        SqlAnalysisTestCase {
            description: "capture turned off by config",
            command: SqlAnalysisCommand::Capture,
            disabled_by_flag: false,
            guidance: Some((
                "the project turns it off".to_owned(),
                "turn it on".to_owned(),
            )),
            expected_error: Some((
                "scenario capture requires SQL analysis; the project turns it off",
                "turn it on",
            )),
        },
        SqlAnalysisTestCase {
            description: "analysis on",
            command: SqlAnalysisCommand::LocalTest,
            disabled_by_flag: false,
            guidance: None,
            expected_error: None,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            sql_analysis_required_error(
                test_case.command,
                test_case.disabled_by_flag,
                test_case.guidance
            )
            .map(|error| (error.code, error.message, error.help)),
            test_case.expected_error.map(|(message, help)| (
                "C455",
                message.to_owned(),
                Some(help.to_owned())
            )),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_limit_sources_when_resolving_then_cli_beats_config_beats_default() {
    let test_cases = [
        LimitsTestCase {
            description: "defaults",
            inputs: SnapshotLimitInputs::default(),
            config: SnapshotLimitConfig::default(),
            expected_limits: SnapshotLimits {
                max_rows_per_relation: 10_000,
                max_total_rows: 50_000,
                max_bytes_per_relation: 5_000_000,
                max_total_bytes: 25_000_000,
                force: false,
            },
        },
        LimitsTestCase {
            description: "cli then config",
            inputs: SnapshotLimitInputs {
                max_rows_per_relation: Some(5),
                force: true,
                ..SnapshotLimitInputs::default()
            },
            config: SnapshotLimitConfig {
                max_rows_per_relation: Some(1),
                max_total_bytes: Some(10),
                ..SnapshotLimitConfig::default()
            },
            expected_limits: SnapshotLimits {
                max_rows_per_relation: 5,
                max_total_rows: 50_000,
                max_bytes_per_relation: 5_000_000,
                max_total_bytes: 10,
                force: true,
            },
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            resolve_snapshot_limits(test_case.inputs, test_case.config),
            test_case.expected_limits,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_epoch_seconds_when_formatting_capture_time_then_writes_utc_seconds() {
    let test_cases = [
        CapturedAtTestCase {
            description: "epoch",
            epoch_seconds: 0,
            expected_text: "1970-01-01T00:00:00Z",
        },
        CapturedAtTestCase {
            description: "leap day",
            epoch_seconds: 1_709_210_096,
            expected_text: "2024-02-29T12:34:56Z",
        },
        CapturedAtTestCase {
            description: "end of year",
            epoch_seconds: 1_798_761_599,
            expected_text: "2026-12-31T23:59:59Z",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            format_captured_at(test_case.epoch_seconds),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_force_when_warning_about_snapshots_then_mentions_bypassed_limits() {
    let test_cases = [
        WarningTestCase {
            description: "limits enforced",
            force: false,
            expected_text: "Review captured scenario snapshots before committing; they may contain sensitive warehouse data.",
        },
        WarningTestCase {
            description: "limits bypassed",
            force: true,
            expected_text: "Review captured scenario snapshots before committing; they may contain sensitive warehouse data. Size limits are bypassed.",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            snapshot_capture_warning(test_case.force),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}
