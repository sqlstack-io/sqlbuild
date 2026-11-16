use std::collections::BTreeSet;

use crate::compile_delta::_helpers::splice::spliced_report;
use crate::compile_delta::models::SpliceScope;
use crate::compile_delta::tests::helpers::names;
use crate::compile_delta::tests::test_types::SpliceTestCase;

const BASE: &str = r#"{"has_errors": false, "summary": {"models": 2, "errors": 0, "warnings": 0}, "resources": {"models": [{"name": "orders", "columns": 1}, {"name": "customers", "columns": 2}], "sources": [{"name": "raw_orders", "column_count": 1}, {"name": "raw_customers", "column_count": 2}], "audits": [{"name": "not_null", "attached_to": {"resource_type": "model", "name": "orders"}}], "tests": [{"name": "orders_test", "status": "compiled"}]}, "diagnostics": [], "compile_timings": {"total_ms": 9}}"#;

#[test]
fn given_stored_and_delta_reports_when_splicing_then_only_rerun_items_and_delta_sections_change() {
    let test_cases = [
        SpliceTestCase {
            description: "a rerun model, its audit and the delta's diagnostics replace the stored ones",
            base: BASE,
            delta: r#"{"has_errors": true, "summary": {"models": 1, "errors": 1, "warnings": 0}, "resources": {"models": [{"name": "orders", "columns": 3}], "sources": [{"name": "raw_orders", "column_count": 1}, {"name": "raw_customers", "column_count": 2}], "audits": [{"name": "unique", "attached_to": {"resource_type": "model", "name": "orders"}}], "tests": []}, "diagnostics": [{"code": "E1"}], "compile_timings": {"total_ms": 1}}"#,
            models: &["orders"],
            tests: &[],
            sources: None,
            expected_report: Some(
                r#"{"has_errors": true, "summary": {"models": 2, "errors": 1, "warnings": 0}, "resources": {"models": [{"name": "orders", "columns": 3}, {"name": "customers", "columns": 2}], "sources": [{"name": "raw_orders", "column_count": 1}, {"name": "raw_customers", "column_count": 2}], "audits": [{"name": "unique", "attached_to": {"resource_type": "model", "name": "orders"}}], "tests": [{"name": "orders_test", "status": "compiled"}]}, "diagnostics": [{"code": "E1"}], "compile_timings": {"total_ms": 1}}"#,
            ),
        },
        SpliceTestCase {
            description: "a rerun SQL test and the only discovered source replace the stored ones",
            base: BASE,
            delta: r#"{"has_errors": false, "summary": {"models": 0, "errors": 0, "warnings": 0}, "resources": {"models": [], "sources": [{"name": "raw_customers", "column_count": 5}], "audits": [], "tests": [{"name": "orders_test", "status": "changed"}]}, "diagnostics": [], "compile_timings": {"total_ms": 1}}"#,
            models: &[],
            tests: &["orders_test"],
            sources: Some(&["raw_customers"]),
            expected_report: Some(
                r#"{"has_errors": false, "summary": {"models": 2, "errors": 0, "warnings": 0}, "resources": {"models": [{"name": "orders", "columns": 1}, {"name": "customers", "columns": 2}], "sources": [{"name": "raw_orders", "column_count": 1}, {"name": "raw_customers", "column_count": 5}], "audits": [{"name": "not_null", "attached_to": {"resource_type": "model", "name": "orders"}}], "tests": [{"name": "orders_test", "status": "changed"}]}, "diagnostics": [], "compile_timings": {"total_ms": 1}}"#,
            ),
        },
        SpliceTestCase {
            description: "a delta item for a resource outside the rerun set must already be stored",
            base: BASE,
            delta: r#"{"has_errors": false, "summary": {"models": 1, "errors": 0, "warnings": 0}, "resources": {"models": [{"name": "customers", "columns": 9}], "sources": [{"name": "raw_orders", "column_count": 1}, {"name": "raw_customers", "column_count": 2}], "audits": [], "tests": []}, "diagnostics": [], "compile_timings": {"total_ms": 1}}"#,
            models: &["orders"],
            tests: &[],
            sources: None,
            expected_report: None,
        },
        SpliceTestCase {
            description: "the delta's partial-check reasons, already the whole project's, replace the stored",
            base: r#"{"has_errors": false, "semantic_checks_partial": {"customers": ["open input: orders"]}, "resources": {}, "diagnostics": []}"#,
            delta: r#"{"has_errors": true, "semantic_checks_partial": {"customers": ["depends on invalid model orders"]}, "resources": {}, "diagnostics": [{"code": "B002"}]}"#,
            models: &["orders"],
            tests: &[],
            sources: None,
            expected_report: Some(
                r#"{"has_errors": true, "semantic_checks_partial": {"customers": ["depends on invalid model orders"]}, "resources": {}, "diagnostics": [{"code": "B002"}]}"#,
            ),
        },
        SpliceTestCase {
            description: "a whole-project section that differs cannot be spliced",
            base: r#"{"has_errors": false, "target": "dev", "summary": {"errors": 0, "warnings": 0}, "resources": {}, "diagnostics": [], "compile_timings": {}}"#,
            delta: r#"{"has_errors": false, "target": "prod", "summary": {"errors": 0, "warnings": 0}, "resources": {}, "diagnostics": [], "compile_timings": {}}"#,
            models: &[],
            tests: &[],
            sources: None,
            expected_report: None,
        },
        SpliceTestCase {
            description: "reports with different sections cannot be spliced",
            base: r#"{"has_errors": false, "resources": {}}"#,
            delta: r#"{"has_errors": false}"#,
            models: &[],
            tests: &[],
            sources: None,
            expected_report: None,
        },
    ];
    for test_case in test_cases {
        let models: BTreeSet<String> = names(test_case.models);
        let tests: BTreeSet<String> = names(test_case.tests);
        let sources: Option<BTreeSet<String>> = test_case.sources.map(names);
        let scope: SpliceScope<'_> = SpliceScope {
            models: &models,
            tests: &tests,
            sources: sources.as_ref(),
        };
        let spliced: Result<String, String> =
            spliced_report(test_case.base, test_case.delta, &scope);
        assert_eq!(
            spliced.ok().as_deref(),
            test_case.expected_report,
            "{}",
            test_case.description
        );
    }
}
