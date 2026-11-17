//! An orchestration problem found after discovery, raised as `OrchestrationDeclarationError` (D017).

use crate::discovery::_helpers::failures::raised::failure_error;
use pyo3::prelude::{PyErr, Python};
use sqlbuild_discovery::models::{DiscoveryFailure, FailureKind};

/// `OrchestrationDeclarationError` (D017) with `message`, which names the file, line and column itself.
pub fn orchestration_failure(py: Python<'_>, message: String) -> PyErr {
    failure_error(
        py,
        DiscoveryFailure {
            kind: FailureKind::Orchestration,
            message,
            help: None,
        },
    )
}
