use crate::analysis_route::main::skipped_analysis::{skipped_analysis, unavailable_analysis};
use crate::analysis_route::models::{LocatedGap, TypeMatch};
use crate::analysis_route::tests::test_types::{
    NativeTypeMatchTestCase, PivotlessDialect, ReadWholeTestCase,
};
use crate::analysis_route::types::NativeDialect;

#[test]
fn given_sql_when_reading_it_whole_then_only_one_parsed_statement_without_a_gap_is_read() {
    let test_cases = [
        ReadWholeTestCase {
            description: "one query is read whole",
            sql: "SELECT a FROM t",
            expected_gap: None,
        },
        ReadWholeTestCase {
            description: "two statements are not one model",
            sql: "SELECT 1; SELECT 2",
            expected_gap: Some("more than one statement"),
        },
        ReadWholeTestCase {
            description: "no statement at all",
            sql: "  ",
            expected_gap: Some("no statement"),
        },
        ReadWholeTestCase {
            description: "a statement kept as raw text is not read",
            sql: "RAW VACUUM t",
            expected_gap: Some("unparsed statement"),
        },
        ReadWholeTestCase {
            description: "a construct the parser refuses stops it first",
            sql: "SELECT * FROM t PIVOT (sum(x) FOR y IN (1))",
            expected_gap: Some("PIVOT"),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            PivotlessDialect
                .read_whole(test_case.sql)
                .err()
                .map(|gap| gap.construct),
            test_case.expected_gap.map(str::to_owned),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_type_the_dialect_refuses_when_matching_types_then_nothing_is_compared() {
    let test_cases = [
        NativeTypeMatchTestCase {
            description: "a refused declared type is not compared",
            declared: "NOT_A_TYPE",
            inferred: "INTEGER",
            expected_match: TypeMatch::NotCompared,
        },
        NativeTypeMatchTestCase {
            description: "a refused inferred type is not compared",
            declared: "INTEGER",
            inferred: "NOT_A_TYPE",
            expected_match: TypeMatch::NotCompared,
        },
        NativeTypeMatchTestCase {
            description: "two known, different types still mismatch",
            declared: "INTEGER",
            inferred: "TEXT",
            expected_match: TypeMatch::Compared {
                same_type: false,
                nullable_into_non_null: false,
            },
        },
        NativeTypeMatchTestCase {
            description: "the same type matches",
            declared: "INTEGER",
            inferred: "integer",
            expected_match: TypeMatch::Compared {
                same_type: true,
                nullable_into_non_null: false,
            },
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            PivotlessDialect.type_match(test_case.declared, test_case.inferred, true),
            test_case.expected_match,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_parser_message_or_no_parser_when_skipping_analysis_then_p026_says_why() {
    let syntax: LocatedGap = LocatedGap {
        construct: "syntax error".to_owned(),
        message: "syntax error at or near \"FROM\"".to_owned(),
        line: 1,
        column: 15,
        syntax: true,
    };
    assert_eq!(
        skipped_analysis("orders", "postgres", &syntax),
        (
            "P026",
            "SQL analysis skipped for model 'orders': the postgres parser cannot read syntax \
             error at line 1, column 15: syntax error at or near \"FROM\""
                .to_owned()
        ),
        "the parser's message follows the construct"
    );
    assert_eq!(
        unavailable_analysis("orders", Some("trino")),
        (
            "P026",
            "SQL analysis skipped for model 'orders': no native parser reads the trino dialect"
                .to_owned()
        ),
        "an unknown dialect is named"
    );
    assert_eq!(
        unavailable_analysis("orders", None).1,
        "SQL analysis skipped for model 'orders': no native parser reads the generic dialect",
        "no dialect is the generic dialect"
    );
}
