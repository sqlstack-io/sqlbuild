//! The check a worker reports an audit's result as, and the outcome it reports.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_host::models::{AuditExecutionResult, AuditOutcome};
use sqlbuild_orchestration::runs::models::NodeOutcome;
use sqlbuild_python_base::native_host::types::ReadResult;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_project::project_snapshot::models::WorkerBuild;

use crate::result_output::main::results::audit_check_id::audit_check_id;

/// `audit`'s published check and outcome: its attached check, else a singular audit's own `audit:<name>`.
pub fn audit_check(
    worker: &WorkerBuild,
    audit: &Bound<'_, PyAny>,
) -> PyResult<(String, NodeOutcome)> {
    let outcome = match AuditExecutionResult::read(audit)?.outcome {
        AuditOutcome::Pass => NodeOutcome::Success,
        AuditOutcome::Warn | AuditOutcome::Insufficient => NodeOutcome::Warn,
        AuditOutcome::Skipped => NodeOutcome::Skipped,
        AuditOutcome::NotRun => NodeOutcome::NotRun,
        AuditOutcome::Error => NodeOutcome::Failed,
    };
    let attached: String = audit_check_id(audit)?;
    let key: String = if worker.records_check(&attached) {
        attached
    } else {
        format!("audit:{}", field(audit, "audit_name")?.str()?)
    };
    Ok((key, outcome))
}
