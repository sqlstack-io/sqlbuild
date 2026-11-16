//! Frozen selection, executor callbacks and completion belong to one verified worker build.

use pyo3::exceptions::PyRuntimeError;
use sqlbuild_core::models::CompiledResourceType;
use sqlbuild_orchestration::runs::models::NodeOutcome;

/// A finished node's outcome, rows affected, error code and error message.
pub(crate) type Completion = (NodeOutcome, Option<i64>, (Option<String>, Option<String>));

/// Failure outranks cancellation, then warnings, then success.
pub(crate) fn severity(outcome: NodeOutcome) -> u8 {
    match outcome {
        NodeOutcome::Skipped => 0,
        NodeOutcome::Success => 1,
        NodeOutcome::Warn => 2,
        NodeOutcome::Cancelled => 3,
        NodeOutcome::Failed => 4,
    }
}

/// Whether a snapshot node key names a scalar or table function.
pub(crate) fn is_function(key: &str) -> bool {
    key.split_once(':').is_some_and(|(kind, _)| {
        [CompiledResourceType::Udf, CompiledResourceType::TableFn]
            .iter()
            .any(|function| function.as_str() == kind)
    })
}

pub(crate) fn poisoned() -> pyo3::PyErr {
    PyRuntimeError::new_err("worker build outcome lock is unavailable")
}
