use crate::compiled_project::main::located_scan_error::located_scan_error;
use crate::compiled_project::models::ScanFaultOrigin;
use crate::compiled_project::tests::helpers::{generic_syntax, span};
use crate::compiled_project::tests::test_types::LocatedScanErrorTestCase;
use sqlbuild_sqltext::sql_references::main::extract_sql_references::extract_sql_references;
use sqlbuild_sqltext::sql_references::models::ReferenceExtraction;

#[test]
fn given_reference_scan_fault_when_locating_then_python_message_is_returned() {
    let test_cases = [
        LocatedScanErrorTestCase {
            description: "unclosed call after the header",
            sql: "SELECT *\nFROM __ref(\"orders\"",
            contents: "MODEL (description \"Orders\");\n\nSELECT *\nFROM __ref(\"orders\"",
            passes: Some(vec![Vec::new(), Vec::new()]),
            expected_message: "sql/models/orders.sql:4:6: SQL reference contains an unclosed parenthesis",
        },
        LocatedScanErrorTestCase {
            description: "unclosed quote counted in code points",
            sql: "SELECT 'é', __ref(\"orders\") WHERE note = 'open",
            contents: "MODEL ();\nSELECT 'é', __ref(\"orders\") WHERE note = 'open",
            passes: Some(vec![Vec::new(), Vec::new()]),
            expected_message: "sql/models/orders.sql:2:42: SQL reference contains an unclosed quoted string",
        },
        LocatedScanErrorTestCase {
            description: "fault after neutral expansions maps through both passes",
            sql: "SELECT 'open', 1 + 2\nFROM __ref(\"orders\"",
            contents: "MODEL ();\nSELECT @enum(\"s\").OPEN, @m()\nFROM __ref(\"orders\"",
            passes: Some(vec![vec![span(7, 22, 7, 13)], vec![span(15, 19, 15, 20)]]),
            expected_message: "sql/models/orders.sql:3:6: SQL reference contains an unclosed parenthesis",
        },
        LocatedScanErrorTestCase {
            description: "fault inside macro output is unlocated",
            sql: "SELECT * FROM __ref(\"orders\") /* expanded",
            contents: "MODEL ();\nSELECT * FROM __ref(\"orders\") @expand()",
            passes: Some(vec![Vec::new(), vec![span(30, 39, 30, 41)]]),
            expected_message: "sql/models/orders.sql: SQL reference contains an unclosed block comment",
        },
        LocatedScanErrorTestCase {
            description: "authored fault shaped by an unclosed macro quote is unlocated",
            sql: "SELECT 'x || ' || note || 'open",
            contents: "MODEL ();\nSELECT @q() || ' || note || 'open",
            passes: Some(vec![Vec::new(), vec![span(7, 11, 7, 9)]]),
            expected_message: "sql/models/orders.sql: SQL reference contains an unclosed quoted string",
        },
        LocatedScanErrorTestCase {
            description: "without authored passes only the file is named",
            sql: "SELECT *\nFROM __ref(\"orders\"",
            contents: "MODEL ();\nSELECT *\nFROM __ref(\"orders\"",
            passes: None,
            expected_message: "sql/models/orders.sql: SQL reference contains an unclosed parenthesis",
        },
    ];

    for test_case in test_cases {
        let ReferenceExtraction::Failed(failure) =
            extract_sql_references(test_case.sql, &generic_syntax())
        else {
            panic!("{}: the scan must fail", test_case.description);
        };
        let query_sql: String = test_case
            .contents
            .split_once(";\n")
            .map_or(test_case.contents, |(_, query)| query.trim_start())
            .to_owned();
        let origin = ScanFaultOrigin {
            relative_path: "sql/models/orders.sql",
            contents: test_case.contents,
            query_sql: &query_sql,
            passes: test_case.passes,
        };
        assert_eq!(
            located_scan_error(&failure, test_case.sql, &origin, &generic_syntax()),
            test_case.expected_message,
            "{}",
            test_case.description
        );
    }
}
