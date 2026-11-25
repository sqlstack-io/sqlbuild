//! A worker build reports each audit and SQL test it ran as a check result, as `sqb audit` and `sqb test` do, without checking for a cancel: the build is already finishing.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_host::models::{AuditExecutionResult, AuditOutcome, SqlTestOutcome};
use sqlbuild_host::types::ResultEnum;
use sqlbuild_orchestration::runs::models::NodeOutcome;
use sqlbuild_python_base::native_host::types::ReadResult;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_project::project_snapshot::models::WorkerBuild;

use crate::result_output::main::events::sql_test_identity::sql_test_identity;
use crate::result_output::main::results::audit_check_id::audit_check_id;

/// Record the outcome of every audit and SQL unit test in `executor_result`, the build's result, so check sensors and run events see them; a check the snapshot doesn't record is skipped.
pub fn report_build_checks(
    py: Python<'_>,
    worker: &WorkerBuild,
    executor_result: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let mut audits: Vec<Bound<'_, PyAny>> = Vec::new();
    for model in field(executor_result, "model_results")?.try_iter()? {
        audits.extend(
            field(&model?, "audit_results")?
                .try_iter()?
                .collect::<PyResult<Vec<_>>>()?,
        );
    }
    for results in ["source_audit_results", "end_audit_results"] {
        audits.extend(
            field(executor_result, results)?
                .try_iter()?
                .collect::<PyResult<Vec<_>>>()?,
        );
    }
    for audit in audits {
        let outcome = match AuditExecutionResult::read(&audit)?.outcome {
            AuditOutcome::Pass => NodeOutcome::Success,
            AuditOutcome::Warn | AuditOutcome::Insufficient => NodeOutcome::Warn,
            AuditOutcome::Skipped => NodeOutcome::Skipped,
            AuditOutcome::NotRun => NodeOutcome::NotRun,
            AuditOutcome::Error => NodeOutcome::Failed,
        };
        worker.report_attached(py, &audit_check_id(&audit)?, outcome)?;
    }
    for test in field(executor_result, "test_results")?.try_iter()? {
        let test = test?;
        let outcome: String = field(&test, "outcome")?.str()?.extract()?;
        let outcome = if SqlTestOutcome::from_value(&outcome) == Some(SqlTestOutcome::Pass) {
            NodeOutcome::Success
        } else {
            NodeOutcome::Failed
        };
        worker.report_attached(py, &sql_test_identity(&test, "test_name")?, outcome)?;
    }
    Ok(())
}
