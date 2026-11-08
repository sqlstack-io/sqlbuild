use crate::analysis_route::main::native_gap::native_gap;
use crate::analysis_route::models::{DatabricksDialect, LocatedGap};
use crate::analysis_route::tests::test_types::{
    NativeGapTestCase, ParseFailureTestCase, PolyglotTypeTestCase,
};
use crate::analysis_route::types::NativeDialect;
use crate::assembly::project::models::SyntaxCheck;
use crate::type_system::main::normalize_type::normalize_type;
use crate::type_system::models::NormalizedType;

#[test]
fn given_databricks_model_sql_when_probing_its_parser_then_only_sql_it_reads_whole_has_no_gap() {
    let test_cases = [
        NativeGapTestCase {
            description: "a query Databricks reads whole",
            sql: "SELECT 1 AS a FROM t WHERE region = @@@region",
            placeholders: &[("region", "'eu'")],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "a UDF call's stub reads as one name",
            sql: "SELECT __udf(\"scaled_amount\")(amount, 2) AS scaled FROM t",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "cursor bounds read as stub calls",
            sql: "SELECT * FROM t WHERE updated_at >= __cursor_start() AND updated_at < __cursor_end( )",
            placeholders: &[],
            expected_gap: None,
        },
        NativeGapTestCase {
            description: "a syntax error is located where Databricks locates it",
            sql: "SELECT 1 FROM t WHERE )",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "Syntax error at or near ')'".to_owned(),
                message: "Syntax error at or near ')'".to_owned(),
                line: 1,
                column: 23,
                syntax: true,
            }),
        },
        NativeGapTestCase {
            description: "a construct the shared tree lacks is a gap named by the parser",
            sql: "VACUUM t",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "vacuum".to_owned(),
                message: "vacuum".to_owned(),
                line: 1,
                column: 1,
                syntax: false,
            }),
        },
        NativeGapTestCase {
            description: "a second statement is not read whole",
            sql: "SELECT 1;\nSELECT 2",
            placeholders: &[],
            expected_gap: Some(LocatedGap {
                construct: "more than one statement".to_owned(),
                message: "more than one statement".to_owned(),
                line: 1,
                column: 1,
                syntax: false,
            }),
        },
    ];
    for test_case in test_cases {
        let check = SyntaxCheck {
            sql: test_case.sql.to_owned(),
            placeholders: test_case
                .placeholders
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        };
        assert_eq!(
            native_gap(&DatabricksDialect, &check),
            Ok(test_case.expected_gap),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_databricks_type_spelling_when_normalising_then_the_type_is_polyglot_s() {
    let test_cases = [
        PolyglotTypeTestCase {
            description: "INTEGER is Polyglot's INT",
            spelling: "INTEGER",
            expected_name: "INT",
        },
        PolyglotTypeTestCase {
            description: "REAL is Polyglot's FLOAT",
            spelling: "real",
            expected_name: "FLOAT",
        },
        PolyglotTypeTestCase {
            description: "DOUBLE PRECISION is Polyglot's DOUBLE, whatever the spacing",
            spelling: "DOUBLE   PRECISION",
            expected_name: "DOUBLE",
        },
        PolyglotTypeTestCase {
            description: "NUMERIC with arguments is DECIMAL with its precision and scale",
            spelling: "NUMERIC(9, 3)",
            expected_name: "DECIMAL(9,3)",
        },
        PolyglotTypeTestCase {
            description: "NUMERIC alone is DECIMAL",
            spelling: "NUMERIC",
            expected_name: "DECIMAL",
        },
        PolyglotTypeTestCase {
            description: "TIMESTAMP_NTZ is Polyglot's TIMESTAMPNTZ",
            spelling: "TIMESTAMP_NTZ",
            expected_name: "TIMESTAMPNTZ",
        },
        PolyglotTypeTestCase {
            description: "TIMESTAMP_LTZ is Polyglot's TIMESTAMPLTZ",
            spelling: "TIMESTAMP_LTZ",
            expected_name: "TIMESTAMPLTZ",
        },
        PolyglotTypeTestCase {
            description: "a name Polyglot keeps stays as written",
            spelling: "LONG",
            expected_name: "LONG",
        },
        PolyglotTypeTestCase {
            description: "a word holding an alias is not changed inside",
            spelling: "INTEGERS",
            expected_name: "INTEGERS",
        },
        PolyglotTypeTestCase {
            description: "a sized string keeps its length",
            spelling: "VARCHAR(10)",
            expected_name: "VARCHAR(10)",
        },
        PolyglotTypeTestCase {
            description: "an alias inside an array, whose family ARRAY decides",
            spelling: "ARRAY<INTEGER>",
            expected_name: "ARRAY<INT>",
        },
        PolyglotTypeTestCase {
            description: "aliases inside a map and a struct, whose family MAP decides",
            spelling: "MAP<STRING, STRUCT<a: NUMERIC(5,2), b: INTEGER>>",
            expected_name: "MAP<STRING,STRUCT<A:DECIMAL(5,2),B:INT>>",
        },
    ];
    for test_case in test_cases {
        let polyglot: NormalizedType = normalize_type(test_case.spelling, "databricks")
            .expect("Polyglot normalizes every Databricks spelling")
            .normalized;
        let native: Option<NormalizedType> = DatabricksDialect.normalize_type(test_case.spelling);
        assert_eq!(
            (
                native.as_ref().map(|native| native.normalized_name.as_str()),
                native.as_ref()
            ),
            (Some(test_case.expected_name), Some(&polyglot)),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_sql_databricks_refuses_when_parsing_then_the_gap_says_whether_it_is_a_syntax_error() {
    let test_cases = [
        ParseFailureTestCase {
            description: "a syntax error, at its byte offset after a multi-byte character",
            sql: "SELECT 'é' FROM t WHERE )".to_owned(),
            expected_syntax: true,
            expected_offset: Some(25),
        },
        ParseFailureTestCase {
            description: "invalid syntax at the end of the input",
            sql: "SELECT (".to_owned(),
            expected_syntax: true,
            expected_offset: Some(8),
        },
        ParseFailureTestCase {
            description: "the nesting limit is not a syntax error",
            sql: format!("SELECT {}1{}", "(".repeat(300), ")".repeat(300)),
            expected_syntax: false,
            expected_offset: None,
        },
    ];
    for test_case in test_cases {
        let gap = DatabricksDialect
            .parse(&test_case.sql)
            .expect_err(test_case.description);
        assert_eq!(
            (gap.syntax, test_case.expected_offset.map(|_| gap.offset)),
            (test_case.expected_syntax, test_case.expected_offset),
            "{}",
            test_case.description
        );
    }
}
