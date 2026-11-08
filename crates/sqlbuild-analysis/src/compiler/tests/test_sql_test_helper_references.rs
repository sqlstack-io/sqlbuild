use serde_json::{Value, json};

use crate::compiler::_helpers::sql_tests::extraction::generic_syntax;
use crate::compiler::main::sql_test_assembly::assemble_sql_test_batch;
use crate::compiler::models::{
    AssembledSqlTestFacts, SqlTestAssemblyBatch, SqlTestAssemblyModel, SqlTestAssemblyModelPayload,
    SqlTestAssemblyOutcome, SqlTestAssemblyPayload, SqlTestAssemblyReference, SqlTestAssemblyTest,
    SqlTestCte, SqlTestHelperDiagnostic, SqlTestParameterValue,
};
use crate::compiler::tests::helpers::{
    chain_helper_reference_case, defined_before, plan_helper_reference_case,
    plan_helper_reference_response,
};
use crate::compiler::tests::test_types::{
    HelperReferenceTestCase, SqlTestAssemblyTestCase, UnresolvedReaderReferenceTestCase,
};

const HELPER_READS_MODEL: (&str, &str) = (
    "doubled",
    "SELECT order_id, amount_doubled FROM __ref(\"orders\")",
);
const MOCK_STG_ORDERS: (&str, &str) = (
    "__ref__stg_orders",
    "SELECT 5 AS order_id, 10 AS amount, 7 AS region_id",
);
const REAL_STG_ORDERS_BODY: &str = "SELECT order_id, amount, region_id FROM __source__raw_orders";
const ASSERT_MOCK_ROWS: (&str, &str) = (
    "__assert__mock_rows",
    "SELECT order_id FROM __ref(\"stg_orders\") WHERE order_id <> 5",
);
const EXPECTED_FROM_MOCK: (&str, &str) = (
    "__expected__orders",
    "SELECT order_id, amount * 2 AS amount_doubled FROM __ref(\"stg_orders\")",
);
const ASSERT_DOUBLED: (&str, &str) = (
    "__assert__doubles_amount",
    "SELECT order_id FROM doubled WHERE amount_doubled <> 20",
);

const MOCK_READS_HELPER_TEST: &str = "TEST (name \"mock_reads_helper\");\n\nWITH\n\
orders_feed AS (SELECT * FROM __source(\"raw_orders\")),\n\
__ref__stg_orders AS (SELECT * FROM orders_feed),\n\
__expected__customer_totals AS (SELECT 1 AS customer_id)\nSELECT 1\n";
/// Python's `build_sql_test_case_fingerprint` of the parameterized case below.
const PYTHON_CASE_FINGERPRINT: &str =
    "8f2adcab557ba4476fcb70de1fc11014acda9f1b597a3ca5c3245afb632e5d0f";
/// Python's fingerprint of the case with 29 ones, which its decimal context rounds to 28 digits.
const PYTHON_ROUNDED_CASE_FINGERPRINT: &str =
    "58f918fcd3fa8df50c2b10a9e8671c1f3b49eb80af3d48b7eb609392e255ccb4";
/// Python's fingerprint of the case with 29 nines, whose rounding carries into a new digit.
const PYTHON_CARRIED_CASE_FINGERPRINT: &str =
    "6da8c843ce65781616436ca7665394b9495efc3881955fbe3b58be610e3faa7a";

#[test]
fn given_helper_cte_references_when_planning_then_references_resolve_in_dependency_order() {
    let test_cases = [
        HelperReferenceTestCase {
            description: "helper reading a model is emitted after the model and its mocks",
            sql_analysis_enabled: true,
            helpers: &[HELPER_READS_MODEL],
            expected: &[],
            assertions: &[ASSERT_DOUBLED],
            read_helpers: &["doubled"],
            reference_targets: &["orders"],
            expected_chain: &["stg_orders", "orders"],
            expected_order: &[
                ("__source__raw_orders", "__ref__stg_orders"),
                ("__ref__stg_orders", "__ref__orders"),
                ("__ref__orders", "__helper__doubled"),
                ("__helper__doubled", "__assert__doubles_amount"),
            ],
            expected_fragments: &[
                "__helper__doubled AS (SELECT order_id, amount_doubled FROM __ref__orders)",
            ],
            expected_absent_fragments: &[],
        },
        HelperReferenceTestCase {
            description: "helper inlined without sql analysis reads the model CTE",
            sql_analysis_enabled: false,
            helpers: &[HELPER_READS_MODEL],
            expected: &[],
            assertions: &[ASSERT_DOUBLED],
            read_helpers: &["doubled"],
            reference_targets: &["orders"],
            expected_chain: &["stg_orders", "orders"],
            expected_order: &[
                ("__source__raw_orders", "__ref__orders"),
                ("__ref__orders", "__assert__doubles_amount"),
            ],
            expected_fragments: &[
                "WITH doubled AS (SELECT order_id, amount_doubled FROM __ref__orders)",
            ],
            expected_absent_fragments: &[],
        },
        HelperReferenceTestCase {
            description: "helper reading another helper, a model and a seed is ordered after all three",
            sql_analysis_enabled: true,
            helpers: &[
                (
                    "with_region",
                    "SELECT b.order_id, r.region_name FROM base AS b \
                     JOIN __ref(\"stg_orders\") AS s USING (order_id) \
                     JOIN __seed(\"regions\") AS r USING (region_id)",
                ),
                (
                    "base",
                    "SELECT order_id, amount_doubled FROM __ref(\"orders\")",
                ),
            ],
            expected: &[],
            assertions: &[(
                "__assert__has_region",
                "SELECT order_id FROM with_region WHERE region_name IS NULL",
            )],
            read_helpers: &["with_region", "base"],
            reference_targets: &["stg_orders", "orders"],
            expected_chain: &["stg_orders", "orders"],
            expected_order: &[
                ("__ref__orders", "__helper__base"),
                ("__helper__base", "__helper__with_region"),
                ("__ref__stg_orders", "__helper__with_region"),
                ("__seed__regions", "__helper__with_region"),
                ("__helper__with_region", "__assert__has_region"),
            ],
            expected_fragments: &[
                "FROM __helper__base AS b JOIN __ref__stg_orders AS s USING (order_id) \
                 JOIN __seed__regions AS r USING (region_id)",
            ],
            expected_absent_fragments: &[],
        },
        HelperReferenceTestCase {
            description: "helper read by expected rows resolves sources and seeds to their mocks",
            sql_analysis_enabled: true,
            helpers: &[(
                "expected_rows",
                "SELECT s.order_id, s.amount * 2 AS amount_doubled FROM __source(\"raw_orders\") AS s \
                 JOIN __seed(\"regions\") AS r USING (region_id)",
            )],
            expected: &[(
                "__expected__orders",
                "SELECT order_id, amount_doubled FROM expected_rows",
            )],
            assertions: &[],
            read_helpers: &["expected_rows"],
            reference_targets: &[],
            expected_chain: &["stg_orders", "orders"],
            expected_order: &[
                ("__source__raw_orders", "__helper__expected_rows"),
                ("__seed__regions", "__helper__expected_rows"),
                ("__helper__expected_rows", "__expected__orders"),
            ],
            expected_fragments: &[
                "FROM __source__raw_orders AS s JOIN __seed__regions AS r USING (region_id)",
            ],
            expected_absent_fragments: &[],
        },
        HelperReferenceTestCase {
            description: "inlined helper read by expected rows reads the model CTE",
            sql_analysis_enabled: false,
            helpers: &[HELPER_READS_MODEL],
            expected: &[(
                "__expected__orders",
                "SELECT order_id, amount_doubled FROM doubled",
            )],
            assertions: &[],
            read_helpers: &["doubled"],
            reference_targets: &["orders"],
            expected_chain: &["stg_orders", "orders"],
            expected_order: &[("__ref__orders", "__expected__orders")],
            expected_fragments: &[
                "__expected__orders AS (WITH doubled AS (SELECT order_id, amount_doubled FROM __ref__orders)",
            ],
            expected_absent_fragments: &["__helper__"],
        },
        HelperReferenceTestCase {
            description: "helper and assertion reading a mocked model through different paths share one mock CTE",
            sql_analysis_enabled: true,
            helpers: &[
                (
                    "__ref__stg_orders",
                    "SELECT order_id, 10 AS amount FROM base_ids",
                ),
                ("base_ids", "SELECT 1 AS order_id"),
                (
                    "doubled",
                    "SELECT o.order_id, o.amount_doubled FROM __ref(\"orders\") AS o",
                ),
            ],
            expected: &[],
            assertions: &[(
                "__assert__doubles_amount",
                "SELECT order_id FROM doubled JOIN __ref(\"stg_orders\") USING (order_id) \
                 WHERE amount_doubled <> 20",
            )],
            read_helpers: &["doubled"],
            reference_targets: &["orders"],
            expected_chain: &["orders"],
            expected_order: &[
                ("__helper__base_ids", "__ref__orders"),
                ("__ref__orders", "__helper__doubled"),
                ("__helper__doubled", "__assert__doubles_amount"),
            ],
            expected_fragments: &[],
            expected_absent_fragments: &[],
        },
        HelperReferenceTestCase {
            description: "helper nothing reads is neither resolved nor emitted",
            sql_analysis_enabled: true,
            helpers: &[("unused_rows", "SELECT order_id FROM __ref(\"orders\")")],
            expected: &[(
                "__expected__stg_orders",
                "SELECT 1 AS order_id, 10 AS amount, 7 AS region_id",
            )],
            assertions: &[],
            read_helpers: &[],
            reference_targets: &[],
            expected_chain: &["stg_orders"],
            expected_order: &[("__source__raw_orders", "__expected__stg_orders")],
            expected_fragments: &[],
            expected_absent_fragments: &["unused_rows", "__ref__orders"],
        },
        HelperReferenceTestCase {
            description: "assertion reading a mocked model reads the mock, not the model",
            sql_analysis_enabled: true,
            helpers: &[MOCK_STG_ORDERS],
            expected: &[(
                "__expected__orders",
                "SELECT 5 AS order_id, 20 AS amount_doubled",
            )],
            assertions: &[ASSERT_MOCK_ROWS],
            read_helpers: &[],
            reference_targets: &[],
            expected_chain: &["orders"],
            expected_order: &[("__ref__stg_orders", "__assert__mock_rows")],
            expected_fragments: &["FROM __ref__stg_orders WHERE order_id <> 5"],
            expected_absent_fragments: &[REAL_STG_ORDERS_BODY, "__sqb_cte_"],
        },
        HelperReferenceTestCase {
            description: "assertion reading a mocked model reads the mock without sql analysis",
            sql_analysis_enabled: false,
            helpers: &[MOCK_STG_ORDERS],
            expected: &[],
            assertions: &[
                ASSERT_MOCK_ROWS,
                (
                    "__assert__doubled",
                    "SELECT order_id FROM __ref(\"orders\") WHERE amount_doubled <> 20",
                ),
            ],
            read_helpers: &[],
            reference_targets: &[],
            expected_chain: &["orders"],
            expected_order: &[
                ("__ref__stg_orders", "__ref__orders"),
                ("__ref__orders", "__assert__doubled"),
            ],
            expected_fragments: &["FROM __ref__stg_orders WHERE order_id <> 5"],
            expected_absent_fragments: &[REAL_STG_ORDERS_BODY, "__sqb_cte_"],
        },
        HelperReferenceTestCase {
            description: "expected rows reading a mocked model resolve to the mock",
            sql_analysis_enabled: true,
            helpers: &[MOCK_STG_ORDERS],
            expected: &[EXPECTED_FROM_MOCK],
            assertions: &[],
            read_helpers: &[],
            reference_targets: &[],
            expected_chain: &["orders"],
            expected_order: &[("__ref__stg_orders", "__expected__orders")],
            expected_fragments: &[
                "__expected__orders AS (SELECT order_id, amount * 2 AS amount_doubled FROM __ref__stg_orders)",
            ],
            expected_absent_fragments: &[REAL_STG_ORDERS_BODY, "__sqb_cte_"],
        },
        HelperReferenceTestCase {
            description: "expected rows reading a mocked model resolve to the mock without sql analysis",
            sql_analysis_enabled: false,
            helpers: &[MOCK_STG_ORDERS],
            expected: &[EXPECTED_FROM_MOCK],
            assertions: &[],
            read_helpers: &[],
            reference_targets: &[],
            expected_chain: &["orders"],
            expected_order: &[("__ref__stg_orders", "__expected__orders")],
            expected_fragments: &[
                "__expected__orders AS (SELECT order_id, amount * 2 AS amount_doubled FROM __ref__stg_orders)",
            ],
            expected_absent_fragments: &[REAL_STG_ORDERS_BODY, "__sqb_cte_"],
        },
        HelperReferenceTestCase {
            description: "expected rows reading an unmocked model run that model",
            sql_analysis_enabled: true,
            helpers: &[],
            expected: &[EXPECTED_FROM_MOCK],
            assertions: &[],
            read_helpers: &[],
            reference_targets: &["stg_orders"],
            expected_chain: &["stg_orders", "orders"],
            expected_order: &[("__ref__stg_orders", "__expected__orders")],
            expected_fragments: &["amount * 2 AS amount_doubled FROM __ref__stg_orders)"],
            expected_absent_fragments: &[],
        },
        HelperReferenceTestCase {
            description: "expected rows reading a source and a seed resolve to their mocks without sql analysis",
            sql_analysis_enabled: false,
            helpers: &[],
            expected: &[(
                "__expected__stg_orders",
                "SELECT s.order_id, s.amount, r.region_id FROM __source(\"raw_orders\") AS s \
                 JOIN __seed(\"regions\") AS r USING (region_id)",
            )],
            assertions: &[],
            read_helpers: &[],
            reference_targets: &[],
            expected_chain: &["stg_orders"],
            expected_order: &[
                ("__source__raw_orders", "__expected__stg_orders"),
                ("__seed__regions", "__expected__stg_orders"),
            ],
            expected_fragments: &[
                "FROM __source__raw_orders AS s JOIN __seed__regions AS r USING (region_id)",
            ],
            expected_absent_fragments: &[],
        },
        HelperReferenceTestCase {
            description: "helper sharing a column name in expected rows is not read",
            sql_analysis_enabled: true,
            helpers: &[(
                "amount_doubled",
                "SELECT order_id FROM __ref(\"archived_orders\")",
            )],
            expected: &[(
                "__expected__orders",
                "SELECT 1 AS order_id, 20 AS amount_doubled",
            )],
            assertions: &[],
            read_helpers: &[],
            reference_targets: &[],
            expected_chain: &["stg_orders", "orders"],
            expected_order: &[("__ref__orders", "__expected__orders")],
            expected_fragments: &["SELECT 1 AS order_id, 20 AS amount_doubled"],
            expected_absent_fragments: &["__ref__archived_orders"],
        },
        HelperReferenceTestCase {
            description: "helper sharing a column name in expected rows is not read without sql analysis",
            sql_analysis_enabled: false,
            helpers: &[(
                "amount_doubled",
                "SELECT order_id FROM __ref(\"archived_orders\")",
            )],
            expected: &[(
                "__expected__orders",
                "SELECT 1 AS order_id, 20 AS amount_doubled",
            )],
            assertions: &[],
            read_helpers: &[],
            reference_targets: &[],
            expected_chain: &["stg_orders", "orders"],
            expected_order: &[("__ref__orders", "__expected__orders")],
            expected_fragments: &["SELECT 1 AS order_id, 20 AS amount_doubled"],
            expected_absent_fragments: &["__ref__archived_orders"],
        },
    ];
    for test_case in test_cases {
        let artifact: Value = plan_helper_reference_case(&test_case);
        let chain: Value = chain_helper_reference_case(&test_case);
        let sql = artifact["sql"].as_str().expect("test assumption must hold");
        let errors: Vec<&Value> = artifact["warnings"]
            .as_array()
            .expect("test assumption must hold")
            .iter()
            .filter(|warning| warning["severity"] == json!("error"))
            .collect();

        assert_eq!(
            chain,
            json!(test_case.expected_chain),
            "{}",
            test_case.description
        );
        for (first, second) in test_case.expected_order {
            assert!(
                defined_before(sql, first, second),
                "{}: {first} before {second}: {sql}",
                test_case.description
            );
        }
        for fragment in test_case.expected_absent_fragments {
            assert!(
                !sql.contains(fragment),
                "{}: {fragment}: {sql}",
                test_case.description
            );
        }
        for fragment in test_case.expected_fragments {
            assert!(
                sql.contains(fragment),
                "{}: {fragment}: {sql}",
                test_case.description
            );
        }
        for marker in ["__ref(", "__source(", "__seed(", "__dbt_ref("] {
            assert!(!sql.contains(marker), "{}: {sql}", test_case.description);
        }
        assert!(errors.is_empty(), "{}: {errors:?}", test_case.description);
    }
}

#[test]
fn given_unresolvable_reader_references_when_planning_directly_then_planner_rejects_test() {
    let test_cases = [
        UnresolvedReaderReferenceTestCase {
            description: "assertion calling an unmocked source",
            sql_analysis_enabled: true,
            helpers: &[],
            expected: &[(
                "__expected__stg_orders",
                "SELECT 1 AS order_id, 10 AS amount, 7 AS region_id",
            )],
            assertions: &[(
                "__assert__no_returns",
                "SELECT order_id FROM __source(\"returns\")",
            )],
            read_helpers: &[],
            reference_targets: &[],
            sends_compiler_reads: true,
            expected_error_fragments: &[
                "sql_test_reference:",
                "\"cteName\":\"__assert__no_returns\"",
                "\"call\":\"__source(\\\"returns\\\")\"",
                "\"mockCte\":\"__source__returns\"",
            ],
        },
        UnresolvedReaderReferenceTestCase {
            description: "expected rows calling an unmocked seed without sql analysis",
            sql_analysis_enabled: false,
            helpers: &[],
            expected: &[(
                "__expected__stg_orders",
                "SELECT order_id, amount, region_id FROM __seed(\"returns\")",
            )],
            assertions: &[],
            read_helpers: &[],
            reference_targets: &[],
            sends_compiler_reads: true,
            expected_error_fragments: &[
                "\"cteName\":\"__expected__stg_orders\"",
                "\"mockCte\":\"__seed__returns\"",
            ],
        },
        UnresolvedReaderReferenceTestCase {
            description: "helper read by an assertion calling an unmocked dbt model",
            sql_analysis_enabled: true,
            helpers: &[(
                "legacy_orders",
                "SELECT order_id FROM __dbt_ref(\"warehouse\", \"orders\")",
            )],
            expected: &[(
                "__expected__stg_orders",
                "SELECT 1 AS order_id, 10 AS amount, 7 AS region_id",
            )],
            assertions: &[("__assert__no_legacy", "SELECT order_id FROM legacy_orders")],
            read_helpers: &["legacy_orders"],
            reference_targets: &[],
            sends_compiler_reads: true,
            expected_error_fragments: &[
                "\"cteName\":\"legacy_orders\"",
                "\"mockCte\":\"__dbt_ref__warehouse__orders\"",
            ],
        },
        UnresolvedReaderReferenceTestCase {
            description: "request without the compiler's reads",
            sql_analysis_enabled: true,
            helpers: &[HELPER_READS_MODEL],
            expected: &[],
            assertions: &[ASSERT_DOUBLED],
            read_helpers: &[],
            reference_targets: &[],
            sends_compiler_reads: false,
            expected_error_fragments: &[
                "planner_input:",
                "omits readHelperNames or referenceTargetModelNames",
            ],
        },
    ];
    for test_case in test_cases {
        let error = plan_helper_reference_response(
            &HelperReferenceTestCase {
                description: test_case.description,
                sql_analysis_enabled: test_case.sql_analysis_enabled,
                helpers: test_case.helpers,
                expected: test_case.expected,
                assertions: test_case.assertions,
                read_helpers: test_case.read_helpers,
                reference_targets: test_case.reference_targets,
                expected_chain: &[],
                expected_order: &[],
                expected_fragments: &[],
                expected_absent_fragments: &[],
            },
            test_case.sends_compiler_reads,
        )
        .expect_err(test_case.description);

        assert!(
            test_case
                .expected_error_fragments
                .iter()
                .all(|fragment| error.contains(fragment)),
            "{}: {error}",
            test_case.description
        );
    }
}

#[test]
fn given_compiled_sql_test_inputs_when_assembling_then_facts_match_python() {
    let cte = |name: &str, sql_body: &str| SqlTestCte {
        name: name.to_owned(),
        sql_body: sql_body.to_owned(),
    };
    let strings =
        |names: &[&str]| -> Vec<String> { names.iter().map(|n| (*n).to_owned()).collect() };
    let model_test = |contents: &str| SqlTestAssemblyTest {
        block_name: Some("mock_reads_helper".to_owned()),
        file_stem: "test_customer_totals".to_owned(),
        relative_path: "tests/unit/test_customer_totals.sql".to_owned(),
        relative_stem: "test_customer_totals".to_owned(),
        contents: contents.to_owned(),
        block_sql: contents
            .split_once("\n\n")
            .map_or("", |(_, body)| body)
            .to_owned(),
        block_index: 1,
        sql_body: "SELECT 1".to_owned(),
        case_name: None,
        parameter_schema: Vec::new(),
        parameter_values: Vec::new(),
        payload: SqlTestAssemblyPayload::Model(SqlTestAssemblyModelPayload {
            authored_ctes: vec![
                cte("orders_feed", "SELECT * FROM __source(\"raw_orders\")"),
                cte("__ref__stg_orders", "SELECT * FROM orders_feed"),
            ],
            expected_ctes: vec![cte(
                "__expected__customer_totals",
                "SELECT 1 AS customer_id",
            )],
            assertion_ctes: Vec::new(),
            expected_model_names: strings(&["customer_totals"]),
            assertion_target_model_names: Vec::new(),
            reference_target_model_names: Vec::new(),
            mock_model_names: strings(&["stg_orders"]),
        }),
    };
    let model = |name: &str, macro_deps: &[&str], source: Option<&str>, refs: &[(&str, &str)]| {
        SqlTestAssemblyModel {
            name: name.to_owned(),
            macro_deps: strings(macro_deps),
            unscanned_macro_source: source.map(str::to_owned),
            references: refs
                .iter()
                .map(|(kind, name)| SqlTestAssemblyReference {
                    kind: (*kind).to_owned(),
                    name: (*name).to_owned(),
                    package: None,
                })
                .collect(),
        }
    };
    let models = || {
        vec![
            model("stg_orders", &[], None, &[("source", "raw_orders")]),
            model("customer_totals", &[], None, &[("ref", "stg_orders")]),
            model(
                "order_cents",
                &[],
                Some("SELECT @enum(\"order_channel\").WEB, @@scale * @ cents(amount) FROM t"),
                &[],
            ),
            model(
                "order_dollars",
                &[],
                Some("SELECT '@py.cents(x)', @py.dollars (amount)"),
                &[],
            ),
        ]
    };
    let batch = |tests: Vec<SqlTestAssemblyTest>| SqlTestAssemblyBatch {
        models: models(),
        tests,
        lexical_syntax: generic_syntax(),
        sql_analysis_dialect: Some("duckdb".to_owned()),
    };
    let macro_test = SqlTestAssemblyTest {
        block_name: None,
        payload: SqlTestAssemblyPayload::Direct {
            mode: "macro".to_owned(),
            tested_resource_names: strings(&["dollars"]),
        },
        ..model_test(MOCK_READS_HELPER_TEST)
    };
    let case_test = |decimal_digits: Vec<u8>| SqlTestAssemblyTest {
        relative_path: "tests/unit/test_cases.sql".to_owned(),
        case_name: Some("first".to_owned()),
        parameter_schema: vec![
            ("p_decimal".to_owned(), "decimal".to_owned(), true),
            ("p_float".to_owned(), "float".to_owned(), false),
        ],
        parameter_values: vec![
            (
                "p_decimal".to_owned(),
                SqlTestParameterValue::Decimal {
                    negative: true,
                    digits: decimal_digits,
                    exponent: -4,
                },
            ),
            ("p_float".to_owned(), SqlTestParameterValue::Float(0.1)),
            (
                "p_string".to_owned(),
                SqlTestParameterValue::String("caf\u{e9}".to_owned()),
            ),
        ],
        payload: SqlTestAssemblyPayload::Model(SqlTestAssemblyModelPayload {
            authored_ctes: Vec::new(),
            expected_ctes: Vec::new(),
            assertion_ctes: Vec::new(),
            expected_model_names: strings(&["orders", "customers"]),
            assertion_target_model_names: strings(&["orders"]),
            reference_target_model_names: Vec::new(),
            mock_model_names: Vec::new(),
        }),
        ..model_test(MOCK_READS_HELPER_TEST)
    };
    let rounded_case = |fingerprint: &str| {
        SqlTestAssemblyOutcome::Assembled(AssembledSqlTestFacts {
            name: "mock_reads_helper [first]".to_owned(),
            scope_deps: vec![
                ("model", "orders".to_owned()),
                ("model", "customers".to_owned()),
            ],
            target_model_names: strings(&["orders", "customers"]),
            case_fingerprint: Some(fingerprint.to_owned()),
            diagnostic_resource_name: "mock_reads_helper".to_owned(),
            diagnostics: Vec::new(),
        })
    };
    let test_cases = [
        SqlTestAssemblyTestCase {
            description: "a mock reading a helper that calls a source reports P013 at the call",
            batch: batch(vec![model_test(MOCK_READS_HELPER_TEST)]),
            expected_outcomes: vec![SqlTestAssemblyOutcome::Assembled(AssembledSqlTestFacts {
                name: "mock_reads_helper".to_owned(),
                scope_deps: vec![("model", "customer_totals".to_owned())],
                target_model_names: strings(&["customer_totals"]),
                case_fingerprint: None,
                diagnostic_resource_name: "mock_reads_helper".to_owned(),
                diagnostics: vec![SqlTestHelperDiagnostic {
                    line: 4,
                    column: 31,
                    end_line: 4,
                    end_column: 53,
                    message: "SQL test mock '__ref__stg_orders' reads helper CTE 'orders_feed', \
                              which calls __source(\"raw_orders\"); mocks and fixtures are \
                              defined before the models the test runs, so the helper cannot \
                              be resolved for them"
                        .to_owned(),
                    help: "Read a mock by its CTE name instead, for example FROM \
                           __source__raw_orders rather than FROM __source(\"raw_orders\"), \
                           defining __source__raw_orders AS (SELECT ...) if the test does not \
                           mock it, or write the rows of '__ref__stg_orders' directly."
                        .to_owned(),
                }],
            })],
        },
        SqlTestAssemblyTestCase {
            description: "non-ASCII contents and decimals Python's context rounds assemble as Python",
            batch: batch(vec![
                model_test(&format!("-- caf\u{e9}\n{MOCK_READS_HELPER_TEST}")),
                case_test(vec![1; 29]),
                case_test(vec![9; 29]),
            ]),
            expected_outcomes: vec![
                SqlTestAssemblyOutcome::Assembled(AssembledSqlTestFacts {
                    name: "mock_reads_helper".to_owned(),
                    scope_deps: vec![("model", "customer_totals".to_owned())],
                    target_model_names: strings(&["customer_totals"]),
                    case_fingerprint: None,
                    diagnostic_resource_name: "mock_reads_helper".to_owned(),
                    diagnostics: vec![SqlTestHelperDiagnostic {
                        line: 5,
                        column: 31,
                        end_line: 5,
                        end_column: 53,
                        message: "SQL test mock '__ref__stg_orders' reads helper CTE \
                                  'orders_feed', which calls __source(\"raw_orders\"); mocks \
                                  and fixtures are defined before the models the test runs, so \
                                  the helper cannot be resolved for them"
                            .to_owned(),
                        help: "Read a mock by its CTE name instead, for example FROM \
                               __source__raw_orders rather than FROM __source(\"raw_orders\"), \
                               defining __source__raw_orders AS (SELECT ...) if the test does \
                               not mock it, or write the rows of '__ref__stg_orders' directly."
                            .to_owned(),
                    }],
                }),
                rounded_case(PYTHON_ROUNDED_CASE_FINGERPRINT),
                rounded_case(PYTHON_CARRIED_CASE_FINGERPRINT),
            ],
        },
        SqlTestAssemblyTestCase {
            description: "a case fingerprint and a macro test's scanned scope match Python",
            batch: batch(vec![case_test(vec![2, 4, 7, 0, 0]), macro_test]),
            expected_outcomes: vec![
                SqlTestAssemblyOutcome::Assembled(AssembledSqlTestFacts {
                    name: "mock_reads_helper [first]".to_owned(),
                    scope_deps: vec![
                        ("model", "orders".to_owned()),
                        ("model", "customers".to_owned()),
                    ],
                    target_model_names: strings(&["orders", "customers"]),
                    case_fingerprint: Some(PYTHON_CASE_FINGERPRINT.to_owned()),
                    diagnostic_resource_name: "mock_reads_helper".to_owned(),
                    diagnostics: Vec::new(),
                }),
                SqlTestAssemblyOutcome::Assembled(AssembledSqlTestFacts {
                    name: "test_customer_totals".to_owned(),
                    scope_deps: vec![("model", "order_dollars".to_owned())],
                    target_model_names: Vec::new(),
                    case_fingerprint: None,
                    diagnostic_resource_name: "test_customer_totals".to_owned(),
                    diagnostics: Vec::new(),
                }),
            ],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            assemble_sql_test_batch(&test_case.batch),
            test_case.expected_outcomes,
            "{}",
            test_case.description
        );
    }
}
