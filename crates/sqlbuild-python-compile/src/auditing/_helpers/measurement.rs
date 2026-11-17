//! A measured value's audit outcome under its sample and threshold policy.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_analysis::auditing::main::evaluate_measurement::evaluate_measurement as evaluate;
use sqlbuild_analysis::auditing::models::{MeasurementOutcome, Thresholds};
use sqlbuild_host::entries::models::FacadeHost;
use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;

use crate::auditing::_helpers::identity::thresholds as read_thresholds;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::types::ErrorId;

fn measurement_error(py: Python<'_>, message: &str) -> PyErr {
    new_plain_error(py, ErrorId::MeasurementAudit, message)
}

/// The canonical outcome for one finite measured value.
#[pyfunction]
#[pyo3(signature = (*, measured_value, sample_count, minimum_samples, thresholds))]
pub(crate) fn evaluate_measurement<'py>(
    measured_value: f64,
    sample_count: Option<i128>,
    minimum_samples: Option<i128>,
    thresholds: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = thresholds.py();
    let thresholds: Thresholds = read_thresholds(thresholds)?.unwrap_or_default();
    let outcome: MeasurementOutcome =
        evaluate(measured_value, (sample_count, minimum_samples), thresholds)
            .map_err(|message| measurement_error(py, message))?;
    let member: &str = match outcome {
        MeasurementOutcome::Pass => "PASS",
        MeasurementOutcome::Warn => "WARN",
        MeasurementOutcome::Error => "ERROR",
        MeasurementOutcome::Insufficient => "INSUFFICIENT",
    };
    host_entry(py, FacadeHost::AuditOutcome)?.getattr(member)
}
