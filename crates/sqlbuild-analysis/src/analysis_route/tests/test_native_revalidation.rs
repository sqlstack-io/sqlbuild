use std::collections::HashMap;
use std::sync::Arc;

use crate::analysis_route::main::native_revalidation::native_revalidation;
use crate::analysis_route::tests::helpers::{finding_rows, outcome};
use crate::analysis_route::tests::test_types::{PivotlessDialect, RevalidationTestCase};
use crate::assembly::analysis_session::models::{
    FinishedSession, ModelOutcome, PivotOutcome, PivotTables,
};
use crate::semantic_checks::models::{RawBinding, RecoveryModel};
use crate::semantic_checks::types::RevisedBinding;
use crate::semantic_validation::main::new_analysis_pool::new_analysis_pool;

#[test]
fn given_poisoned_outputs_when_revalidating_natively_then_findings_keep_session_positions() {
    let test_cases = [
        RevalidationTestCase {
            description: "a finding on a typed column stays at the session's span",
            sql: "SELECT id FROM orders WHERE status",
            poisoned: &[],
            expected_rows: &[("B211", Some(28), Some(34))],
        },
        RevalidationTestCase {
            description: "a finding only a poisoned column causes drops out",
            sql: "SELECT id FROM orders WHERE status",
            poisoned: &[("orders", "status")],
            expected_rows: &[],
        },
        RevalidationTestCase {
            description: "a model the parser cannot read again keeps its own findings",
            sql: "SELECT id FROM orders PIVOT (sum(id) FOR status IN (1))",
            poisoned: &[("orders", "status")],
            expected_rows: &[("B002", Some(7), Some(9))],
        },
    ];

    for test_case in test_cases {
        let finished: FinishedSession = FinishedSession {
            tables: PivotTables::default(),
            pool: new_analysis_pool().map(Arc::new),
            models: HashMap::new(),
            dynamic_proofs: HashMap::new(),
            relations: None,
            model_names: vec!["other".to_owned(), "downstream".to_owned()],
            declared_functions: Vec::new(),
        };
        let outcomes: Vec<(ModelOutcome, PivotOutcome)> = vec![
            (outcome("SELECT 1"), PivotOutcome::Absent),
            (outcome(test_case.sql), PivotOutcome::Absent),
        ];
        let model: RecoveryModel = RecoveryModel {
            name: "downstream".to_owned(),
            query_sql: test_case.sql.to_owned(),
            inferred_columns: None,
            references: vec!["orders".to_owned()],
            lineage: Vec::new(),
            bindings: Vec::new(),
            raw_bindings: vec![RawBinding {
                id: 0,
                code: "B002".to_owned(),
                message: "Unknown column".to_owned(),
                start: Some(7),
                end: Some(9),
            }],
        };
        let poisoned: Vec<(String, String)> = test_case
            .poisoned
            .iter()
            .map(|(relation, column)| ((*relation).to_owned(), (*column).to_owned()))
            .collect();
        let revised: Result<Vec<Vec<RevisedBinding>>, String> = native_revalidation(
            &PivotlessDialect,
            (&finished, &outcomes),
            &[&model],
            &poisoned,
        );
        assert_eq!(
            revised.as_deref().map(finding_rows),
            Ok(test_case.expected_rows.to_vec()),
            "{}",
            test_case.description
        );
    }
}
