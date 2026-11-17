//! Lifecycle export policy, severity comparison and narrowing for Python callers.

use pyo3::prelude::{Bound, PyAny, PyResult, Python, pyfunction};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_core::event_export::main::severity_at_least::severity_at_least;
use sqlbuild_core::event_export::main::stricter_severity::stricter_severity;
use sqlbuild_core::event_export::models::{ExportPolicy, ExportSeverity};
use sqlbuild_core::lifecycle_events::constants::CURRENT_LIFECYCLE_EVENT_SCHEMA_VERSION;
use sqlbuild_core::lifecycle_events::main::catalog::catalog;

use sqlbuild_python_base::event_exporting::main::export_policy::export_policy;
use sqlbuild_python_base::event_exporting::main::severity_from::severity_from;
use sqlbuild_python_base::event_exporting::main::severity_object::severity_object;

/// `{event_type: (kind, severity, priority)}` for every catalogued lifecycle event, in catalogue order.
#[pyfunction]
pub(crate) fn lifecycle_export_policy_catalog(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let policies: Bound<'_, PyDict> = PyDict::new(py);
    for definition in catalog(CURRENT_LIFECYCLE_EVENT_SCHEMA_VERSION).unwrap_or_default() {
        let event_type: &str = definition.event_type;
        let policy: ExportPolicy = export_policy(py, event_type)?;
        policies.set_item(
            event_type,
            (
                policy.kind,
                severity_object(py, policy.severity)?,
                policy.priority,
            ),
        )?;
    }
    Ok(policies)
}

/// Whether a catalogued severity meets a configured minimum.
#[pyfunction]
#[pyo3(signature = (*, severity, minimum))]
pub(crate) fn event_export_severity_at_least(
    severity: &Bound<'_, PyAny>,
    minimum: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    Ok(severity_at_least(
        severity_from(severity)?,
        severity_from(minimum)?,
    ))
}

/// The stricter of a required and an optional minimum severity, as `EventExportSeverity`.
#[pyfunction]
#[pyo3(signature = (*, first, second))]
pub(crate) fn stricter_event_export_severity<'py>(
    first: &Bound<'py, PyAny>,
    second: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let second: Option<ExportSeverity> = second.map(severity_from).transpose()?;
    severity_object(first.py(), stricter_severity(severity_from(first)?, second))
}
