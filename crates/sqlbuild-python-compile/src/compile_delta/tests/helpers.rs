use std::collections::{BTreeMap, BTreeSet};

use sqlbuild_analysis::assembly::analysis_session::models::DynamicFamily;
use sqlbuild_cache::project_reuse::types::ImageSections;

use sqlbuild_rules::models::{EvaluateRequest, RetainedRows};

use crate::compile_delta::constants::FACTS_SECTION;
use crate::compile_delta::constants::GATE_SECTION;
use crate::compile_delta::constants::IMAGE_FORMAT;
use crate::compile_delta::constants::RULE_DIGESTS_SECTION;
use crate::compile_delta::models::ImageRelations;
use crate::compile_delta::models::ImageRules;
use crate::compile_delta::models::ProjectImage;
use sqlbuild_python_base::compile_context::models::BaseRules;
use sqlbuild_python_base::compile_context::models::CapturedDiagnostic;
use sqlbuild_python_base::compile_context::models::CapturedModel;
use sqlbuild_python_base::compile_context::models::CapturedTest;
use sqlbuild_python_base::compile_context::models::ModelCoverage;
use sqlbuild_python_base::compile_context::models::PendingRules;

/// The rows of a request without models: a zero model count.
const RULE_ROWS: [u8; 8] = [0; 8];
const OTHER_FORMAT: &str = "sqlbuild-project-image-v0";

pub(crate) fn pairs(values: &[(&str, &str)]) -> Vec<(String, String)> {
    values
        .iter()
        .map(|(left, right)| ((*left).to_owned(), (*right).to_owned()))
        .collect()
}

pub(crate) fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// An image holding at least one value of every kind it stores.
pub(crate) fn image() -> ProjectImage {
    let order_types: Vec<(String, String)> = pairs(&[("order_id", "BIGINT"), ("amount", "DOUBLE")]);
    let customer_types: Vec<(String, String)> = pairs(&[("customer_id", "BIGINT")]);
    let request: Vec<u8> = serde_json::to_vec(&EvaluateRequest::default()).unwrap_or_default();
    ProjectImage {
        format: String::new(),
        json_output: true,
        models: vec![
            CapturedModel {
                name: "orders".to_owned(),
                relative_path: "sql/models/orders.sql".to_owned(),
                upstream: pairs(&[("source", "raw_orders"), ("model", "customers")]),
            },
            CapturedModel {
                name: "customers".to_owned(),
                relative_path: "sql/models/customers.sql".to_owned(),
                upstream: Vec::new(),
            },
        ],
        usages: BTreeMap::from([("sql/models/orders.sql".to_owned(), "digest-1".to_owned())]),
        tests: vec![CapturedTest {
            key: "orders_test".to_owned(),
            relative_path: "tests/unit/orders.sql".to_owned(),
            chain: strings(&["orders", "customers"]),
        }],
        relations: ImageRelations {
            types: vec![
                ("orders".to_owned(), order_types.clone()),
                ("customers".to_owned(), customer_types.clone()),
            ],
            nullability: vec![("orders".to_owned(), pairs(&[("order_id", "never")]))],
            complete: vec![("orders".to_owned(), order_types.clone())],
            schemas: vec![("customers".to_owned(), pairs(&[("customer_id", "INTEGER")]))],
            dependency_ordered: true,
            cyclic: false,
            legacy_models: strings(&["customers"]),
            families: vec![(
                "order_pivot".to_owned(),
                vec![DynamicFamily {
                    name: "category_amounts".to_owned(),
                    pivot_column: "category".to_owned(),
                    value_column: "amount".to_owned(),
                    aggregate: "MAX".to_owned(),
                    data_type: "DOUBLE".to_owned(),
                    name_pattern: Some("^amount_".to_owned()),
                }],
            )],
        },
        fact_digests: vec![(strings(&["model", "orders"]), "fact-digest".to_owned())],
        model_subjects: Some(vec![
            (
                "sql/models/orders.sql".to_owned(),
                "orders".to_owned(),
                Some("table".to_owned()),
            ),
            (
                "sql/models/customers.sql".to_owned(),
                "customers".to_owned(),
                None,
            ),
        ]),
        diagnostics: vec![CapturedDiagnostic {
            json: "{\"code\": \"W1\"}".to_owned(),
            resource_name: Some("orders".to_owned()),
            paths: strings(&["sql/models/orders.sql"]),
            error: false,
        }],
        text_models: Some(vec![("orders".to_owned(), true, 3)]),
        environment_names: strings(&["SQLBUILD_TARGET"]),
        pinned_sources: strings(&["raw_customers"]),
        test_sources: BTreeMap::from([(
            "tests/unit/orders.sql".to_owned(),
            strings(&["raw_orders"]),
        )]),
        model_shapes: vec![
            ("orders".to_owned(), order_types),
            ("customers".to_owned(), pairs(&[("customer_id", "VARCHAR")])),
        ],
        coverage: Some(vec![ModelCoverage {
            name: "orders".to_owned(),
            disabled: false,
            blocked: true,
            partial: strings(&["open source: raw_orders"]),
        }]),
        lint_model_keys: Some(pairs(&[("sql/models/orders.sql", "lint-key")])),
        lint_file_keys: Some(vec![
            ("sql/models/orders.sql".to_owned(), Some("file-key".to_owned())),
            ("sql/models/customers.sql".to_owned(), None),
        ]),
        header_keys: BTreeMap::from([(
            "sql/models/orders.sql".to_owned(),
            pairs(&[("description", "0123")]),
        )]),
        audit_references: BTreeMap::from([(
            "sql/models/orders.sql".to_owned(),
            strings(&["order_quality"]),
        )]),
        rules: Some(ImageRules::Retained(RetainedRows {
            request_json: request,
            rows: RULE_ROWS.to_vec(),
            digests: Vec::new(),
        })),
    }
}

/// Every stored fact of `image` but its rules, in a comparable form.
pub(crate) fn facts(image: &ProjectImage) -> String {
    format!(
        "{:?}",
        (
            (
                &image.json_output,
                &image.models,
                &image.usages,
                &image.tests,
                &image.relations,
                &image.fact_digests,
                &image.model_subjects,
            ),
            (
                &image.diagnostics,
                &image.text_models,
                &image.environment_names,
                &image.pinned_sources,
                &image.test_sources,
                &image.model_shapes,
                &image.coverage,
                &image.lint_model_keys,
                &image.lint_file_keys,
                &image.header_keys,
                &image.audit_references,
            ),
        )
    )
}

pub(crate) fn current_format() -> String {
    IMAGE_FORMAT.to_owned()
}

pub(crate) fn encoded(image: ProjectImage) -> ImageSections {
    let stored: ProjectImage = ProjectImage {
        format: current_format(),
        ..image
    };
    stored.encode().unwrap_or_default()
}

/// The image `image()` without any of its optional facts.
pub(crate) fn sparse_image() -> ProjectImage {
    ProjectImage {
        model_subjects: None,
        text_models: None,
        lint_model_keys: None,
        lint_file_keys: None,
        rules: None,
        ..image()
    }
}

/// The rules request a decoded image holds, resolved.
pub(crate) fn decoded_rules(image: ProjectImage) -> Option<BaseRules> {
    image
        .rules
        .and_then(ImageRules::pending)
        .and_then(PendingRules::resolved)
}

/// `sections` with the gate of an image of another format.
pub(crate) fn with_other_format(sections: &mut ImageSections) {
    let other: ProjectImage = ProjectImage {
        format: OTHER_FORMAT.to_owned(),
        ..ProjectImage::default()
    };
    let gate: Vec<u8> = other
        .encode()
        .unwrap_or_default()
        .into_iter()
        .find(|(name, _)| name == GATE_SECTION)
        .map(|(_, bytes)| bytes)
        .unwrap_or_default();
    sections
        .iter_mut()
        .filter(|(name, _)| name == GATE_SECTION)
        .for_each(|section| section.1 = gate.clone());
}

/// `sections` with their facts cut in half.
pub(crate) fn with_truncated_facts(sections: &mut ImageSections) {
    sections
        .iter_mut()
        .filter(|(name, _)| name == FACTS_SECTION)
        .for_each(|(_, bytes)| bytes.truncate(bytes.len() / 2));
}

/// `sections` without the rules request's digests.
pub(crate) fn without_rule_digests(sections: &mut ImageSections) {
    sections.retain(|(name, _)| name != RULE_DIGESTS_SECTION);
}

/// The names in `values`, as a set.
pub(crate) fn names(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// One model's semantic coverage facts.
pub(crate) fn coverage(
    name: &str,
    (disabled, blocked): (bool, bool),
    partial: &[&str],
) -> ModelCoverage {
    ModelCoverage {
        name: name.to_owned(),
        disabled,
        blocked,
        partial: strings(partial),
    }
}

/// Models paired with their upstream names, or with their reasons.
pub(crate) fn order(models: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
    models
        .iter()
        .map(|(name, upstream)| ((*name).to_owned(), strings(upstream)))
        .collect()
}
