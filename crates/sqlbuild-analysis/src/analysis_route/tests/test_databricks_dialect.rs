use crate::analysis_route::models::{DatabricksDialect, ParserGap};
use crate::analysis_route::tests::test_types::{NormalizeTypeTestCase, ReadWholeTestCase};
use crate::analysis_route::types::NativeDialect;
use crate::type_system::main::normalize_type::normalize_type;
use crate::type_system::models::NormalizedType;

const DATABRICKS: &str = "databricks";

#[test]
fn given_databricks_sql_when_reading_it_whole_then_gaps_errors_and_extra_statements_stop_it() {
    let test_cases = [
        ReadWholeTestCase {
            description: "a query with a stub call is read whole",
            sql: "SELECT __sqlbuild_udf_f(a) AS b FROM t",
            expected_gap: None,
        },
        ReadWholeTestCase {
            description: "a construct the shared tree lacks is a gap named by the parser",
            sql: "VACUUM t",
            expected_gap: Some(("vacuum", 0, false)),
        },
        ReadWholeTestCase {
            description: "invalid syntax is the parser's error, flagged as syntax",
            sql: "SELECT (",
            expected_gap: Some(("Syntax error at or near end of input", 8, true)),
        },
        ReadWholeTestCase {
            description: "the nesting limit is a refusal, not invalid syntax",
            sql: "SELECT ((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((1",
            expected_gap: Some(("the statement nests too deeply", 205, false)),
        },
        ReadWholeTestCase {
            description: "two statements are not one model",
            sql: "SELECT 1; SELECT 2",
            expected_gap: Some(("more than one statement", 18, false)),
        },
    ];
    for test_case in test_cases {
        let gap: Option<(String, usize, bool)> = DatabricksDialect
            .read_whole(test_case.sql)
            .err()
            .map(|gap: ParserGap| (gap.construct, gap.offset, gap.syntax));
        assert_eq!(
            gap,
            test_case.expected_gap.map(|(construct, offset, syntax)| (
                construct.to_owned(),
                offset,
                syntax
            )),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_databricks_type_spellings_when_normalized_then_the_fallback_gives_polyglots_type() {
    let test_cases = [
        NormalizeTypeTestCase {
            description: "INTEGER is Polyglot's INT",
            spelling: "INTEGER",
        },
        NormalizeTypeTestCase {
            description: "REAL is Polyglot's FLOAT",
            spelling: "real",
        },
        NormalizeTypeTestCase {
            description: "DOUBLE PRECISION is Polyglot's DOUBLE, whatever the spacing",
            spelling: "DOUBLE   PRECISION",
        },
        NormalizeTypeTestCase {
            description: "NUMERIC with arguments is DECIMAL with its precision and scale",
            spelling: "NUMERIC(9, 3)",
        },
        NormalizeTypeTestCase {
            description: "NUMERIC alone is DECIMAL",
            spelling: "NUMERIC",
        },
        NormalizeTypeTestCase {
            description: "TIMESTAMP_NTZ is Polyglot's TIMESTAMPNTZ",
            spelling: "TIMESTAMP_NTZ",
        },
        NormalizeTypeTestCase {
            description: "TIMESTAMP_LTZ is Polyglot's TIMESTAMPLTZ",
            spelling: "TIMESTAMP_LTZ",
        },
        NormalizeTypeTestCase {
            description: "a name Polyglot keeps stays as written",
            spelling: "LONG",
        },
        NormalizeTypeTestCase {
            description: "a word holding an alias is not changed inside",
            spelling: "INTEGERS",
        },
        NormalizeTypeTestCase {
            description: "a sized string keeps its length",
            spelling: "VARCHAR(10)",
        },
        NormalizeTypeTestCase {
            description: "an alias inside an array",
            spelling: "ARRAY<INTEGER>",
        },
        NormalizeTypeTestCase {
            description: "aliases inside a map and a struct",
            spelling: "MAP<STRING, STRUCT<a: NUMERIC(5,2), b: INTEGER>>",
        },
    ];
    for test_case in test_cases {
        let polyglot: NormalizedType = normalize_type(test_case.spelling, DATABRICKS)
            .expect("Polyglot normalizes every Databricks spelling")
            .normalized;
        let native: Option<NormalizedType> = DatabricksDialect.normalize_type(test_case.spelling);
        assert_eq!(native, Some(polyglot), "{}", test_case.description);
    }
}
