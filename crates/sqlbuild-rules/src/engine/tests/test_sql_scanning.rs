use crate::engine::tests::test_types::{
    DialectCallsTestCase, DollarQuoteSiteTestCase, LintDialectSiteTestCase, SqlScannerTestCase,
};
use crate::lint_text::_helpers::preparation::prepare;
use crate::query_facts::main::model_shape::model_shape;
use crate::rules::_helpers::numeric_decisions::compact_sql;
use sqlbuild_sqltext::sql_scan::main::matching_paren::matching_paren;
use sqlbuild_sqltext::sql_scan::main::quote_policy::quote_policy as rules_quote_policy;
use sqlbuild_sqltext::sql_scan::models::QuotePolicy;
use sqlbuild_sqltext::sql_scan::models::Unclosed;

#[test]
fn given_quoted_commented_and_malformed_fragments_when_scanning_then_every_scanner_is_characterised()
-> Result<(), String> {
    let test_cases = [
        SqlScannerTestCase {
            description: "plain code",
            fragment: "a",
            expected_compiler_paren: Ok(4),
            expected_dependency_calls: &["__ref(\"o\")"],
            expected_compact: "a=aandb",
            expected_snowflake_compact: "a=aandb",
            expected_lint_site: Some("@py.m(a x)"),
        },
        SqlScannerTestCase {
            description: "single quote containing open paren",
            fragment: "'a(b'",
            expected_compiler_paren: Ok(8),
            expected_dependency_calls: &["__ref(\"o\")"],
            expected_compact: "a='a(b'andb",
            expected_snowflake_compact: "a='a(b'andb",
            expected_lint_site: Some("@py.m('a(b' x)"),
        },
        SqlScannerTestCase {
            description: "single quote containing close paren",
            fragment: "'a)b'",
            expected_compiler_paren: Ok(8),
            expected_dependency_calls: &["__ref(\"o\")"],
            expected_compact: "a='a)b'andb",
            expected_snowflake_compact: "a='a)b'andb",
            expected_lint_site: Some("@py.m('a)b' x)"),
        },
        SqlScannerTestCase {
            description: "doubled single quote",
            fragment: "'it''s )'",
            expected_compiler_paren: Ok(12),
            expected_dependency_calls: &["__ref(\"o\")"],
            expected_compact: "a='it''s )'andb",
            expected_snowflake_compact: "a='it''s )'andb",
            expected_lint_site: Some("@py.m('it''s )' x)"),
        },
        SqlScannerTestCase {
            description: "backslash escaped quote before close paren",
            fragment: "'a\\')'",
            expected_compiler_paren: Ok(5),
            expected_dependency_calls: &[],
            expected_compact: "a='a\\')' and  b",
            expected_snowflake_compact: "a='a\\')'andb",
            expected_lint_site: Some("@py.m('a\\')' x)"),
        },
        SqlScannerTestCase {
            description: "backslash escaped quote before open paren",
            fragment: "'a\\'('",
            expected_compiler_paren: Err(Unclosed::Quote),
            expected_dependency_calls: &[],
            expected_compact: "a='a\\'(' and  b",
            expected_snowflake_compact: "a='a\\'('andb",
            expected_lint_site: Some("@py.m('a\\'(' x)"),
        },
        SqlScannerTestCase {
            description: "double quote containing close paren",
            fragment: "\"a)b\"",
            expected_compiler_paren: Ok(8),
            expected_dependency_calls: &["__ref(\"o\")"],
            expected_compact: "a=\"a)b\"andb",
            expected_snowflake_compact: "a=\"a)b\"andb",
            expected_lint_site: Some("@py.m(\"a)b\" x)"),
        },
        SqlScannerTestCase {
            description: "backtick containing close paren",
            fragment: "`a)b`",
            expected_compiler_paren: Ok(8),
            expected_dependency_calls: &["__ref(\"o\")"],
            expected_compact: "a=`a)b`andb",
            expected_snowflake_compact: "a=`a)b`andb",
            expected_lint_site: Some("@py.m(`a)"),
        },
        SqlScannerTestCase {
            description: "doubled backtick",
            fragment: "`a``)b`",
            expected_compiler_paren: Ok(10),
            expected_dependency_calls: &["__ref(\"o\")"],
            expected_compact: "a=`a``)b`andb",
            expected_snowflake_compact: "a=`a``)b`andb",
            expected_lint_site: Some("@py.m(`a``)"),
        },
        SqlScannerTestCase {
            description: "line comment",
            fragment: "-- )\n",
            expected_compiler_paren: Ok(8),
            expected_dependency_calls: &["__ref(\"o\")"],
            expected_compact: "a=andb",
            expected_snowflake_compact: "a=andb",
            expected_lint_site: Some("@py.m(-- )\n x)"),
        },
        SqlScannerTestCase {
            description: "block comment",
            fragment: "/* ) */",
            expected_compiler_paren: Ok(10),
            expected_dependency_calls: &["__ref(\"o\")"],
            expected_compact: "a=andb",
            expected_snowflake_compact: "a=andb",
            expected_lint_site: Some("@py.m(/* ) */ x)"),
        },
        SqlScannerTestCase {
            description: "nested block comment",
            fragment: "/* /* ) */ ) */",
            expected_compiler_paren: Ok(12),
            expected_dependency_calls: &["__ref(\"o\")"],
            expected_compact: "a=)*/andb",
            expected_snowflake_compact: "a=)*/andb",
            expected_lint_site: Some("@py.m(/* /* ) */ )"),
        },
        SqlScannerTestCase {
            description: "dollar quote",
            fragment: "$$ ) $$",
            expected_compiler_paren: Ok(10),
            expected_dependency_calls: &["__ref(\"o\")"],
            expected_compact: "a=$$)$$andb",
            expected_snowflake_compact: "a=$$)$$andb",
            expected_lint_site: Some("@py.m($$ ) $$ x)"),
        },
        SqlScannerTestCase {
            description: "unterminated quote",
            fragment: "'abc",
            expected_compiler_paren: Err(Unclosed::Quote),
            expected_dependency_calls: &[],
            expected_compact: "a='abc and  b",
            expected_snowflake_compact: "a='abc and  b",
            expected_lint_site: Some(""),
        },
        SqlScannerTestCase {
            description: "unterminated block comment",
            fragment: "/* abc",
            expected_compiler_paren: Err(Unclosed::BlockComment),
            expected_dependency_calls: &[],
            expected_compact: "a=",
            expected_snowflake_compact: "a=",
            expected_lint_site: Some(""),
        },
        SqlScannerTestCase {
            description: "multibyte utf8",
            fragment: "'é)ü' ñ",
            expected_compiler_paren: Ok(13),
            expected_dependency_calls: &["__ref(\"o\")"],
            expected_compact: "a='é)ü'ñandb",
            expected_snowflake_compact: "a='é)ü'ñandb",
            expected_lint_site: Some("@py.m('é)ü' ñ x)"),
        },
    ];

    for test_case in test_cases {
        let fragment = test_case.fragment;
        assert_eq!(
            matching_paren(
                format!("({fragment} x) y").as_bytes(),
                0,
                QuotePolicy::COMPILER
            ),
            test_case.expected_compiler_paren,
            "compiler matching paren: {}",
            test_case.description
        );
        let calls = model_shape(&format!("SELECT {fragment} x FROM __ref(\"o\")"), "duckdb")
            .map(|shape| {
                shape
                    .calls
                    .into_iter()
                    .map(|call| call.text)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        assert_eq!(
            calls, test_case.expected_dependency_calls,
            "rules dependency calls: {}",
            test_case.description
        );
        assert_eq!(
            compact_sql(
                &format!("A = {fragment} AND  B"),
                rules_quote_policy("duckdb")
            ),
            test_case.expected_compact,
            "rules numeric decision compaction: {}",
            test_case.description
        );
        assert_eq!(
            compact_sql(
                &format!("A = {fragment} AND  B"),
                rules_quote_policy("snowflake")
            ),
            test_case.expected_snowflake_compact,
            "rules Snowflake numeric decision compaction: {}",
            test_case.description
        );
        let lint_sql = format!("SELECT @py.m({fragment} x) AS y");
        let lint_site = prepare(&lint_sql, &lint_sql, &[], "postgres")?.map(|prepared| {
            prepared
                .1
                .first()
                .map(|site| site.original_text.clone())
                .unwrap_or_default()
        });
        assert_eq!(
            lint_site.as_deref(),
            test_case.expected_lint_site,
            "SQL lint macro site: {}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_lint_dialect_when_preparing_macro_sites_then_backticks_follow_rules_quoting()
-> Result<(), String> {
    let test_cases = [
        LintDialectSiteTestCase {
            description: "databricks backtick containing close paren",
            dialect: "databricks",
            fragment: "`a)b`",
            expected_lint_site: "@py.m(`a)b` x)",
        },
        LintDialectSiteTestCase {
            description: "bigquery doubled backtick",
            dialect: "bigquery",
            fragment: "`a``)b`",
            expected_lint_site: "@py.m(`a``)b` x)",
        },
        LintDialectSiteTestCase {
            description: "databricks single quote inside backticks",
            dialect: "databricks",
            fragment: "`it's)`",
            expected_lint_site: "@py.m(`it's)` x)",
        },
        LintDialectSiteTestCase {
            description: "databricks backslash in single quote still escapes",
            dialect: "databricks",
            fragment: "'a\\')'",
            expected_lint_site: "@py.m('a\\')' x)",
        },
        LintDialectSiteTestCase {
            description: "postgres backtick remains code",
            dialect: "postgres",
            fragment: "`a)b`",
            expected_lint_site: "@py.m(`a)",
        },
        LintDialectSiteTestCase {
            description: "duckdb dollar quote hides an apostrophe and close paren",
            dialect: "duckdb",
            fragment: "$$Customer's order)$$",
            expected_lint_site: "@py.m($$Customer's order)$$ x)",
        },
        LintDialectSiteTestCase {
            description: "snowflake tagged dollar quote hides a nested dollar quote",
            dialect: "snowflake",
            fragment: "$note$ $$ ) $note$",
            expected_lint_site: "@py.m($note$ $$ ) $note$ x)",
        },
        LintDialectSiteTestCase {
            description: "sqlserver backtick remains code",
            dialect: "tsql",
            fragment: "`a)b`",
            expected_lint_site: "@py.m(`a)",
        },
    ];
    for test_case in test_cases {
        let lint_sql = format!("SELECT @py.m({} x) AS y", test_case.fragment);
        let lint_site = prepare(&lint_sql, &lint_sql, &[], test_case.dialect)?
            .and_then(|prepared| prepared.1.first().map(|site| site.original_text.clone()))
            .unwrap_or_default();
        assert_eq!(
            lint_site, test_case.expected_lint_site,
            "SQL lint macro site: {}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_dollar_quoted_apostrophe_when_preparing_lint_sites_then_later_references_are_sites()
-> Result<(), String> {
    let test_cases = [
        DollarQuoteSiteTestCase {
            description: "duckdb untagged and tagged quotes",
            dialect: "duckdb",
            expected_sites: &["__ref(\"orders\")"],
        },
        DollarQuoteSiteTestCase {
            description: "postgres untagged and tagged quotes",
            dialect: "postgres",
            expected_sites: &["__ref(\"orders\")"],
        },
        DollarQuoteSiteTestCase {
            description: "snowflake untagged and tagged quotes",
            dialect: "snowflake",
            expected_sites: &["__ref(\"orders\")"],
        },
    ];
    let sql = "SELECT $$Customer's order$$ AS note, $tag$it's$tag$ AS tag FROM __ref(\"orders\") o";
    for test_case in test_cases {
        let sites: Vec<String> = prepare(sql, sql, &[], test_case.dialect)?
            .map(|prepared| {
                prepared
                    .1
                    .into_iter()
                    .map(|site| site.original_text)
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(sites, test_case.expected_sites, "{}", test_case.description);
    }
    Ok(())
}

#[test]
fn given_hash_before_reference_when_shaping_then_only_hash_comment_dialects_skip_it()
-> Result<(), String> {
    let test_cases = [
        DialectCallsTestCase {
            description: "bigquery treats # as a line comment",
            dialect: "bigquery",
            expected_calls: &["__ref(\"o\")"],
        },
        DialectCallsTestCase {
            description: "snowflake reads code after #",
            dialect: "snowflake",
            expected_calls: &["__ref(\"o\")", "__ref(\"p\")"],
        },
    ];
    let sql = "SELECT o.order_id FROM __ref(\"o\") AS o\nWHERE o.order_id > 0 # AND o.order_id IN (SELECT order_id FROM __ref(\"p\"))\n";
    for test_case in test_cases {
        let calls: Vec<String> = model_shape(sql, test_case.dialect)
            .map(|shape| shape.calls.into_iter().map(|call| call.text).collect())
            .unwrap_or_default();
        assert_eq!(calls, test_case.expected_calls, "{}", test_case.description);
    }
    Ok(())
}
