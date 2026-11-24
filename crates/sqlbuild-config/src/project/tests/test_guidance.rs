use crate::project::main::explicit_references_help::explicit_references_help;
use crate::project::main::microbatch_guidance::microbatch_guidance;
use crate::project::main::setting_help_text::setting_help_text;
use crate::project::main::snippet_help_text::snippet_help_text;
use crate::project::main::sql_analysis_off_guidance::sql_analysis_off_guidance;
use crate::project::models::{SettingSite, SettingText};
use crate::project::tests::test_types::GuidanceTestCase;

#[test]
fn given_setting_guidance_when_rendering_then_text_matches_python() {
    let test_cases = [
        GuidanceTestCase {
            description: "microbatch concurrency",
            rendered: microbatch_guidance(),
            expected_note: "the current value is [settings] microbatch_concurrency = false (from sqlbuild_project.toml or its default)",
            expected_help: "to run microbatches concurrently, set this in sqlbuild_project.toml:\n            [settings]\n            microbatch_concurrency = true",
        },
        GuidanceTestCase {
            description: "SQL analysis turned off locally",
            rendered: sql_analysis_off_guidance(true, "to run X"),
            expected_note: "sqlbuild_local.toml sets [settings] sql_analysis = false",
            expected_help: "to run X, set this in sqlbuild_local.toml:\n            [settings]\n            sql_analysis = true",
        },
        GuidanceTestCase {
            description: "explicit references",
            rendered: (explicit_references_help("X"), String::new()),
            expected_note: "the current value is [references] enforce_explicit = true (from sqlbuild_project.toml or its default); while migrating a project, to allow X, set this in sqlbuild_project.toml:\n            [references]\n            enforce_explicit = false",
            expected_help: "",
        },
        GuidanceTestCase {
            description: "a quoted string setting and an integer setting",
            rendered: (
                setting_help_text(
                    "to stage",
                    SettingSite {
                        file_name: "sqlbuild_project.toml",
                        section: "settings",
                        key: "table_promotion_mode",
                    },
                    &SettingText::Text("st\"aged\\".to_owned()),
                ),
                setting_help_text(
                    "to wait",
                    SettingSite {
                        file_name: "sqlbuild_local.toml",
                        section: "settings",
                        key: "timeout",
                    },
                    &SettingText::Integer("30".to_owned()),
                ),
            ),
            expected_note: "to stage, set this in sqlbuild_project.toml:\n            [settings]\n            table_promotion_mode = \"st\\\"aged\\\\\"",
            expected_help: "to wait, set this in sqlbuild_local.toml:\n            [settings]\n            timeout = 30",
        },
        GuidanceTestCase {
            description: "a snippet help shows every line to write",
            rendered: (
                snippet_help_text(
                    "to describe it",
                    "add this to sql/seeds/lookups.yml",
                    &[
                        "- name: product_types".to_owned(),
                        "  description: Types".to_owned(),
                    ],
                ),
                String::new(),
            ),
            expected_note: "to describe it, add this to sql/seeds/lookups.yml:\n            - name: product_types\n              description: Types",
            expected_help: "",
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            (test_case.rendered.0.as_str(), test_case.rendered.1.as_str()),
            (test_case.expected_note, test_case.expected_help),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_target_warehouse_without_session_warehouses_when_rejecting_then_text_matches_python() {
    let test_cases = [GuidanceTestCase {
        description: "a quoted local query warehouse",
        rendered: crate::project::main::unsupported_target_warehouse::unsupported_target_warehouse(
            ("/p/sqlbuild_local.toml", "sqlbuild_local.toml"),
            "duckdb",
            ("dev", "query", "WH \"x\""),
        ),
        expected_note: "/p/sqlbuild_local.toml [targets.dev.warehouses] selects a query warehouse, but the 'duckdb' adapter has no warehouse to select; sqlbuild_local.toml sets [targets.dev.warehouses] query = \"WH \\\"x\\\"\"",
        expected_help: "remove the [targets.dev.warehouses] section from sqlbuild_local.toml; command-group warehouses need an adapter with a session warehouse, such as snowflake or an adapter extending it",
    }];

    for test_case in test_cases {
        assert_eq!(
            (test_case.rendered.0.as_str(), test_case.rendered.1.as_str()),
            (test_case.expected_note, test_case.expected_help),
            "{}",
            test_case.description
        );
    }
}
