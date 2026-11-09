use crate::assembly::analysis_session::main::prove_dynamic_contracts::prove_dynamic_contracts;
use crate::assembly::analysis_session::main::prove_finished_dynamic_contracts::prove_finished_dynamic_contracts;
use crate::assembly::analysis_session::main::run_analysis_session::run_analysis_session;
use crate::assembly::analysis_session::main::start_analysis_session::start_analysis_session;
use crate::assembly::analysis_session::models::PivotOutcome;
use crate::assembly::analysis_session::tests::helpers::{
    catalog, completed, described, expected_facts, fact_lines, failed, finished_session,
    kept_model_analyses, legacy_lines, lines, model, model_requests, orders_request, pivot_request,
    proven, recovered_facts, session_lines, session_steps, started,
};
use crate::assembly::analysis_session::tests::test_types::{
    CteRecoveryTestCase, KeptAnalysisTestCase, LegacyAnalysisTestCase, ModelSpec, PivotTestCase,
    SessionFactsTestCase, SessionFailureTestCase, SessionTestCase, StarExpansionTestCase,
    UnscheduledTestCase,
};

/// A model whose missing column stays untyped over a known input, so completion re-analyses it.
const UNTYPED_ORDERS: ModelSpec = (
    "orders_untyped",
    "SELECT order_id, missing_column FROM __source(\"raw_orders\")",
    &["raw_orders"],
    &[],
);

/// A producer whose output types stay unknown without rich inference.
const STAR_PRODUCER: ModelSpec = ("stg_orders", "SELECT 1 AS order_id, 10 AS amount", &[], &[]);

#[test]
fn given_models_when_running_the_session_then_publishes_as_python_does() {
    let test_cases = [
        SessionTestCase {
            description: "typed producers publish closed shapes their consumers bind",
            models: &[
                (
                    "stg_orders",
                    "SELECT order_id, amount * 2 AS doubled FROM __source(\"raw_orders\")",
                    &["raw_orders"],
                    &[],
                ),
                (
                    "orders_mart",
                    "SELECT doubled, order_id + 1 AS next_id FROM __ref(\"stg_orders\")",
                    &[],
                    &["stg_orders"],
                ),
                (
                    "orders_star",
                    "SELECT * FROM __ref(\"stg_orders\")",
                    &[],
                    &["stg_orders"],
                ),
            ],
            expected_steps: &[&[
                "publish stg_orders order_id:INTEGER doubled:DOUBLE",
                "publish orders_mart doubled:DOUBLE next_id:INTEGER",
                "publish orders_star order_id:INTEGER doubled:DOUBLE",
            ]],
            expected_outcomes: &[
                &[
                    "order_id INTEGER non_null",
                    "doubled DOUBLE unknown",
                    "succeeded=true star=false/true diagnostics=[]",
                ],
                &[
                    "doubled DOUBLE unknown",
                    "next_id INTEGER unknown",
                    "succeeded=true star=false/true diagnostics=[]",
                ],
                &[
                    "order_id INTEGER unknown",
                    "doubled DOUBLE unknown",
                    "succeeded=true star=true/true diagnostics=[]",
                ],
            ],
        },
        SessionTestCase {
            description: "an unknown column is a binding diagnostic and enriches natively",
            models: &[(
                "orders_bad",
                "SELECT missing_column FROM __source(\"raw_orders\")",
                &["raw_orders"],
                &[],
            )],
            expected_steps: &[&[]],
            expected_outcomes: &[&[
                "missing_column - unknown",
                "succeeded=true star=false/true diagnostics=[\"B002\"]",
            ]],
        },
        SessionTestCase {
            description: "an open source's consumers take the legacy analysis natively",
            models: &[
                (
                    "events",
                    "SELECT event_id FROM __source(\"raw_events\")",
                    &["raw_events"],
                    &[],
                ),
                (
                    "events_mart",
                    "SELECT event_id, 1 AS one FROM __ref(\"events\")",
                    &[],
                    &["events"],
                ),
            ],
            expected_steps: &[&[
                "publish events event_id:UNKNOWN",
                "publish events_mart event_id:UNKNOWN one:INTEGER",
            ]],
            expected_outcomes: &[
                &[
                    "event_id - unknown",
                    "succeeded=true star=false/true diagnostics=[]",
                ],
                &[
                    "event_id - unknown",
                    "one INTEGER non_null",
                    "succeeded=true star=false/true diagnostics=[]",
                ],
            ],
        },
        SessionTestCase {
            description: "a non-ASCII filter column is analysed natively",
            models: &[
                (
                    "events",
                    "SELECT event_id AS \"gr\u{f6}\u{df}e\" FROM __source(\"raw_events\")",
                    &["raw_events"],
                    &[],
                ),
                (
                    "events_mart",
                    "SELECT \"gr\u{f6}\u{df}e\" FROM __ref(\"events\") \
                     WHERE \"gr\u{f6}\u{df}e\" IS NOT NULL",
                    &[],
                    &["events"],
                ),
            ],
            expected_steps: &[&[
                "publish events gr\u{f6}\u{df}e:UNKNOWN",
                "publish events_mart gr\u{f6}\u{df}e:UNKNOWN",
            ]],
            expected_outcomes: &[
                &[
                    "gr\u{f6}\u{df}e - unknown",
                    "succeeded=true star=false/true diagnostics=[]",
                ],
                &[
                    "gr\u{f6}\u{df}e - non_null",
                    "succeeded=true star=false/true diagnostics=[]",
                ],
            ],
        },
        SessionTestCase {
            description: "a set operation never re-analyses with its inputs",
            models: &[(
                "orders_union",
                "SELECT order_id FROM __source(\"raw_events\") UNION ALL SELECT 1",
                &["raw_events"],
                &[],
            )],
            expected_steps: &[&["publish orders_union order_id:UNKNOWN"]],
            expected_outcomes: &[&[
                "order_id - unknown",
                "succeeded=true star=false/true diagnostics=[]",
            ]],
        },
    ];
    for test_case in test_cases {
        let (steps, outcomes) = session_lines(test_case.models);

        assert_eq!(steps, test_case.expected_steps, "{}", test_case.description);
        assert_eq!(
            outcomes, test_case.expected_outcomes,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_star_beside_a_cast_constant_when_running_the_session_then_expands_the_star() {
    let test_cases = [
        StarExpansionTestCase {
            description: "fast inference keeps the starred columns before the cast constant",
            rich_type_inference: false,
            models: &[
                STAR_PRODUCER,
                (
                    "orders_flagged",
                    "SELECT *, CAST(1 AS INTEGER) AS flag FROM __ref(\"stg_orders\")",
                    &[],
                    &["stg_orders"],
                ),
            ],
            expected_steps: &[&[
                "publish stg_orders order_id:INTEGER amount:INTEGER",
                "publish orders_flagged order_id:INTEGER amount:INTEGER flag:INTEGER",
            ]],
        },
        StarExpansionTestCase {
            description: "fast inference expands a star over a derived table",
            rich_type_inference: false,
            models: &[
                STAR_PRODUCER,
                (
                    "orders_flagged",
                    "SELECT *, CAST(1 AS INTEGER) AS flag \
                     FROM (SELECT order_id, amount FROM __ref(\"stg_orders\")) AS edited",
                    &[],
                    &["stg_orders"],
                ),
            ],
            expected_steps: &[&[
                "publish stg_orders order_id:INTEGER amount:INTEGER",
                "publish orders_flagged order_id:INTEGER amount:INTEGER flag:INTEGER",
            ]],
        },
        StarExpansionTestCase {
            description: "rich inference keeps the starred columns beside the cast constant",
            rich_type_inference: true,
            models: &[
                STAR_PRODUCER,
                (
                    "orders_flagged",
                    "SELECT *, CAST(1 AS INTEGER) AS flag FROM __ref(\"stg_orders\")",
                    &[],
                    &["stg_orders"],
                ),
            ],
            expected_steps: &[&[
                "publish stg_orders order_id:INTEGER amount:INTEGER",
                "publish orders_flagged order_id:INTEGER amount:INTEGER flag:INTEGER",
            ]],
        },
    ];
    for test_case in test_cases {
        let steps = session_steps(test_case.models, test_case.rich_type_inference);
        assert_eq!(steps, test_case.expected_steps, "{}", test_case.description);
    }
}

#[test]
fn given_finished_sessions_when_reading_model_facts_then_keeps_only_native_successes() {
    let test_cases = [
        SessionFactsTestCase {
            description: "native lineage keeps typed output columns and sources",
            models: &[
                (
                    "stg_orders",
                    "SELECT order_id, amount * 2 AS doubled FROM __source(\"raw_orders\")",
                    &["raw_orders"],
                    &[],
                ),
                (
                    "orders_bad",
                    "SELECT missing_column FROM __source(\"raw_orders\")",
                    &["raw_orders"],
                    &[],
                ),
            ],
            expected_facts: &[
                "Some([\"order_id INTEGER non_null\", \"doubled DOUBLE unknown\"]) \
                 order_id<-[(\"raw_orders\", \"order_id\")] \
                 doubled<-[(\"raw_orders\", \"amount\")]",
                "Some([\"missing_column - unknown\"]) missing_column<-[(\"raw_orders\", \"missing_column\")]",
            ],
        },
        SessionFactsTestCase {
            description: "a native legacy analysis keeps its typed output columns and sources",
            models: &[
                (
                    "events",
                    "SELECT event_id FROM __source(\"raw_events\")",
                    &["raw_events"],
                    &[],
                ),
                (
                    "events_mart",
                    "SELECT event_id, 1 AS one FROM __ref(\"events\")",
                    &[],
                    &["events"],
                ),
            ],
            expected_facts: &[
                "Some([\"event_id - unknown\"]) event_id<-[(\"raw_events\", \"event_id\")]",
                "Some([\"event_id - unknown\", \"one INTEGER non_null\"]) event_id<-[(\"events\", \"event_id\")] one<-[]",
            ],
        },
        SessionFactsTestCase {
            description: "a non-ASCII model analysed natively keeps its session facts",
            models: &[
                (
                    "events",
                    "SELECT event_id AS \"gr\u{f6}\u{df}e\" FROM __source(\"raw_events\")",
                    &["raw_events"],
                    &[],
                ),
                (
                    "events_mart",
                    "SELECT \"gr\u{f6}\u{df}e\" FROM __ref(\"events\") \
                     WHERE \"gr\u{f6}\u{df}e\" IS NOT NULL",
                    &[],
                    &["events"],
                ),
            ],
            expected_facts: &[
                "Some([\"gr\u{f6}\u{df}e - unknown\"]) gr\u{f6}\u{df}e<-[(\"raw_events\", \"event_id\")]",
                "Some([\"gr\u{f6}\u{df}e - non_null\"]) gr\u{f6}\u{df}e<-[(\"events\", \"gr\u{f6}\u{df}e\")]",
            ],
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            fact_lines(test_case.models),
            test_case.expected_facts,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_models_when_starting_then_only_duplicate_names_fail_as_an_internal_error() {
    let test_cases = [
        UnscheduledTestCase {
            description: "models that reference each other analyse in one unordered wave",
            models: &[
                (
                    "orders",
                    "SELECT order_id FROM __ref(\"returns\")",
                    &[],
                    &["returns"],
                ),
                (
                    "returns",
                    "SELECT order_id FROM __ref(\"orders\")",
                    &[],
                    &["orders"],
                ),
            ],
            expected_error: None,
        },
        UnscheduledTestCase {
            description: "two models with one name",
            models: &[
                ("orders", "SELECT 1 AS order_id", &[], &[]),
                ("orders", "SELECT 2 AS order_id", &[], &[]),
            ],
            expected_error: Some(
                "NativeCompilerError: native model analysis: two analysed models share a name",
            ),
        },
    ];
    for test_case in test_cases {
        let request = orders_request(model_requests(test_case.models));
        let catalog = catalog(&request.dialect, &request.catalog_schemas);

        assert_eq!(
            start_analysis_session(request, &catalog).err().as_deref(),
            test_case.expected_error,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_enrichment_dialect_when_running_then_answers_natively_or_fails_internally() {
    let test_cases = [
        SessionFailureTestCase {
            description: "the dialect compile passes for the postgres adapter",
            dialect: "postgresql",
            models: &[UNTYPED_ORDERS],
            expected_error: None,
        },
        SessionFailureTestCase {
            description: "the postgres analysis dialect answers natively",
            dialect: "postgres",
            models: &[UNTYPED_ORDERS],
            expected_error: None,
        },
    ];
    for test_case in test_cases {
        let mut request = orders_request(model_requests(test_case.models));
        request.dialect = test_case.dialect.to_owned();
        let catalog = catalog(&request.dialect, &request.catalog_schemas);
        let mut session = start_analysis_session(request, &catalog).expect("the session starts");

        let error: Option<String> = run_analysis_session(&mut session).err();

        assert_eq!(
            error
                .as_deref()
                .map(|error| error.split(',').next().unwrap_or(error)),
            test_case.expected_error,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_dynamic_pivots_when_proving_then_matches_python_or_defers() {
    let test_cases = [
        PivotTestCase {
            description: "a bare DuckDB pivot keeps its GROUP BY columns",
            dialect: "duckdb",
            sql: "PIVOT __source(\"raw_orders\") ON status USING MAX(amount) GROUP BY customer_id",
            family: Some(("status", "amount", "MAX")),
            expected_outcome: proven(
                &[("customer_id", "INTEGER", "non_null")],
                Some("DOUBLE"),
                "raw_orders",
                true,
            ),
        },
        PivotTestCase {
            description: "a cast aggregate renders its target type",
            dialect: "duckdb",
            sql: "PIVOT __source(\"raw_orders\") ON status \
                USING MIN(CAST(amount AS DECIMAL(12, 2))) GROUP BY customer_id",
            family: Some(("status", "amount", "MIN")),
            expected_outcome: proven(
                &[("customer_id", "INTEGER", "non_null")],
                Some("DECIMAL(12, 2)"),
                "raw_orders",
                true,
            ),
        },
        PivotTestCase {
            description: "a CTE input without GROUP BY keeps the remaining input columns",
            dialect: "snowflake",
            sql: "WITH base AS (SELECT customer_id, status, amount FROM __source(\"raw_orders\")) \
                SELECT * FROM base PIVOT (SUM(amount) FOR status IN (ANY ORDER BY status))",
            family: Some(("status", "amount", "SUM")),
            expected_outcome: proven(
                &[("customer_id", "INTEGER", "non_null")],
                None,
                "raw_orders",
                false,
            ),
        },
        PivotTestCase {
            description: "a passthrough redeclaring the upstream families",
            dialect: "duckdb",
            sql: "SELECT * FROM __ref(\"status_amounts\")",
            family: Some(("status", "amount", "MAX")),
            expected_outcome: proven(
                &[("customer_id", "INTEGER", "unknown")],
                Some("DOUBLE"),
                "status_amounts",
                false,
            ),
        },
        PivotTestCase {
            description: "static pivot values",
            dialect: "snowflake",
            sql: "SELECT * FROM __source(\"raw_orders\") PIVOT (MAX(amount) FOR status IN ('placed'))",
            family: Some(("status", "amount", "MAX")),
            expected_outcome: failed(
                "static pivot values must use ordinary exact column declarations",
            ),
        },
        PivotTestCase {
            description: "DuckDB values on the ON clause",
            dialect: "duckdb",
            sql: "PIVOT __source(\"raw_orders\") ON status IN ('placed') USING MAX(amount)",
            family: Some(("status", "amount", "MAX")),
            expected_outcome: failed(
                "dynamic column contracts currently require exactly one pivot column",
            ),
        },
        PivotTestCase {
            description: "a dialect without dynamic pivots",
            dialect: "postgres",
            sql: "SELECT 1",
            family: Some(("status", "amount", "MAX")),
            expected_outcome: failed(
                "adapter dialect 'postgres' does not support compiler-proven dynamic pivots",
            ),
        },
        PivotTestCase {
            description: "a declared pivot column the pivot does not use",
            dialect: "duckdb",
            sql: "PIVOT __source(\"raw_orders\") ON status USING MAX(amount) GROUP BY customer_id",
            family: Some(("order_id", "amount", "MAX")),
            expected_outcome: failed(
                "dynamic family 'amounts' declares pivot_column 'order_id' but the output pivot uses 'status'",
            ),
        },
        PivotTestCase {
            description: "SQL Python reports as a parse error",
            dialect: "duckdb",
            sql: "PIVOT FROM WHERE",
            family: Some(("status", "amount", "MAX")),
            expected_outcome: failed(
                "dynamic pivot SQL could not be parsed: Parse error at line 1, column 11: \
                 Unexpected token: From",
            ),
        },
        PivotTestCase {
            description: "non-ASCII names fold as Python casefolds them",
            dialect: "duckdb",
            sql: "SELECT * FROM __ref(\"stra\u{df}e\")",
            family: Some(("status", "amount", "MAX")),
            expected_outcome: failed(
                "dynamic-family passthrough from 'stra\u{df}e' must redeclare the upstream \
                 families exactly",
            ),
        },
        PivotTestCase {
            description: "no declared families",
            dialect: "duckdb",
            sql: "SELECT 1",
            family: None,
            expected_outcome: PivotOutcome::Absent,
        },
    ];
    for test_case in test_cases {
        let request = pivot_request(test_case.dialect, test_case.sql, test_case.family);
        let session = finished_session(request.tables.clone());

        let standalone = prove_dynamic_contracts(&request).expect("the proofs run");
        let in_session =
            prove_finished_dynamic_contracts(&session, &request.models).expect("the proofs run");

        assert_eq!(
            standalone,
            vec![test_case.expected_outcome.clone(), PivotOutcome::Absent],
            "{}",
            test_case.description
        );
        assert_eq!(
            in_session,
            vec![test_case.expected_outcome, PivotOutcome::Absent],
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_cte_reads_when_recovering_facts_then_matches_python_or_defers() {
    let test_cases = [
        CteRecoveryTestCase {
            description: "direct CTE reads keep input and cast types",
            sql: "WITH base AS (SELECT order_id, CAST(amount AS DECIMAL(10, 2)) AS amount \
                  FROM orders) SELECT order_id, amount FROM base",
            expected_facts: Some((
                &[("order_id", "INTEGER"), ("amount", "DECIMAL(10, 2)")],
                &[],
                &["amount", "order_id"],
                &[],
            )),
        },
        CteRecoveryTestCase {
            description: "literals and filtered columns are non-null through an aliased CTE",
            sql: "WITH base AS (SELECT 'x' AS tag, UPPER(status) AS up, order_id FROM orders \
                  WHERE order_id IS NOT NULL) SELECT b.tag, b.up, b.order_id FROM base AS b",
            expected_facts: Some((
                &[("order_id", "INTEGER")],
                &[("tag", "non_null"), ("order_id", "non_null")],
                &["order_id", "tag", "up"],
                &[],
            )),
        },
        CteRecoveryTestCase {
            description: "a set operation branch with an unnamed NULL proves nothing",
            sql: "WITH u AS (SELECT order_id AS k, status AS s FROM orders UNION ALL \
                  SELECT NULL, 'y') SELECT k, s FROM u",
            expected_facts: Some((&[], &[], &["k", "s"], &[])),
        },
        CteRecoveryTestCase {
            description: "a star with EXCLUDE and a joined relation",
            sql: "WITH a AS (SELECT * EXCLUDE (amount) FROM orders) SELECT o.status, a.order_id \
                  FROM a LEFT JOIN orders AS o ON a.order_id = o.order_id",
            expected_facts: Some((
                &[("status", "VARCHAR"), ("order_id", "INTEGER")],
                &[],
                &["order_id", "status"],
                &[],
            )),
        },
        CteRecoveryTestCase {
            description: "no CTE leaves only the filtered non-null outputs",
            sql: "SELECT order_id, status FROM orders WHERE order_id IS NOT NULL",
            expected_facts: Some((&[], &[], &[], &["order_id"])),
        },
        CteRecoveryTestCase {
            description: "non-ASCII names fold as Python casefolds them",
            sql: "WITH b\u{e4}se AS (SELECT order_id FROM orders) SELECT order_id FROM b\u{e4}se",
            expected_facts: Some((&[("order_id", "INTEGER")], &[], &["order_id"], &[])),
        },
    ];
    for test_case in test_cases {
        let facts = recovered_facts(test_case.sql);

        assert_eq!(
            facts,
            expected_facts(test_case.expected_facts),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_handed_back_models_when_analysing_legacy_then_matches_python_or_defers() {
    let test_cases = [
        LegacyAnalysisTestCase {
            description: "a qualified join keeps casts, aggregates and joined nullability",
            sql: "SELECT o.order_id, CAST(o.amount AS DECIMAL(10, 2)) AS amount, COUNT(*) AS n, \
                  UPPER(o.status) AS up, c.customer_id FROM orders AS o LEFT JOIN customers AS c \
                  ON o.order_id = c.customer_id GROUP BY 1, 2, 4, 5",
            expected_lines: Some(&[
                "succeeded=true star=false",
                "order_id - non_null",
                "amount DECIMAL(10, 2) unknown",
                "n - non_null",
                "up - unknown",
                "customer_id - nullable",
                "order_id direct high [model:orders:order_id]",
                "amount cast high [model:orders:amount]",
                "n aggregation unknown []",
                "up expression high [model:orders:status]",
                "customer_id direct high [source:customers:customer_id]",
            ]),
        },
        LegacyAnalysisTestCase {
            description: "unqualified columns of one resource are medium confidence",
            sql: "SELECT order_id, 'x' AS tag, status IS NULL AS missing, amount + 1 AS bumped \
                  FROM orders WHERE status IS NOT NULL",
            expected_lines: Some(&[
                "succeeded=true star=false",
                "order_id - non_null",
                "tag - non_null",
                "missing BOOLEAN non_null",
                "bumped - unknown",
                "order_id direct medium [model:orders:order_id]",
                "tag constant high []",
                "missing expression medium [model:orders:status]",
                "bumped expression medium [model:orders:amount]",
            ]),
        },
        LegacyAnalysisTestCase {
            description: "CTE pass-through outputs take the recovered facts beside a star",
            sql: "WITH base AS (SELECT order_id, CAST(amount AS DOUBLE) AS amount FROM orders) \
                  SELECT order_id, amount, * FROM base",
            expected_lines: Some(&[
                "succeeded=true star=true",
                "order_id INTEGER non_null",
                "amount DOUBLE unknown",
                "order_id direct medium [model:orders:order_id]",
                "amount direct medium [model:orders:amount]",
            ]),
        },
        LegacyAnalysisTestCase {
            description: "a set operation reads its first select without nullability",
            sql: "SELECT order_id FROM orders UNION ALL SELECT customer_id FROM customers",
            expected_lines: Some(&[
                "succeeded=true star=false",
                "order_id - unknown",
                "order_id direct medium [model:orders:order_id]",
            ]),
        },
        LegacyAnalysisTestCase {
            description: "a parse error fails the analysis as Python's PolyglotError does",
            sql: "SELECT FROM WHERE (",
            expected_lines: Some(&["succeeded=false star=false"]),
        },
        LegacyAnalysisTestCase {
            description: "a non-ASCII literal names no function Python declares a type for",
            sql: "SELECT 'na\u{ef}ve r\u{e9}sum\u{e9}' AS label, '\u{6771}' AS city FROM orders",
            expected_lines: Some(&[
                "succeeded=true star=false",
                "label - non_null",
                "city - non_null",
                "label constant high []",
                "city constant high []",
            ]),
        },
        LegacyAnalysisTestCase {
            description: "non-ASCII names fold as Python casefolds them",
            sql: "SELECT \"gr\u{f6}\u{df}e\" FROM orders WHERE \"gr\u{f6}\u{df}e\" IS NOT NULL",
            expected_lines: Some(&[
                "succeeded=true star=false",
                "gr\u{f6}\u{df}e - unknown",
                "gr\u{f6}\u{df}e direct medium [model:orders:gr\u{f6}\u{df}e]",
            ]),
        },
    ];
    for test_case in test_cases {
        let lines = legacy_lines(test_case.sql);

        assert_eq!(
            lines,
            test_case
                .expected_lines
                .map(|lines| lines.iter().map(|line| (*line).to_owned()).collect()),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_finished_session_when_reading_model_analyses_then_keeps_typed_columns_and_proofs() {
    let test_cases = [KeptAnalysisTestCase {
        description: "a proven pivot publishes its fixed columns; a plain model its analysis",
        dialect: "duckdb",
        sql: "PIVOT __source(\"raw_orders\") ON status USING MAX(amount) GROUP BY customer_id",
        family: Some(("status", "amount", "MAX")),
        expected_outcome: proven(
            &[("customer_id", "INTEGER", "non_null")],
            Some("DOUBLE"),
            "raw_orders",
            true,
        ),
        expected_columns: [&["customer_id INTEGER non_null"], &["1 INTEGER non_null"]],
    }];
    for test_case in test_cases {
        let kept = kept_model_analyses(test_case.dialect, test_case.sql, test_case.family);

        assert_eq!(
            kept,
            vec![
                (
                    test_case.expected_outcome,
                    Some(lines(test_case.expected_columns[0]))
                ),
                (
                    PivotOutcome::Absent,
                    Some(lines(test_case.expected_columns[1]))
                ),
            ],
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_project_function_called_by_its_bare_name_when_running_the_session_then_it_is_known() {
    let test_cases: [(&str, &[&str], &str, &str); 3] = [
        (
            "a declared project function is not an unknown function",
            &["is_large_order", "__sqlbuild_udf_is_large_order"],
            "SELECT order_id, is_large_order(amount) AS is_large FROM __source(\"raw_orders\")",
            "succeeded=true star=false/true diagnostics=[]",
        ),
        (
            "the same call without the declaration is an unknown function",
            &[],
            "SELECT order_id, is_large_order(amount) AS is_large FROM __source(\"raw_orders\")",
            "succeeded=true star=false/true diagnostics=[\"B101\"]",
        ),
        (
            "a misspelt built-in stays an unknown function beside a declared one",
            &["is_large_order"],
            "SELECT order_id, lenght('x') AS n FROM __source(\"raw_orders\")",
            "succeeded=true star=false/true diagnostics=[\"B101\"]",
        ),
    ];
    for (description, known, sql, expected) in test_cases {
        let mut request = orders_request(vec![model("orders", sql, &["raw_orders"], &[])]);
        request.known_functions = known.iter().map(|name| (*name).to_owned()).collect();
        let (_, outcomes, finished) = completed(started(request));
        assert_eq!(
            described(&outcomes[0]).last().map(String::as_str),
            Some(expected),
            "{description}"
        );
        assert!(
            known
                .iter()
                .all(|name| finished.declared_functions.iter().any(|kept| kept == name)),
            "{description}: revalidation knows the declared functions"
        );
    }
}
