use crate::macro_calls::errors::ScanError;
use crate::macro_calls::main::scan_macro_call_sites::scan_macro_call_sites;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;
use std::time::Instant;

use crate::macro_calls::tests::test_types::{
    CallBoundsTestCase, DeepNestingTestCase, ExpectedSite, ScanMacroCallSitesTestCase,
};

#[test]
fn given_authored_sql_when_scanning_macro_calls_then_sites_match_python_expansion() {
    let test_cases = [
        ScanMacroCallSitesTestCase {
            description: "one call",
            sql: "SELECT @py.a(1) FROM t",
            expected_sites: &[(7, 15, "a", Some(&["a"]), false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "nested calls belong to the enclosing site",
            sql: "@py.outer(@py.inner(@py.leaf()), '@py.quoted()', @py.inner(2)) y",
            expected_sites: &[(0, 62, "outer", Some(&["outer", "inner", "leaf"]), false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "quoted text and comments hide calls",
            sql: "'@py.a(1)' -- @py.b(2)\n/* @py.c(3) */ \"@py.d()\" `@py.e()` $$@py.f()$$ @py.g()",
            expected_sites: &[(70, 77, "g", Some(&["g"]), false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "a one-letter name followed by whitespace is not a call",
            sql: "@py.a (1)",
            expected_sites: &[],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "a longer name may be separated from its parenthesis",
            sql: "@py.ab (1)",
            expected_sites: &[(0, 10, "ab", Some(&["ab"]), false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "whitespace inside a name fails in that call, as Python's evaluation does",
            sql: "@py.ab c(1)",
            expected_sites: &[],
            expected_failure: Some((Some(0), ScanError::MissingParenthesis)),
        },
        ScanMacroCallSitesTestCase {
            description: "an unclosed quote after a call fails after it",
            sql: "@py.a(1) 'oops",
            expected_sites: &[(0, 8, "a", Some(&["a"]), false)],
            expected_failure: Some((None, ScanError::UnclosedQuote)),
        },
        ScanMacroCallSitesTestCase {
            description: "an unclosed call fails in that call",
            sql: "@py.a(1",
            expected_sites: &[],
            expected_failure: Some((Some(0), ScanError::UnclosedParenthesis)),
        },
        ScanMacroCallSitesTestCase {
            description: "offsets count code points",
            sql: "SELECT 'é', @py.a(1)",
            expected_sites: &[(12, 20, "a", Some(&["a"]), false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "a non-ASCII letter after @ starts a call as Python's isalpha says",
            sql: "@py.é(1)",
            expected_sites: &[(0, 8, "é", Some(&["é"]), false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "a non-letter non-ASCII character after @ is not a call",
            sql: "@€(1) @py.m()",
            expected_sites: &[(6, 13, "m", Some(&["m"]), false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "an unclosed block comment between calls fails there",
            sql: "@py.a() /* @py.b()",
            expected_sites: &[(0, 7, "a", Some(&["a"]), false)],
            expected_failure: Some((None, ScanError::UnclosedComment)),
        },
        ScanMacroCallSitesTestCase {
            description: "an unclosed dollar quote inside a call fails in that call",
            sql: "x @py.a() é @py.b($tag$ )",
            expected_sites: &[(2, 9, "a", Some(&["a"]), false)],
            expected_failure: Some((Some(12), ScanError::UnclosedQuote)),
        },
        ScanMacroCallSitesTestCase {
            description: "typed reference text is flagged",
            sql: "@py.m(__ref('x')) @py.n('__seed') @py.o(1)",
            expected_sites: &[
                (0, 17, "m", Some(&["m"]), true),
                (18, 33, "n", Some(&["n"]), true),
                (34, 42, "o", Some(&["o"]), false),
            ],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "a dollar sign continuing a word does not open a quote",
            sql: "a$$ @py.m()",
            expected_sites: &[(4, 11, "m", Some(&["m"]), false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "interpolation markers are not calls",
            sql: "x @@var @py.m()",
            expected_sites: &[(8, 15, "m", Some(&["m"]), false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "nested names follow source order across siblings and plain parentheses",
            sql: "@py.a((@py.b(@py.c(1)), (@py.d()) ), \"@py.x()\", @py.c())",
            expected_sites: &[(0, 56, "a", Some(&["a", "b", "c", "d"]), false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "a quoted parenthesis inside a nested call does not close it",
            sql: "@py.a(@py.b(')'))",
            expected_sites: &[(0, 17, "a", Some(&["a", "b"]), false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "a nested name without its parenthesis leaves the tree unscanned",
            sql: "@py.a(@py.bb c(1))",
            expected_sites: &[(0, 18, "a", None, false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "parentheses inside quoted arguments do not close the call",
            sql: "@py.m(')', \"(\", `)`) z",
            expected_sites: &[(0, 20, "m", Some(&["m"]), false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "a call without a kind fails in that call",
            sql: "@py.a(1) @b(2)",
            expected_sites: &[(0, 8, "a", Some(&["a"]), false)],
            expected_failure: Some((Some(9), ScanError::BareCall)),
        },
        ScanMacroCallSitesTestCase {
            description: "a name followed by words and a parenthesis is not a call",
            sql: "@value IS NULL OR TRIM(@value) = '' AND @x IN ('a', 'b') AND @y IS NULL AND (@y IS NULL)",
            expected_sites: &[],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "a name without a kind spaced from its parenthesis is not a call",
            sql: "@ab (1) @py.m()",
            expected_sites: &[(8, 15, "m", Some(&["m"]), false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "a SQL macro call fails the Python macro scan",
            sql: "@sql.a(1)",
            expected_sites: &[],
            expected_failure: Some((Some(0), ScanError::SqlCall)),
        },
        ScanMacroCallSitesTestCase {
            description: "declaration references need no kind",
            sql: "@enum('s').A @const('c') @py.m()",
            expected_sites: &[
                (0, 10, "enum", Some(&["enum"]), false),
                (13, 24, "const", Some(&["const"]), false),
                (25, 32, "m", Some(&["m"]), false),
            ],
            expected_failure: None,
        },
    ];

    for test_case in test_cases {
        let scan = scan_macro_call_sites(test_case.sql, None);
        let actual: Vec<ExpectedSite> = scan
            .sites
            .into_iter()
            .map(|site| {
                let names: Option<&'static [&'static str]> = site.tree_names.map(|names| {
                    let names: Vec<&'static str> = names
                        .into_iter()
                        .map(|name| &*Box::leak(name.into_boxed_str()))
                        .collect();
                    &*Box::leak(names.into_boxed_slice())
                });
                (
                    site.start,
                    site.end,
                    &*Box::leak(site.name.into_boxed_str()),
                    names,
                    site.typed_reference_text,
                )
            })
            .collect();
        assert_eq!(
            (
                actual,
                scan.failure
                    .map(|failure| (failure.call_start, failure.error))
            ),
            (
                test_case.expected_sites.to_vec(),
                test_case.expected_failure
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_deeply_nested_calls_when_scanning_then_time_is_linear_and_nothing_overflows() {
    let test_cases = [
        DeepNestingTestCase {
            description: "one thousand levels",
            depth: 1_000,
            expected_tree_names: &["m"],
            expected_max_seconds: 2.0,
        },
        DeepNestingTestCase {
            description: "twenty thousand levels",
            depth: 20_000,
            expected_tree_names: &["m"],
            expected_max_seconds: 2.0,
        },
        DeepNestingTestCase {
            description: "one hundred thousand levels",
            depth: 100_000,
            expected_tree_names: &["m"],
            expected_max_seconds: 2.0,
        },
    ];

    for test_case in test_cases {
        let sql = format!(
            "{}x{}",
            "@py.m(".repeat(test_case.depth),
            ")".repeat(test_case.depth)
        );
        let started = Instant::now();

        let scan = scan_macro_call_sites(&sql, None);

        assert!(
            started.elapsed().as_secs_f64() < test_case.expected_max_seconds,
            "{}",
            test_case.description
        );
        assert_eq!(
            (
                scan.sites
                    .iter()
                    .map(|site| site.tree_names.clone())
                    .collect::<Vec<_>>(),
                scan.failure,
            ),
            (
                vec![Some(
                    test_case
                        .expected_tree_names
                        .iter()
                        .map(|name| (*name).to_owned())
                        .collect()
                )],
                None
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_calls_when_scanning_then_argument_starts_and_failing_call_names_match_python() {
    let test_cases = [
        CallBoundsTestCase {
            description: "argument starts skip the whitespace before the parenthesis",
            sql: "x @py.ab  (1) @py.\u{e9}(@py.b())",
            expected_args_starts: &[11, 20],
            expected_failure: None,
        },
        CallBoundsTestCase {
            description: "a call without its parenthesis names the failing call",
            sql: "@py.a() @py.bb c(1)",
            expected_args_starts: &[6],
            expected_failure: Some((Some(8), Some("bb"), ScanError::MissingParenthesis)),
        },
        CallBoundsTestCase {
            description: "an unclosed call after non-ASCII text names the failing call",
            sql: "'\u{e9}' @py.c\u{e9}(1",
            expected_args_starts: &[],
            expected_failure: Some((Some(4), Some("c\u{e9}"), ScanError::UnclosedParenthesis)),
        },
        CallBoundsTestCase {
            description: "a failure between calls names none",
            sql: "@py.a(1) 'open",
            expected_args_starts: &[6],
            expected_failure: Some((None, None, ScanError::UnclosedQuote)),
        },
    ];
    for test_case in test_cases {
        let scan = scan_macro_call_sites(test_case.sql, None);

        assert_eq!(
            (
                scan.sites
                    .iter()
                    .map(|site| site.args_start)
                    .collect::<Vec<_>>(),
                scan.failure
                    .map(|failure| (failure.call_start, failure.call_name, failure.error)),
            ),
            (
                test_case.expected_args_starts.to_vec(),
                test_case.expected_failure.map(|(start, name, error)| (
                    start,
                    name.map(str::to_owned),
                    error
                )),
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_backslash_escapes_when_scanning_macro_calls_then_dialect_quotes_are_text() {
    let syntax = LexicalSyntax {
        backslash_escape_quotes: vec!["'".to_owned()],
        line_comment_prefixes: vec!["--".to_owned(), "//".to_owned()],
        ..LexicalSyntax::default()
    };
    let test_cases = [
        ScanMacroCallSitesTestCase {
            description: "an escaped quote before a call stays inside its string",
            sql: "'O\\'Brien @py.hidden()' @py.m('a\\')')",
            expected_sites: &[(24, 37, "m", Some(&["m"]), false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "a backslash at the end of a literal escapes itself",
            sql: "'C:\\\\' @py.m(')')",
            expected_sites: &[(7, 17, "m", Some(&["m"]), false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "a dialect line comment hides a call",
            sql: "// it's @py.hidden()\n@py.m()",
            expected_sites: &[(21, 28, "m", Some(&["m"]), false)],
            expected_failure: None,
        },
        ScanMacroCallSitesTestCase {
            description: "a dialect line comment marker inside Python arguments is an operator",
            sql: "@py.m(10 // 3) AS a",
            expected_sites: &[(0, 14, "m", Some(&["m"]), false)],
            expected_failure: None,
        },
    ];

    for test_case in test_cases {
        let scan = scan_macro_call_sites(test_case.sql, Some(&syntax));
        let actual: Vec<(usize, usize, String)> = scan
            .sites
            .into_iter()
            .map(|site| (site.start, site.end, site.name))
            .collect();

        assert_eq!(
            (
                actual,
                scan.failure
                    .map(|failure| (failure.call_start, failure.error))
            ),
            (
                test_case
                    .expected_sites
                    .iter()
                    .map(|(start, end, name, _, _)| (*start, *end, (*name).to_owned()))
                    .collect::<Vec<(usize, usize, String)>>(),
                test_case.expected_failure
            ),
            "{}",
            test_case.description
        );
    }
}
