use crate::diff_report::main::parse_diff_name_range::parse_diff_name_range;
use crate::diff_report::main::query_column_problem::query_column_problem;
use crate::diff_report::main::size_guard_report::size_guard_report;
use crate::diff_report::main::validate_diff_request::validate_diff_request;
use crate::diff_report::tests::helpers::{blocked_orders, request, strings};
use crate::diff_report::tests::test_types::{
    ColumnsTestCase, NameRangeTestCase, RequestTestCase, SizeGuardTestCase,
};

#[test]
fn given_target_range_when_parsing_then_targets_or_code_match() {
    let parsed = |from: &str, to: Option<&str>| Ok((from.to_owned(), to.map(str::to_owned)));
    let test_cases = [
        NameRangeTestCase {
            description: "explicit range keeps both targets",
            range: "prod:dev",
            expected_targets: parsed("prod", Some("dev")),
        },
        NameRangeTestCase {
            description: "bare target leaves TO unset",
            range: "prod",
            expected_targets: parsed("prod", None),
        },
        NameRangeTestCase {
            description: "empty target",
            range: "",
            expected_targets: Err("C209".to_owned()),
        },
        NameRangeTestCase {
            description: "empty TO",
            range: "prod:",
            expected_targets: Err("C209".to_owned()),
        },
        NameRangeTestCase {
            description: "empty FROM",
            range: ":dev",
            expected_targets: Err("C209".to_owned()),
        },
        NameRangeTestCase {
            description: "too many separators",
            range: "a:b:c",
            expected_targets: Err("C209".to_owned()),
        },
    ];

    for test_case in test_cases {
        let parsed = parse_diff_name_range(test_case.range).map_err(|error| error.code);

        assert_eq!(
            parsed, test_case.expected_targets,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_request_flags_when_validating_then_first_violation_is_reported() {
    let test_cases = [
        RequestTestCase {
            description: "model diff with a range and selector passes",
            request: request(|facts| facts.from_name = Some("prod".to_owned())),
            expected_code: None,
        },
        RequestTestCase {
            description: "two comparison modes",
            request: request(|facts| {
                facts.full = true;
                facts.schema_only = true;
            }),
            expected_code: Some("C201"),
        },
        RequestTestCase {
            description: "duplicate keys ignoring case",
            request: request(|facts| {
                facts.from_name = Some("prod".to_owned());
                facts.unique_key_override = strings(&["ID", "id"]);
            }),
            expected_code: Some("C242"),
        },
        RequestTestCase {
            description: "model diff without a range",
            request: request(|_| {}),
            expected_code: Some("C224"),
        },
        RequestTestCase {
            description: "raw-query diff without a right query",
            request: request(|facts| facts.left_query = true),
            expected_code: Some("C218"),
        },
        RequestTestCase {
            description: "raw-query diff without key or unkeyed",
            request: request(|facts| {
                facts.left_query = true;
                facts.right_query_file = true;
            }),
            expected_code: Some("C221"),
        },
        RequestTestCase {
            description: "raw-query diff rejects selectors before inputs",
            request: request(|facts| {
                facts.left_query = true;
                facts.select = strings(&["orders"]);
            }),
            expected_code: Some("C213"),
        },
    ];

    for test_case in test_cases {
        let code = validate_diff_request(&test_case.request)
            .err()
            .map(|error| error.code);

        assert_eq!(
            code.as_deref(),
            test_case.expected_code,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_blocked_models_when_building_guard_report_then_names_sizes_and_commands() {
    let test_cases = [
        SizeGuardTestCase {
            description: "model with cursor offers bounded command",
            request: request(|facts| facts.select = strings(&["orders"])),
            has_cursor: true,
            expected_full: "sqb diff prod:dev --full --select orders",
            expected_bounded: Some("sqb diff prod:dev --bounded <window> --select orders"),
            expected_fragments: &[
                "orders: prod 12,500,000 rows, limit 10,000,000; dev size unknown: no row count (view), limit 10,000,000",
                "Run one of these instead:",
                "sqb diff prod:dev --schema-only --select orders",
            ],
        },
        SizeGuardTestCase {
            description: "model without cursor omits bounded command and quotes key flags",
            request: request(|facts| {
                facts.select = strings(&["tag:daily", "path:sql/models/marts"]);
                facts.unique_key_override = strings(&["order_id", "line number"]);
            }),
            has_cursor: false,
            expected_full: "sqb diff prod:dev --full --select tag:daily path:sql/models/marts --key order_id 'line number'",
            expected_bounded: None,
            expected_fragments: &["; no cursor"],
        },
        SizeGuardTestCase {
            description: "project dir is a global flag placed before diff",
            request: request(|facts| {
                facts.select = strings(&["orders"]);
                facts.project_dir = Some("projects/shop".to_owned());
            }),
            has_cursor: false,
            expected_full: "sqb --project-dir projects/shop diff prod:dev --full --select orders",
            expected_bounded: None,
            expected_fragments: &[],
        },
    ];

    for test_case in test_cases {
        let report = size_guard_report(
            &test_case.request,
            ("prod", "dev"),
            "full diff stopped",
            &blocked_orders(test_case.has_cursor),
        );
        let lines: Vec<&str> = report.message.lines().map(str::trim).collect();
        let found: Vec<&str> = test_case
            .expected_fragments
            .iter()
            .copied()
            .filter(|fragment| report.message.contains(fragment))
            .collect();

        assert_eq!(
            (
                lines.contains(&test_case.expected_full),
                test_case
                    .expected_bounded
                    .is_none_or(|bounded| lines.contains(&bounded)),
                report
                    .help
                    .contains("targets.prod.diff.max_full_rows and targets.dev.diff.max_full_rows"),
                report.help.contains("--bounded") == test_case.expected_bounded.is_some(),
                found,
            ),
            (
                true,
                true,
                true,
                true,
                test_case.expected_fragments.to_vec()
            ),
            "{}: {}",
            test_case.description,
            report.message
        );
    }
}

#[test]
fn given_query_columns_when_preflighting_then_problems_are_coded() {
    let test_cases = [
        ColumnsTestCase {
            description: "distinct columns pass",
            columns: &["id", "v"],
            max_columns: Some(2),
            expected_outcome: Ok(()),
        },
        ColumnsTestCase {
            description: "no columns",
            columns: &[],
            max_columns: None,
            expected_outcome: Err(("C237", "left query returned no columns")),
        },
        ColumnsTestCase {
            description: "duplicates ignore case",
            columns: &["ID", "id", "b", "B"],
            max_columns: None,
            expected_outcome: Err(("C238", "left query returns duplicate column names: b, id")),
        },
        ColumnsTestCase {
            description: "too many columns",
            columns: &["a", "b"],
            max_columns: Some(1),
            expected_outcome: Err((
                "C239",
                "left query has 2 columns, exceeding --max-columns 1",
            )),
        },
    ];

    for test_case in test_cases {
        let outcome =
            query_column_problem("left", &strings(test_case.columns), test_case.max_columns);
        let outcome = outcome.map_err(|error| (error.code, error.message));
        let outcome = outcome
            .as_ref()
            .copied()
            .map_err(|(code, message)| (code.as_str(), message.as_str()));

        assert_eq!(
            outcome, test_case.expected_outcome,
            "{}",
            test_case.description
        );
    }
}
