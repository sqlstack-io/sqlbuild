use crate::semantic_validation::_helpers::diagnostics::map_diagnostics;
use crate::semantic_validation::_helpers::function_names::unsupported_calls;
use crate::semantic_validation::tests::test_types::{
    LocatedFunctionDiagnosticTestCase, UnsupportedFunctionTestCase,
};
use polyglot_sql::{DialectType, ValidationResult};

#[test]
fn given_unsupported_builtin_spelling_when_scanning_then_reports_dialect_spelling() {
    let test_cases = [
        UnsupportedFunctionTestCase {
            description: "Snowflake STARTS_WITH suggests STARTSWITH",
            dialect: "snowflake",
            sql: "SELECT STARTS_WITH(product_name, 'a') AS flagged FROM products",
            expected_calls: &[("STARTS_WITH", "STARTSWITH")],
        },
        UnsupportedFunctionTestCase {
            description: "matching is case-insensitive and keeps the authored spelling",
            dialect: "snowflake",
            sql: "select ends_with(product_name, 'a'), lcase(product_name) from products",
            expected_calls: &[("ends_with", "ENDSWITH"), ("lcase", "LOWER")],
        },
        UnsupportedFunctionTestCase {
            description: "conditional and length aliases suggest the Snowflake spelling",
            dialect: "snowflake",
            sql: "SELECT IF(quantity > 0, 1, 0), CHAR_LENGTH(product_name) FROM products",
            expected_calls: &[("IF", "IFF"), ("CHAR_LENGTH", "LENGTH")],
        },
        UnsupportedFunctionTestCase {
            description: "calls inside windows, lambdas and table functions are checked",
            dialect: "snowflake",
            sql: "SELECT SUM(quantity) OVER (PARTITION BY UCASE(category)), \
                  FILTER(tags, tag -> STARTS_WITH(tag, 'a')) \
                  FROM products, TABLE(FLATTEN(input => ARRAY_CONSTRUCT(LCASE(sku))))",
            expected_calls: &[
                ("UCASE", "UPPER"),
                ("STARTS_WITH", "STARTSWITH"),
                ("LCASE", "LOWER"),
            ],
        },
        UnsupportedFunctionTestCase {
            description: "an object literal value after a string key is checked",
            dialect: "snowflake",
            sql: "SELECT {'featured': STARTS_WITH(product_name, 'a')} FROM products",
            expected_calls: &[("STARTS_WITH", "STARTSWITH")],
        },
        UnsupportedFunctionTestCase {
            description: "a MATCH_RECOGNIZE DEFINE condition after AS is checked",
            dialect: "snowflake",
            sql: "SELECT * FROM products MATCH_RECOGNIZE (ORDER BY sku PATTERN (featured) \
                  DEFINE featured AS STARTS_WITH(product_name, 'a'))",
            expected_calls: &[("STARTS_WITH", "STARTSWITH")],
        },
        UnsupportedFunctionTestCase {
            description: "comments between the name and its parenthesis are skipped",
            dialect: "snowflake",
            sql: "SELECT LCASE /* lower */ (sku), UCASE -- upper\n (sku) FROM products",
            expected_calls: &[("LCASE", "LOWER"), ("UCASE", "UPPER")],
        },
        UnsupportedFunctionTestCase {
            description: "operand FROM and select lists inside a FROM subquery are checked",
            dialect: "snowflake",
            sql: "SELECT TRIM(BOTH 'x' FROM LCASE(sku)), sku IS DISTINCT FROM UCASE(sku) \
                  FROM (SELECT sku, LCASE(sku) AS lowered FROM products) AS listed, catalog \
                  WHERE sku IN (1, 2) AND LCASE(sku) = 'a'",
            expected_calls: &[
                ("LCASE", "LOWER"),
                ("UCASE", "UPPER"),
                ("LCASE", "LOWER"),
                ("LCASE", "LOWER"),
            ],
        },
        UnsupportedFunctionTestCase {
            description: "a call followed by AS NOT without MATERIALIZED is still checked",
            dialect: "duckdb",
            sql: "SELECT STARTSWITH(product_name, 'a') AS not_featured FROM products",
            expected_calls: &[("STARTSWITH", "STARTS_WITH")],
        },
        UnsupportedFunctionTestCase {
            description: "DuckDB STARTSWITH suggests the catalogue spelling",
            dialect: "duckdb",
            sql: "SELECT STARTSWITH(product_name, 'a') FROM products",
            expected_calls: &[("STARTSWITH", "STARTS_WITH")],
        },
    ];
    for test_case in &test_cases {
        let calls = unsupported_calls(test_case.sql, test_case.dialect);
        let observed: Vec<(&str, &str)> = calls
            .iter()
            .map(|call| {
                (
                    &test_case.sql[call.start..call.end],
                    call.suggestion.as_str(),
                )
            })
            .collect();

        assert_eq!(
            observed, test_case.expected_calls,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_supported_or_non_builtin_call_when_scanning_then_reports_nothing() {
    let test_cases = [
        UnsupportedFunctionTestCase {
            description: "listed Snowflake synonyms are accepted",
            dialect: "snowflake",
            sql: "SELECT STARTSWITH(a, 'x'), ENDSWITH(a, 'x'), CONTAINS(a, 'x'), SUBSTR(a, 1, 2), \
                  SUBSTRING(a, 1, 2), TIMESTAMPDIFF(day, b, c), DATEDIFF(day, b, c), \
                  TRY_TO_DECIMAL(a, 10, 2), TRY_TO_NUMBER(a), IFF(d, 1, 0), LEN(a), \
                  LENGTH(a), CEIL(e), DATE_PART(year, b), COUNT_IF(d), POSITION('x', a), \
                  CHARINDEX('x', a) FROM products",
            expected_calls: &[],
        },
        UnsupportedFunctionTestCase {
            description: "quoted identifiers, literals and comments are not call sites",
            dialect: "snowflake",
            sql: "SELECT \"STARTS_WITH\"(a, 'x'), 'STARTS_WITH(a, b)', $$LCASE(a)$$ \
                  -- STARTS_WITH(a, 'x')\n /* LCASE(a) */ FROM products",
            expected_calls: &[],
        },
        UnsupportedFunctionTestCase {
            description: "methods, qualified functions, sentinels and macros are not checked",
            dialect: "snowflake",
            sql: "SELECT tools.starts_with(a, 'x'), analytics.helpers.lcase(a), \
                  __sqlbuild_udf_starts_with(a, 'x'), @py.starts_with(a), payload:lcase(a) \
                  FROM __ref(\"products\")",
            expected_calls: &[],
        },
        UnsupportedFunctionTestCase {
            description: "unknown and user-defined function names are never flagged",
            dialect: "snowflake",
            sql: "SELECT normalize_sku(a), DATE_ADD(day, 1, b), my_udf() FROM products",
            expected_calls: &[],
        },
        UnsupportedFunctionTestCase {
            description: "type positions and keywords before parentheses are not calls",
            dialect: "snowflake",
            sql: "WITH ranked (sku, position_rank) AS (SELECT DISTINCT (a), \
                  ROW_NUMBER() OVER (ORDER BY b) FROM products WHERE NOT (a IN (1, 2)) \
                  AND EXISTS (SELECT 1)) \
                  SELECT CAST(sku AS VARCHAR(10)), position_rank::NUMBER(10, 2) FROM ranked",
            expected_calls: &[],
        },
        UnsupportedFunctionTestCase {
            description: "CTE column lists are not calls",
            dialect: "snowflake",
            sql: "WITH listed AS (SELECT 1 AS dow), day_of_week (dow) AS (SELECT 1), \
                  lcase(x) AS (SELECT 2) SELECT dow FROM day_of_week",
            expected_calls: &[],
        },
        UnsupportedFunctionTestCase {
            description: "materialized CTE column lists are not calls",
            dialect: "duckdb",
            sql: "WITH day_of_week (d) AS MATERIALIZED (SELECT 1), \
                  day_of_month (d) AS /* hint */ NOT -- keep\n MATERIALIZED (SELECT 2) \
                  SELECT * FROM day_of_week",
            expected_calls: &[],
        },
        UnsupportedFunctionTestCase {
            description: "derived-table, table-function and table aliases are not calls",
            dialect: "snowflake",
            sql: "SELECT x FROM (VALUES (1)) day_of_week (x), products lcase (y), \
                  TABLE(FLATTEN(input => tags)) ucase (z), day_of_month (w) \
                  JOIN char_length (v) ON TRUE",
            expected_calls: &[],
        },
        UnsupportedFunctionTestCase {
            description: "INSERT and CREATE column lists are not calls",
            dialect: "snowflake",
            sql: "CREATE TABLE day_of_week (dow INTEGER); CREATE VIEW lcase (x) AS SELECT 1; \
                  INSERT INTO ucase (x) SELECT 1; CREATE TABLE IF NOT EXISTS char_length (x INT)",
            expected_calls: &[],
        },
        UnsupportedFunctionTestCase {
            description: "dialects without a spelling table are not checked",
            dialect: "postgres",
            sql: "SELECT STARTS_WITH(a, 'x'), LCASE(a) FROM products",
            expected_calls: &[],
        },
        UnsupportedFunctionTestCase {
            description: "DuckDB catalogue spellings are accepted",
            dialect: "duckdb",
            sql: "SELECT starts_with(a, 'x'), prefix(a, 'x'), lcase(a), substr(a, 1, 2) \
                  FROM products",
            expected_calls: &[],
        },
    ];
    for test_case in &test_cases {
        let calls = unsupported_calls(test_case.sql, test_case.dialect);
        let observed: Vec<(&str, &str)> = calls
            .iter()
            .map(|call| (call.name.as_str(), call.suggestion.as_str()))
            .collect();

        assert_eq!(
            observed, test_case.expected_calls,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_unsupported_spelling_when_mapping_diagnostics_then_returns_located_b101()
-> Result<(), String> {
    let test_cases = [LocatedFunctionDiagnosticTestCase {
        description: "the diagnostic points at the authored name after a multibyte literal",
        sql: "SELECT\n    'é' AS label, STARTS_WITH(product_name, 'a')\nFROM products",
        expected_message: "Unknown function 'STARTS_WITH' for dialect Snowflake; \
                           did you mean STARTSWITH?",
        expected_location: (Some(2), Some(19)),
        expected_span: (Some(25), Some(36)),
    }];
    for test_case in &test_cases {
        let result = map_diagnostics(
            test_case.sql,
            DialectType::Snowflake,
            ValidationResult {
                valid: true,
                errors: Vec::new(),
            },
        )?;
        let [error] = result.errors.as_slice() else {
            return Err(format!("{}: {:?}", test_case.description, result.errors));
        };

        assert!(!result.valid, "{}", test_case.description);
        assert_eq!(error.code, "B101", "{}", test_case.description);
        assert_eq!(
            error.message, test_case.expected_message,
            "{}",
            test_case.description
        );
        assert_eq!(
            (error.line, error.column),
            test_case.expected_location,
            "{}",
            test_case.description
        );
        assert_eq!(
            (error.start, error.end),
            test_case.expected_span,
            "{}",
            test_case.description
        );
    }
    Ok(())
}
