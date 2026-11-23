use crate::resource_diagnostics::main::cursor_lacks_inputs::cursor_lacks_inputs;
use crate::resource_diagnostics::main::header_code_offset::header_code_offset;
use crate::resource_diagnostics::main::header_help::header_help;
use crate::resource_diagnostics::main::missing_description::missing_description;
use crate::resource_diagnostics::main::yaml_help::yaml_help;
use crate::resource_diagnostics::models::{CursorModel, HeaderHelp};
use crate::resource_diagnostics::tests::test_types::{
    CursorInputsTestCase, HeaderOffsetTestCase, HelpTestCase, MissingDescriptionTestCase,
    YamlHelpTestCase,
};

#[test]
fn given_sql_files_when_locating_the_header_then_whitespace_and_comments_are_skipped() {
    let test_cases = [
        HeaderOffsetTestCase {
            description: "a header on the first line",
            contents: "MODEL (name x);",
            expected_offset: Ok(0),
        },
        HeaderOffsetTestCase {
            description: "line and block comments before the header",
            contents: "  -- lead\n\t/* block\n */ MODEL (x)",
            expected_offset: Ok(24),
        },
        HeaderOffsetTestCase {
            description: "a file of only a line comment ends the scan",
            contents: "-- only comment",
            expected_offset: Ok(15),
        },
        HeaderOffsetTestCase {
            description: "Unicode whitespace counts as Python's",
            contents: "\u{3000}\nFUNCTION ()",
            expected_offset: Ok(2),
        },
        HeaderOffsetTestCase {
            description: "a line comment right after a block comment",
            contents: "/* a */-- b\n  HOOK ();",
            expected_offset: Ok(14),
        },
        HeaderOffsetTestCase {
            description: "an unclosed block comment is Python's error",
            contents: "/* open",
            expected_offset: Err("SQL contains an unclosed block comment"),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            header_code_offset(test_case.contents),
            test_case.expected_offset,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_descriptions_when_checking_then_missing_and_blank_ones_are_reported() {
    let test_cases = [
        MissingDescriptionTestCase {
            description: "no description",
            authored: None,
            expected_message: Some("model 'orders' has no description"),
        },
        MissingDescriptionTestCase {
            description: "a blank description",
            authored: Some(" \u{3000}\n"),
            expected_message: Some("model 'orders' has an empty description"),
        },
        MissingDescriptionTestCase {
            description: "a description",
            authored: Some("One row per order."),
            expected_message: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            missing_description("model", "orders", test_case.authored).as_deref(),
            test_case.expected_message,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_resources_when_building_description_help_then_snippets_match_python() {
    let test_cases = [
        HelpTestCase {
            description: "a MODEL header snippet",
            help: HeaderHelp {
                kind: "model",
                name: "orders",
                statement: "MODEL",
                entry: "description \"What one row of orders represents\"",
                path: "sql/models/orders.sql",
                description_only: false,
            },
            expected_help: "to describe model 'orders', add this to the MODEL header in sql/models/orders.sql:\n            MODEL (\n              description \"What one row of orders represents\",\n              ...\n            );",
        },
        HelpTestCase {
            description: "a one-line HOOK header snippet",
            help: HeaderHelp {
                kind: "hook",
                name: "grant",
                statement: "HOOK",
                entry: "description \"What grant does\"",
                path: "sql/hooks/grant.sql",
                description_only: true,
            },
            expected_help: "to describe hook 'grant', add this to the HOOK header in sql/hooks/grant.sql:\n            HOOK (description \"What grant does\");",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            header_help(&test_case.help),
            test_case.expected_help,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_yaml_entries_when_building_description_help_then_snippets_match_python() {
    let test_cases = [YamlHelpTestCase {
        description: "a seed entry snippet",
        entry: ("seed", "customers", "sql/seeds/schema.yml"),
        expected_help: "to describe seed 'customers', add `description` to its entry in sql/seeds/schema.yml:\n            - name: customers\n              description: What one row of customers represents",
    }];
    for test_case in test_cases {
        let (kind, name, path) = test_case.entry;

        assert_eq!(
            yaml_help(kind, name, path),
            test_case.expected_help,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_cursor_models_when_checking_inputs_then_only_unbounded_windows_are_reported() {
    let test_cases = [
        CursorInputsTestCase {
            description: "an incremental cursor model reading nothing",
            materialized: Some("incremental"),
            microbatch_strategy: None,
            has_cursor_inputs: false,
            reference_kinds: &["udf", "table_fn"],
            expected_lacks: true,
        },
        CursorInputsTestCase {
            description: "reading a source bounds the window",
            materialized: Some("incremental"),
            microbatch_strategy: None,
            has_cursor_inputs: false,
            reference_kinds: &["source"],
            expected_lacks: false,
        },
        CursorInputsTestCase {
            description: "declared cursor inputs bound the window",
            materialized: Some("incremental"),
            microbatch_strategy: None,
            has_cursor_inputs: true,
            reference_kinds: &[],
            expected_lacks: false,
        },
        CursorInputsTestCase {
            description: "a rolling window needs no inputs",
            materialized: Some("incremental"),
            microbatch_strategy: Some("rolling_window"),
            has_cursor_inputs: false,
            reference_kinds: &[],
            expected_lacks: false,
        },
        CursorInputsTestCase {
            description: "a table has no cursor window",
            materialized: Some("table"),
            microbatch_strategy: None,
            has_cursor_inputs: false,
            reference_kinds: &[],
            expected_lacks: false,
        },
    ];
    for test_case in test_cases {
        let model = CursorModel {
            materialized: test_case.materialized,
            cursor: Some("event_time"),
            microbatch_strategy: test_case.microbatch_strategy,
            has_cursor_inputs: test_case.has_cursor_inputs,
            reference_kinds: test_case.reference_kinds,
        };

        assert_eq!(
            cursor_lacks_inputs("events", &model).is_some(),
            test_case.expected_lacks,
            "{}",
            test_case.description
        );
    }
}
