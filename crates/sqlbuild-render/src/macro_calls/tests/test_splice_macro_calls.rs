use crate::macro_calls::main::splice_macro_calls::splice_macro_calls;
use crate::macro_calls::tests::test_types::SpliceMacroCallsTestCase;

#[test]
fn given_call_sites_and_outputs_when_splicing_then_sql_and_spans_match_python() {
    let test_cases = [
        SpliceMacroCallsTestCase {
            description: "one call",
            sql: "SELECT @a(1) FROM t",
            sites: &[(7, 12)],
            outputs: &["x+1"],
            expected_splice: Some(("SELECT x+1 FROM t", &[(7, 12, 7, 10)])),
        },
        SpliceMacroCallsTestCase {
            description: "code-point offsets around non-ASCII text",
            sql: "é @a() ü @b()",
            sites: &[(2, 6), (9, 13)],
            outputs: &["ö", "zz"],
            expected_splice: Some(("é ö ü zz", &[(2, 6, 2, 3), (9, 13, 6, 8)])),
        },
        SpliceMacroCallsTestCase {
            description: "no sites leave the text unchanged",
            sql: "a @ b",
            sites: &[],
            outputs: &[],
            expected_splice: Some(("a @ b", &[])),
        },
        SpliceMacroCallsTestCase {
            description: "descending sites are rejected",
            sql: "@a() @b()",
            sites: &[(5, 9), (0, 4)],
            outputs: &["x", "y"],
            expected_splice: None,
        },
        SpliceMacroCallsTestCase {
            description: "out-of-bounds sites are rejected",
            sql: "é @a()",
            sites: &[(2, 9)],
            outputs: &["x"],
            expected_splice: None,
        },
    ];

    for test_case in test_cases {
        let outputs: Vec<String> = test_case
            .outputs
            .iter()
            .map(|output| (*output).to_owned())
            .collect();
        let actual = splice_macro_calls(test_case.sql, test_case.sites, &outputs, None).map(
            |(sql, spans)| {
                (
                    sql,
                    spans
                        .into_iter()
                        .map(|span| {
                            (
                                span.source_start,
                                span.source_end,
                                span.output_start,
                                span.output_end,
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            },
        );
        assert_eq!(
            actual,
            test_case
                .expected_splice
                .map(|(sql, spans)| (sql.to_owned(), spans.to_vec())),
            "{}",
            test_case.description
        );
    }
}
