use crate::cli_text::_helpers::diagnostics::diagnostics_text;
use crate::cli_text::main::format_coded_error::format_coded_error;
use crate::cli_text::main::format_compile_text::format_compile_text;
use crate::cli_text::main::format_dbt_interop_plan::format_dbt_interop_plan;
use crate::cli_text::main::format_plan_completion::format_plan_completion;
use crate::cli_text::main::format_plan_text::format_plan_text;
use crate::cli_text::main::semantic_coverage_notice::semantic_coverage_notice;
use crate::cli_text::models::{
    CliStyle, CompileTextReport, DbtInteropPlanText, DbtLsNodeText, DiagnosticText, DisplayLimit,
    LineageNote, ModelLine, ModelPlanText, PlanTextReport, PlanWarningText, RelatedText,
};
use crate::cli_text::tests::helpers::{diagnostic, location, plan_report, table_model};
use crate::cli_text::tests::test_types::{
    CodedErrorTestCase, CompileTextTestCase, DbtPlanTextTestCase, DiagnosticsTextTestCase,
    NoticeTestCase, PlanTextTestCase,
};

/// Expectations recorded from the retired Python formatter.
#[test]
fn given_diagnostics_when_rendering_then_matches_python_text() {
    let test_cases = [DiagnosticsTextTestCase {
        description: "located error with related location and a repeated warning",
        diagnostics: vec![
            DiagnosticText {
                resource_type: Some("sql_test".to_owned()),
                resource_name: Some("orders_test".to_owned()),
                location: Some(location(2, 3, (Some(2), Some(7)))),
                related: vec![RelatedText {
                    label: "declared here".to_owned(),
                    location: location(1, 1, (Some(3), Some(1))),
                    message: Some("here".to_owned()),
                }],
                notes: vec!["n1".to_owned()],
                affected_rules: vec!["R1".to_owned(), "R2".to_owned()],
                help: Some("fix it".to_owned()),
                ..diagnostic("error", "C001", "bad column")
            },
            DiagnosticText {
                path: Some("models/b.sql".to_owned()),
                occurrences: 2,
                ..diagnostic("warning", "W001", "careful")
            },
        ],
        style: CliStyle { use_color: true },
        expected_text: "\x1b[38;5;167merror[C001]: bad column\x1b[0m\n  sql test: orders_test\n  --> models/a.sql:2:3\n    |\n  2 | FROM orders\u{e9} x\n    |   ^^^^\n\n  declared here:\n  --> models/a.sql:1:1\n    |\n  1 | SELECT 1\n    | ^^^^^^^^ here\n  \x1b[2m= note:\x1b[0m n1\n  \x1b[2m= affected Rules:\x1b[0m R1, R2\n  \x1b[2m= help:\x1b[0m fix it\n\n\x1b[33mwarning[W001]: careful\x1b[0m\n  --> models/b.sql\n  = 2 occurrences; all retained in JSON and the warning count",
    }];

    for test_case in test_cases {
        assert_eq!(
            diagnostics_text(&test_case.diagnostics, test_case.style),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

/// Expectations recorded from the retired Python notice.
#[test]
fn given_partial_reasons_when_building_notice_then_groups_by_reason() {
    let test_cases = [
        NoticeTestCase {
            description: "reasons grouped across models with known and unknown actions",
            reasons: vec![
                (
                    "zeta".to_owned(),
                    vec![
                        "open source: raw_orders".to_owned(),
                        "SQL analysis disabled".to_owned(),
                    ],
                ),
                (
                    "alpha".to_owned(),
                    vec!["open source: raw_orders".to_owned(), "weird".to_owned()],
                ),
            ],
            expected_notice: Some(
                "Semantic checks were partial for 2 models:\n  zeta: SQL analysis disabled; enable sql_analysis, then run `sqb compile --no-cache`.\n  alpha, zeta: reads open source raw_orders; run `sqb contract generate --from <target> --select source:raw_orders --write` and enforce the complete source contract.\n  alpha: weird; declare the input shape, then run `sqb compile --no-cache`.",
            ),
        },
        NoticeTestCase {
            description: "one partial model is counted in the singular",
            reasons: vec![("alpha".to_owned(), vec!["SQL analysis disabled".to_owned()])],
            expected_notice: Some(
                "Semantic checks were partial for 1 model:\n  alpha: SQL analysis disabled; enable sql_analysis, then run `sqb compile --no-cache`.",
            ),
        },
        NoticeTestCase {
            description: "complete checks print no notice",
            reasons: Vec::new(),
            expected_notice: None,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            semantic_coverage_notice(&test_case.reasons).as_deref(),
            test_case.expected_notice,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_report_when_formatting_then_lists_models_and_summary() {
    let test_cases = [
        CompileTextTestCase {
            description: "one plain model with a manifest and unavailable lineage",
            report: CompileTextReport {
                style: CliStyle { use_color: false },
                models: vec![ModelLine {
                    name: "orders".to_owned(),
                    failed: false,
                    column_count: 2,
                }],
                semantic_notice: None,
                default_rules_notice: None,
                diagnostics: Vec::new(),
                seed_count: 1,
                function_count: 0,
                error_count: 0,
                warning_count: 0,
                compiled_dir: "target/compiled".to_owned(),
                failed_model_sql: Vec::new(),
                manifest: true,
                lineage: LineageNote::Unavailable,
            },
            expected_text: "\nCompile ready  1 model\n\n\u{2514}\u{2500}\u{2500} orders                   OK   2 columns\n\n\u{2713} Project compiled  1 model, 1 seed, 0 functions, 0 errors, 0 warnings\n  Wrote: target/compiled/\n  Wrote: target/manifest.json\n  Column lineage: unavailable\n",
        },
        CompileTextTestCase {
            description: "a compile whose Rules failed names each failing model's SQL",
            report: CompileTextReport {
                style: CliStyle { use_color: false },
                models: vec![ModelLine {
                    name: "orders".to_owned(),
                    failed: true,
                    column_count: 2,
                }],
                semantic_notice: None,
                default_rules_notice: None,
                diagnostics: Vec::new(),
                seed_count: 0,
                function_count: 0,
                error_count: 1,
                warning_count: 0,
                compiled_dir: "target/compiled-failed".to_owned(),
                failed_model_sql: vec!["target/compiled-failed/models/orders.sql".to_owned()],
                manifest: false,
                lineage: LineageNote::Silent,
            },
            expected_text: "\nCompile ready  1 model\n\n\u{2514}\u{2500}\u{2500} orders                   FAIL 2 columns\n\n\u{2717} Project compiled  1 model, 0 seeds, 0 functions, 1 error, 0 warnings\n  Failed SQL: target/compiled-failed/models/orders.sql\n  Wrote: target/compiled-failed/\n",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            format_compile_text(&test_case.report),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_plan_report_when_rendering_then_groups_models_by_reason() {
    let test_cases = [
        PlanTextTestCase {
            description: "a changed query with its diff, a routine model and a warning",
            report: PlanTextReport {
                warnings: vec![PlanWarningText {
                    severity: "warning".to_owned(),
                    message: "orders lag\nsecond line".to_owned(),
                    model_name: Some("orders".to_owned()),
                }],
                ..plan_report(vec![
                    ModelPlanText {
                        query_diff: Some(vec![
                            "      -SELECT 1".to_owned(),
                            "      +SELECT 2".to_owned(),
                        ]),
                        ..table_model("orders", "query_changed")
                    },
                    table_model("customers", "no_change"),
                    ModelPlanText {
                        action: "skip".to_owned(),
                        ..table_model("skipped", "no_change")
                    },
                ])
            },
            expected_text: "Plan ready  3 selected\n\nQuery changed (1)\n\u{2514}\u{2500}\u{2500} orders               full rebuild\n    \u{251c}\u{2500}\u{2500} policy  replay_on_change=full\n    \u{2514}\u{2500}\u{2500} query diff:\n      -SELECT 1\n      +SELECT 2\n\nModels (1)\n\u{2514}\u{2500}\u{2500} customers            table\n\nWarnings (1)\n\u{2514}\u{2500}\u{2500} orders\n    \u{251c}\u{2500}\u{2500} orders lag\n    \u{2514}\u{2500}\u{2500} second line",
            expected_completion: "\u{2713} Plan complete  3 selected",
        },
        PlanTextTestCase {
            description: "detail rows under a non-last entry keep the tree's vertical bar",
            report: plan_report(vec![
                ModelPlanText {
                    query_diff: Some(vec!["      -SELECT 1".to_owned()]),
                    ..table_model("orders", "query_changed")
                },
                table_model("customers", "query_changed"),
            ]),
            expected_text: "Plan ready  2 selected\n\nQuery changed (2)\n\u{251c}\u{2500}\u{2500} orders               full rebuild\n\u{2502}   \u{251c}\u{2500}\u{2500} policy  replay_on_change=full\n\u{2502}   \u{2514}\u{2500}\u{2500} query diff:\n\u{2502}     -SELECT 1\n\u{2514}\u{2500}\u{2500} customers            full rebuild\n    \u{2514}\u{2500}\u{2500} policy  replay_on_change=full",
            expected_completion: "\u{2713} Plan complete  2 selected",
        },
        PlanTextTestCase {
            description: "an error finding makes the final line state failure",
            report: PlanTextReport {
                warnings: vec![PlanWarningText {
                    severity: "error".to_owned(),
                    message: "would replace analytics.orders".to_owned(),
                    model_name: None,
                }],
                ..plan_report(vec![table_model("orders", "no_change")])
            },
            expected_text: "Plan ready  1 selected\n\nModels (1)\n\u{2514}\u{2500}\u{2500} orders               table\n\nErrors (1)\n\u{2514}\u{2500}\u{2500} would replace analytics.orders",
            expected_completion: "\u{2717} Plan failed with errors  1 selected",
        },
        PlanTextTestCase {
            description: "every active model is a full refresh",
            report: plan_report(vec![
                table_model("orders", "full_refresh"),
                table_model("customers", "full_refresh"),
                ModelPlanText {
                    materialization_label: "view".to_owned(),
                    ..table_model("stg_orders", "full_refresh")
                },
            ]),
            expected_text: "Plan ready  full refresh, 3 selected\n\nFull refresh (3)\n    2 table\n    1 view",
            expected_completion: "\u{2713} Plan complete  full refresh, 3 selected",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            (
                format_plan_text(&test_case.report).as_str(),
                format_plan_completion(&test_case.report).as_str()
            ),
            (test_case.expected_text, test_case.expected_completion),
            "{}",
            test_case.description
        );
    }
}

/// Expectations recorded from the retired Python formatter.
#[test]
fn given_dbt_interop_plan_when_formatting_then_matches_python_text() {
    let node = |unique_id: &str, resource_type: Option<&str>| DbtLsNodeText {
        unique_id: unique_id.to_owned(),
        resource_type: resource_type.map(str::to_owned),
        name: None,
        fqn: Vec::new(),
    };
    let test_cases = [DbtPlanTextTestCase {
        description: "grouped nodes with overflow, required ids, anchors and warnings",
        plan: DbtInteropPlanText {
            command: "plan".to_owned(),
            dbt_argv: vec!["dbt".to_owned(), "ls".to_owned()],
            dbt_nodes: vec![
                node("model.a.b", Some("model")),
                node("model.a.a", Some("model")),
                node("x", None),
            ],
            dbt_selected_unique_ids: vec![
                "model.a.a".to_owned(),
                "model.a.b".to_owned(),
                "x".to_owned(),
            ],
            dbt_required_unique_ids: vec!["model.a.r".to_owned()],
            sqlbuild_models: vec!["orders".to_owned()],
            anchors: vec![("a+".to_owned(), vec!["u".to_owned()])],
            warnings: vec!["careful".to_owned()],
            ..DbtInteropPlanText::default()
        },
        max_entries: Some(1),
        expected_text: "Plan ready  5 selected resources\n\ndbt (3 selected, 1 required)\n  command: dbt ls\n\n  model (2)\n  ├── model.a.a\n    ... and 1 more (use --verbose to show all)\n\n  resource (1)\n  └── x\n\n  required (1)\n  └── model.a.r\n\nSQLBuild (1)\n  orders\n\ndbt selector anchors\n  a+: 1 dbt resources\n\nWarnings\n  careful",
    }];

    for test_case in test_cases {
        let display: DisplayLimit = DisplayLimit {
            max_entries: test_case.max_entries,
            overflow_flag: "--verbose".to_owned(),
        };
        assert_eq!(
            format_dbt_interop_plan(&test_case.plan, None, (&display, false)),
            test_case.expected_text,
            "{}",
            test_case.description
        );
    }
}

/// Expectations recorded from the retired Python coded-error formatter.
#[test]
fn given_coded_error_when_formatting_then_renders_consistently() {
    let plain: CliStyle = CliStyle { use_color: false };
    let color: CliStyle = CliStyle { use_color: true };
    let test_cases = [
        CodedErrorTestCase {
            description: "renders plain coded error",
            parts: ("R002", "materialization failed", None),
            include_error_label: true,
            style: plain,
            expected_rendered: "error[R002]: materialization failed",
        },
        CodedErrorTestCase {
            description: "renders coded detail beneath an existing error label",
            parts: ("T002", "test execution failed", None),
            include_error_label: false,
            style: plain,
            expected_rendered: "[T002] test execution failed",
        },
        CodedErrorTestCase {
            description: "renders plain coded error with help",
            parts: (
                "K011",
                "contract requires staged promotion",
                Some("set table_promotion_mode to staged"),
            ),
            include_error_label: true,
            style: plain,
            expected_rendered: "error[K011]: contract requires staged promotion\n  = help: set table_promotion_mode to staged",
        },
        CodedErrorTestCase {
            description: "renders colorized coded error with help",
            parts: ("R006", "audit failed", Some("fix failing audit rows")),
            include_error_label: true,
            style: color,
            expected_rendered: "\x1b[38;5;167m\x1b[1merror[R006]:\x1b[0m audit failed\n  \x1b[2m= help:\x1b[0m fix failing audit rows",
        },
        CodedErrorTestCase {
            description: "renders multiline coded error without color unchanged",
            parts: (
                "D012",
                "Provider 'slack_provider' in providers/slack.py has invalid settings:\n1 validation error for SlackProvider\nSLACK_TOKEN",
                None,
            ),
            include_error_label: true,
            style: plain,
            expected_rendered: "error[D012]: Provider 'slack_provider' in providers/slack.py has invalid settings:\n1 validation error for SlackProvider\nSLACK_TOKEN",
        },
        CodedErrorTestCase {
            description: "renders multiline coded error with muted continuation lines",
            parts: (
                "D012",
                "Provider 'slack_provider' in providers/slack.py has invalid settings:\n1 validation error for SlackProvider\nSLACK_TOKEN",
                None,
            ),
            include_error_label: true,
            style: color,
            expected_rendered: "\x1b[38;5;167m\x1b[1merror[D012]:\x1b[0m Provider 'slack_provider' in providers/slack.py has invalid settings:\n\x1b[2m1 validation error for SlackProvider\x1b[0m\n\x1b[2mSLACK_TOKEN\x1b[0m",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            format_coded_error(
                test_case.parts,
                test_case.include_error_label,
                test_case.style
            ),
            test_case.expected_rendered,
            "{}",
            test_case.description
        );
    }
}
