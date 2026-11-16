//! `MeasurementThresholds` of `parsed`, or `None`.

use crate::model_config::_helpers::headers::header_metadata::contract_object;
use crate::model_config::_helpers::headers::header_metadata::threshold_bound;
use crate::model_config::models::ContractClasses;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_model_config::header_metadata::models::ParsedThresholds;

/// `MeasurementThresholds` of `parsed`, or `None`.
pub fn thresholds<'py>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    parsed: Option<&ParsedThresholds>,
) -> PyResult<Bound<'py, PyAny>> {
    let Some(parsed) = parsed else {
        return Ok(py.None().into_bound(py));
    };
    contract_object(
        &classes.measurement_thresholds,
        [
            threshold_bound(py, classes, parsed.warn)?,
            threshold_bound(py, classes, parsed.error)?,
        ],
    )
}
