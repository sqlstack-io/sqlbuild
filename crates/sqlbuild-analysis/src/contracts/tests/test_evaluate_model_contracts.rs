use crate::contracts::main::evaluate_model_contracts::evaluate_model_contracts;
use crate::contracts::models::{ContractDiagnostic, ContractModel, ContractRequest};
use crate::contracts::tests::helpers::{
    declared, dynamic_model, inferred, model, outcome_lines, proof, schema, with_inferred,
};
use crate::contracts::tests::test_types::{
    ContractTestCase, SharedTypesTestCase, UnknownDialectTestCase,
};

#[test]
fn given_model_contracts_when_evaluating_then_diagnostics_follow_python_order() {
    let test_cases = [
        ContractTestCase {
            description: "enforced contract without declared columns",
            implicit: true,
            model: model(Some("enforced"), None),
            expected_lines: &[
                "K006 error - model 'orders' has contract enforced but declares no columns",
            ],
        },
        ContractTestCase {
            description: "no contract, no schema",
            implicit: true,
            model: model(None, None),
            expected_lines: &[],
        },
        ContractTestCase {
            description: "explicit mode skips declared columns without type enforcement",
            implicit: false,
            model: with_inferred(
                model(
                    None,
                    Some(schema(vec![declared("missing", None, false)], false)),
                ),
                vec![inferred("order_id", None, false)],
            ),
            expected_lines: &[],
        },
        ContractTestCase {
            description: "enforced: extras first, then missing, nullability and type per column",
            implicit: true,
            model: with_inferred(
                model(
                    Some("enforced"),
                    Some(schema(
                        vec![
                            declared("order_id", Some("INTEGER"), true),
                            declared("missing", Some("INTEGER"), false),
                            declared("amount", Some("VARCHAR"), false),
                        ],
                        false,
                    )),
                ),
                vec![
                    inferred("order_id", Some("INTEGER"), true),
                    inferred("extra", None, false),
                    inferred("amount", Some("DOUBLE"), false),
                ],
            ),
            expected_lines: &[
                "K005 error output:extra column 'extra' is not declared in enforced contract for \
                 model 'orders'",
                "K004 error declared:0 column 'order_id' is declared non-null but may be nullable \
                 [order_id: output expression proven nullable]",
                "K001 error declared:1 enforced contract column 'missing' was not found in \
                 statically inferred output for model 'orders'",
                "K002 error declared:2 column 'amount' inferred as DOUBLE but declared type is \
                 VARCHAR [amount: inferred DOUBLE]",
            ],
        },
        ContractTestCase {
            description: "implicit mode: a mismatch is a warning and a star hides missing columns",
            implicit: true,
            model: {
                let mut starred = with_inferred(
                    model(
                        None,
                        Some(schema(
                            vec![
                                declared("missing", None, false),
                                declared("amount", Some("VARCHAR"), false),
                            ],
                            false,
                        )),
                    ),
                    vec![inferred("amount", Some("DOUBLE"), false)],
                );
                starred.fast_lineage_has_star = true;
                starred
            },
            expected_lines: &[
                "K002 warning declared:1 column 'amount' inferred as DOUBLE but declared type is \
                 VARCHAR [amount: inferred DOUBLE]",
            ],
        },
        ContractTestCase {
            description: "type enforcement warns about unproven types except unchecked outputs",
            implicit: false,
            model: {
                let mut enforced_types = with_inferred(
                    model(
                        None,
                        Some(schema(
                            vec![
                                declared("amount", Some("DOUBLE"), false),
                                declared("label", Some("VARCHAR"), false),
                            ],
                            true,
                        )),
                    ),
                    vec![
                        inferred("amount", None, false),
                        inferred("label", None, false),
                    ],
                );
                enforced_types.unchecked_output_columns = vec!["label".to_owned()];
                enforced_types
            },
            expected_lines: &[
                "K003 warning declared:0 column 'amount' type could not be proven against \
                 declared DOUBLE [amount: output expression with unproven type]",
            ],
        },
        ContractTestCase {
            description: "equal types under the dialect raise nothing",
            implicit: true,
            model: with_inferred(
                model(
                    Some("enforced"),
                    Some(schema(vec![declared("amount", Some("INT"), false)], false)),
                ),
                vec![inferred("amount", Some("INTEGER"), false)],
            ),
            expected_lines: &[],
        },
        ContractTestCase {
            description: "a non-ASCII type upper-cases with Python's Unicode mapping",
            implicit: true,
            model: with_inferred(
                model(
                    Some("enforced"),
                    Some(schema(vec![declared("amount", Some("TÉXT"), false)], false)),
                ),
                vec![inferred("amount", Some("VARCHAR"), false)],
            ),
            expected_lines: &[
                "K002 error declared:0 column 'amount' inferred as VARCHAR but declared type is \
                 T\u{c9}XT [amount: inferred VARCHAR]",
            ],
        },
        ContractTestCase {
            description: "an unproven dynamic contract without a reason",
            implicit: true,
            model: dynamic_model(&[("amount_*", "DOUBLE")], None),
            expected_lines: &[
                "K011 error - model 'orders' dynamic column contract is not proven: no compiler \
                 evidence",
            ],
        },
        ContractTestCase {
            description: "an unproven dynamic contract with its reason",
            implicit: true,
            model: dynamic_model(
                &[("amount_*", "DOUBLE")],
                Some(proof(false, Some("no pivot"), &[])),
            ),
            expected_lines: &[
                "K011 error - model 'orders' dynamic column contract is not proven: no pivot",
            ],
        },
        ContractTestCase {
            description: "proven families match case-insensitively and compare their types",
            implicit: true,
            model: dynamic_model(
                &[
                    ("Amount_*", "DOUBLE"),
                    ("count_*", "BIGINT"),
                    ("label_*", "VARCHAR"),
                ],
                Some(proof(
                    true,
                    None,
                    &[("amount_*", Some("DOUBLE")), ("COUNT_*", Some("VARCHAR"))],
                )),
            ),
            expected_lines: &[
                "K002 error - dynamic column family 'count_*' inferred as VARCHAR but declared \
                 type is BIGINT",
                "K003 warning - dynamic column family 'label_*' type could not be proven against \
                 declared VARCHAR",
            ],
        },
        ContractTestCase {
            description: "a family name outside ASCII matches case-insensitively",
            implicit: true,
            model: dynamic_model(
                &[("Straße_*", "DOUBLE")],
                Some(proof(true, None, &[("straße_*", Some("DOUBLE"))])),
            ),
            expected_lines: &[],
        },
    ];
    for test_case in test_cases {
        let request = ContractRequest {
            dialect: "duckdb".to_owned(),
            implicit_column_contracts: test_case.implicit,
            models: vec![test_case.model],
        };
        let outcomes: Vec<Vec<ContractDiagnostic>> =
            evaluate_model_contracts(&request).expect("duckdb types normalize");
        assert_eq!(
            outcomes
                .iter()
                .flat_map(|diagnostics| outcome_lines(diagnostics))
                .collect::<Vec<_>>(),
            test_case.expected_lines,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_models_sharing_types_when_evaluating_together_then_each_matches_its_own_request() {
    let enforced = |declared_type: &str, inferred_type: &str| {
        with_inferred(
            model(
                Some("enforced"),
                Some(schema(
                    vec![declared("amount", Some(declared_type), false)],
                    false,
                )),
            ),
            vec![inferred("amount", Some(inferred_type), false)],
        )
    };
    let test_cases = [SharedTypesTestCase {
        description: "repeated equal, mismatched and non-ASCII types",
        models: vec![
            enforced("INT", "INTEGER"),
            enforced("INT", "VARCHAR"),
            enforced("TÉXT", "VARCHAR"),
            enforced("INT", "INTEGER"),
            enforced("INT", "VARCHAR"),
            enforced("TÉXT", "VARCHAR"),
        ],
        expected_lines: &[
            "K002 error declared:0 column 'amount' inferred as VARCHAR but declared type is INT [amount: inferred VARCHAR]",
            "K002 error declared:0 column 'amount' inferred as VARCHAR but declared type is T\u{c9}XT [amount: inferred VARCHAR]",
            "K002 error declared:0 column 'amount' inferred as VARCHAR but declared type is INT [amount: inferred VARCHAR]",
            "K002 error declared:0 column 'amount' inferred as VARCHAR but declared type is T\u{c9}XT [amount: inferred VARCHAR]",
        ],
    }];
    for test_case in test_cases {
        let request = |models: Vec<ContractModel>| ContractRequest {
            dialect: "duckdb".to_owned(),
            implicit_column_contracts: true,
            models,
        };
        let together: Vec<String> = evaluate_model_contracts(&request(test_case.models.clone()))
            .expect("duckdb types normalize")
            .iter()
            .flat_map(|diagnostics| outcome_lines(diagnostics))
            .collect();
        let alone: Vec<String> = test_case
            .models
            .iter()
            .flat_map(|model| {
                evaluate_model_contracts(&request(vec![model.clone()]))
                    .expect("duckdb types normalize")
            })
            .flat_map(|diagnostics| outcome_lines(&diagnostics))
            .collect();

        assert_eq!(together, alone, "{}", test_case.description);
        assert_eq!(
            together, test_case.expected_lines,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_dialect_no_adapter_ships_when_comparing_types_then_python_error_is_returned() {
    let test_cases = [UnknownDialectTestCase {
        description: "an enforced contract compared under an unknown dialect",
        dialect: "mysql",
        expected_error: "Unknown dialect: mysql",
    }];
    for test_case in test_cases {
        let request = ContractRequest {
            dialect: test_case.dialect.to_owned(),
            implicit_column_contracts: true,
            models: vec![with_inferred(
                model(
                    Some("enforced"),
                    Some(schema(vec![declared("amount", Some("INT"), false)], false)),
                ),
                vec![inferred("amount", Some("INTEGER"), false)],
            )],
        };

        assert_eq!(
            evaluate_model_contracts(&request).map_err(|error| error.message()),
            Err(test_case.expected_error.to_owned()),
            "{}",
            test_case.description
        );
    }
}
