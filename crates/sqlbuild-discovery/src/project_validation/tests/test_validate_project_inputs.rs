use crate::models::FailureKind;
use crate::project_validation::models::{
    NodeKind, ProjectFacts, SeedFileFact, SourceFact, SourceFileFact,
};
use crate::project_validation::tests::helpers::{
    model, model_at, named, node, on, path_default_facts, seed_header_outcome, validation_outcome,
};
use crate::project_validation::tests::test_types::{
    SeedHeaderTestCase, ValidationTestCase, failure,
};

#[test]
fn given_discovered_facts_when_validating_then_reports_python_first_conflict() {
    let test_cases = [
        ValidationTestCase {
            description: "a valid project passes",
            facts: ProjectFacts {
                models: vec![model("orders"), model("customers")],
                ..ProjectFacts::default()
            },
            expected_failure: Ok(()),
        },
        ValidationTestCase {
            description: "a non-canonical model identity is rejected first",
            facts: ProjectFacts {
                models: vec![model("Orders"), model("_chain_")],
                ..ProjectFacts::default()
            },
            expected_failure: failure(
                FailureKind::ResourceIdentity,
                "Invalid model identity 'Orders' in sql/models/Orders.sql; use snake_case 'orders'",
            ),
        },
        ValidationTestCase {
            description: "a reserved model name fails its identity check first",
            facts: ProjectFacts {
                models: vec![model("_chain_")],
                ..ProjectFacts::default()
            },
            expected_failure: failure(
                FailureKind::ResourceIdentity,
                "Invalid model identity '_chain_' in sql/models/_chain_.sql; use snake_case 'chain'",
            ),
        },
        ValidationTestCase {
            description: "a source and a model share a logical relation name",
            facts: ProjectFacts {
                models: vec![model("orders")],
                sources: vec![SourceFileFact {
                    path: "sql/sources/shop.yml".to_owned(),
                    entries: vec![SourceFact {
                        name: "orders".to_owned(),
                        ..SourceFact::default()
                    }],
                }],
                ..ProjectFacts::default()
            },
            expected_failure: failure(
                FailureKind::Conflict,
                "Logical relation name 'orders' is declared as both model in sql/models/orders.sql \
                 and source in sql/sources/shop.yml",
            ),
        },
        ValidationTestCase {
            description: "an asset cycle names its path",
            facts: ProjectFacts {
                nodes: vec![
                    node(NodeKind::Asset, "extract", vec![on(1, "load")]),
                    node(NodeKind::Asset, "load", vec![on(0, "extract")]),
                ],
                ..ProjectFacts::default()
            },
            expected_failure: failure(
                FailureKind::Conflict,
                "Python node dependency cycle detected: extract -> load -> extract",
            ),
        },
        ValidationTestCase {
            description: "a check depending on a check",
            facts: ProjectFacts {
                nodes: vec![
                    node(NodeKind::Check, "first", Vec::new()),
                    node(NodeKind::Check, "second", vec![on(0, "first")]),
                ],
                ..ProjectFacts::default()
            },
            expected_failure: failure(
                FailureKind::Conflict,
                "Check 'second' depends on another check; checks may depend on loaders, \
                 assets, models, sources, and seeds only",
            ),
        },
        ValidationTestCase {
            description: "an undeclared seed CSV",
            facts: ProjectFacts {
                seeds: vec![SeedFileFact {
                    file: named("countries", "sql/seeds/countries.csv"),
                    absolute_path: "/missing/seeds/countries.csv".to_owned(),
                }],
                ..ProjectFacts::default()
            },
            expected_failure: failure(
                FailureKind::Seed,
                "Seed CSV sql/seeds/countries.csv has no matching declaration for seed 'countries' \
                 under sql/seeds/**/*.yml",
            ),
        },
        ValidationTestCase {
            description: "an unmatched path default lists known folders",
            facts: ProjectFacts {
                models: vec![model_at("orders", "sql/models/marts/orders.sql")],
                path_default_keys: vec!["staging".to_owned()],
                ..ProjectFacts::default()
            },
            expected_failure: failure(
                FailureKind::Conflict,
                "path_defaults['staging'] does not match any model paths. Known model folders \
                 include: marts",
            ),
        },
        ValidationTestCase {
            description: "a path default matching only undiscovered project models passes",
            facts: ProjectFacts {
                models: vec![model_at("orders", "sql/models/marts/orders.sql")],
                path_default_keys: vec!["staging".to_owned()],
                project_model_paths: Some(vec![
                    "sql/models/marts/orders.sql".to_owned(),
                    "sql/models/staging/customers.sql".to_owned(),
                ]),
                ..ProjectFacts::default()
            },
            expected_failure: Ok(()),
        },
        ValidationTestCase {
            description: "accepts Windows-style model paths that match path defaults",
            facts: path_default_facts(&["models\\staging\\orders.sql"], &["staging"]),
            expected_failure: Ok(()),
        },
        ValidationTestCase {
            description: "accepts recursive glob matching nested staging prefix",
            facts: path_default_facts(
                &["sql/models/commerce/partner_feed/staging/orders.sql"],
                &["commerce/**/staging"],
            ),
            expected_failure: Ok(()),
        },
        ValidationTestCase {
            description: "accepts recursive glob matching zero segments",
            facts: path_default_facts(
                &["sql/models/commerce/staging/orders.sql"],
                &["commerce/**/staging"],
            ),
            expected_failure: Ok(()),
        },
        ValidationTestCase {
            description: "counts broad glob as matched when a literal wins selection",
            facts: path_default_facts(
                &["sql/models/commerce/partner_feed/staging/orders.sql"],
                &["commerce/**/staging", "commerce/partner_feed/staging"],
            ),
            expected_failure: Ok(()),
        },
        ValidationTestCase {
            description: "rejects recursive glob matching no model paths",
            facts: path_default_facts(
                &["sql/models/commerce/partner_feed/marts/orders.sql"],
                &["commerce/**/staging"],
            ),
            expected_failure: failure(
                FailureKind::Conflict,
                "path_defaults['commerce/**/staging'] does not match any model paths. Known \
                 model folders include: commerce/partner_feed/marts",
            ),
        },
        ValidationTestCase {
            description: "rejects equally specific matching globs during discovery",
            facts: path_default_facts(
                &["sql/models/commerce/eu/staging/orders.sql"],
                &["commerce/*/staging", "commerce/eu/*"],
            ),
            expected_failure: failure(
                FailureKind::Conflict,
                "Model path 'commerce/eu/staging/orders.sql' matches equally specific \
                 path_defaults keys: 'commerce/*/staging', 'commerce/eu/*'.",
            ),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            validation_outcome(&test_case.facts),
            test_case.expected_failure,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_seed_csv_headers_when_validating_then_reports_python_tuple_reprs() {
    let test_cases = [
        SeedHeaderTestCase {
            description: "a matching header passes",
            csv_text: "code,label\nGB,United Kingdom\n",
            declared_columns: &["code", "label"],
            expected_failure: Ok(()),
        },
        SeedHeaderTestCase {
            description: "an extra column shows both tuples",
            csv_text: "code,label\nGB,United Kingdom\n",
            declared_columns: &["code"],
            expected_failure: failure(
                FailureKind::Seed,
                "sql/seeds/countries.csv header ('code', 'label') does not match declared seed \
                 columns ('code',) for 'countries'",
            ),
        },
        SeedHeaderTestCase {
            description: "a duplicate header column",
            csv_text: "code,code\n",
            declared_columns: &["code", "code"],
            expected_failure: failure(
                FailureKind::Seed,
                "sql/seeds/countries.csv contains duplicate CSV header column 'code'",
            ),
        },
        SeedHeaderTestCase {
            description: "an empty file has no header row",
            csv_text: "",
            declared_columns: &["code"],
            expected_failure: failure(
                FailureKind::Seed,
                "sql/seeds/countries.csv must contain a CSV header row",
            ),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            seed_header_outcome(test_case.csv_text, test_case.declared_columns),
            test_case.expected_failure,
            "{}",
            test_case.description
        );
    }
}
