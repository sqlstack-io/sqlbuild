use crate::engine::tests::{helpers, test_types};
use crate::models::EvaluateRequest;
use serde_json::json;
use tempfile::TempDir;

#[test]
fn given_supported_request_without_scope_facts_when_deserializing_then_scope_is_incomplete()
-> Result<(), String> {
    let test_cases = [test_types::MissingScopeFactsTestCase {
        description: "omitted legacy scope facts are explicitly incomplete",
        expected_complete: false,
        expected_runtime_usage: false,
    }];

    for test_case in &test_cases {
        let project_dir = TempDir::new().map_err(|error| error.to_string())?;
        let request: EvaluateRequest =
            serde_json::from_str(&helpers::request(&project_dir, &json!({})))
                .map_err(|error| error.to_string())?;

        assert_eq!(
            request.scope_index.complete, test_case.expected_complete,
            "{}",
            test_case.description
        );
        assert_eq!(
            request.scope_index.completeness.runtime_usage, test_case.expected_runtime_usage,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_complete_scope_facts_when_deserializing_then_typed_request_is_accepted() {
    let test_cases = [test_types::ScopeFactsTestCase {
        description: "complete scope facts deserialize",
        scope: helpers::scope_index(),
        expected_complete: true,
        expected_runtime_usage: true,
        expected_declaration_count: 2,
        expected_visibility_count: 1,
    }];

    for test_case in &test_cases {
        let request: EvaluateRequest =
            serde_json::from_value(helpers::request_with_scope(test_case.scope.clone()))
                .expect(test_case.description);

        assert_eq!(
            request.scope_index.complete, test_case.expected_complete,
            "{}",
            test_case.description
        );
        assert_eq!(
            request.scope_index.declarations.len(),
            test_case.expected_declaration_count,
            "{}",
            test_case.description
        );
        assert_eq!(
            request.scope_index.visibility.len(),
            test_case.expected_visibility_count,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_partial_scope_facts_when_deserializing_then_completeness_is_preserved() {
    let mut partial = helpers::scope_index();
    partial["complete"] = json!(false);
    partial["completeness"]["runtime_usage"] = json!(false);
    partial["diagnostics"] = json!([{
        "code": "S013",
        "message": "runtime usage is incomplete",
        "severity": "warning",
        "path": "sql/models/orders.sql",
        "line": 1,
        "column": 1,
        "declaration": null,
        "resource": "model:orders"
    }]);
    let test_cases = [test_types::ScopeFactsTestCase {
        description: "partial scope completeness deserializes",
        scope: partial,
        expected_complete: false,
        expected_runtime_usage: false,
        expected_declaration_count: 2,
        expected_visibility_count: 1,
    }];

    for test_case in &test_cases {
        let request: EvaluateRequest =
            serde_json::from_value(helpers::request_with_scope(test_case.scope.clone()))
                .expect(test_case.description);

        assert_eq!(
            request.scope_index.complete, test_case.expected_complete,
            "{}",
            test_case.description
        );
        assert_eq!(
            request.scope_index.completeness.runtime_usage, test_case.expected_runtime_usage,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_malformed_scope_records_when_deserializing_then_request_is_rejected() {
    let mut missing_field = helpers::scope_index();
    missing_field["declarations"][0]
        .as_object_mut()
        .expect("declaration should be an object")
        .remove("scope");
    let mut enum_value_leak = helpers::scope_index();
    enum_value_leak["declarations"][0]["metadata"]["enum"]["members"][0]["value"] = json!("secret");
    let mut unknown_field = helpers::scope_index();
    unknown_field["resources"][0]["absolute_path"] = json!("/tmp/orders.sql");
    let mut unsupported_schema = helpers::scope_index();
    unsupported_schema["schema_version"] = json!(999);
    let test_cases = [
        test_types::MalformedScopeFactsTestCase {
            description: "missing declaration field is rejected",
            scope: missing_field,
            expected_rejected: true,
        },
        test_types::MalformedScopeFactsTestCase {
            description: "enum value leak is rejected",
            scope: enum_value_leak,
            expected_rejected: true,
        },
        test_types::MalformedScopeFactsTestCase {
            description: "unknown resource field is rejected",
            scope: unknown_field,
            expected_rejected: true,
        },
        test_types::MalformedScopeFactsTestCase {
            description: "unsupported schema is rejected",
            scope: unsupported_schema,
            expected_rejected: true,
        },
    ];

    for test_case in &test_cases {
        let rejected = serde_json::from_value::<EvaluateRequest>(helpers::request_with_scope(
            test_case.scope.clone(),
        ))
        .is_err();

        assert_eq!(
            rejected, test_case.expected_rejected,
            "{}",
            test_case.description
        );
    }
}
